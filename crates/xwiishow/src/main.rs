mod app;
mod render;

use std::env;
use std::io::{self, Write};
use std::os::fd::RawFd;
use std::path::PathBuf;
use std::process;
use std::time::Duration;

use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use wiiland_hid::{Interface, Monitor};

use app::{Action, App, Selector, parse_selector, poll_interface};

const HELP: &str = "Usage:\n  xwiishow -h|--help\n  xwiishow list\n  xwiishow <positive-ordinal>\n  xwiishow /sys/path/to/device\nUI commands:\n  q: Quit application\n  f: Freeze/Unfreeze screen\n  s: Refresh static values and recalibrate MotionPlus\n  k: Toggle key events\n  r: Toggle rumble motor (when writable)\n  a: Toggle accelerometer\n  i: Toggle IR camera\n  m: Toggle motion plus\n  n: Toggle normalization for motion plus\n  N: Toggle Nunchuk\n  c: Toggle Classic Controller\n  b: Toggle balance board\n  p: Toggle pro controller\n  g: Toggle guitar controller\n  d: Toggle drums controller\n  1-4: Toggle LEDs (when writable)\n";

fn main() {
    process::exit(run(env::args_os()));
}

pub fn run<I>(args: I) -> i32
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    let args: Vec<_> = args.into_iter().collect();
    let program = args
        .first()
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|| std::borrow::Cow::Borrowed("xwiishow"));
    if args.len() == 2 && (args[1] == "-h" || args[1] == "--help") {
        print!("{HELP}");
        return 0;
    }
    if args.len() != 2 {
        eprintln!("{program}: expected exactly one selector");
        eprint!("{HELP}");
        return 1;
    }
    let selector_arg = args[1].to_string_lossy().into_owned();
    if selector_arg == "list" {
        return list_devices();
    }
    let selector = match parse_selector(&selector_arg) {
        Ok(value) => value,
        Err(reason) => {
            if reason == "selector ordinal is out of range" {
                eprintln!("{program}: selector ordinal is out of range: {selector_arg}");
            } else {
                eprintln!("{program}: {reason}: {selector_arg}");
            }
            return 1;
        }
    };
    let path = match selector {
        Selector::Path(path) => path.to_path_buf(),
        Selector::Ordinal(n) => match ordinal_path(n) {
            Some(path) => path,
            None => return 1,
        },
    };
    let mut iface = match Interface::new(&path) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{program}: cannot open device '{selector_arg}': {error}");
            return 1;
        }
    };
    let shown = iface.syspath().to_string_lossy().into_owned();
    // This line intentionally precedes every terminal guard, including TTY and TERM checks.
    println!("Using Wii Remote: {shown}");
    let _ = io::stdout().flush();
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 0 {
        eprintln!(
            "{program}: interactive UI requires a terminal on stdin; use 'xwiishow list' for pipelines"
        );
        return 1;
    }
    if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 0 {
        eprintln!(
            "{program}: interactive UI requires a terminal on stdout; use 'xwiishow list' for redirected output"
        );
        return 1;
    }
    let term = env::var("TERM").unwrap_or_default();
    if term.is_empty() || term == "dumb" {
        eprintln!(
            "{program}: interactive UI requires a usable TERM value (for example, TERM=xterm-256color)"
        );
        return 1;
    }
    match interactive(&mut iface) {
        Ok(()) => 0,
        Err(error) if error.raw_os_error() == Some(libc::ENOSPC) => {
            eprintln!(
                "{program}: interactive UI requires a terminal at least 80 columns by 24 lines"
            );
            1
        }
        Err(error) => {
            eprintln!("{program}: interactive session failed: {error}");
            1
        }
    }
}

fn list_devices() -> i32 {
    let Some(mut monitor) = Monitor::new(false, false) else {
        eprintln!("xwiishow: cannot create device monitor");
        return 1;
    };
    let mut number = 0usize;
    while let Some(path) = monitor.poll() {
        number += 1;
        println!("{number}\t{}", path.display());
    }
    0
}

fn ordinal_path(ordinal: usize) -> Option<PathBuf> {
    let Some(mut monitor) = Monitor::new(false, false) else {
        eprintln!("xwiishow: cannot create device monitor");
        return None;
    };
    let mut number = 0usize;
    while let Some(path) = monitor.poll() {
        number += 1;
        if number == ordinal {
            return Some(path);
        }
    }
    eprintln!("xwiishow: no device with ordinal {ordinal}");
    None
}

struct TerminalGuard {
    active: bool,
}
impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen, crossterm::cursor::Hide) {
            let _ = terminal::disable_raw_mode();
            return Err(error);
        }
        Ok(Self { active: true })
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = terminal::disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
            self.active = false;
        }
    }
}

fn interactive(iface: &mut Interface) -> io::Result<()> {
    let guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let (width, height) = terminal::size()?;
    let mut app = App::default();
    app.resize(width, height);
    if matches!(app.mode, app::ViewMode::Error) {
        drop(terminal);
        drop(guard);
        return Err(io::Error::from_raw_os_error(libc::ENOSPC));
    }
    app.open_available(iface);
    if let Err(error) = iface.watch(true) {
        app.error(format!(
            "Cannot initialize hotplug watch descriptor: {error}"
        ));
    }
    loop {
        terminal.draw(|frame| render::render(frame, &app))?;
        let fd = iface.fd();
        wait_fd(fd, 50)?;
        while poll_interface(iface, &mut app).map_err(io_error)? {}
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(key) => {
                    if app.handle_key(key, iface)? == Action::Quit {
                        return Ok(());
                    }
                }
                Event::Resize(w, h) => {
                    app.resize(w, h);
                    if matches!(app.mode, app::ViewMode::Error) {
                        app.error("Screen smaller than 80x24; no view");
                    } else if matches!(app.mode, app::ViewMode::Basic) {
                        app.info("Screen smaller than 160x48; limited view");
                    }
                }
                _ => {}
            }
        }
    }
}

fn wait_fd(fd: RawFd, timeout_ms: i32) -> io::Result<()> {
    if fd < 0 {
        std::thread::sleep(Duration::from_millis(timeout_ms as u64));
        return Ok(());
    }
    let mut pollfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    loop {
        let result = unsafe { libc::poll(&mut pollfd, 1, timeout_ms) };
        if result >= 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}
fn io_error(error: i32) -> io::Error {
    io::Error::from_raw_os_error(error.unsigned_abs() as i32)
}
