//! Single-thread monitor/device reactor.
use crate::bridge::{BridgeAction, BridgeDevice};
use crate::signal::SignalPipe;
use crate::uinput::{Backend, SystemBackend};
use std::cell::Cell;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};
use wiiland_core::{Config, TraceConfig};
use wiiland_hid::Monitor;

pub const MAX_DEVICES: usize = 32;
pub const POINTER_TICK: Duration = Duration::from_micros(16_000);
pub const RECONCILE_TICK: Duration = Duration::from_secs(1);

type DiagnosticSink = Box<dyn FnMut(&str)>;

enum Lifecycle<'a> {
    Add(&'a Path),
    Remove(&'a Path),
    Gone(&'a Path),
    Reconcile {
        snapshot: usize,
        queued: usize,
        active: usize,
    },
    Error {
        operation: &'static str,
        path: Option<&'a Path>,
        code: i32,
    },
}

impl fmt::Display for Lifecycle<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add(path) => write!(f, "wiilandd: add: {}", path.display()),
            Self::Remove(path) => write!(f, "wiilandd: remove: {}", path.display()),
            Self::Gone(path) => write!(f, "wiilandd: gone: {}", path.display()),
            Self::Reconcile {
                snapshot,
                queued,
                active,
            } => write!(
                f,
                "wiilandd: reconcile: snapshot={snapshot} queued={queued} active={active}"
            ),
            Self::Error {
                operation,
                path: Some(path),
                code,
            } => write!(f, "wiilandd: error: {operation} {}: {code}", path.display()),
            Self::Error {
                operation,
                path: None,
                code,
            } => write!(f, "wiilandd: error: {operation}: {code}"),
        }
    }
}

struct Diagnostics {
    enabled: bool,
    sink: DiagnosticSink,
}

impl Diagnostics {
    fn stderr() -> Self {
        Self {
            enabled: false,
            sink: Box::new(|line| eprintln!("{line}")),
        }
    }

    fn emit(&mut self, event: Lifecycle<'_>) {
        if self.enabled {
            (self.sink)(&event.to_string());
        }
    }
}

fn outputs_enabled(dry_run: bool) -> bool {
    !dry_run
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) -> bool {
    if paths.contains(&path) {
        false
    } else {
        paths.push(path);
        true
    }
}

fn missing_slots<'a>(
    active: impl Iterator<Item = (usize, &'a Path)>,
    present: &[PathBuf],
) -> Vec<usize> {
    active
        .filter_map(|(slot, path)| (!present.iter().any(|item| item == path)).then_some(slot))
        .collect()
}

pub struct Runtime<B: Backend + Clone = SystemBackend> {
    pub config: Config,
    pub slots: [Option<BridgeDevice<B>>; MAX_DEVICES],
    monitor: Option<Monitor>,
    signal: SignalPipe,
    backend: B,
    dry_run: bool,
    diagnostics: Diagnostics,
    trace: TraceConfig,
    trace_sequence: Rc<Cell<u64>>,
}
impl Runtime<SystemBackend> {
    pub fn new(config: Config) -> Result<Self, i32> {
        Self::with_backend(config, SystemBackend)
    }
}
impl<B: Backend + Clone> Runtime<B> {
    pub fn with_backend(config: Config, backend: B) -> Result<Self, i32> {
        Ok(Self {
            config,
            slots: std::array::from_fn(|_| None),
            monitor: None,
            signal: SignalPipe::install()?,
            backend,
            dry_run: false,
            diagnostics: Diagnostics::stderr(),
            trace: TraceConfig::default(),
            trace_sequence: Rc::new(Cell::new(0)),
        })
    }
    pub fn set_dry_run(&mut self, value: bool) {
        self.dry_run = value;
    }
    pub fn set_verbose(&mut self, value: bool) {
        self.diagnostics.enabled = value;
    }
    pub fn set_trace(&mut self, value: TraceConfig) {
        self.trace = value;
    }
    pub fn add_path(&mut self, path: impl AsRef<Path>) -> Result<bool, i32> {
        let path = path.as_ref();
        if self.find(path).is_some() {
            return Ok(false);
        }
        let Some(slot) = self.slots.iter().position(Option::is_none) else {
            let code = -libc::ENOSPC;
            self.diagnostics.emit(Lifecycle::Error {
                operation: "add",
                path: Some(path),
                code,
            });
            return Err(code);
        };
        match BridgeDevice::with_backend_outputs(
            path,
            &self.config,
            self.backend.clone(),
            outputs_enabled(self.dry_run),
        ) {
            Ok(mut dev) => {
                if self.trace.enabled {
                    dev.set_trace_sink_with_sequence(
                        self.trace.filter,
                        Rc::clone(&self.trace_sequence),
                        |line| {
                            println!("{}", line);
                        },
                    );
                }
                self.slots[slot] = Some(dev);
                self.diagnostics.emit(Lifecycle::Add(path));
                Ok(true)
            }
            Err(code) => {
                self.diagnostics.emit(Lifecycle::Error {
                    operation: "add",
                    path: Some(path),
                    code,
                });
                Err(code)
            }
        }
    }
    fn find(&self, path: &Path) -> Option<usize> {
        self.slots
            .iter()
            .position(|x| x.as_ref().is_some_and(|d| d.path() == path))
    }
    fn reconcile(&mut self) {
        let Some(mut snapshot) = Monitor::new(false, false) else {
            self.diagnostics.emit(Lifecycle::Error {
                operation: "reconcile snapshot",
                path: None,
                code: -libc::ENOMEM,
            });
            return;
        };
        let mut paths = Vec::<PathBuf>::new();
        while let Some(path) = snapshot.poll() {
            push_unique(&mut paths, path);
        }
        let snapshot_count = paths.len();
        let active = self.slots.iter().filter(|slot| slot.is_some()).count();
        let remove = missing_slots(
            self.slots
                .iter()
                .enumerate()
                .filter_map(|(slot, dev)| dev.as_ref().map(|dev| (slot, dev.path()))),
            &paths,
        );
        for slot in remove {
            if let Some(dev) = self.slots[slot].take() {
                self.diagnostics.emit(Lifecycle::Remove(dev.path()));
            }
        }
        for path in paths {
            let _ = self.add_path(path);
        }

        let mut queued = Vec::new();
        let mut queued_count = 0;
        if let Some(mon) = self.monitor.as_mut() {
            while let Some(path) = mon.poll() {
                queued_count += 1;
                push_unique(&mut queued, path);
            }
        }
        for path in queued {
            let _ = self.add_path(path);
        }
        self.diagnostics.emit(Lifecycle::Reconcile {
            snapshot: snapshot_count,
            queued: queued_count,
            active,
        });
    }
    fn poll_once(&mut self, timeout_ms: i32, single: bool) -> Result<bool, i32> {
        let mut fds = [libc::pollfd {
            fd: self.signal.read_fd(),
            events: libc::POLLIN,
            revents: 0,
        }; MAX_DEVICES + 2];
        let mut owners = [-2i32; MAX_DEVICES + 2];
        let mut n = 1usize;
        if let Some(mon) = self.monitor.as_mut()
            && let Some(fd) = mon.fd(false)
        {
            fds[n] = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            owners[n] = -1;
            n += 1;
        }
        for (i, dev) in self.slots.iter().enumerate() {
            if let Some(dev) = dev {
                fds[n] = libc::pollfd {
                    fd: dev.fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                owners[n] = i as i32;
                n += 1;
            }
        }
        let result = unsafe { libc::poll(fds.as_mut_ptr(), n as libc::nfds_t, timeout_ms) };
        if result < 0 {
            let e = io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::EINTR) {
                return Ok(false);
            }
            let code = -e.raw_os_error().unwrap_or(libc::EIO);
            self.diagnostics.emit(Lifecycle::Error {
                operation: "poll",
                path: None,
                code,
            });
            return Err(code);
        }
        if self.signal.requested() {
            let _ = self.signal.drain();
            return Ok(true);
        }
        if result > 0 {
            for i in 0..n {
                if fds[i].revents == 0 {
                    continue;
                }
                match owners[i] {
                    -2 => {
                        let _ = self.signal.drain();
                        return Ok(true);
                    }
                    -1 => {
                        self.reconcile();
                        break;
                    }
                    slot => {
                        let slot = slot as usize;
                        let outcome = self.slots[slot].as_mut().map(|dev| dev.drain());
                        match outcome {
                            Some(Ok(BridgeAction::Continue)) | None => {}
                            Some(Ok(BridgeAction::Gone)) => {
                                if let Some(dev) = self.slots[slot].take() {
                                    self.diagnostics.emit(Lifecycle::Gone(dev.path()));
                                }
                            }
                            Some(Err(code)) => {
                                if let Some(dev) = self.slots[slot].take() {
                                    self.diagnostics.emit(Lifecycle::Error {
                                        operation: "drain",
                                        path: Some(dev.path()),
                                        code,
                                    });
                                }
                                if single {
                                    return Err(code);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(false)
    }
    fn loop_run(&mut self, single: bool) -> Result<(), i32> {
        let mut next_pointer = Instant::now() + POINTER_TICK;
        let mut next_reconcile = Instant::now() + RECONCILE_TICK;
        loop {
            if self.signal.requested() {
                return Ok(());
            }
            let now = Instant::now();
            let mut deadline = next_pointer.min(next_reconcile);
            if single {
                deadline = next_pointer;
            }
            let timeout = deadline.saturating_duration_since(now);
            let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
            if self.poll_once(ms, single)? {
                return Ok(());
            }
            let now = Instant::now();
            if now >= next_pointer {
                for slot in 0..MAX_DEVICES {
                    let active = self.slots[slot]
                        .as_ref()
                        .is_some_and(|dev| dev.pointer.pointer_keys() != 0);
                    if active {
                        let result = self.slots[slot].as_mut().map(|dev| dev.tick_pointer());
                        if let Some(Err(code)) = result {
                            if let Some(dev) = self.slots[slot].take() {
                                self.diagnostics.emit(Lifecycle::Error {
                                    operation: "pointer tick",
                                    path: Some(dev.path()),
                                    code,
                                });
                            }
                            if single {
                                return Err(code);
                            }
                        }
                    }
                }
                while next_pointer <= now {
                    next_pointer += POINTER_TICK;
                }
            }
            if !single && now >= next_reconcile {
                self.reconcile();
                while next_reconcile <= now {
                    next_reconcile += RECONCILE_TICK;
                }
            }
            if single && self.slots.iter().all(Option::is_none) {
                return Ok(());
            }
        }
    }
    pub fn run_monitor(&mut self) -> Result<(), i32> {
        let Some(mon) = Monitor::new(true, false) else {
            let code = -libc::ENOMEM;
            self.diagnostics.emit(Lifecycle::Error {
                operation: "monitor",
                path: None,
                code,
            });
            return Err(code);
        };
        self.monitor = Some(mon);
        self.reconcile();
        self.loop_run(false)
    }
    pub fn run_single(&mut self, path: impl AsRef<Path>) -> Result<(), i32> {
        self.add_path(path)?;
        self.loop_run(true)
    }
}
impl<B: Backend + Clone> Drop for Runtime<B> {
    fn drop(&mut self) {
        for slot in &mut self.slots {
            slot.take();
        }
        self.monitor.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn empty_event_queue_does_not_remove_snapshot_devices() {
        let path = PathBuf::from("/sys/devices/wiimote0");
        let present = vec![path.clone()];
        let active = [(0, path.as_path())];

        assert!(missing_slots(active.into_iter(), &present).is_empty());
    }

    #[test]
    fn snapshot_removals_and_queued_additions_reconcile_independently() {
        let retained = PathBuf::from("/sys/devices/wiimote0");
        let removed = PathBuf::from("/sys/devices/wiimote1");
        let snapshot_add = PathBuf::from("/sys/devices/wiimote2");
        let queued_add = PathBuf::from("/sys/devices/wiimote3");
        let snapshot = vec![retained.clone(), snapshot_add.clone()];
        let mut queued = Vec::new();

        assert!(push_unique(&mut queued, queued_add.clone()));
        assert!(!push_unique(&mut queued, queued_add.clone()));

        let active = [(0, retained.as_path()), (1, removed.as_path())];
        assert_eq!(missing_slots(active.into_iter(), &snapshot), vec![1]);
        assert_eq!(snapshot, vec![retained, snapshot_add]);
        assert_eq!(queued, vec![queued_add]);
    }

    #[test]
    fn dry_run_disables_bridge_outputs() {
        assert!(outputs_enabled(false));
        assert!(!outputs_enabled(true));
    }

    #[test]
    fn lifecycle_diagnostics_are_quiet_unless_verbose_and_deterministic() {
        let lines = Rc::new(RefCell::new(Vec::<String>::new()));
        let captured = Rc::clone(&lines);
        let mut diagnostics = Diagnostics {
            enabled: false,
            sink: Box::new(move |line| captured.borrow_mut().push(line.to_owned())),
        };
        let path = Path::new("/sys/devices/wiimote0");

        diagnostics.emit(Lifecycle::Add(path));
        assert!(lines.borrow().is_empty());

        diagnostics.enabled = true;
        diagnostics.emit(Lifecycle::Add(path));
        diagnostics.emit(Lifecycle::Remove(path));
        diagnostics.emit(Lifecycle::Gone(path));
        diagnostics.emit(Lifecycle::Reconcile {
            snapshot: 2,
            queued: 1,
            active: 1,
        });
        diagnostics.emit(Lifecycle::Error {
            operation: "drain",
            path: Some(path),
            code: -libc::EIO,
        });
        diagnostics.emit(Lifecycle::Error {
            operation: "monitor",
            path: None,
            code: -libc::ENOMEM,
        });

        assert_eq!(
            *lines.borrow(),
            [
                "wiilandd: add: /sys/devices/wiimote0",
                "wiilandd: remove: /sys/devices/wiimote0",
                "wiilandd: gone: /sys/devices/wiimote0",
                "wiilandd: reconcile: snapshot=2 queued=1 active=1",
                "wiilandd: error: drain /sys/devices/wiimote0: -5",
                "wiilandd: error: monitor: -12",
            ]
        );
    }
}
