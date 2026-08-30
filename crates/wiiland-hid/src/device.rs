use crate::backend::{self, Nodes, UdevContext, UdevDevice, UdevMonitor};
use crate::decode::{self, Decoder, Event, EventKind, InterfaceKind};
use crate::model::{self, InputEvent};
use crate::sys;
use std::ffi::{CString, OsStr};
use std::os::fd::RawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;

const MONITOR_EPOLL_TAG: u64 = u64::MAX;
const RECOVERY_EPOLL_TAG: u64 = u64::MAX - 1;

fn lifecycle_event(kind: EventKind) -> Event {
    Event {
        time: libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
        kind,
    }
}

const INTERFACE_KINDS: [InterfaceKind; 10] = [
    InterfaceKind::Core,
    InterfaceKind::Accel,
    InterfaceKind::Ir,
    InterfaceKind::MotionPlus,
    InterfaceKind::Nunchuk,
    InterfaceKind::Classic,
    InterfaceKind::BalanceBoard,
    InterfaceKind::Pro,
    InterfaceKind::Drums,
    InterfaceKind::Guitar,
];

const KEY_STATE_WORDS: usize = 12;
const KEY_STATE_BYTES: usize = KEY_STATE_WORDS * std::mem::size_of::<u64>();
const NATIVE_KEY_BITS: usize = std::mem::size_of::<libc::c_ulong>() * 8;
const NATIVE_KEY_WORDS: usize = KEY_STATE_BYTES / std::mem::size_of::<libc::c_ulong>();
const INPUT_NAME_BYTES: usize = 256;
type RecoveryState = ([u64; KEY_STATE_WORDS], [(u16, i32); 9], usize);

const fn eviocgname_request() -> libc::c_ulong {
    (2u64 << 30 | (INPUT_NAME_BYTES as u64) << 16 | (b'E' as u64) << 8 | 0x06) as libc::c_ulong
}

const fn eviocgkey_request() -> libc::c_ulong {
    (2u64 << 30 | (KEY_STATE_BYTES as u64) << 16 | (b'E' as u64) << 8 | 0x18) as libc::c_ulong
}

const fn eviocgabs_request(code: u16) -> libc::c_ulong {
    sys::EVIOCGABS_BASE + code as libc::c_ulong
}

fn is_child_path(root: &[u8], candidate: &[u8]) -> bool {
    candidate
        .strip_prefix(root)
        .is_some_and(|suffix| suffix.first() == Some(&b'/'))
}

fn in_watch_scope(root: &[u8], candidate: &[u8], subsystem: Option<&[u8]>) -> bool {
    (candidate == root && subsystem == Some(b"hid"))
        || (subsystem == Some(b"input") && is_child_path(root, candidate))
}

fn query_abs_state(
    kind: InterfaceKind,
    mut query: impl FnMut(u16) -> Result<i32, i32>,
) -> Result<([(u16, i32); 9], usize), i32> {
    let mut abs = [(0u16, 0i32); 9];
    let mut len = 0;
    let codes = decode::abs_codes(kind);
    if kind == InterfaceKind::Ir {
        let (pairs, remainder) = codes.as_chunks::<2>();
        debug_assert!(remainder.is_empty());
        for &[x_code, y_code] in pairs {
            let x = query(x_code);
            let y = query(y_code);
            match (x, y) {
                (Err(errno), _) if errno != -libc::EINVAL => return Err(errno),
                (_, Err(errno)) if errno != -libc::EINVAL => return Err(errno),
                (Ok(x), Ok(y)) => {
                    abs[len] = (x_code, x);
                    abs[len + 1] = (y_code, y);
                    len += 2;
                }
                _ => {
                    abs[len] = (x_code, 1023);
                    abs[len + 1] = (y_code, 1023);
                    len += 2;
                }
            }
        }
        return Ok((abs, len));
    }
    for &code in codes {
        match query(code) {
            Ok(value) => {
                abs[len] = (code, value);
                len += 1;
            }
            Err(errno) if errno == -libc::EINVAL => {}
            Err(errno) => return Err(errno),
        }
    }
    Ok((abs, len))
}

fn query_recovery_state(fd: RawFd, kind: InterfaceKind) -> Result<RecoveryState, i32> {
    let mut kernel_keys = [0 as libc::c_ulong; NATIVE_KEY_WORDS];
    if unsafe { sys::ioctl(fd, eviocgkey_request(), kernel_keys.as_mut_ptr()) } < 0 {
        return Err(-sys::errno());
    }
    let mut keys = [0u64; KEY_STATE_WORDS];
    for code in 0..KEY_STATE_WORDS * 64 {
        if kernel_keys[code / NATIVE_KEY_BITS] & (1 << (code % NATIVE_KEY_BITS)) != 0 {
            keys[code / 64] |= 1 << (code % 64);
        }
    }
    let (abs, len) = query_abs_state(kind, |code| {
        let mut info = sys::InputAbsInfo::default();
        if unsafe { sys::ioctl(fd, eviocgabs_request(code), &mut info) } < 0 {
            Err(-sys::errno())
        } else {
            Ok(info.value)
        }
    })?;
    Ok((keys, abs, len))
}

fn validate_evdev_name(name: &[u8], expected: &std::ffi::CStr) -> Result<(), i32> {
    let len = name
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(name.len());
    (&name[..len] == expected.to_bytes())
        .then_some(())
        .ok_or(-libc::ENODEV)
}

fn query_initial_state(fd: RawFd, index: usize) -> Result<RecoveryState, i32> {
    let mut name = [0u8; INPUT_NAME_BYTES];
    if unsafe { sys::ioctl(fd, eviocgname_request(), name.as_mut_ptr()) } < 0 {
        return Err(-sys::errno());
    }
    validate_evdev_name(&name, backend::IF_NAMES[index])?;
    query_recovery_state(fd, INTERFACE_KINDS[index])
}

fn epoll_input_index(tag: u64, input_count: usize) -> Option<usize> {
    let index = usize::try_from(tag.checked_sub(1)?).ok()?;
    (index < input_count).then_some(index)
}

fn monitor_epoll_status(events: u32) -> Result<(), i32> {
    if events & (libc::EPOLLERR | libc::EPOLLHUP) as u32 != 0 {
        Err(-libc::EPIPE)
    } else {
        Ok(())
    }
}

fn monitor_epoll_event(
    events: u32,
    mut receive: impl FnMut() -> Result<Option<Event>, i32>,
) -> Result<Option<Event>, i32> {
    if events & libc::EPOLLIN as u32 != 0
        && let Some(event) = receive()?
    {
        return Ok(Some(event));
    }
    monitor_epoll_status(events)?;
    Ok(None)
}

fn refresh_requires_close(
    fd: RawFd,
    old_node: &Option<PathBuf>,
    new_node: &Option<PathBuf>,
    available: bool,
) -> bool {
    fd >= 0 && (!available || old_node != new_node)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadFailureAction {
    Close,
    Refresh,
}

fn recovery_failure_watch(mut lifecycle: impl FnMut(ReadFailureAction)) -> Event {
    lifecycle(ReadFailureAction::Close);
    lifecycle(ReadFailureAction::Refresh);
    lifecycle_event(EventKind::Watch)
}

fn read_failure_watch(mut lifecycle: impl FnMut(ReadFailureAction)) -> Event {
    lifecycle(ReadFailureAction::Close);
    lifecycle(ReadFailureAction::Refresh);
    lifecycle_event(EventKind::Watch)
}

struct InputInterface {
    node: Option<PathBuf>,
    fd: RawFd,
    available: bool,
    awaiting_sync: bool,
    decoder: Decoder,
}
impl InputInterface {
    fn new(index: usize) -> Self {
        Self {
            node: None,
            fd: -1,
            available: false,
            awaiting_sync: false,
            decoder: Decoder::new(INTERFACE_KINDS[index]),
        }
    }
    fn reset_decoder(&mut self) {
        self.awaiting_sync = false;
        let normalization = self.decoder.motion_plus;
        self.decoder = Decoder::new(self.decoder.interface);
        self.decoder.motion_plus = normalization;
    }
    fn seed_state(&mut self, keys: &[u64; KEY_STATE_WORDS], abs: &[(u16, i32); 9], abs_len: usize) {
        self.decoder.seed_state(keys, &abs[..abs_len]);
    }
    fn close(&mut self) {
        sys::close_fd(&mut self.fd);
        self.reset_decoder();
    }
    fn push(&mut self, input: InputEvent) -> Option<Event> {
        self.decoder.push(input)
    }
    fn take_recovered(&mut self) -> Option<Event> {
        while self.decoder.recovery.has_pending() {
            if let Some(event) = self.decoder.push_recovered() {
                return Some(event);
            }
        }
        None
    }
}

pub struct Interface {
    pub(crate) efd: RawFd,
    pub(crate) udev: UdevContext,
    pub(crate) dev: UdevDevice,
    pub(crate) syspath: PathBuf,
    inputs: [InputInterface; 10],
    nodes: Nodes,
    monitor: Option<UdevMonitor>,
    pub(crate) rumble_fd: RawFd,
    wake_fd: RawFd,
    pub(crate) rumble_id: i32,
    next_input: usize,
    gone: bool,
}

impl Interface {
    pub fn new(path: &Path) -> Result<Self, i32> {
        let cpath = CString::new(path.as_os_str().as_encoded_bytes()).map_err(|_| -libc::EINVAL)?;
        let udev = UdevContext::new().ok_or(-libc::ENOMEM)?;
        let dev = UdevDevice::from_path(&udev, &cpath).ok_or(-libc::ENODEV)?;
        if dev.driver().is_none_or(|x| x.to_bytes() != b"wiimote")
            || dev.subsystem().is_none_or(|x| x.to_bytes() != b"hid")
        {
            return Err(-libc::ENODEV);
        }
        let syspath_c = dev.syspath().ok_or(-libc::ENODEV)?.to_owned();
        let syspath = PathBuf::from(OsStr::from_bytes(syspath_c.as_bytes()));
        let efd = unsafe { sys::epoll_create() };
        if efd < 0 {
            return Err(-sys::errno());
        }
        let wake_fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if wake_fd < 0 {
            let errno = sys::errno();
            unsafe { libc::close(efd) };
            return Err(-errno);
        }
        let mut wake_event = sys::EpollEvent {
            events: libc::EPOLLIN as u32,
            data: RECOVERY_EPOLL_TAG,
        };
        if unsafe { sys::epoll_ctl(efd, libc::EPOLL_CTL_ADD, wake_fd, &mut wake_event) } < 0 {
            let errno = sys::errno();
            unsafe {
                libc::close(wake_fd);
                libc::close(efd);
            }
            return Err(-errno);
        }
        let mut out = Self {
            efd,
            udev,
            dev,
            syspath,
            inputs: std::array::from_fn(InputInterface::new),
            nodes: Nodes::default(),
            monitor: None,
            rumble_fd: -1,
            wake_fd,
            rumble_id: -1,
            next_input: 0,
            gone: false,
        };
        out.refresh()?;
        Ok(out)
    }
    pub fn syspath(&self) -> &Path {
        &self.syspath
    }
    pub fn fd(&self) -> RawFd {
        self.efd
    }
    fn drain_recovery_wake(&self) {
        let mut value = 0u64;
        unsafe {
            libc::read(
                self.wake_fd,
                (&mut value as *mut u64).cast(),
                std::mem::size_of::<u64>(),
            );
        }
    }
    fn recovery_pending(&self) -> bool {
        self.inputs
            .iter()
            .any(|input| input.decoder.recovery.has_pending())
    }
    fn signal_recovery_if_pending(&self) {
        if !self.recovery_pending() {
            return;
        }
        let value = 1u64;
        unsafe {
            libc::write(
                self.wake_fd,
                (&value as *const u64).cast(),
                std::mem::size_of::<u64>(),
            );
        }
    }
    pub fn opened(&self) -> model::InterfaceMask {
        let mut bits = 0;
        for i in 0..10 {
            if self.inputs[i].fd >= 0 {
                bits |= backend::IF_BITS[i];
            }
        }
        model::InterfaceMask::from_bits(bits)
    }
    pub fn available(&self) -> model::InterfaceMask {
        model::InterfaceMask::from_bits(self.nodes.available)
    }
    pub fn open(&mut self, ifaces: model::InterfaceMask) -> Result<(), i32> {
        let ifaces = ifaces.bits();
        let write = ifaces & backend::WRITABLE != 0;
        let requested = ifaces & backend::ALL_INTERFACES;
        let mut first = 0;
        for i in 0..10 {
            if requested & backend::IF_BITS[i] == 0 || self.inputs[i].fd >= 0 {
                continue;
            }
            if !self.inputs[i].available {
                first = if first == 0 { -libc::ENODEV } else { first };
                continue;
            }
            let Some(path) = self.inputs[i].node.as_ref() else {
                first = if first == 0 { -libc::ENODEV } else { first };
                continue;
            };
            match backend::open_node(path, write) {
                Ok(fd) => {
                    let (keys, abs, abs_len) = match query_initial_state(fd, i) {
                        Ok(state) => state,
                        Err(errno) => {
                            unsafe { libc::close(fd) };
                            first = if first == 0 { errno } else { first };
                            continue;
                        }
                    };
                    self.inputs[i].reset_decoder();
                    self.inputs[i].seed_state(&keys, &abs, abs_len);
                    self.inputs[i].fd = fd;
                    let mut event = sys::EpollEvent {
                        events: libc::EPOLLIN as u32,
                        data: (i + 1) as u64,
                    };
                    if unsafe { sys::epoll_ctl(self.efd, libc::EPOLL_CTL_ADD, fd, &mut event) } < 0
                    {
                        let e = -sys::errno();
                        self.inputs[i].close();
                        first = if first == 0 { e } else { first };
                    }
                    if self.inputs[i].fd >= 0 && (i == 0 || i == 7) {
                        self.upload_rumble(fd);
                    }
                }
                Err(e) => {
                    if first == 0 {
                        first = e
                    }
                }
            }
        }
        if first != 0 { Err(first) } else { Ok(()) }
    }
    fn upload_rumble(&mut self, fd: RawFd) {
        let mut effect = libc::ff_effect {
            type_: sys::FF_RUMBLE,
            id: -1,
            direction: 0,
            trigger: libc::ff_trigger {
                button: 0,
                interval: 0,
            },
            replay: libc::ff_replay {
                length: 0,
                delay: 0,
            },
            u: Default::default(),
        };
        let rumble = effect.u.as_mut_ptr().cast::<libc::ff_rumble_effect>();
        unsafe {
            rumble.write(libc::ff_rumble_effect {
                strong_magnitude: 1,
                weak_magnitude: 0,
            });
        }
        let effect_ptr: *mut libc::ff_effect = &mut effect;
        if unsafe { sys::ioctl(fd, sys::EVIOCSFF, effect_ptr) } >= 0 {
            self.rumble_fd = fd;
            self.rumble_id = i32::from(effect.id);
        }
    }
    pub fn close(&mut self, ifaces: model::InterfaceMask) {
        let requested = ifaces.bits() & backend::ALL_INTERFACES;
        for i in 0..10 {
            if requested & backend::IF_BITS[i] != 0 && self.inputs[i].fd >= 0 {
                if self.rumble_fd == self.inputs[i].fd {
                    self.rumble_fd = -1;
                    self.rumble_id = -1;
                }
                unsafe {
                    sys::epoll_ctl(
                        self.efd,
                        libc::EPOLL_CTL_DEL,
                        self.inputs[i].fd,
                        ptr::null_mut(),
                    )
                };
                self.inputs[i].close();
            }
        }
    }
    pub fn watch(&mut self, enabled: bool) -> Result<(), i32> {
        if !enabled {
            if let Some(mon) = self.monitor.take() {
                let fd = unsafe { sys::udev_monitor_get_fd(mon.0) };
                unsafe { sys::epoll_ctl(self.efd, libc::EPOLL_CTL_DEL, fd, ptr::null_mut()) };
            }
            return Ok(());
        }
        if self.monitor.is_some() {
            return Ok(());
        }
        let name = CString::new("udev").unwrap();
        let p = unsafe { sys::udev_monitor_new_from_netlink(self.udev.0, name.as_ptr()) };
        if p.is_null() {
            return Err(-libc::ENOMEM);
        }
        let mon = UdevMonitor(p);
        let input = CString::new("input").unwrap();
        let hid = CString::new("hid").unwrap();
        if unsafe {
            sys::udev_monitor_filter_add_match_subsystem_devtype(p, input.as_ptr(), ptr::null())
        } != 0
            || unsafe {
                sys::udev_monitor_filter_add_match_subsystem_devtype(p, hid.as_ptr(), ptr::null())
            } != 0
            || unsafe { sys::udev_monitor_enable_receiving(p) } != 0
        {
            return Err(-libc::ENOMEM);
        }
        let fd = unsafe { sys::udev_monitor_get_fd(p) };
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(-sys::errno());
        };
        let mut ep = sys::EpollEvent {
            events: libc::EPOLLIN as u32,
            data: MONITOR_EPOLL_TAG,
        };
        if unsafe { sys::epoll_ctl(self.efd, libc::EPOLL_CTL_ADD, fd, &mut ep) } < 0 {
            return Err(-sys::errno());
        };
        self.monitor = Some(mon);
        Ok(())
    }
    fn refresh(&mut self) -> Result<(), i32> {
        backend::refresh_nodes(&self.udev, &self.dev, &mut self.nodes)?;
        for i in 0..10 {
            let available = self.nodes.available & backend::IF_BITS[i] != 0;
            let node = self.nodes.nodes[i].clone();
            if refresh_requires_close(self.inputs[i].fd, &self.inputs[i].node, &node, available) {
                self.close(model::InterfaceMask::from_bits(backend::IF_BITS[i]));
            }
            self.inputs[i].available = available;
            self.inputs[i].node = node;
        }
        Ok(())
    }
    fn monitor_event(&mut self) -> Result<Option<Event>, i32> {
        let Some(monitor) = self.monitor.as_ref().map(|monitor| monitor.0) else {
            return Ok(None);
        };
        loop {
            let p = unsafe { sys::udev_monitor_receive_device(monitor) };
            if p.is_null() {
                return Ok(None);
            }
            let event = UdevDevice(p);
            let action = event.action().map(|value| value.to_bytes());
            let path = event.syspath().map(|value| value.to_bytes());
            let subsystem = event.subsystem().map(|value| value.to_bytes());
            let root = self.syspath.as_os_str().as_bytes();

            if action == Some(b"remove") && path == Some(root) {
                if self.gone {
                    continue;
                }
                self.gone = true;
                self.nodes.available = 0;
                self.close(model::InterfaceMask::ALL);
                for input in &mut self.inputs {
                    input.available = false;
                    input.node = None;
                }
                self.nodes.nodes = std::array::from_fn(|_| None);
                return Ok(Some(lifecycle_event(EventKind::Gone)));
            }
            let Some(path) = path else {
                continue;
            };
            if self.gone
                || !in_watch_scope(root, path, subsystem)
                || (action != Some(b"add")
                    && action != Some(b"change")
                    && action != Some(b"remove"))
            {
                continue;
            }
            let old_available = self.nodes.available;
            let old_nodes = self.nodes.nodes.clone();
            self.refresh()?;
            if old_available == self.nodes.available && old_nodes == self.nodes.nodes {
                continue;
            }
            return Ok(Some(lifecycle_event(EventKind::Watch)));
        }
    }
    pub fn dispatch(&mut self) -> Result<Event, i32> {
        self.drain_recovery_wake();
        for offset in 0..self.inputs.len() {
            let index = (self.next_input + offset) % self.inputs.len();
            if let Some(event) = self.inputs[index].take_recovered() {
                self.next_input = (index + 1) % self.inputs.len();
                self.signal_recovery_if_pending();
                return Ok(event);
            }
        }

        let mut eps = [sys::EpollEvent::default(); 32];
        let n = unsafe { sys::epoll_wait(self.efd, &mut eps, 0) };
        if n < 0 {
            let errno = sys::errno();
            return if errno == libc::EINTR {
                Err(-libc::EAGAIN)
            } else {
                Err(-errno)
            };
        }
        for ep in eps.iter().take(n as usize) {
            let tag = ep.data;
            if tag == MONITOR_EPOLL_TAG {
                if let Some(event) = monitor_epoll_event(ep.events, || self.monitor_event())? {
                    return Ok(event);
                }
                continue;
            }
            if tag == RECOVERY_EPOLL_TAG {
                continue;
            }
            let Some(index) = epoll_input_index(tag, self.inputs.len()) else {
                continue;
            };
            if let Some(event) = self.read_one(index)? {
                self.next_input = (index + 1) % self.inputs.len();
                self.signal_recovery_if_pending();
                return Ok(event);
            }
        }
        Err(-libc::EAGAIN)
    }
    fn read_one(&mut self, index: usize) -> Result<Option<Event>, i32> {
        let fd = self.inputs[index].fd;
        if fd < 0 {
            return Ok(None);
        }
        let mut input = sys::InputEvent::default();
        loop {
            let n = unsafe {
                libc::read(
                    fd,
                    (&mut input as *mut sys::InputEvent).cast::<libc::c_void>(),
                    std::mem::size_of::<sys::InputEvent>(),
                )
            };
            if n == std::mem::size_of::<sys::InputEvent>() as isize {
                break;
            }
            if n < 0 {
                let errno = sys::errno();
                if errno == libc::EINTR {
                    continue;
                }
                if errno == libc::EAGAIN {
                    return Ok(None);
                }
            }
            let event = read_failure_watch(|action| match action {
                ReadFailureAction::Close => {
                    self.close(model::InterfaceMask::from_bits(backend::IF_BITS[index]))
                }
                ReadFailureAction::Refresh => {
                    let _ = self.refresh();
                }
            });
            return Ok(Some(event));
        }

        let input = InputEvent {
            time: input.time,
            event_type: input.type_,
            code: input.code,
            value: input.value,
        };
        if self.inputs[index].awaiting_sync {
            if input.event_type != model::EV_SYN || input.code != model::SYN_REPORT {
                return Ok(None);
            }
            let (keys, abs, abs_len) =
                match query_recovery_state(fd, self.inputs[index].decoder.interface) {
                    Ok(state) => state,
                    Err(_) => {
                        let event = recovery_failure_watch(|action| match action {
                            ReadFailureAction::Close => {
                                self.close(model::InterfaceMask::from_bits(backend::IF_BITS[index]))
                            }
                            ReadFailureAction::Refresh => {
                                let _ = self.refresh();
                            }
                        });
                        return Ok(Some(event));
                    }
                };
            self.inputs[index]
                .decoder
                .recover(&keys, &abs[..abs_len], input.time);
            self.inputs[index].awaiting_sync = false;
            return Ok(self.inputs[index].take_recovered());
        }
        if input.event_type == model::EV_SYN && input.code == model::SYN_DROPPED {
            self.inputs[index].decoder.push(input);
            self.inputs[index].awaiting_sync = true;
            return Ok(None);
        }
        Ok(self.inputs[index].push(input))
    }
    pub fn rumble(&mut self, on: bool) -> Result<(), i32> {
        if self.rumble_fd < 0 || self.rumble_id < 0 {
            return Err(-libc::ENODEV);
        };
        let e = sys::InputEvent {
            type_: sys::EV_FF,
            code: self.rumble_id as u16,
            value: if on { 1 } else { 0 },
            ..Default::default()
        };
        let p: *const u8 = (&e as *const sys::InputEvent).cast();
        let mut n = 0;
        while n < std::mem::size_of::<sys::InputEvent>() {
            let r = unsafe {
                libc::write(
                    self.rumble_fd,
                    p.add(n).cast::<libc::c_void>(),
                    std::mem::size_of::<sys::InputEvent>() - n,
                )
            };
            if r > 0 {
                n += r as usize
            } else if r < 0 && sys::errno() == libc::EINTR {
                continue;
            } else {
                return Err(-sys::errno());
            }
        }
        Ok(())
    }
    pub fn get_led(&self, led: usize) -> Result<bool, i32> {
        if led >= 4 {
            return Err(-libc::EINVAL);
        };
        let p = self.nodes.leds[led].as_ref().ok_or(-libc::ENODEV)?;
        let v = backend::read_attr(p)?;
        Ok(v.first() == Some(&b'1'))
    }
    pub fn set_led(&self, led: usize, on: bool) -> Result<(), i32> {
        if led >= 4 {
            return Err(-libc::EINVAL);
        };
        backend::write_attr(
            self.nodes.leds[led].as_ref().ok_or(-libc::ENODEV)?,
            if on { b"1" } else { b"0" },
        )
    }
    pub fn battery(&self) -> Result<u8, i32> {
        let p = self.nodes.battery.as_ref().ok_or(-libc::ENODEV)?;
        let v = backend::read_attr(p)?;
        std::str::from_utf8(&v)
            .ok()
            .and_then(|x| x.trim().parse().ok())
            .ok_or(-libc::EINVAL)
    }
    pub fn attr(&self, name: &str) -> Result<Vec<u8>, i32> {
        let p = self.syspath.join(name);
        backend::read_attr(&p)
    }
    pub fn set_mp_normalization(&mut self, x: i32, y: i32, z: i32, f: i32) {
        self.inputs[3].decoder.set_mp_normalization(x, y, z, f);
    }
    pub fn mp_normalization(&self) -> ([i32; 3], i32) {
        let (x, y, z, factor) = self.inputs[3].decoder.mp_normalization();
        ([x, y, z], factor)
    }
}
impl Drop for Interface {
    fn drop(&mut self) {
        self.close(model::InterfaceMask::ALL);
        let _ = self.watch(false);
        sys::close_fd(&mut self.rumble_fd);
        sys::close_fd(&mut self.wake_fd);
        sys::close_fd(&mut self.efd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(event_type: u16, code: u16, value: i32) -> InputEvent {
        InputEvent {
            time: libc::timeval {
                tv_sec: 7,
                tv_usec: 11,
            },
            event_type,
            code,
            value,
        }
    }

    fn set_key(keys: &mut [u64; KEY_STATE_WORDS], code: u16) {
        keys[code as usize / 64] |= 1 << (code as usize % 64);
    }

    #[test]
    fn persistent_decoder_routes_keys_and_complete_absolute_frames() {
        let mut core = InputInterface::new(0);
        let key = core.push(event(model::EV_KEY, model::BTN_1, 1)).unwrap();
        assert_eq!(
            key.kind,
            EventKind::Key(decode::Key {
                code: model::BUTTON_ONE,
                state: 1
            })
        );
        let mut accel = InputInterface::new(1);
        assert!(
            accel
                .push(event(model::EV_ABS, model::ABS_RX, 10))
                .is_none()
        );
        assert!(
            accel
                .push(event(model::EV_ABS, model::ABS_RY, -20))
                .is_none()
        );
        assert!(
            accel
                .push(event(model::EV_ABS, model::ABS_RZ, 30))
                .is_none()
        );
        let frame = accel
            .push(event(model::EV_SYN, model::SYN_REPORT, 0))
            .unwrap();
        assert_eq!(
            frame.kind,
            EventKind::Accel(decode::Abs {
                x: 10,
                y: -20,
                z: 30
            })
        );
    }

    #[test]
    fn decoder_recovery_snapshot_drains_key_transitions_before_abs_report() {
        let mut input = InputInterface::new(4);
        assert!(input.push(event(model::EV_KEY, model::BTN_C, 1)).is_some());
        assert!(
            input
                .push(event(model::EV_SYN, model::SYN_DROPPED, 0))
                .is_none()
        );
        let mut keys = [0u64; KEY_STATE_WORDS];
        set_key(&mut keys, model::BTN_Z);
        let at = libc::timeval {
            tv_sec: 8,
            tv_usec: 12,
        };
        input.decoder.recover(
            &keys,
            &[
                (model::ABS_HAT0X, 4),
                (model::ABS_HAT0Y, 5),
                (model::ABS_RX, 6),
                (model::ABS_RY, 7),
                (model::ABS_RZ, 8),
            ],
            at,
        );
        let c_release = input.take_recovered().unwrap();
        assert_eq!(
            c_release.kind,
            EventKind::NunchukKey(decode::Key {
                code: model::BUTTON_C,
                state: 0
            })
        );
        let z_press = input.take_recovered().unwrap();
        assert_eq!(
            z_press.kind,
            EventKind::NunchukKey(decode::Key {
                code: model::BUTTON_Z,
                state: 1
            })
        );
        let frame = input.take_recovered().unwrap();
        assert_eq!(
            frame.kind,
            EventKind::NunchukMove([
                decode::Abs { x: 4, y: 5, z: 0 },
                decode::Abs { x: 6, y: 7, z: 8 }
            ])
        );
        assert!(input.take_recovered().is_none());
    }

    #[test]
    fn motion_plus_normalization_is_applied_by_the_open_input_decoder() {
        let mut input = InputInterface::new(3);
        input.decoder.set_mp_normalization(1, 2, 3, 0);
        input.push(event(model::EV_ABS, model::ABS_RX, 11));
        input.push(event(model::EV_ABS, model::ABS_RY, 22));
        input.push(event(model::EV_ABS, model::ABS_RZ, 33));
        let frame = input
            .push(event(model::EV_SYN, model::SYN_REPORT, 0))
            .unwrap();
        assert_eq!(
            frame.kind,
            EventKind::MotionPlus(decode::Abs {
                x: 10,
                y: 20,
                z: 30
            })
        );
    }

    #[test]
    fn watch_scope_accepts_only_this_hid_interface_and_its_input_children() {
        let root = b"/sys/devices/pci/hid";
        assert!(in_watch_scope(root, root, Some(b"hid")));
        assert!(in_watch_scope(
            root,
            b"/sys/devices/pci/hid/input/input1",
            Some(b"input")
        ));
        assert!(!in_watch_scope(
            root,
            b"/sys/devices/pci/hid/input/input1",
            Some(b"hid")
        ));
        assert!(!is_child_path(root, root));
        assert!(!is_child_path(root, b"/sys/devices/pci/hid-other/input1"));
        assert!(!is_child_path(root, b"/sys/devices/pci/unrelated/input1"));
    }

    #[test]
    fn recovery_key_ioctl_requests_the_full_linux_key_bitmap() {
        assert_eq!(eviocgname_request(), 0x8100_4506);
        assert_eq!(KEY_STATE_BYTES, 96);
        assert_eq!(eviocgabs_request(model::ABS_RX), 0x8018_4543);
        assert_eq!(eviocgkey_request(), 0x8060_4518);
    }

    #[test]
    fn invalid_epoll_tags_are_rejected_without_subtraction_underflow() {
        assert_eq!(epoll_input_index(0, 10), None);
        assert_eq!(epoll_input_index(1, 10), Some(0));
        assert_eq!(epoll_input_index(10, 10), Some(9));
        assert_eq!(epoll_input_index(11, 10), None);
        assert_eq!(epoll_input_index(u64::MAX, 10), None);
    }

    #[test]
    fn initial_snapshot_seeds_keys_and_axes_without_synthetic_emissions() {
        let mut input = InputInterface::new(4);
        let mut keys = [0u64; KEY_STATE_WORDS];
        set_key(&mut keys, model::BTN_C);
        let mut abs = [(0u16, 0i32); 9];
        abs[..5].copy_from_slice(&[
            (model::ABS_HAT0X, 10),
            (model::ABS_HAT0Y, 20),
            (model::ABS_RX, 30),
            (model::ABS_RY, 40),
            (model::ABS_RZ, 50),
        ]);
        input.seed_state(&keys, &abs, 5);
        assert!(input.take_recovered().is_none());
        assert_ne!(
            input.decoder.recovery.key_state()[model::BTN_C as usize / 64]
                & (1 << (model::BTN_C as usize % 64)),
            0
        );
        let frame = input
            .push(event(model::EV_SYN, model::SYN_REPORT, 0))
            .unwrap();
        assert_eq!(
            frame.kind,
            EventKind::NunchukMove([
                decode::Abs { x: 10, y: 20, z: 0 },
                decode::Abs {
                    x: 30,
                    y: 40,
                    z: 50
                }
            ])
        );
    }

    #[test]
    fn evdev_name_validation_requires_the_expected_interface_identity() {
        let expected = backend::IF_NAMES[0];
        let mut exact = [0u8; INPUT_NAME_BYTES];
        exact[..expected.to_bytes().len()].copy_from_slice(expected.to_bytes());
        assert_eq!(validate_evdev_name(&exact, expected), Ok(()));

        let mut wrong = exact;
        wrong[0] = b'X';
        assert_eq!(validate_evdev_name(&wrong, expected), Err(-libc::ENODEV));

        let mut suffix = exact;
        let end = expected.to_bytes().len();
        suffix[end] = b'!';
        suffix[end + 1] = 0;
        assert_eq!(validate_evdev_name(&suffix, expected), Err(-libc::ENODEV));
    }

    #[test]
    fn unsupported_recovery_axes_are_compacted_and_keep_decoder_sentinels() {
        let (abs, len) = query_abs_state(InterfaceKind::Ir, |code| {
            if code == model::ABS_HAT0Y {
                Err(-libc::EINVAL)
            } else {
                Ok(i32::from(code))
            }
        })
        .unwrap();
        assert_eq!(len, 8);
        assert_eq!(
            (abs[0], abs[1]),
            ((model::ABS_HAT0X, 1023), (model::ABS_HAT0Y, 1023))
        );
        let mut input = InputInterface::new(2);
        input.seed_state(&[0; KEY_STATE_WORDS], &abs, len);
        let frame = input
            .push(event(model::EV_SYN, model::SYN_REPORT, 0))
            .unwrap();
        let EventKind::Ir(points) = frame.kind else {
            panic!("expected IR frame")
        };
        assert_eq!((points[0].x, points[0].y), (1023, 1023));
        let (compact, compact_len) = query_abs_state(InterfaceKind::Accel, |code| {
            if code == model::ABS_RY {
                Err(-libc::EINVAL)
            } else {
                Ok(i32::from(code))
            }
        })
        .unwrap();
        assert_eq!(compact_len, 2);
        assert_eq!(
            &compact[..compact_len],
            &[
                (model::ABS_RX, i32::from(model::ABS_RX)),
                (model::ABS_RZ, i32::from(model::ABS_RZ))
            ]
        );
        assert_eq!(
            query_abs_state(InterfaceKind::Accel, |_| Err(-libc::EIO)),
            Err(-libc::EIO)
        );
        assert_eq!(
            query_abs_state(InterfaceKind::Ir, |code| {
                if code == model::ABS_HAT0X {
                    Err(-libc::EINVAL)
                } else if code == model::ABS_HAT0Y {
                    Err(-libc::EIO)
                } else {
                    Ok(0)
                }
            }),
            Err(-libc::EIO)
        );
        let (none, none_len) =
            query_abs_state(InterfaceKind::Accel, |_| Err(-libc::EINVAL)).unwrap();
        assert_eq!(none_len, 0);
        let mut recovered = InputInterface::new(1);
        recovered.decoder.recover(
            &[0; KEY_STATE_WORDS],
            &none[..none_len],
            libc::timeval {
                tv_sec: 9,
                tv_usec: 13,
            },
        );
        let frame = recovered.take_recovered().unwrap();
        assert_eq!(
            frame.kind,
            EventKind::Accel(decode::Abs { x: 0, y: 0, z: 0 })
        );
    }

    #[test]
    fn recovery_failure_closes_and_refreshes_before_returning_watch() {
        let mut actions = Vec::new();
        let event = recovery_failure_watch(|action| actions.push(action));
        assert_eq!(
            actions,
            [ReadFailureAction::Close, ReadFailureAction::Refresh]
        );
        assert_eq!(event.kind, EventKind::Watch);
    }

    #[test]
    fn refresh_closes_only_open_replaced_or_unavailable_nodes() {
        let old = Some(PathBuf::from("/dev/input/event1"));
        let same = Some(PathBuf::from("/dev/input/event1"));
        let replacement = Some(PathBuf::from("/dev/input/event2"));
        assert!(!refresh_requires_close(4, &old, &same, true));
        assert!(refresh_requires_close(4, &old, &replacement, true));
        assert!(refresh_requires_close(4, &old, &same, false));
        assert!(!refresh_requires_close(-1, &old, &replacement, true));
    }

    #[test]
    fn monitor_error_and_hangup_are_broken_pipe() {
        assert_eq!(monitor_epoll_status(libc::EPOLLIN as u32), Ok(()));
        assert_eq!(
            monitor_epoll_status(libc::EPOLLERR as u32),
            Err(-libc::EPIPE)
        );
        assert_eq!(
            monitor_epoll_status(libc::EPOLLHUP as u32),
            Err(-libc::EPIPE)
        );

        let queued = lifecycle_event(EventKind::Watch);
        let mut receives = 0;
        let delivered = monitor_epoll_event((libc::EPOLLIN | libc::EPOLLHUP) as u32, || {
            receives += 1;
            Ok(Some(queued))
        })
        .unwrap()
        .unwrap();
        assert_eq!(delivered.kind, EventKind::Watch);
        assert_eq!(receives, 1);

        assert!(matches!(
            monitor_epoll_event((libc::EPOLLIN | libc::EPOLLERR) as u32, || Ok(None)),
            Err(errno) if errno == -libc::EPIPE
        ));

        let mut called = false;
        assert!(matches!(
            monitor_epoll_event(libc::EPOLLHUP as u32, || {
                called = true;
                Ok(None)
            }),
            Err(errno) if errno == -libc::EPIPE
        ));
        assert!(!called);
    }

    #[test]
    fn read_failure_seam_closes_then_refreshes_before_watch() {
        let mut actions = Vec::new();
        let event = read_failure_watch(|action| actions.push(action));
        assert_eq!(
            actions,
            [ReadFailureAction::Close, ReadFailureAction::Refresh]
        );
        assert_eq!(event.kind, EventKind::Watch);
    }

    #[test]
    fn decoder_reset_preserves_fractional_motion_plus_normalization() {
        let mut input = InputInterface::new(3);
        input.decoder.set_mp_normalization(1, 2, 3, 1);
        input.push(event(model::EV_ABS, model::ABS_RX, 11));
        input.push(event(model::EV_ABS, model::ABS_RY, 22));
        input.push(event(model::EV_ABS, model::ABS_RZ, 33));
        assert!(
            input
                .push(event(model::EV_SYN, model::SYN_REPORT, 0))
                .is_some()
        );
        let exact = input.decoder.motion_plus;

        input.reset_decoder();

        assert_eq!(input.decoder.motion_plus, exact);
    }
}
