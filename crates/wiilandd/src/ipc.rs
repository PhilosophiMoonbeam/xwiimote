use std::collections::{HashMap, VecDeque};
use std::ffi::{CString, OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use wiiland_ipc::{
    Command, DeviceInfo, FrameBuffer, Notification, PROTOCOL_MAJOR, PROTOCOL_MINOR, ProtocolError,
    ProtocolErrorCode, Request, ResponseResult, ServerMessage, Status, Subscription, encode_frame,
};

const LISTENER_TOKEN: u64 = 1;
const MAX_CLIENTS: usize = 64;
const MAX_QUEUED_BYTES: usize = 256 * 1024;
const ACCEPT_BUDGET: usize = 8;
const READ_BUDGET: usize = 8;
const READ_CHUNK: usize = 16 * 1024;
const FRAME_BUDGET: usize = 8;
const WRITE_BUDGET: usize = 64 * 1024;

static QUARANTINE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct PollSource {
    pub token: u64,
    pub fd: RawFd,
    pub events: i16,
}

#[derive(Debug)]
struct Pending {
    bytes: Vec<u8>,
    offset: usize,
}

#[derive(Debug)]
struct SocketLock {
    _file: File,
}

#[derive(Debug)]
struct PinnedParent {
    file: File,
    public_path: PathBuf,
    inode: (u64, u64),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct EntryMetadata {
    inode: (u64, u64),
    mode: libc::mode_t,
    uid: libc::uid_t,
}

impl EntryMetadata {
    fn is_socket(self) -> bool {
        self.mode & libc::S_IFMT == libc::S_IFSOCK
    }
}

#[derive(Debug)]
struct BoundSocketGuard<'a> {
    parent: &'a PinnedParent,
    name: OsString,
    inode: (u64, u64),
    armed: bool,
}

impl<'a> BoundSocketGuard<'a> {
    fn new(parent: &'a PinnedParent, name: &OsStr, inode: (u64, u64)) -> Self {
        Self {
            parent,
            name: name.to_os_string(),
            inode,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for BoundSocketGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = remove_expected_entry(self.parent, &self.name, self.inode);
        }
    }
}

#[derive(Debug)]
struct Client {
    stream: UnixStream,
    frames: FrameBuffer,
    pending_frames: VecDeque<Vec<u8>>,
    output: VecDeque<Pending>,
    queued_bytes: usize,
    negotiated: bool,
    immediate_close: bool,
    closing: bool,
    input: bool,
    devices: bool,
}

impl Client {
    fn new(stream: UnixStream) -> Self {
        Self {
            stream,
            frames: FrameBuffer::new(),
            pending_frames: VecDeque::new(),
            output: VecDeque::new(),
            queued_bytes: 0,
            negotiated: false,
            immediate_close: false,
            closing: false,
            input: false,
            devices: false,
        }
    }

    fn wants(&self, notification: &Notification) -> bool {
        match notification {
            Notification::Input { .. } => self.input,
            Notification::DeviceAdded { .. } | Notification::DeviceRemoved { .. } => self.devices,
            _ => false,
        }
    }

    fn queue(&mut self, message: &ServerMessage) -> bool {
        let Ok(bytes) = encode_frame(message) else {
            self.immediate_close = true;
            return false;
        };
        self.queue_encoded(bytes)
    }

    fn queue_response(&mut self, id: u64, result: ResponseResult) -> bool {
        let message = ServerMessage::Response { id, result };
        match encode_frame(&message) {
            Ok(bytes) => self.queue_encoded(bytes),
            Err(_) => self.queue(&ServerMessage::Error {
                id: Some(id),
                error: protocol_error(
                    ProtocolErrorCode::Internal,
                    "response exceeds the maximum IPC frame size",
                ),
            }),
        }
    }

    fn queue_encoded(&mut self, bytes: Vec<u8>) -> bool {
        if self.queued_bytes.saturating_add(bytes.len()) > MAX_QUEUED_BYTES {
            self.immediate_close = true;
            return false;
        }
        self.queued_bytes += bytes.len();
        self.output.push_back(Pending { bytes, offset: 0 });
        true
    }

    fn write_ready(&mut self) -> bool {
        let mut budget = WRITE_BUDGET;
        while budget != 0 {
            let Some(front) = self.output.front_mut() else {
                break;
            };
            let remaining = &front.bytes[front.offset..];
            match self.stream.write(&remaining[..remaining.len().min(budget)]) {
                Ok(0) => {
                    self.closing = true;
                    return false;
                }
                Ok(written) => {
                    front.offset += written;
                    self.queued_bytes = self.queued_bytes.saturating_sub(written);
                    budget -= written;
                    if front.offset == front.bytes.len() {
                        self.output.pop_front();
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    self.closing = true;
                    return false;
                }
            }
        }
        true
    }
}

/// Single-threaded, nonblocking Unix-socket IPC server.
///
/// Runtime-owned status and device snapshots are supplied to [`Self::handle_ready`]
/// and are never retained by this transport.
pub(crate) struct IpcServer {
    listener: UnixListener,
    path: PathBuf,
    parent: PinnedParent,
    _lock: SocketLock,
    inode: (u64, u64),
    clients: HashMap<u64, Client>,
    next_token: u64,
    #[cfg(test)]
    before_drop_cleanup: Option<Box<dyn FnMut() + Send>>,
}

impl std::fmt::Debug for IpcServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpcServer")
            .field("path", &self.path)
            .field("clients", &self.clients.len())
            .finish()
    }
}

impl IpcServer {
    pub(crate) fn bind(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::bind_with_setup(path, |_, _, _| Ok(()))
    }

    fn bind_with_setup<F>(path: impl AsRef<Path>, setup_hook: F) -> io::Result<Self>
    where
        F: FnOnce(&Path, &UnixListener, (u64, u64)) -> io::Result<()>,
    {
        Self::bind_with_hooks(path, setup_hook, || {})
    }

    fn bind_with_hooks<F, S>(
        path: impl AsRef<Path>,
        setup_hook: F,
        mut before_stale_capture: S,
    ) -> io::Result<Self>
    where
        F: FnOnce(&Path, &UnixListener, (u64, u64)) -> io::Result<()>,
        S: FnMut(),
    {
        let path = path.as_ref().to_path_buf();
        let name = socket_name(&path)?.to_os_string();
        let parent_path = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .to_path_buf();
        ensure_private_parent(&parent_path)?;
        let parent = PinnedParent::open(parent_path)?;

        let lock = acquire_socket_lock(&parent, &name)?;

        match parent.entry_metadata(&name) {
            Ok(meta) => {
                if !meta.is_socket() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "IPC path is not a socket",
                    ));
                }
                ensure_entry_owner(meta, "IPC stale socket")?;
                match UnixStream::connect(parent.entry_path(&name)) {
                    Ok(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::AddrInUse,
                            "IPC socket is already in use",
                        ));
                    }
                    Err(error) if is_stale_connect_error(&error) => {
                        before_stale_capture();
                        remove_expected_entry(&parent, &name, meta.inode)?;
                    }
                    Err(error) => return Err(error),
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }

        let listener = UnixListener::bind(parent.entry_path(&name))?;
        let inode = match parent.entry_metadata(&name) {
            Ok(meta) if meta.is_socket() => meta.inode,
            Ok(_) => {
                drop(listener);
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "IPC socket path changed during bind",
                ));
            }
            Err(error) => {
                drop(listener);
                return Err(error);
            }
        };
        let mut guard = BoundSocketGuard::new(&parent, &name, inode);
        setup_hook(&path, &listener, inode)?;
        setup_bound_socket(&parent, &name, &listener, inode)?;
        parent.verify_public_identity()?;
        guard.disarm();
        drop(guard);

        Ok(Self {
            listener,
            path,
            parent,
            inode,
            _lock: lock,
            clients: HashMap::new(),
            next_token: 2,
            #[cfg(test)]
            before_drop_cleanup: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn has_input_subscribers(&self) -> bool {
        self.clients.values().any(|client| {
            client.negotiated && client.input && !client.closing && !client.immediate_close
        })
    }

    pub(crate) fn poll_sources(&self, sources: &mut Vec<PollSource>) {
        sources.clear();
        sources.push(PollSource {
            token: LISTENER_TOKEN,
            fd: self.listener.as_raw_fd(),
            events: libc::POLLIN,
        });
        for (&token, client) in &self.clients {
            let mut events = 0;
            if !client.closing && !client.immediate_close {
                events |= libc::POLLIN;
            }
            if !client.immediate_close
                && (!client.output.is_empty() || !client.pending_frames.is_empty())
            {
                events |= libc::POLLOUT;
            }
            sources.push(PollSource {
                token,
                fd: client.stream.as_raw_fd(),
                events,
            });
        }
    }

    pub(crate) fn handle_ready(
        &mut self,
        token: u64,
        revents: i16,
        status: &mut dyn FnMut(&Path) -> Status,
        devices: &mut dyn FnMut() -> Vec<DeviceInfo>,
    ) -> io::Result<()> {
        if token == LISTENER_TOKEN {
            if revents & libc::POLLNVAL != 0 {
                return Err(io::Error::other("IPC listener became invalid"));
            }
            if revents & (libc::POLLIN | libc::POLLERR | libc::POLLHUP) != 0 {
                self.accept_ready()?;
            }
            return Ok(());
        }
        if !self.clients.contains_key(&token) {
            return Ok(());
        }
        let socket_path = self.path.as_path();
        let mut remove = false;
        if let Some(client) = self.clients.get_mut(&token) {
            if !client.closing
                && !client.immediate_close
                && (revents & (libc::POLLIN | libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0
                    || !client.pending_frames.is_empty())
            {
                remove = !read_client(client, socket_path, status, devices);
            }
            if client.immediate_close {
                remove = true;
            }
            if !remove && (revents & libc::POLLOUT != 0 || !client.output.is_empty()) {
                remove = !client.write_ready();
            }
            if client.immediate_close || (client.closing && client.output.is_empty()) {
                remove = true;
            }
        }
        if remove {
            self.clients.remove(&token);
        }
        Ok(())
    }

    fn accept_ready(&mut self) -> io::Result<()> {
        for _ in 0..ACCEPT_BUDGET {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if stream.set_nonblocking(true).is_err() {
                        continue;
                    }
                    if self.clients.len() >= MAX_CLIENTS {
                        continue;
                    }
                    let token = self.allocate_token();
                    self.clients.insert(token, Client::new(stream));
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn allocate_token(&mut self) -> u64 {
        loop {
            let token = self.next_token;
            self.next_token = self.next_token.wrapping_add(1);
            if self.next_token == 0 || self.next_token == LISTENER_TOKEN {
                self.next_token = 2;
            }
            if token > LISTENER_TOKEN && !self.clients.contains_key(&token) {
                return token;
            }
        }
    }

    pub(crate) fn publish(&mut self, notification: Notification) {
        let Ok(frame) = encode_frame(&ServerMessage::Notification(notification.clone())) else {
            return;
        };
        let tokens: Vec<u64> = self
            .clients
            .iter()
            .filter_map(|(&token, client)| {
                (client.negotiated
                    && !client.closing
                    && !client.immediate_close
                    && client.wants(&notification))
                .then_some(token)
            })
            .collect();
        for token in tokens {
            let Some(client) = self.clients.get_mut(&token) else {
                continue;
            };
            if client.queued_bytes.saturating_add(frame.len()) > MAX_QUEUED_BYTES {
                client.immediate_close = true;
                continue;
            }
            client.queued_bytes += frame.len();
            client.output.push_back(Pending {
                bytes: frame.clone(),
                offset: 0,
            });
        }
        self.clients.retain(|_, client| !client.immediate_close);
    }
}

fn setup_bound_socket(
    parent: &PinnedParent,
    name: &OsStr,
    listener: &UnixListener,
    inode: (u64, u64),
) -> io::Result<()> {
    let socket = parent.open_entry(name, libc::O_PATH | libc::O_NOFOLLOW | libc::O_CLOEXEC, 0)?;
    let meta = socket.metadata()?;
    ensure_owner(&meta, "IPC socket")?;
    if !meta.file_type().is_socket() || (meta.dev(), meta.ino()) != inode {
        return Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "IPC socket path changed during setup",
        ));
    }
    // SAFETY: `socket` is a live O_PATH descriptor, the empty C string is valid,
    // and fchmodat2 does not retain either pointer after the syscall.
    let result = unsafe {
        libc::syscall(
            libc::SYS_fchmodat2,
            socket.as_raw_fd(),
            c"".as_ptr(),
            0o600,
            libc::AT_EMPTY_PATH | libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    listener.set_nonblocking(true)?;
    let meta = parent.entry_metadata(name)?;
    if !meta.is_socket() || meta.inode != inode || meta.mode & 0o777 != 0o600 {
        return Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "IPC socket path changed during setup",
        ));
    }
    Ok(())
}

impl PinnedParent {
    fn open(public_path: PathBuf) -> io::Result<Self> {
        let path = path_c_string(&public_path)?;
        // SAFETY: `path` is a valid NUL-terminated pathname and open does not
        // retain its pointer. The returned descriptor is uniquely owned below.
        let fd = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: open returned a fresh descriptor which is transferred once.
        let file = unsafe { File::from_raw_fd(fd) };
        let meta = file.metadata()?;
        validate_private_directory(&meta, "IPC socket parent")?;
        let inode = (meta.dev(), meta.ino());
        Ok(Self {
            file,
            public_path,
            inode,
        })
    }

    fn verify_public_identity(&self) -> io::Result<()> {
        let current = Self::open(self.public_path.clone()).map_err(|error| {
            io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("IPC socket parent changed during bind: {error}"),
            )
        })?;
        if current.inode != self.inode {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "IPC socket parent changed during bind",
            ));
        }
        Ok(())
    }

    fn entry_path(&self, name: &OsStr) -> PathBuf {
        let mut path = PathBuf::from(format!("/proc/self/fd/{}", self.file.as_raw_fd()));
        path.push(name);
        path
    }

    fn entry_metadata(&self, name: &OsStr) -> io::Result<EntryMetadata> {
        let name = os_c_string(name)?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: `name` is a valid C string, `stat` points to writable storage,
        // and fstatat initializes it completely on success.
        let result = unsafe {
            libc::fstatat(
                self.file.as_raw_fd(),
                name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the successful fstatat above initialized every field.
        let stat = unsafe { stat.assume_init() };
        Ok(EntryMetadata {
            inode: (stat.st_dev, stat.st_ino),
            mode: stat.st_mode,
            uid: stat.st_uid,
        })
    }

    fn open_entry(&self, name: &OsStr, flags: libc::c_int, mode: libc::mode_t) -> io::Result<File> {
        let name = os_c_string(name)?;
        // SAFETY: `name` is a valid C string, the directory descriptor remains
        // open for the call, and openat does not retain the pointer.
        let fd = unsafe { libc::openat(self.file.as_raw_fd(), name.as_ptr(), flags, mode) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: openat returned a fresh descriptor which is transferred once.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn path_c_string(path: &Path) -> io::Result<CString> {
    os_c_string(path.as_os_str())
}

fn os_c_string(value: &OsStr) -> io::Result<CString> {
    CString::new(value.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "IPC path contains a NUL byte"))
}

fn socket_name(path: &Path) -> io::Result<&OsStr> {
    path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "IPC socket path has no file name",
        )
    })
}

fn remove_expected_entry(parent: &PinnedParent, name: &OsStr, inode: (u64, u64)) -> io::Result<()> {
    remove_expected_entry_with_hook(parent, name, inode, || {})
}

fn remove_expected_entry_with_hook<F>(
    parent: &PinnedParent,
    name: &OsStr,
    inode: (u64, u64),
    before_capture: F,
) -> io::Result<()>
where
    F: FnOnce(),
{
    before_capture();
    let Some(quarantine) = capture_entry(parent, name, inode)? else {
        return Ok(());
    };
    let captured = match parent.entry_metadata(&quarantine) {
        Ok(meta) => meta,
        Err(error) => {
            let _ = restore_capture(parent, &quarantine, name);
            return Err(error);
        }
    };
    if !captured.is_socket() || captured.inode != inode {
        let restored = restore_capture(parent, &quarantine, name);
        let detail = match restored {
            Ok(()) => "replacement was restored".to_string(),
            Err(error) => format!("replacement is preserved in quarantine: {error}"),
        };
        return Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            format!("IPC socket entry changed during cleanup; {detail}"),
        ));
    }

    let quarantine_c = os_c_string(&quarantine)?;
    // SAFETY: `quarantine_c` is a valid C string and the pinned directory
    // descriptor remains live for the duration of unlinkat.
    if unsafe { libc::unlinkat(parent.file.as_raw_fd(), quarantine_c.as_ptr(), 0) } != 0 {
        let error = io::Error::last_os_error();
        let _ = restore_capture(parent, &quarantine, name);
        return Err(error);
    }
    Ok(())
}

fn capture_entry(
    parent: &PinnedParent,
    name: &OsStr,
    inode: (u64, u64),
) -> io::Result<Option<OsString>> {
    let name_c = os_c_string(name)?;
    for _ in 0..64 {
        let sequence = QUARANTINE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let quarantine = OsString::from(format!(
            ".wiilandd-quarantine-{:x}-{:x}-{:x}-{:x}",
            std::process::id(),
            inode.0,
            inode.1,
            sequence
        ));
        let quarantine_c = os_c_string(&quarantine)?;
        // SAFETY: both names are valid C strings, both directory descriptors
        // are the same live pinned descriptor, and renameat2 retains no pointers.
        let result = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                parent.file.as_raw_fd(),
                name_c.as_ptr(),
                parent.file.as_raw_fd(),
                quarantine_c.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result == 0 {
            return Ok(Some(quarantine));
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EEXIST) => continue,
            Some(libc::ENOENT) => return Ok(None),
            _ => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not reserve a private IPC cleanup quarantine name",
    ))
}

fn restore_capture(parent: &PinnedParent, quarantine: &OsStr, name: &OsStr) -> io::Result<()> {
    let quarantine = os_c_string(quarantine)?;
    let name = os_c_string(name)?;
    // SAFETY: both names are valid C strings, both directory descriptors are
    // the same live pinned descriptor, and renameat2 retains no pointers.
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            parent.file.as_raw_fd(),
            quarantine.as_ptr(),
            parent.file.as_raw_fd(),
            name.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn ensure_private_parent(parent: &Path) -> io::Result<()> {
    match fs::symlink_metadata(parent) {
        Ok(meta) => validate_private_directory(&meta, "IPC socket parent"),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let grandparent = parent
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let grandparent_meta = fs::symlink_metadata(grandparent)?;
            validate_private_directory(&grandparent_meta, "IPC socket grandparent")?;

            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            match builder.create(parent) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
            let parent_meta = fs::symlink_metadata(parent)?;
            validate_private_directory(&parent_meta, "IPC socket parent")
        }
        Err(error) => Err(error),
    }
}

fn validate_private_directory(meta: &fs::Metadata, what: &str) -> io::Result<()> {
    if !meta.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!("{what} is not a directory"),
        ));
    }
    ensure_owner(meta, what)?;
    if meta.permissions().mode() & 0o777 != 0o700 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{what} is not mode 0700"),
        ));
    }
    Ok(())
}

#[cfg(test)]
fn socket_lock_path(path: &Path) -> io::Result<PathBuf> {
    Ok(path.with_file_name(socket_lock_name(socket_name(path)?)))
}

fn socket_lock_name(name: &OsStr) -> OsString {
    let mut lock_name = name.to_os_string();
    lock_name.push(".lock");
    lock_name
}

fn acquire_socket_lock(parent: &PinnedParent, name: &OsStr) -> io::Result<SocketLock> {
    let lock_name = socket_lock_name(name);
    let create_flags = libc::O_RDWR
        | libc::O_CREAT
        | libc::O_EXCL
        | libc::O_NOFOLLOW
        | libc::O_NONBLOCK
        | libc::O_CLOEXEC;
    let (file, created) = match parent.open_entry(&lock_name, create_flags, 0o600) {
        Ok(file) => (file, true),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (
            parent.open_entry(
                &lock_name,
                libc::O_RDWR | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                0,
            )?,
            false,
        ),
        Err(error) => return Err(error),
    };

    let meta = file.metadata()?;
    if !meta.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "IPC socket lock is not a regular file",
        ));
    }
    ensure_owner(&meta, "IPC socket lock")?;
    if created {
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    let meta = file.metadata()?;
    if meta.permissions().mode() & 0o777 != 0o600 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "IPC socket lock is not mode 0600",
        ));
    }

    // SAFETY: `file` owns a live descriptor and flock only inspects that value.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::WouldBlock {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "IPC socket startup lock is already held",
            ));
        }
        return Err(error);
    }

    let named = parent.entry_metadata(&lock_name)?;
    if named.inode != (meta.dev(), meta.ino()) || named.mode & libc::S_IFMT != libc::S_IFREG {
        return Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "IPC socket lock changed during acquisition",
        ));
    }
    Ok(SocketLock { _file: file })
}

fn ensure_owner(meta: &fs::Metadata, what: &str) -> io::Result<()> {
    // SAFETY: geteuid has no arguments and no memory-safety preconditions.
    if meta.uid() != unsafe { libc::geteuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{what} is not owned by effective uid"),
        ));
    }
    Ok(())
}

fn ensure_entry_owner(meta: EntryMetadata, what: &str) -> io::Result<()> {
    // SAFETY: geteuid has no arguments and no memory-safety preconditions.
    if meta.uid != unsafe { libc::geteuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{what} is not owned by effective uid"),
        ));
    }
    Ok(())
}

fn is_stale_connect_error(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(libc::ECONNREFUSED | libc::ENOENT | libc::ECONNRESET | libc::ENOTCONN)
    )
}

fn read_client(
    client: &mut Client,
    socket_path: &Path,
    status: &mut dyn FnMut(&Path) -> Status,
    devices: &mut dyn FnMut() -> Vec<DeviceInfo>,
) -> bool {
    let mut frame_budget = FRAME_BUDGET;
    if !process_pending(client, socket_path, status, devices, &mut frame_budget) {
        return false;
    }
    if frame_budget == 0
        || !client.pending_frames.is_empty()
        || client.closing
        || client.immediate_close
    {
        return true;
    }
    let mut scratch = [0u8; READ_CHUNK];
    for _ in 0..READ_BUDGET {
        match client.stream.read(&mut scratch) {
            Ok(0) => return false,
            Ok(size) => {
                let frames = match client.frames.push(&scratch[..size]) {
                    Ok(frames) => frames,
                    Err(error) => {
                        client.closing = true;
                        let _ = client.queue(&ServerMessage::Error { id: None, error });
                        return true;
                    }
                };
                client.pending_frames.extend(frames);
                if !process_pending(client, socket_path, status, devices, &mut frame_budget) {
                    return false;
                }
                if client.closing
                    || client.immediate_close
                    || frame_budget == 0
                    || !client.pending_frames.is_empty()
                    || size < scratch.len()
                {
                    break;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(_) => return false,
        }
    }
    true
}

fn process_pending(
    client: &mut Client,
    socket_path: &Path,
    status: &mut dyn FnMut(&Path) -> Status,
    devices: &mut dyn FnMut() -> Vec<DeviceInfo>,
    budget: &mut usize,
) -> bool {
    while *budget != 0 {
        let Some(frame) = client.pending_frames.pop_front() else {
            break;
        };
        *budget -= 1;
        if !handle_frame(client, &frame, socket_path, status, devices) {
            return false;
        }
        if client.closing || client.immediate_close {
            break;
        }
    }
    true
}
fn handle_frame(
    client: &mut Client,
    frame: &[u8],
    socket_path: &Path,
    status: &mut dyn FnMut(&Path) -> Status,
    devices: &mut dyn FnMut() -> Vec<DeviceInfo>,
) -> bool {
    let request: Request = match wiiland_ipc::decode_frame(frame) {
        Ok(request) => request,
        Err(error) => {
            client.queue(&ServerMessage::Error { id: None, error });
            return true;
        }
    };
    let id = request.id;
    if !client.negotiated {
        match request.command {
            Command::Hello {
                min_major,
                max_major,
            } if min_major <= PROTOCOL_MAJOR && max_major >= PROTOCOL_MAJOR => {
                client.negotiated = true;
                client.queue_response(
                    id,
                    ResponseResult::Hello {
                        major: PROTOCOL_MAJOR,
                        minor: PROTOCOL_MINOR,
                        daemon_version: env!("CARGO_PKG_VERSION").to_string(),
                    },
                );
            }
            Command::Hello { .. } => {
                client.queue(&ServerMessage::Error {
                    id: Some(id),
                    error: protocol_error(
                        ProtocolErrorCode::UnsupportedVersion,
                        "unsupported protocol version",
                    ),
                });
                client.closing = true;
            }
            _ => {
                client.queue(&ServerMessage::Error {
                    id: Some(id),
                    error: protocol_error(
                        ProtocolErrorCode::InvalidRequest,
                        "first request must be hello",
                    ),
                });
                client.closing = true;
            }
        }
        return true;
    }

    match request.command {
        Command::Hello { .. } => {
            client.queue(&ServerMessage::Error {
                id: Some(id),
                error: protocol_error(
                    ProtocolErrorCode::InvalidRequest,
                    "hello was already completed",
                ),
            });
        }
        Command::Ping => {
            client.queue_response(id, ResponseResult::Pong);
        }
        Command::Status => {
            client.queue_response(id, ResponseResult::Status(status(socket_path)));
        }
        Command::Devices => {
            client.queue_response(id, ResponseResult::Devices(devices()));
        }
        Command::Subscribe { subscriptions } => {
            apply_subscriptions(client, &subscriptions, true);
            client.queue_response(id, ResponseResult::Subscribed);
        }
        Command::Unsubscribe { subscriptions } => {
            apply_subscriptions(client, &subscriptions, false);
            client.queue_response(id, ResponseResult::Unsubscribed);
        }
        Command::Unknown => {
            client.queue(&ServerMessage::Error {
                id: Some(id),
                error: protocol_error(ProtocolErrorCode::UnknownCommand, "unknown command"),
            });
        }
        _ => {
            client.queue(&ServerMessage::Error {
                id: Some(id),
                error: protocol_error(ProtocolErrorCode::UnknownCommand, "unknown command"),
            });
        }
    }
    true
}

fn protocol_error(code: ProtocolErrorCode, message: &str) -> ProtocolError {
    ProtocolError {
        code,
        message: message.to_owned(),
    }
}

fn apply_subscriptions(client: &mut Client, subscriptions: &[Subscription], enabled: bool) {
    for subscription in subscriptions {
        match subscription {
            Subscription::All => {
                client.input = enabled;
                client.devices = enabled;
            }
            Subscription::Input => client.input = enabled,
            Subscription::Devices => client.devices = enabled,
            _ => {}
        }
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        #[cfg(test)]
        if let Some(hook) = self.before_drop_cleanup.as_mut() {
            hook();
        }
        if let Some(name) = self.path.file_name() {
            let _ = remove_expected_entry(&self.parent, name, self.inode);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixStream;
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;
    use std::time::Duration;
    use tempfile::tempdir;
    use wiiland_ipc::{
        ButtonEvent, InputPayload, Profile, ResponseResult, Timestamp, decode_frame, encode_frame,
    };

    fn private_socket_path(root: &Path, directory: &str) -> PathBuf {
        fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
        let parent = root.join(directory);
        assert!(!parent.exists());
        parent.join("wiilandd.sock")
    }

    fn status(path: &Path) -> Status {
        Status {
            daemon_version: "test".into(),
            pid: 7,
            device_count: 1,
            dry_run: true,
            socket_path: path.display().to_string(),
        }
    }

    fn device() -> DeviceInfo {
        DeviceInfo {
            syspath: "/sys/test".into(),
            profile: Profile::Gamepad,
            opened_interfaces: 1,
            pending_interfaces: 0,
            gamepad_output: true,
            desktop_output: false,
        }
    }

    fn ready(
        server: &mut IpcServer,
        token: u64,
        revents: i16,
        st: &Status,
        device_snapshot: &[DeviceInfo],
    ) -> io::Result<()> {
        let mut status_provider = |_: &Path| st.clone();
        let mut devices_provider = || device_snapshot.to_vec();
        server.handle_ready(token, revents, &mut status_provider, &mut devices_provider)
    }

    fn input_notification_with_frame_len(sequence: u64, frame_len: usize) -> Notification {
        let make = |syspath: String| Notification::Input {
            sequence,
            syspath,
            timestamp: Timestamp {
                seconds: 0,
                micros: 0,
            },
            payload: InputPayload::Key(ButtonEvent { code: 1, state: 1 }),
        };
        let base = make(String::new());
        let base_len = encode_frame(&ServerMessage::Notification(base))
            .unwrap()
            .len();
        assert!(frame_len >= base_len);
        let notification = make("x".repeat(frame_len - base_len));
        assert_eq!(
            encode_frame(&ServerMessage::Notification(notification.clone()))
                .unwrap()
                .len(),
            frame_len
        );
        notification
    }

    fn connect(server: &mut IpcServer) -> (UnixStream, u64) {
        let mut sources = Vec::new();
        server.poll_sources(&mut sources);
        let existing_tokens: Vec<_> = sources
            .iter()
            .filter(|source| source.token != LISTENER_TOKEN)
            .map(|source| source.token)
            .collect();

        let stream = UnixStream::connect(server.path()).unwrap();
        let path = server.path().to_path_buf();
        let st = status(&path);
        ready(server, LISTENER_TOKEN, libc::POLLIN, &st, &[]).unwrap();
        server.poll_sources(&mut sources);
        let mut new_tokens = sources
            .iter()
            .filter(|source| {
                source.token != LISTENER_TOKEN && !existing_tokens.contains(&source.token)
            })
            .map(|source| source.token);
        let token = new_tokens.next().expect("accepted client token");
        assert_eq!(
            new_tokens.next(),
            None,
            "accepted client token must be unique"
        );
        (stream, token)
    }

    fn read_message(stream: &mut UnixStream) -> ServerMessage {
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut byte = [0; 1];
            stream.read_exact(&mut byte).unwrap();
            bytes.push(byte[0]);
            if byte[0] == b'\n' {
                return decode_frame(&bytes).unwrap();
            }
        }
    }

    fn request(
        server: &mut IpcServer,
        stream: &mut UnixStream,
        token: u64,
        id: u64,
        command: Command,
    ) -> ServerMessage {
        stream
            .write_all(&encode_frame(&Request { id, command }).unwrap())
            .unwrap();
        let st = status(server.path());
        ready(server, token, libc::POLLIN, &st, &[device()]).unwrap();
        ready(server, token, libc::POLLOUT, &st, &[device()]).unwrap();
        read_message(stream)
    }

    #[test]
    fn secure_bind_modes_live_collision_stale_and_cleanup() {
        let dir = tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            IpcServer::bind(dir.path().join("nonprivate.sock"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();

        let path = private_socket_path(dir.path(), "private");
        let server = IpcServer::bind(&path).unwrap();
        assert_eq!(
            fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            IpcServer::bind(&path).unwrap_err().kind(),
            io::ErrorKind::AddrInUse
        );
        drop(server);
        assert!(!path.exists());

        let replacement = UnixListener::bind(&path).unwrap();
        assert_eq!(
            IpcServer::bind(&path).unwrap_err().kind(),
            io::ErrorKind::AddrInUse
        );
        drop(replacement);
        assert!(path.exists());

        let server = IpcServer::bind(&path).unwrap();
        drop(server);
        assert!(!path.exists());
    }

    #[test]
    fn drop_cleanup_restores_and_never_unlinks_a_replacement_entry() {
        let dir = tempdir().unwrap();
        let path = private_socket_path(dir.path(), "private");
        let mut server = IpcServer::bind(&path).unwrap();
        let moved = path.with_file_name("moved.sock");
        let replacement = Arc::new(Mutex::new(None::<UnixListener>));
        let replacement_for_hook = Arc::clone(&replacement);
        let path_for_hook = path.clone();
        let moved_for_hook = moved.clone();
        server.before_drop_cleanup = Some(Box::new(move || {
            fs::rename(&path_for_hook, &moved_for_hook).unwrap();
            let listener = UnixListener::bind(&path_for_hook).unwrap();
            *replacement_for_hook.lock().unwrap() = Some(listener);
        }));

        drop(server);
        assert!(path.exists());
        assert!(moved.exists());
        drop(replacement.lock().unwrap().take());
        fs::remove_file(&path).unwrap();
        fs::remove_file(&moved).unwrap();
    }

    #[test]
    fn stale_cleanup_restores_and_never_unlinks_a_replacement_entry() {
        let dir = tempdir().unwrap();
        let path = private_socket_path(dir.path(), "private");
        fs::create_dir(path.parent().unwrap()).unwrap();
        fs::set_permissions(path.parent().unwrap(), fs::Permissions::from_mode(0o700)).unwrap();
        drop(UnixListener::bind(&path).unwrap());
        let moved = path.with_file_name("observed-stale.sock");
        let replacement = Arc::new(Mutex::new(None::<UnixListener>));
        let replacement_for_hook = Arc::clone(&replacement);
        let path_for_hook = path.clone();
        let moved_for_hook = moved.clone();

        let error = IpcServer::bind_with_hooks(
            &path,
            |_, _, _| Ok(()),
            move || {
                fs::rename(&path_for_hook, &moved_for_hook).unwrap();
                let listener = UnixListener::bind(&path_for_hook).unwrap();
                *replacement_for_hook.lock().unwrap() = Some(listener);
            },
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
        assert!(path.exists());
        assert!(moved.exists());
        drop(replacement.lock().unwrap().take());
        fs::remove_file(&path).unwrap();
        fs::remove_file(&moved).unwrap();
    }

    #[test]
    fn parent_replacement_before_bind_success_is_rejected() {
        let dir = tempdir().unwrap();
        let path = private_socket_path(dir.path(), "private");
        let parent = path.parent().unwrap().to_path_buf();
        let moved_parent = dir.path().join("pinned-parent");
        let parent_for_hook = parent.clone();
        let moved_for_hook = moved_parent.clone();

        let error = IpcServer::bind_with_setup(&path, move |_, _, _| {
            fs::rename(&parent_for_hook, &moved_for_hook)?;
            fs::create_dir(&parent_for_hook)?;
            fs::set_permissions(&parent_for_hook, fs::Permissions::from_mode(0o700))
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
        assert!(!path.exists());
        assert!(!moved_parent.join("wiilandd.sock").exists());
        assert!(moved_parent.join("wiilandd.sock.lock").exists());
    }
    #[test]
    fn missing_parent_requires_private_grandparent_without_side_effects() {
        let dir = tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        let path = dir.path().join("private").join("wiilandd.sock");

        assert_eq!(
            IpcServer::bind(&path).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert!(!path.parent().unwrap().exists());
        assert!(!socket_lock_path(&path).unwrap().exists());
    }

    #[test]
    fn symlink_parent_and_lock_are_rejected_without_chmod_following() {
        let dir = tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let real_parent = dir.path().join("real");
        fs::create_dir(&real_parent).unwrap();
        fs::set_permissions(&real_parent, fs::Permissions::from_mode(0o700)).unwrap();
        let linked_parent = dir.path().join("linked");
        symlink(&real_parent, &linked_parent).unwrap();
        let linked_path = linked_parent.join("wiilandd.sock");
        assert_eq!(
            IpcServer::bind(&linked_path).unwrap_err().kind(),
            io::ErrorKind::NotADirectory
        );

        let path = real_parent.join("wiilandd.sock");
        let victim = real_parent.join("victim");
        fs::write(&victim, b"not a lock").unwrap();
        fs::set_permissions(&victim, fs::Permissions::from_mode(0o640)).unwrap();
        symlink(&victim, socket_lock_path(&path).unwrap()).unwrap();
        assert!(IpcServer::bind(&path).is_err());
        assert_eq!(
            fs::metadata(&victim).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert!(!path.exists());
    }

    #[test]
    fn setup_never_chmods_a_replacement_symlink_target() {
        let dir = tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = private_socket_path(dir.path(), "private");
        let moved = path.with_file_name("bound.sock");
        let victim = dir.path().join("victim");
        fs::write(&victim, b"victim").unwrap();
        fs::set_permissions(&victim, fs::Permissions::from_mode(0o640)).unwrap();

        let error = IpcServer::bind_with_setup(&path, |path, _, _| {
            fs::rename(path, &moved)?;
            symlink(&victim, path)
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
        assert_eq!(
            fs::metadata(&victim).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert!(
            fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::remove_file(&path).unwrap();
        fs::remove_file(&moved).unwrap();
    }

    #[test]
    fn startup_lock_serializes_concurrent_bind_and_persists_after_drop() {
        let dir = tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.path().join("private").join("wiilandd.sock");
        let barrier = Arc::new(Barrier::new(3));
        let mut joins = Vec::new();
        for _ in 0..2 {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            joins.push(thread::spawn(move || {
                barrier.wait();
                IpcServer::bind(path)
            }));
        }
        barrier.wait();
        let mut winner = None;
        let mut loser = None;
        for join in joins {
            match join.join().unwrap() {
                Ok(server) => winner = Some(server),
                Err(error) => loser = Some(error),
            }
        }
        assert!(winner.is_some());
        assert_eq!(loser.unwrap().kind(), io::ErrorKind::AddrInUse);
        assert!(path.exists());
        let lock_path = socket_lock_path(&path).unwrap();
        assert_eq!(
            fs::symlink_metadata(&lock_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        drop(winner);
        assert!(!path.exists());
        assert!(
            fs::symlink_metadata(&lock_path)
                .unwrap()
                .file_type()
                .is_file()
        );
        let replacement = IpcServer::bind(&path).unwrap();
        drop(replacement);
        assert!(lock_path.exists());
    }

    #[test]
    fn failed_post_bind_setup_removes_created_socket() {
        let dir = tempdir().unwrap();
        let path = private_socket_path(dir.path(), "private");
        let error = IpcServer::bind_with_setup(&path, |_, _, _| {
            Err(io::Error::other("injected setup failure"))
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(!path.exists());
    }

    #[test]
    fn failed_post_bind_setup_preserves_replacement_inode() {
        let dir = tempdir().unwrap();
        let path = private_socket_path(dir.path(), "private");
        let moved = path.with_file_name("created.sock");
        let error = IpcServer::bind_with_setup(&path, |path, _, _| {
            fs::rename(path, &moved)?;
            let replacement = UnixListener::bind(path)?;
            drop(replacement);
            Err(io::Error::other("injected setup failure"))
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(path.exists());
        fs::remove_file(path).unwrap();
        fs::remove_file(moved).unwrap();
    }

    #[test]
    fn hello_errors_are_correlated_and_status_devices_work() {
        let dir = tempdir().unwrap();
        let path = private_socket_path(dir.path(), "initial");
        let mut server = IpcServer::bind(&path).unwrap();
        let (mut stream, token) = connect(&mut server);
        let error = request(&mut server, &mut stream, token, 9, Command::Ping);
        assert!(matches!(error, ServerMessage::Error { id: Some(9), .. }));
        let path = private_socket_path(dir.path(), "unsupported");
        let mut server = IpcServer::bind(&path).unwrap();
        let (mut stream, token) = connect(&mut server);
        let error = request(
            &mut server,
            &mut stream,
            token,
            8,
            Command::Hello {
                min_major: 9,
                max_major: 9,
            },
        );
        assert!(matches!(
            error,
            ServerMessage::Error {
                id: Some(8),
                error: ProtocolError {
                    code: ProtocolErrorCode::UnsupportedVersion,
                    ..
                }
            }
        ));

        let path = private_socket_path(dir.path(), "successful");
        let mut server = IpcServer::bind(&path).unwrap();
        let (mut stream, token) = connect(&mut server);
        let hello = request(
            &mut server,
            &mut stream,
            token,
            1,
            Command::Hello {
                min_major: 1,
                max_major: 1,
            },
        );
        assert!(matches!(
            hello,
            ServerMessage::Response {
                result: ResponseResult::Hello { .. },
                ..
            }
        ));
        let status_message = request(&mut server, &mut stream, token, 2, Command::Status);
        assert!(matches!(
            status_message,
            ServerMessage::Response {
                result: ResponseResult::Status(_),
                ..
            }
        ));
        let devices_message = request(&mut server, &mut stream, token, 3, Command::Devices);
        assert!(matches!(
            devices_message,
            ServerMessage::Response {
                result: ResponseResult::Devices(_),
                ..
            }
        ));
    }

    #[test]
    fn snapshot_providers_are_invoked_only_for_their_commands() {
        let dir = tempdir().unwrap();
        let path = private_socket_path(dir.path(), "lazy");
        let mut server = IpcServer::bind(&path).unwrap();
        let expected_status = Status {
            daemon_version: "lazy-status".into(),
            pid: 99,
            device_count: 1,
            dry_run: false,
            socket_path: path.display().to_string(),
        };
        let mut expected_device = device();
        expected_device.syspath = "/sys/lazy-device".into();
        let expected_devices = vec![expected_device];
        let status_calls = Cell::new(0);
        let devices_calls = Cell::new(0);
        let mut status_provider = |socket_path: &Path| {
            assert_eq!(socket_path, path);
            status_calls.set(status_calls.get() + 1);
            expected_status.clone()
        };
        let mut devices_provider = || {
            devices_calls.set(devices_calls.get() + 1);
            expected_devices.clone()
        };

        let mut stream = UnixStream::connect(&path).unwrap();
        server
            .handle_ready(
                LISTENER_TOKEN,
                libc::POLLIN,
                &mut status_provider,
                &mut devices_provider,
            )
            .unwrap();
        assert_eq!((status_calls.get(), devices_calls.get()), (0, 0));
        let mut sources = Vec::new();
        server.poll_sources(&mut sources);
        let token = sources
            .iter()
            .find(|source| source.token != LISTENER_TOKEN)
            .unwrap()
            .token;

        for (id, command) in [
            (
                1,
                Command::Hello {
                    min_major: PROTOCOL_MAJOR,
                    max_major: PROTOCOL_MAJOR,
                },
            ),
            (2, Command::Ping),
            (
                3,
                Command::Subscribe {
                    subscriptions: vec![Subscription::Input],
                },
            ),
        ] {
            stream
                .write_all(&encode_frame(&Request { id, command }).unwrap())
                .unwrap();
            server
                .handle_ready(
                    token,
                    libc::POLLIN,
                    &mut status_provider,
                    &mut devices_provider,
                )
                .unwrap();
            let _ = read_message(&mut stream);
            assert_eq!((status_calls.get(), devices_calls.get()), (0, 0));
        }

        server.publish(Notification::Input {
            sequence: 1,
            syspath: "/sys/lazy-device".into(),
            timestamp: Timestamp {
                seconds: 1,
                micros: 2,
            },
            payload: InputPayload::Key(ButtonEvent { code: 1, state: 1 }),
        });
        server
            .handle_ready(
                token,
                libc::POLLOUT,
                &mut status_provider,
                &mut devices_provider,
            )
            .unwrap();
        assert!(matches!(
            read_message(&mut stream),
            ServerMessage::Notification(Notification::Input { .. })
        ));
        assert_eq!((status_calls.get(), devices_calls.get()), (0, 0));

        stream
            .write_all(
                &encode_frame(&Request {
                    id: 4,
                    command: Command::Status,
                })
                .unwrap(),
            )
            .unwrap();
        server
            .handle_ready(
                token,
                libc::POLLIN,
                &mut status_provider,
                &mut devices_provider,
            )
            .unwrap();
        assert_eq!(
            read_message(&mut stream),
            ServerMessage::Response {
                id: 4,
                result: ResponseResult::Status(expected_status.clone()),
            }
        );
        assert_eq!((status_calls.get(), devices_calls.get()), (1, 0));

        stream
            .write_all(
                &encode_frame(&Request {
                    id: 5,
                    command: Command::Devices,
                })
                .unwrap(),
            )
            .unwrap();
        server
            .handle_ready(
                token,
                libc::POLLIN,
                &mut status_provider,
                &mut devices_provider,
            )
            .unwrap();
        assert_eq!(
            read_message(&mut stream),
            ServerMessage::Response {
                id: 5,
                result: ResponseResult::Devices(expected_devices.clone()),
            }
        );
        assert_eq!((status_calls.get(), devices_calls.get()), (1, 1));

        drop(stream);
        server
            .handle_ready(
                token,
                libc::POLLHUP,
                &mut status_provider,
                &mut devices_provider,
            )
            .unwrap();
        assert_eq!((status_calls.get(), devices_calls.get()), (1, 1));
        assert!(!server.clients.contains_key(&token));
    }

    #[test]
    fn partial_multiple_frames_and_subscription_filtering() {
        let dir = tempdir().unwrap();
        let path = private_socket_path(dir.path(), "private");
        let mut server = IpcServer::bind(&path).unwrap();
        let (mut stream, token) = connect(&mut server);
        assert!(!server.has_input_subscribers());
        let hello = encode_frame(&Request {
            id: 1,
            command: Command::Hello {
                min_major: 1,
                max_major: 1,
            },
        })
        .unwrap();
        let ping = encode_frame(&Request {
            id: 2,
            command: Command::Ping,
        })
        .unwrap();
        stream.write_all(&hello[..hello.len() / 2]).unwrap();
        let st = status(&path);
        ready(&mut server, token, libc::POLLIN, &st, &[]).unwrap();
        stream
            .write_all(&[hello[hello.len() / 2..].as_ref(), ping.as_ref()].concat())
            .unwrap();
        ready(&mut server, token, libc::POLLIN, &st, &[]).unwrap();
        ready(&mut server, token, libc::POLLOUT, &st, &[]).unwrap();
        assert!(matches!(
            read_message(&mut stream),
            ServerMessage::Response {
                result: ResponseResult::Hello { .. },
                ..
            }
        ));
        assert!(matches!(
            read_message(&mut stream),
            ServerMessage::Response {
                result: ResponseResult::Pong,
                ..
            }
        ));

        let _ = request(
            &mut server,
            &mut stream,
            token,
            3,
            Command::Subscribe {
                subscriptions: vec![Subscription::Input],
            },
        );
        assert!(server.has_input_subscribers());
        server.publish(Notification::DeviceAdded {
            sequence: 1,
            device: device(),
        });
        server.publish(Notification::Input {
            sequence: 2,
            syspath: "/sys/test".into(),
            timestamp: Timestamp {
                seconds: 1,
                micros: 2,
            },
            payload: InputPayload::Key(ButtonEvent { code: 1, state: 1 }),
        });
        ready(&mut server, token, libc::POLLOUT, &st, &[]).unwrap();
        let notification = read_message(&mut stream);
        assert!(matches!(
            notification,
            ServerMessage::Notification(Notification::Input { sequence: 2, .. })
        ));
    }

    #[test]
    fn buffered_frames_are_rescheduled_with_a_bounded_frame_budget() {
        let dir = tempdir().unwrap();
        let path = private_socket_path(dir.path(), "private");
        let mut server = IpcServer::bind(&path).unwrap();
        let (mut stream, token) = connect(&mut server);
        let request_count = FRAME_BUDGET * 3 + 1;
        let mut batch = Vec::new();
        for id in 1..=request_count as u64 {
            let command = if id == 1 {
                Command::Hello {
                    min_major: PROTOCOL_MAJOR,
                    max_major: PROTOCOL_MAJOR,
                }
            } else {
                Command::Ping
            };
            batch.extend(encode_frame(&Request { id, command }).unwrap());
        }
        stream.write_all(&batch).unwrap();

        let st = status(&path);
        ready(&mut server, token, libc::POLLIN, &st, &[]).unwrap();
        let client = server.clients.get(&token).unwrap();
        assert_eq!(client.pending_frames.len(), request_count - FRAME_BUDGET);
        assert!(client.output.is_empty());

        let max_iterations = request_count.div_ceil(FRAME_BUDGET);
        let mut iterations = 1;
        while !server.clients[&token].pending_frames.is_empty() {
            assert!(iterations < max_iterations);
            let mut sources = Vec::new();
            server.poll_sources(&mut sources);
            let source = sources.iter().find(|source| source.token == token).unwrap();
            assert_ne!(source.events & libc::POLLOUT, 0);
            ready(&mut server, token, libc::POLLOUT, &st, &[]).unwrap();
            iterations += 1;
        }
        assert_eq!(iterations, max_iterations);

        for expected_id in 1..=request_count as u64 {
            match read_message(&mut stream) {
                ServerMessage::Response { id, result } => {
                    assert_eq!(id, expected_id);
                    if expected_id == 1 {
                        assert!(matches!(result, ResponseResult::Hello { .. }));
                    } else {
                        assert_eq!(result, ResponseResult::Pong);
                    }
                }
                message => panic!("unexpected response: {message:?}"),
            }
        }
    }

    #[test]
    fn oversized_client_isolated_without_blocking_the_test_writer() {
        let dir = tempdir().unwrap();
        let path = private_socket_path(dir.path(), "private");
        let mut server = IpcServer::bind(&path).unwrap();
        let (mut oversized, oversized_token) = connect(&mut server);
        let (mut healthy, healthy_token) = connect(&mut server);
        let st = status(&path);
        let bytes = vec![b'x'; wiiland_ipc::MAX_FRAME_BYTES + 1];
        for chunk in bytes.chunks(4 * 1024) {
            oversized.write_all(chunk).unwrap();
            ready(&mut server, oversized_token, libc::POLLIN, &st, &[]).unwrap();
        }
        let mut sources = Vec::new();
        server.poll_sources(&mut sources);
        assert!(!sources.iter().any(|source| source.token == oversized_token));

        let hello = encode_frame(&Request {
            id: 1,
            command: Command::Hello {
                min_major: PROTOCOL_MAJOR,
                max_major: PROTOCOL_MAJOR,
            },
        })
        .unwrap();
        healthy.write_all(&hello).unwrap();
        ready(&mut server, healthy_token, libc::POLLIN, &st, &[]).unwrap();
        assert!(matches!(
            read_message(&mut healthy),
            ServerMessage::Response {
                result: ResponseResult::Hello { .. },
                ..
            }
        ));
    }

    #[test]
    fn oversized_response_returns_correlated_internal_error_and_connection_survives() {
        let dir = tempdir().unwrap();
        let path = private_socket_path(dir.path(), "private");
        let mut server = IpcServer::bind(&path).unwrap();
        let (mut stream, token) = connect(&mut server);
        let _ = request(
            &mut server,
            &mut stream,
            token,
            1,
            Command::Hello {
                min_major: PROTOCOL_MAJOR,
                max_major: PROTOCOL_MAJOR,
            },
        );

        let mut oversized = device();
        oversized.syspath = "x".repeat(wiiland_ipc::MAX_FRAME_BYTES);
        stream
            .write_all(
                &encode_frame(&Request {
                    id: 2,
                    command: Command::Devices,
                })
                .unwrap(),
            )
            .unwrap();
        let st = status(&path);
        ready(&mut server, token, libc::POLLIN, &st, &[oversized]).unwrap();
        assert!(matches!(
            read_message(&mut stream),
            ServerMessage::Error {
                id: Some(2),
                error: ProtocolError {
                    code: ProtocolErrorCode::Internal,
                    ..
                }
            }
        ));
        assert!(server.clients.contains_key(&token));
        assert!(matches!(
            request(&mut server, &mut stream, token, 3, Command::Ping),
            ServerMessage::Response {
                id: 3,
                result: ResponseResult::Pong
            }
        ));
    }

    #[test]
    fn protocol_error_is_flushed_before_client_close() {
        let dir = tempdir().unwrap();
        let path = private_socket_path(dir.path(), "private");
        let mut server = IpcServer::bind(&path).unwrap();
        let (mut stream, token) = connect(&mut server);
        let invalid_first_request = encode_frame(&Request {
            id: 7,
            command: Command::Ping,
        })
        .unwrap();
        let st = status(&path);
        let mut status_provider = |_: &Path| st.clone();
        let mut devices_provider = || Vec::new();
        assert!(handle_frame(
            server.clients.get_mut(&token).unwrap(),
            &invalid_first_request,
            &path,
            &mut status_provider,
            &mut devices_provider,
        ));
        assert!(server.clients[&token].closing);
        let mut sources = Vec::new();
        server.poll_sources(&mut sources);
        let source = sources.iter().find(|source| source.token == token).unwrap();
        assert_eq!(source.events & libc::POLLIN, 0);
        assert_ne!(source.events & libc::POLLOUT, 0);

        server.publish(Notification::DeviceAdded {
            sequence: 1,
            device: device(),
        });
        assert!(server.clients.contains_key(&token));
        ready(&mut server, token, libc::POLLOUT, &st, &[]).unwrap();
        assert!(!server.clients.contains_key(&token));
        assert!(matches!(
            read_message(&mut stream),
            ServerMessage::Error {
                id: Some(7),
                error: ProtocolError {
                    code: ProtocolErrorCode::InvalidRequest,
                    ..
                }
            }
        ));
    }

    #[test]
    fn response_queue_overflow_removes_client_without_flushing() {
        let dir = tempdir().unwrap();
        let path = private_socket_path(dir.path(), "private");
        let mut server = IpcServer::bind(&path).unwrap();
        let (mut stream, token) = connect(&mut server);
        let _ = request(
            &mut server,
            &mut stream,
            token,
            1,
            Command::Hello {
                min_major: PROTOCOL_MAJOR,
                max_major: PROTOCOL_MAJOR,
            },
        );

        let client = server.clients.get_mut(&token).unwrap();
        assert!(client.queue_encoded(vec![b'x'; MAX_QUEUED_BYTES]));
        assert!(!client.queue_encoded(vec![b'y']));
        ready(
            &mut server,
            token,
            libc::POLLIN | libc::POLLOUT,
            &status(&path),
            &[],
        )
        .unwrap();
        assert!(!server.clients.contains_key(&token));
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).unwrap(), 0);
    }

    #[test]
    fn queued_byte_limit_keeps_boundary_and_evicts_first_crossing_frame() {
        let dir = tempdir().unwrap();
        let path = private_socket_path(dir.path(), "private");
        let mut server = IpcServer::bind(&path).unwrap();
        let (mut stream, token) = connect(&mut server);
        let _ = request(
            &mut server,
            &mut stream,
            token,
            1,
            Command::Hello {
                min_major: PROTOCOL_MAJOR,
                max_major: PROTOCOL_MAJOR,
            },
        );
        let _ = request(
            &mut server,
            &mut stream,
            token,
            2,
            Command::Subscribe {
                subscriptions: vec![Subscription::Input],
            },
        );

        const LARGE_FRAME: usize = 48 * 1024;
        for sequence in 0..5 {
            server.publish(input_notification_with_frame_len(sequence, LARGE_FRAME));
            assert!(server.clients.contains_key(&token));
        }
        let remainder = MAX_QUEUED_BYTES - 5 * LARGE_FRAME;
        server.publish(input_notification_with_frame_len(5, remainder));
        assert_eq!(server.clients[&token].queued_bytes, MAX_QUEUED_BYTES);

        let mut sources = Vec::new();
        server.poll_sources(&mut sources);
        assert!(sources.iter().any(|source| source.token == token));

        let crossing = input_notification_with_frame_len(6, 1024);
        server.publish(crossing);
        server.poll_sources(&mut sources);
        assert!(!sources.iter().any(|source| source.token == token));
    }
}
