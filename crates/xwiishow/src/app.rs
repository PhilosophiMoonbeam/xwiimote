use std::collections::VecDeque;
use std::io;
use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use wiiland_hid::Interface;
use wiiland_hid::decode::{Event, EventKind};
use wiiland_hid::model::{
    Axis3, BUTTON_A, BUTTON_B, BUTTON_C, BUTTON_DOWN, BUTTON_FRET_FAR_LOW, BUTTON_FRET_FAR_UP,
    BUTTON_FRET_LOW, BUTTON_FRET_MID, BUTTON_FRET_UP, BUTTON_HOME, BUTTON_LEFT, BUTTON_MINUS,
    BUTTON_ONE, BUTTON_PLUS, BUTTON_RIGHT, BUTTON_STRUM_BAR_DOWN, BUTTON_STRUM_BAR_UP, BUTTON_TL,
    BUTTON_TR, BUTTON_TWO, BUTTON_UP, BUTTON_X, BUTTON_Y, BUTTON_Z, BUTTON_ZL, BUTTON_ZR,
    ButtonEvent, InterfaceMask,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewMode {
    Error,
    Basic,
    Extended,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Continue,
    Quit,
}

/// All values shown by xwiishow live here.  The reducer is intentionally independent
/// of ratatui, making event handling deterministic and display-neutral.
pub struct App {
    pub mode: ViewMode,
    pub frozen: bool,
    pub gone: bool,
    pub keys_enabled: bool,
    pub accel_enabled: bool,
    pub ir_enabled: bool,
    pub motion_plus_enabled: bool,
    pub nunchuk_enabled: bool,
    pub classic_enabled: bool,
    pub balance_enabled: bool,
    pub pro_enabled: bool,
    pub drums_enabled: bool,
    pub guitar_enabled: bool,
    pub available: InterfaceMask,
    pub opened: InterfaceMask,
    pub rumble_writable: bool,
    pub rumble: bool,
    pub led_writable: [bool; 4],
    pub leds: [bool; 4],
    pub battery: Option<u8>,
    pub device_type: String,
    pub extension: String,
    pub accel: Axis3,
    pub ir: [Axis3; 4],
    pub motion_plus: Axis3,
    pub nunchuk: [Axis3; 2],
    pub classic: [Axis3; 3],
    pub balance: [Axis3; 4],
    pub pro: [Axis3; 2],
    pub drums: [Axis3; 8],
    pub guitar: [Axis3; 3],
    pub key_state: [bool; 28],
    pub nunchuk_keys: [bool; 28],
    pub classic_keys: [bool; 28],
    pub pro_keys: [bool; 28],
    pub drums_keys: [bool; 28],
    pub guitar_keys: [bool; 28],
    pub status: VecDeque<String>,
    pub last_event: Option<u32>,
    mp_pending_calibration: bool,
    pub mp_position: [i32; 2],
}

impl Default for App {
    fn default() -> Self {
        let mut ir = [Axis3::default(); 4];
        for point in &mut ir {
            point.x = 1023;
            point.y = 1023;
        }
        Self {
            mode: ViewMode::Error,
            frozen: false,
            gone: false,
            keys_enabled: false,
            accel_enabled: false,
            ir_enabled: false,
            motion_plus_enabled: false,
            nunchuk_enabled: false,
            classic_enabled: false,
            balance_enabled: false,
            pro_enabled: false,
            drums_enabled: false,
            guitar_enabled: false,
            available: InterfaceMask::empty(),
            opened: InterfaceMask::empty(),
            rumble_writable: false,
            rumble: false,
            led_writable: [true; 4],
            leds: [false; 4],
            battery: None,
            device_type: String::from("N/A"),
            extension: String::from("N/A"),
            accel: Axis3::default(),
            ir,
            motion_plus: Axis3::default(),
            nunchuk: [Axis3::default(); 2],
            classic: [Axis3::default(); 3],
            balance: [Axis3::default(); 4],
            pro: [Axis3::default(); 2],
            drums: [Axis3::default(); 8],
            guitar: [Axis3::default(); 3],
            key_state: [false; 28],
            nunchuk_keys: [false; 28],
            classic_keys: [false; 28],
            pro_keys: [false; 28],
            drums_keys: [false; 28],
            guitar_keys: [false; 28],
            status: VecDeque::with_capacity(8),
            last_event: None,
            mp_pending_calibration: false,
            mp_position: [5000, 5000],
        }
    }
}

impl App {
    pub fn resize(&mut self, width: u16, height: u16) {
        self.mode = if width < 80 || height < 24 {
            ViewMode::Error
        } else if width < 160 || height < 48 {
            ViewMode::Basic
        } else {
            ViewMode::Extended
        };
    }

    pub fn info(&mut self, message: impl Into<String>) {
        self.push_status(message.into(), false);
    }
    pub fn error(&mut self, message: impl Into<String>) {
        self.push_status(message.into(), true);
    }
    fn push_status(&mut self, message: String, is_error: bool) {
        let prefix = if is_error { "Error: " } else { "" };
        self.status.push_back(format!("{prefix}{message}"));
        while self.status.len() > 8 {
            self.status.pop_front();
        }
    }

    pub fn apply_event(&mut self, event: &Event) -> bool {
        self.last_event = Some(event.kind.raw_type());
        match event.kind {
            EventKind::Watch => {
                self.info("Watch event");
                return true;
            }
            EventKind::Gone => {
                self.gone = true;
                self.opened = InterfaceMask::empty();
                self.info("Device gone");
                return false;
            }
            _ if self.frozen => return false,
            EventKind::Key(key) => set_key(&mut self.key_state, key),
            EventKind::Accel(value) => self.accel = value,
            EventKind::Ir(values) => self.ir = values,
            EventKind::MotionPlus(value) => {
                self.motion_plus = value;
                self.mp_position[0] = (self.mp_position[0] + value.x / 100).clamp(0, 10_000);
                self.mp_position[1] = (self.mp_position[1] + value.z / 100).clamp(0, 10_000);
            }
            EventKind::NunchukKey(key) => set_key(&mut self.nunchuk_keys, key),
            EventKind::NunchukMove(values) => self.nunchuk = values,
            EventKind::ClassicControllerKey(key) => set_key(&mut self.classic_keys, key),
            EventKind::ClassicControllerMove(values) => self.classic = values,
            EventKind::BalanceBoard(values) => self.balance = values,
            EventKind::ProControllerKey(key) => set_key(&mut self.pro_keys, key),
            EventKind::ProControllerMove(values) => self.pro = values,
            EventKind::DrumsKey(key) => set_key(&mut self.drums_keys, key),
            EventKind::DrumsMove(values) => self.drums = values,
            EventKind::GuitarKey(key) => set_key(&mut self.guitar_keys, key),
            EventKind::GuitarMove(values) => self.guitar = values,
            _ => {}
        }
        false
    }

    fn handle_local_command(&mut self, ch: char) -> Option<Action> {
        match ch {
            'q' => Some(Action::Quit),
            'f' => {
                self.frozen = !self.frozen;
                self.info(if self.frozen {
                    "Freeze screen"
                } else {
                    "Unfreeze screen"
                });
                Some(Action::Continue)
            }
            's' => {
                self.mp_pending_calibration = true;
                self.info("Keep Motion Plus flat and still for recalibration");
                Some(Action::Continue)
            }
            _ => None,
        }
    }

    fn finish_mp_calibration(
        &mut self,
        sample: Axis3,
        current: ([i32; 3], i32),
    ) -> Option<([i32; 3], i32)> {
        if !self.mp_pending_calibration
            || sample.x.unsigned_abs() >= 5000
            || sample.y.unsigned_abs() >= 5000
            || sample.z.unsigned_abs() >= 5000
        {
            return None;
        }
        let ([x, y, z], factor) = current;
        let offsets = [
            x.saturating_add(sample.x),
            y.saturating_add(sample.y),
            z.saturating_add(sample.z),
        ];
        self.mp_pending_calibration = false;
        self.info(format!(
            "Calibrate MP Norm: ({}:{}:{})",
            offsets[0], offsets[1], offsets[2]
        ));
        Some((offsets, factor))
    }

    pub fn handle_key(&mut self, key: KeyEvent, iface: &mut Interface) -> io::Result<Action> {
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return Ok(Action::Continue);
        }
        let KeyCode::Char(ch) = key.code else {
            return Ok(Action::Continue);
        };
        if ch == 's' {
            self.refresh_static(iface);
        }
        if let Some(action) = self.handle_local_command(ch) {
            return Ok(action);
        }
        match ch {
            'k' => self.toggle_interface(iface, InterfaceMask::CORE, "key events", true),
            'a' => self.toggle_interface(iface, InterfaceMask::ACCEL, "accelerometer", false),
            'i' => self.toggle_interface(iface, InterfaceMask::IR, "IR", false),
            'm' => self.toggle_interface(iface, InterfaceMask::MOTION_PLUS, "Motion Plus", false),
            'n' => {
                let ([x, y, z], factor) = iface.mp_normalization();
                if factor == 0 {
                    iface.set_mp_normalization(x, y, z, 50);
                    self.info(format!("Enable MP Norm: ({x}:{y}:{z})"));
                } else {
                    iface.set_mp_normalization(x, y, z, 0);
                    self.info(format!("Disable MP Norm: ({x}:{y}:{z})"));
                }
            }
            'N' => self.toggle_interface(iface, InterfaceMask::NUNCHUK, "Nunchuk", false),
            'c' => self.toggle_interface(
                iface,
                InterfaceMask::CLASSIC_CONTROLLER,
                "Classic Controller",
                false,
            ),
            'b' => {
                self.toggle_interface(iface, InterfaceMask::BALANCE_BOARD, "Balance Board", false)
            }
            'p' => self.toggle_interface(
                iface,
                InterfaceMask::PRO_CONTROLLER,
                "Pro Controller",
                false,
            ),
            'g' => self.toggle_interface(iface, InterfaceMask::GUITAR, "Guitar controller", false),
            'd' => self.toggle_interface(iface, InterfaceMask::DRUMS, "Drums controller", false),
            'r' => self.toggle_rumble(iface),
            '1'..='4' => self.toggle_led(iface, (ch as usize) - ('1' as usize)),
            _ => {}
        }
        Ok(Action::Continue)
    }

    fn toggle_interface(
        &mut self,
        iface: &mut Interface,
        bit: InterfaceMask,
        label: &str,
        writable: bool,
    ) {
        if iface.opened().contains(bit) {
            iface.close(bit);
            self.opened = iface.opened();
            if bit == InterfaceMask::CORE {
                self.keys_enabled = false;
                self.rumble_writable = self.opened.contains(InterfaceMask::PRO_CONTROLLER);
            }
            self.info(format!("Disable {label}"));
            return;
        }
        let result = iface.open(
            bit | if writable {
                InterfaceMask::WRITABLE
            } else {
                InterfaceMask::empty()
            },
        );
        let mut readable = result.is_ok();
        if result.is_err() && writable {
            readable = iface.open(bit).is_ok();
            self.rumble_writable = false;
        }
        if readable {
            self.opened = iface.opened();
            self.set_enabled(bit, true);
            if writable && result.is_ok() {
                self.rumble_writable = true;
            }
            self.info(format!("Enable {label}"));
        } else {
            self.error(format!(
                "Cannot enable {label}: {}",
                result.err().unwrap_or(-libc::EIO)
            ));
        }
    }

    fn set_enabled(&mut self, bit: InterfaceMask, value: bool) {
        match bit {
            InterfaceMask::CORE => self.keys_enabled = value,
            InterfaceMask::ACCEL => self.accel_enabled = value,
            InterfaceMask::IR => self.ir_enabled = value,
            InterfaceMask::MOTION_PLUS => self.motion_plus_enabled = value,
            InterfaceMask::NUNCHUK => self.nunchuk_enabled = value,
            InterfaceMask::CLASSIC_CONTROLLER => self.classic_enabled = value,
            InterfaceMask::BALANCE_BOARD => self.balance_enabled = value,
            InterfaceMask::PRO_CONTROLLER => self.pro_enabled = value,
            InterfaceMask::DRUMS => self.drums_enabled = value,
            InterfaceMask::GUITAR => self.guitar_enabled = value,
            _ => {}
        }
    }

    fn toggle_rumble(&mut self, iface: &mut Interface) {
        if !self.rumble_writable {
            self.error("Rumble unavailable (read-only input)");
            return;
        }
        self.rumble = !self.rumble;
        if let Err(e) = iface.rumble(self.rumble) {
            self.rumble = !self.rumble;
            self.error(format!("Cannot set rumble: {e}"));
        } else {
            self.info(if self.rumble {
                "Enable rumble"
            } else {
                "Disable rumble"
            });
        }
    }
    fn toggle_led(&mut self, iface: &mut Interface, index: usize) {
        if !self.led_writable[index] {
            self.error(format!("LED {} unavailable", index + 1));
            return;
        }
        let next = !self.leds[index];
        if let Err(e) = iface.set_led(index, next) {
            self.led_writable[index] = false;
            self.error(format!("Cannot set LED {}: {e}", index + 1));
        } else {
            self.leds[index] = next;
            self.info(format!(
                "LED {} {}",
                index + 1,
                if next { "on" } else { "off" }
            ));
        }
    }

    pub fn open_available(&mut self, iface: &mut Interface) {
        self.available = iface.available();
        let controls = InterfaceMask::CORE | InterfaceMask::PRO_CONTROLLER;
        if (iface.opened() & controls).is_empty() {
            self.rumble_writable = false;
        }
        for bit in [InterfaceMask::CORE, InterfaceMask::PRO_CONTROLLER] {
            if self.available.contains(bit)
                && !iface.opened().contains(bit)
                && iface.open(bit | InterfaceMask::WRITABLE).is_ok()
            {
                self.rumble_writable = true;
            }
        }
        if let Err(e) = iface.open(self.available & InterfaceMask::ALL) {
            let missing = self.available & InterfaceMask::ALL & !iface.opened();
            if !missing.is_empty() {
                self.error(format!("Cannot open some readable interfaces: {e}"));
            }
        }
        self.opened = iface.opened();
        for bit in [
            InterfaceMask::CORE,
            InterfaceMask::ACCEL,
            InterfaceMask::IR,
            InterfaceMask::MOTION_PLUS,
            InterfaceMask::NUNCHUK,
            InterfaceMask::CLASSIC_CONTROLLER,
            InterfaceMask::BALANCE_BOARD,
            InterfaceMask::PRO_CONTROLLER,
            InterfaceMask::DRUMS,
            InterfaceMask::GUITAR,
        ] {
            self.set_enabled(bit, self.opened.contains(bit));
        }
        if !(self.available & controls).is_empty() && !self.rumble_writable {
            self.info("Read-only input; rumble is unavailable");
        }
        self.refresh_static(iface);
    }

    fn refresh_static(&mut self, iface: &Interface) {
        self.battery = iface.battery().ok();
        for index in 0..self.leds.len() {
            match iface.get_led(index) {
                Ok(state) => {
                    self.leds[index] = state;
                    self.led_writable[index] = true;
                }
                Err(_) => self.led_writable[index] = false,
            }
        }
        self.device_type = read_attr(iface, "devtype");
        self.extension = read_attr(iface, "extension");
        self.info("Refresh static values");
    }
}

fn read_attr(iface: &Interface, name: &str) -> String {
    iface
        .attr(name)
        .ok()
        .and_then(|v| String::from_utf8(v).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| String::from("N/A"))
}

fn set_key(keys: &mut [bool; 28], key: ButtonEvent) {
    if (key.code as usize) < keys.len() {
        keys[key.code as usize] = key.state != 0;
    }
}

pub fn key_name(code: u32) -> &'static str {
    match code {
        BUTTON_LEFT => "LEFT",
        BUTTON_RIGHT => "RIGHT",
        BUTTON_UP => "UP",
        BUTTON_DOWN => "DOWN",
        BUTTON_A => "A",
        BUTTON_B => "B",
        BUTTON_PLUS => "PLUS",
        BUTTON_MINUS => "MINUS",
        BUTTON_HOME => "HOME",
        BUTTON_ONE => "1",
        BUTTON_TWO => "2",
        BUTTON_X => "X",
        BUTTON_Y => "Y",
        BUTTON_TL => "TL",
        BUTTON_TR => "TR",
        BUTTON_ZL => "ZL",
        BUTTON_ZR => "ZR",
        wiiland_hid::model::BUTTON_THUMBL => "THUMBL",
        wiiland_hid::model::BUTTON_THUMBR => "THUMBR",
        BUTTON_C => "C",
        BUTTON_Z => "Z",
        BUTTON_STRUM_BAR_UP => "STRUM_UP",
        BUTTON_STRUM_BAR_DOWN => "STRUM_DOWN",
        BUTTON_FRET_FAR_UP => "FRET_FAR_UP",
        BUTTON_FRET_UP => "FRET_UP",
        BUTTON_FRET_MID => "FRET_MID",
        BUTTON_FRET_LOW => "FRET_LOW",
        BUTTON_FRET_FAR_LOW => "FRET_FAR_LOW",
        _ => "UNKNOWN",
    }
}

pub fn parse_selector(arg: &str) -> Result<Selector<'_>, &'static str> {
    if let Some(path) = arg.strip_prefix("/sys/")
        && !path.is_empty()
    {
        return Ok(Selector::Path(Path::new(arg)));
    }
    if arg.is_empty() || !arg.bytes().all(|b| b.is_ascii_digit()) {
        return Err("selector must be a positive ordinal or an absolute /sys path");
    }
    let value = arg
        .parse::<usize>()
        .map_err(|_| "selector ordinal is out of range")?;
    if value == 0 {
        return Err("selector must be a positive ordinal or an absolute /sys path");
    }
    Ok(Selector::Ordinal(value))
}
pub enum Selector<'a> {
    Ordinal(usize),
    Path(&'a Path),
}

pub fn poll_interface(iface: &mut Interface, app: &mut App) -> Result<bool, i32> {
    match iface.dispatch() {
        Ok(event) => {
            let reopen = app.apply_event(&event);
            if let EventKind::MotionPlus(sample) = event.kind
                && let Some((offsets, factor)) =
                    app.finish_mp_calibration(sample, iface.mp_normalization())
            {
                iface.set_mp_normalization(offsets[0], offsets[1], offsets[2], factor);
            }
            if reopen {
                app.open_available(iface);
            }
            Ok(true)
        }
        Err(e) if e == -libc::EAGAIN => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: EventKind) -> Event {
        Event {
            time: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            kind,
        }
    }

    #[test]
    fn frozen_values_stay_fixed_while_lifecycle_events_request_reopen_and_mark_gone() {
        let mut app = App {
            frozen: true,
            opened: InterfaceMask::CORE | InterfaceMask::ACCEL,
            accel: Axis3 { x: 1, y: 2, z: 3 },
            ..App::default()
        };
        app.key_state[BUTTON_A as usize] = true;

        assert!(!app.apply_event(&event(EventKind::Accel(Axis3 {
            x: 10,
            y: 20,
            z: 30,
        }))));
        let key = event(EventKind::Key(ButtonEvent {
            code: BUTTON_A,
            state: 0,
        }));
        assert!(!app.apply_event(&key));
        assert_eq!(app.accel, Axis3 { x: 1, y: 2, z: 3 });
        assert!(app.key_state[BUTTON_A as usize]);

        assert!(app.apply_event(&event(EventKind::Watch)));
        assert_eq!(app.status.back().map(String::as_str), Some("Watch event"));

        assert!(!app.apply_event(&event(EventKind::Gone)));
        assert!(app.gone);
        assert!(app.opened.is_empty());
        assert_eq!(app.status.back().map(String::as_str), Some("Device gone"));
    }

    #[test]
    fn s_starts_flat_calibration_and_stable_sample_preserves_normalization_factor() {
        let mut app = App {
            frozen: true,
            motion_plus: Axis3 {
                x: 91,
                y: 92,
                z: 93,
            },
            ..App::default()
        };

        assert_eq!(app.handle_local_command('s'), Some(Action::Continue));
        assert!(app.mp_pending_calibration);
        let sample = Axis3 {
            x: 120,
            y: -80,
            z: 40,
        };
        assert!(!app.apply_event(&event(EventKind::MotionPlus(sample))));
        assert_eq!(
            app.motion_plus,
            Axis3 {
                x: 91,
                y: 92,
                z: 93
            }
        );
        assert_eq!(
            app.finish_mp_calibration(sample, ([1000, -2000, 3000], 50)),
            Some(([1120, -2080, 3040], 50))
        );
        assert!(!app.mp_pending_calibration);

        assert_eq!(app.handle_local_command('s'), Some(Action::Continue));
        assert_eq!(
            app.finish_mp_calibration(Axis3 { x: 1, y: 2, z: 3 }, ([1120, -2080, 3040], 0)),
            Some(([1121, -2078, 3043], 0))
        );
    }

    #[test]
    fn flat_calibration_rejects_implausible_sample_without_clearing_request() {
        let mut app = App::default();
        assert_eq!(app.handle_local_command('s'), Some(Action::Continue));

        assert_eq!(
            app.finish_mp_calibration(
                Axis3 {
                    x: 4999,
                    y: -5000,
                    z: 0
                },
                ([10, 20, 30], 50)
            ),
            None
        );
        assert!(app.mp_pending_calibration);
        assert_eq!(
            app.finish_mp_calibration(
                Axis3 {
                    x: i32::MIN,
                    y: 0,
                    z: 0
                },
                ([10, 20, 30], 50)
            ),
            None
        );
        assert!(app.mp_pending_calibration);
    }
}
