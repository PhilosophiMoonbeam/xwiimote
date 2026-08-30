use std::collections::VecDeque;
use std::io;
use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use xwiimote::abi::{
    CEventAbs, CEventKey, XWII_IFACE_ACCEL, XWII_IFACE_ALL, XWII_IFACE_BALANCE_BOARD,
    XWII_IFACE_CLASSIC_CONTROLLER, XWII_IFACE_CORE, XWII_IFACE_DRUMS, XWII_IFACE_GUITAR,
    XWII_IFACE_IR, XWII_IFACE_MOTION_PLUS, XWII_IFACE_NUNCHUK, XWII_IFACE_PRO_CONTROLLER,
    XWII_IFACE_WRITABLE, XWII_KEY_A, XWII_KEY_B, XWII_KEY_C, XWII_KEY_DOWN, XWII_KEY_FRET_FAR_LOW,
    XWII_KEY_FRET_FAR_UP, XWII_KEY_FRET_LOW, XWII_KEY_FRET_MID, XWII_KEY_FRET_UP, XWII_KEY_HOME,
    XWII_KEY_LEFT, XWII_KEY_MINUS, XWII_KEY_ONE, XWII_KEY_PLUS, XWII_KEY_RIGHT,
    XWII_KEY_STRUM_BAR_DOWN, XWII_KEY_STRUM_BAR_UP, XWII_KEY_TL, XWII_KEY_TR, XWII_KEY_TWO,
    XWII_KEY_UP, XWII_KEY_X, XWII_KEY_Y, XWII_KEY_Z, XWII_KEY_ZL, XWII_KEY_ZR,
};
use xwiimote::device::{
    EVENT_ACCEL, EVENT_BALANCE, EVENT_CLASSIC_KEY, EVENT_CLASSIC_MOVE, EVENT_DRUMS_KEY,
    EVENT_DRUMS_MOVE, EVENT_GONE, EVENT_GUITAR_KEY, EVENT_GUITAR_MOVE, EVENT_IR, EVENT_KEY,
    EVENT_MP, EVENT_NUNCHUK_KEY, EVENT_NUNCHUK_MOVE, EVENT_PRO_KEY, EVENT_PRO_MOVE, EVENT_WATCH,
    Interface, RawEvent,
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
    pub available: u32,
    pub opened: u32,
    pub rumble_writable: bool,
    pub rumble: bool,
    pub led_writable: [bool; 4],
    pub leds: [bool; 4],
    pub battery: Option<u8>,
    pub device_type: String,
    pub extension: String,
    pub accel: CEventAbs,
    pub ir: [CEventAbs; 4],
    pub motion_plus: CEventAbs,
    pub nunchuk: [CEventAbs; 2],
    pub classic: [CEventAbs; 3],
    pub balance: [CEventAbs; 4],
    pub pro: [CEventAbs; 2],
    pub drums: [CEventAbs; 8],
    pub guitar: [CEventAbs; 3],
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
        let mut ir = [CEventAbs::default(); 4];
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
            available: 0,
            opened: 0,
            rumble_writable: false,
            rumble: false,
            led_writable: [true; 4],
            leds: [false; 4],
            battery: None,
            device_type: String::from("N/A"),
            extension: String::from("N/A"),
            accel: CEventAbs::default(),
            ir,
            motion_plus: CEventAbs::default(),
            nunchuk: [CEventAbs::default(); 2],
            classic: [CEventAbs::default(); 3],
            balance: [CEventAbs::default(); 4],
            pro: [CEventAbs::default(); 2],
            drums: [CEventAbs::default(); 8],
            guitar: [CEventAbs::default(); 3],
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

    pub fn apply_raw(&mut self, raw: &RawEvent) -> bool {
        self.last_event = Some(raw.kind);
        match raw.kind {
            EVENT_WATCH => {
                self.info("Watch event");
                return true;
            }
            EVENT_GONE => {
                self.gone = true;
                self.opened = 0;
                self.info("Device gone");
                return false;
            }
            _ if self.frozen => return false,
            _ => {}
        }
        let event = match raw.kind {
            EVENT_KEY => Event::Key(read_key(raw)),
            EVENT_ACCEL => Event::Accel(read_abs(raw, 0)),
            EVENT_IR => Event::Ir(read_abs_array::<4>(raw)),
            EVENT_MP => Event::MotionPlus(read_abs(raw, 0)),
            EVENT_NUNCHUK_KEY => Event::NunchukKey(read_key(raw)),
            EVENT_NUNCHUK_MOVE => Event::NunchukMove([read_abs(raw, 0), read_abs(raw, 1)]),
            EVENT_CLASSIC_KEY => Event::ClassicKey(read_key(raw)),
            EVENT_CLASSIC_MOVE => {
                Event::ClassicMove([read_abs(raw, 0), read_abs(raw, 1), read_abs(raw, 2)])
            }
            EVENT_BALANCE => Event::Balance(read_abs_array::<4>(raw)),
            EVENT_PRO_KEY => Event::ProKey(read_key(raw)),
            EVENT_PRO_MOVE => Event::ProMove([read_abs(raw, 0), read_abs(raw, 1)]),
            EVENT_DRUMS_KEY => Event::DrumsKey(read_key(raw)),
            EVENT_DRUMS_MOVE => Event::DrumsMove(read_abs_array::<8>(raw)),
            EVENT_GUITAR_KEY => Event::GuitarKey(read_key(raw)),
            EVENT_GUITAR_MOVE => {
                Event::GuitarMove([read_abs(raw, 0), read_abs(raw, 1), read_abs(raw, 2)])
            }
            _ => return false,
        };
        self.apply_event(event);
        false
    }

    fn apply_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => set_key(&mut self.key_state, key),
            Event::Accel(v) => self.accel = v,
            Event::Ir(v) => self.ir = v,
            Event::MotionPlus(v) => {
                self.motion_plus = v;
                self.mp_position[0] = (self.mp_position[0] + v.x / 100).clamp(0, 10_000);
                self.mp_position[1] = (self.mp_position[1] + v.z / 100).clamp(0, 10_000);
            }
            Event::NunchukKey(key) => set_key(&mut self.nunchuk_keys, key),
            Event::NunchukMove(v) => self.nunchuk = v,
            Event::ClassicKey(key) => set_key(&mut self.classic_keys, key),
            Event::ClassicMove(v) => self.classic = v,
            Event::Balance(v) => self.balance = v,
            Event::ProKey(key) => set_key(&mut self.pro_keys, key),
            Event::ProMove(v) => self.pro = v,
            Event::DrumsKey(key) => set_key(&mut self.drums_keys, key),
            Event::DrumsMove(v) => self.drums = v,
            Event::GuitarKey(key) => set_key(&mut self.guitar_keys, key),
            Event::GuitarMove(v) => self.guitar = v,
        }
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
        sample: CEventAbs,
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
            'k' => self.toggle_interface(iface, XWII_IFACE_CORE, "key events", true),
            'a' => self.toggle_interface(iface, XWII_IFACE_ACCEL, "accelerometer", false),
            'i' => self.toggle_interface(iface, XWII_IFACE_IR, "IR", false),
            'm' => self.toggle_interface(iface, XWII_IFACE_MOTION_PLUS, "Motion Plus", false),
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
            'N' => self.toggle_interface(iface, XWII_IFACE_NUNCHUK, "Nunchuk", false),
            'c' => self.toggle_interface(
                iface,
                XWII_IFACE_CLASSIC_CONTROLLER,
                "Classic Controller",
                false,
            ),
            'b' => self.toggle_interface(iface, XWII_IFACE_BALANCE_BOARD, "Balance Board", false),
            'p' => self.toggle_interface(iface, XWII_IFACE_PRO_CONTROLLER, "Pro Controller", false),
            'g' => self.toggle_interface(iface, XWII_IFACE_GUITAR, "Guitar controller", false),
            'd' => self.toggle_interface(iface, XWII_IFACE_DRUMS, "Drums controller", false),
            'r' => self.toggle_rumble(iface),
            '1'..='4' => self.toggle_led(iface, (ch as usize) - ('1' as usize)),
            _ => {}
        }
        Ok(Action::Continue)
    }

    fn toggle_interface(&mut self, iface: &mut Interface, bit: u32, label: &str, writable: bool) {
        if iface.opened() & bit != 0 {
            iface.close(bit);
            self.opened = iface.opened();
            if bit == XWII_IFACE_CORE {
                self.keys_enabled = false;
                self.rumble_writable = self.opened & XWII_IFACE_PRO_CONTROLLER != 0;
            }
            self.info(format!("Disable {label}"));
            return;
        }
        let result = iface.open(bit | if writable { XWII_IFACE_WRITABLE } else { 0 });
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

    fn set_enabled(&mut self, bit: u32, value: bool) {
        match bit {
            XWII_IFACE_CORE => self.keys_enabled = value,
            XWII_IFACE_ACCEL => self.accel_enabled = value,
            XWII_IFACE_IR => self.ir_enabled = value,
            XWII_IFACE_MOTION_PLUS => self.motion_plus_enabled = value,
            XWII_IFACE_NUNCHUK => self.nunchuk_enabled = value,
            XWII_IFACE_CLASSIC_CONTROLLER => self.classic_enabled = value,
            XWII_IFACE_BALANCE_BOARD => self.balance_enabled = value,
            XWII_IFACE_PRO_CONTROLLER => self.pro_enabled = value,
            XWII_IFACE_DRUMS => self.drums_enabled = value,
            XWII_IFACE_GUITAR => self.guitar_enabled = value,
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
        let controls = XWII_IFACE_CORE | XWII_IFACE_PRO_CONTROLLER;
        if iface.opened() & controls == 0 {
            self.rumble_writable = false;
        }
        for bit in [XWII_IFACE_CORE, XWII_IFACE_PRO_CONTROLLER] {
            if self.available & bit != 0
                && iface.opened() & bit == 0
                && iface.open(bit | XWII_IFACE_WRITABLE).is_ok()
            {
                self.rumble_writable = true;
            }
        }
        if let Err(e) = iface.open(self.available & XWII_IFACE_ALL) {
            let missing = self.available & XWII_IFACE_ALL & !iface.opened();
            if missing != 0 {
                self.error(format!("Cannot open some readable interfaces: {e}"));
            }
        }
        self.opened = iface.opened();
        for bit in [
            XWII_IFACE_CORE,
            XWII_IFACE_ACCEL,
            XWII_IFACE_IR,
            XWII_IFACE_MOTION_PLUS,
            XWII_IFACE_NUNCHUK,
            XWII_IFACE_CLASSIC_CONTROLLER,
            XWII_IFACE_BALANCE_BOARD,
            XWII_IFACE_PRO_CONTROLLER,
            XWII_IFACE_DRUMS,
            XWII_IFACE_GUITAR,
        ] {
            self.set_enabled(bit, self.opened & bit != 0);
        }
        if (self.available & controls != 0) && !self.rumble_writable {
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

#[derive(Clone, Copy)]
enum Event {
    Key(CEventKey),
    Accel(CEventAbs),
    Ir([CEventAbs; 4]),
    MotionPlus(CEventAbs),
    NunchukKey(CEventKey),
    NunchukMove([CEventAbs; 2]),
    ClassicKey(CEventKey),
    ClassicMove([CEventAbs; 3]),
    Balance([CEventAbs; 4]),
    ProKey(CEventKey),
    ProMove([CEventAbs; 2]),
    DrumsKey(CEventKey),
    DrumsMove([CEventAbs; 8]),
    GuitarKey(CEventKey),
    GuitarMove([CEventAbs; 3]),
}
fn read_u32(raw: &RawEvent, offset: usize) -> u32 {
    let mut bytes = [0; 4];
    bytes.copy_from_slice(&raw.payload[offset..offset + 4]);
    u32::from_ne_bytes(bytes)
}
fn read_i32(raw: &RawEvent, offset: usize) -> i32 {
    read_u32(raw, offset) as i32
}
fn read_key(raw: &RawEvent) -> CEventKey {
    CEventKey {
        code: read_u32(raw, 0),
        state: read_u32(raw, 4),
    }
}
fn read_abs(raw: &RawEvent, index: usize) -> CEventAbs {
    let off = index.saturating_mul(12);
    CEventAbs {
        x: read_i32(raw, off),
        y: read_i32(raw, off + 4),
        z: read_i32(raw, off + 8),
    }
}
fn read_abs_array<const N: usize>(raw: &RawEvent) -> [CEventAbs; N] {
    std::array::from_fn(|i| read_abs(raw, i))
}
fn set_key(keys: &mut [bool; 28], key: CEventKey) {
    if (key.code as usize) < keys.len() {
        keys[key.code as usize] = key.state != 0;
    }
}

pub fn key_name(code: u32) -> &'static str {
    match code {
        XWII_KEY_LEFT => "LEFT",
        XWII_KEY_RIGHT => "RIGHT",
        XWII_KEY_UP => "UP",
        XWII_KEY_DOWN => "DOWN",
        XWII_KEY_A => "A",
        XWII_KEY_B => "B",
        XWII_KEY_PLUS => "PLUS",
        XWII_KEY_MINUS => "MINUS",
        XWII_KEY_HOME => "HOME",
        XWII_KEY_ONE => "1",
        XWII_KEY_TWO => "2",
        XWII_KEY_X => "X",
        XWII_KEY_Y => "Y",
        XWII_KEY_TL => "TL",
        XWII_KEY_TR => "TR",
        XWII_KEY_ZL => "ZL",
        XWII_KEY_ZR => "ZR",
        xwiimote::abi::XWII_KEY_THUMBL => "THUMBL",
        xwiimote::abi::XWII_KEY_THUMBR => "THUMBR",
        XWII_KEY_C => "C",
        XWII_KEY_Z => "Z",
        XWII_KEY_STRUM_BAR_UP => "STRUM_UP",
        XWII_KEY_STRUM_BAR_DOWN => "STRUM_DOWN",
        XWII_KEY_FRET_FAR_UP => "FRET_FAR_UP",
        XWII_KEY_FRET_UP => "FRET_UP",
        XWII_KEY_FRET_MID => "FRET_MID",
        XWII_KEY_FRET_LOW => "FRET_LOW",
        XWII_KEY_FRET_FAR_LOW => "FRET_FAR_LOW",
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
    let mut raw = RawEvent::default();
    match iface.dispatch(Some(&mut raw)) {
        Ok(()) => {
            let reopen = app.apply_raw(&raw);
            if raw.kind == EVENT_MP
                && let Some((offsets, factor)) =
                    app.finish_mp_calibration(read_abs(&raw, 0), iface.mp_normalization())
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

    fn raw_abs(kind: u32, x: i32, y: i32, z: i32) -> RawEvent {
        let mut raw = RawEvent {
            kind,
            ..RawEvent::default()
        };
        raw.abs(0, x, y, z);
        raw
    }

    #[test]
    fn frozen_values_stay_fixed_while_lifecycle_events_request_reopen_and_mark_gone() {
        let mut app = App {
            frozen: true,
            opened: XWII_IFACE_CORE | XWII_IFACE_ACCEL,
            accel: CEventAbs { x: 1, y: 2, z: 3 },
            ..App::default()
        };
        app.key_state[XWII_KEY_A as usize] = true;

        assert!(!app.apply_raw(&raw_abs(EVENT_ACCEL, 10, 20, 30)));
        let mut key = RawEvent {
            kind: EVENT_KEY,
            ..RawEvent::default()
        };
        key.key(XWII_KEY_A, 0);
        assert!(!app.apply_raw(&key));
        assert_eq!(app.accel, CEventAbs { x: 1, y: 2, z: 3 });
        assert!(app.key_state[XWII_KEY_A as usize]);

        assert!(app.apply_raw(&RawEvent {
            kind: EVENT_WATCH,
            ..RawEvent::default()
        }));
        assert_eq!(app.status.back().map(String::as_str), Some("Watch event"));

        assert!(!app.apply_raw(&RawEvent {
            kind: EVENT_GONE,
            ..RawEvent::default()
        }));
        assert!(app.gone);
        assert_eq!(app.opened, 0);
        assert_eq!(app.status.back().map(String::as_str), Some("Device gone"));
    }

    #[test]
    fn s_starts_flat_calibration_and_stable_sample_preserves_normalization_factor() {
        let mut app = App {
            frozen: true,
            motion_plus: CEventAbs {
                x: 91,
                y: 92,
                z: 93,
            },
            ..App::default()
        };

        assert_eq!(app.handle_local_command('s'), Some(Action::Continue));
        assert!(app.mp_pending_calibration);
        let sample = CEventAbs {
            x: 120,
            y: -80,
            z: 40,
        };
        assert!(!app.apply_raw(&raw_abs(EVENT_MP, sample.x, sample.y, sample.z)));
        assert_eq!(
            app.motion_plus,
            CEventAbs {
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
            app.finish_mp_calibration(CEventAbs { x: 1, y: 2, z: 3 }, ([1120, -2080, 3040], 0)),
            Some(([1121, -2078, 3043], 0))
        );
    }

    #[test]
    fn flat_calibration_rejects_implausible_sample_without_clearing_request() {
        let mut app = App::default();
        assert_eq!(app.handle_local_command('s'), Some(Action::Continue));

        assert_eq!(
            app.finish_mp_calibration(
                CEventAbs {
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
                CEventAbs {
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
