//! Pure evdev decoders and state machines.
//!
//! No file descriptors or ioctl calls live here.  The device layer supplies
//! [`InputEvent`] values and, after a SYN_DROPPED, the current key/absolute
//! state to [`RecoveryState`].

use crate::abi::*;
use libc::timeval;

pub type Abs = CEventAbs;
pub type Key = CEventKey;

/// Typed event discriminant with an explicit future-value case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum EventType {
    Key,
    Accel,
    Ir,
    BalanceBoard,
    MotionPlus,
    ProControllerKey,
    ProControllerMove,
    Watch,
    ClassicControllerKey,
    ClassicControllerMove,
    NunchukKey,
    NunchukMove,
    DrumsKey,
    DrumsMove,
    GuitarKey,
    GuitarMove,
    Gone,
    Unknown(u32),
}
impl EventType {
    pub const fn from_raw(value: u32) -> Self {
        match value {
            XWII_EVENT_KEY => Self::Key,
            XWII_EVENT_ACCEL => Self::Accel,
            XWII_EVENT_IR => Self::Ir,
            XWII_EVENT_BALANCE_BOARD => Self::BalanceBoard,
            XWII_EVENT_MOTION_PLUS => Self::MotionPlus,
            XWII_EVENT_PRO_CONTROLLER_KEY => Self::ProControllerKey,
            XWII_EVENT_PRO_CONTROLLER_MOVE => Self::ProControllerMove,
            XWII_EVENT_WATCH => Self::Watch,
            XWII_EVENT_CLASSIC_CONTROLLER_KEY => Self::ClassicControllerKey,
            XWII_EVENT_CLASSIC_CONTROLLER_MOVE => Self::ClassicControllerMove,
            XWII_EVENT_NUNCHUK_KEY => Self::NunchukKey,
            XWII_EVENT_NUNCHUK_MOVE => Self::NunchukMove,
            XWII_EVENT_DRUMS_KEY => Self::DrumsKey,
            XWII_EVENT_DRUMS_MOVE => Self::DrumsMove,
            XWII_EVENT_GUITAR_KEY => Self::GuitarKey,
            XWII_EVENT_GUITAR_MOVE => Self::GuitarMove,
            XWII_EVENT_GONE => Self::Gone,
            other => Self::Unknown(other),
        }
    }
    pub const fn raw(self) -> u32 {
        match self {
            Self::Key => XWII_EVENT_KEY,
            Self::Accel => XWII_EVENT_ACCEL,
            Self::Ir => XWII_EVENT_IR,
            Self::BalanceBoard => XWII_EVENT_BALANCE_BOARD,
            Self::MotionPlus => XWII_EVENT_MOTION_PLUS,
            Self::ProControllerKey => XWII_EVENT_PRO_CONTROLLER_KEY,
            Self::ProControllerMove => XWII_EVENT_PRO_CONTROLLER_MOVE,
            Self::Watch => XWII_EVENT_WATCH,
            Self::ClassicControllerKey => XWII_EVENT_CLASSIC_CONTROLLER_KEY,
            Self::ClassicControllerMove => XWII_EVENT_CLASSIC_CONTROLLER_MOVE,
            Self::NunchukKey => XWII_EVENT_NUNCHUK_KEY,
            Self::NunchukMove => XWII_EVENT_NUNCHUK_MOVE,
            Self::DrumsKey => XWII_EVENT_DRUMS_KEY,
            Self::DrumsMove => XWII_EVENT_DRUMS_MOVE,
            Self::GuitarKey => XWII_EVENT_GUITAR_KEY,
            Self::GuitarMove => XWII_EVENT_GUITAR_MOVE,
            Self::Gone => XWII_EVENT_GONE,
            Self::Unknown(v) => v,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InterfaceKind {
    Core,
    Accel,
    Ir,
    MotionPlus,
    Nunchuk,
    Classic,
    BalanceBoard,
    Pro,
    Drums,
    Guitar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum EventKind {
    Key(Key),
    Accel(Abs),
    Ir([Abs; 4]),
    BalanceBoard([Abs; 4]),
    MotionPlus(Abs),
    ProControllerKey(Key),
    ProControllerMove([Abs; 2]),
    Watch,
    ClassicControllerKey(Key),
    ClassicControllerMove([Abs; 3]),
    NunchukKey(Key),
    NunchukMove([Abs; 2]),
    DrumsKey(Key),
    DrumsMove([Abs; XWII_DRUMS_ABS_NUM]),
    GuitarKey(Key),
    GuitarMove([Abs; 3]),
    Gone,
    Unknown(u32),
}

impl EventKind {
    pub const fn raw_type(self) -> u32 {
        match self {
            Self::Key(_) => XWII_EVENT_KEY,
            Self::Accel(_) => XWII_EVENT_ACCEL,
            Self::Ir(_) => XWII_EVENT_IR,
            Self::BalanceBoard(_) => XWII_EVENT_BALANCE_BOARD,
            Self::MotionPlus(_) => XWII_EVENT_MOTION_PLUS,
            Self::ProControllerKey(_) => XWII_EVENT_PRO_CONTROLLER_KEY,
            Self::ProControllerMove(_) => XWII_EVENT_PRO_CONTROLLER_MOVE,
            Self::Watch => XWII_EVENT_WATCH,
            Self::ClassicControllerKey(_) => XWII_EVENT_CLASSIC_CONTROLLER_KEY,
            Self::ClassicControllerMove(_) => XWII_EVENT_CLASSIC_CONTROLLER_MOVE,
            Self::NunchukKey(_) => XWII_EVENT_NUNCHUK_KEY,
            Self::NunchukMove(_) => XWII_EVENT_NUNCHUK_MOVE,
            Self::DrumsKey(_) => XWII_EVENT_DRUMS_KEY,
            Self::DrumsMove(_) => XWII_EVENT_DRUMS_MOVE,
            Self::GuitarKey(_) => XWII_EVENT_GUITAR_KEY,
            Self::GuitarMove(_) => XWII_EVENT_GUITAR_MOVE,
            Self::Gone => XWII_EVENT_GONE,
            Self::Unknown(v) => v,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub time: timeval,
    pub kind: EventKind,
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.time.tv_sec == other.time.tv_sec
            && self.time.tv_usec == other.time.tv_usec
            && self.kind == other.kind
    }
}

impl Eq for Event {}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MotionPlusNormalizer {
    offsets: Abs,
    factor: i32,
}

impl MotionPlusNormalizer {
    pub const fn new() -> Self {
        Self {
            offsets: Abs { x: 0, y: 0, z: 0 },
            factor: 0,
        }
    }
    pub fn set(&mut self, x: i32, y: i32, z: i32, factor: i32) {
        self.offsets.x = scale_offset(x);
        self.offsets.y = scale_offset(y);
        self.offsets.z = scale_offset(z);
        self.factor = factor;
    }
    pub const fn values(&self) -> (i32, i32, i32, i32) {
        (
            self.offsets.x / 100,
            self.offsets.y / 100,
            self.offsets.z / 100,
            self.factor,
        )
    }
    pub fn normalize(&mut self, value: Abs) -> Abs {
        Abs {
            x: normalize_axis(value.x, &mut self.offsets.x, self.factor),
            y: normalize_axis(value.y, &mut self.offsets.y, self.factor),
            z: normalize_axis(value.z, &mut self.offsets.z, self.factor),
        }
    }
}

fn scale_offset(value: i32) -> i32 {
    (i64::from(value) * 100).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}
fn normalize_axis(value: i32, offset: &mut i32, factor: i32) -> i32 {
    let normalized = (i64::from(value) - i64::from(*offset) / 100)
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    if normalized != 0 {
        *offset = (i64::from(*offset)
            + if normalized > 0 {
                i64::from(factor)
            } else {
                -i64::from(factor)
            })
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    }
    normalized
}

const KEY_WORDS: usize = 12; // KEY_MAX is 0x2ff on Linux (768 bits).

/// State needed to reproduce libxwiimote's SYN_DROPPED recovery ordering.
#[derive(Clone, Debug)]
pub struct RecoveryState {
    key_state: [u64; KEY_WORDS],
    key_pending: [u64; KEY_WORDS],
    desynced: bool,
    key_resync_pending: bool,
    report_resync_pending: bool,
    resync_time: timeval,
}

impl Default for RecoveryState {
    fn default() -> Self {
        Self {
            key_state: [0; KEY_WORDS],
            key_pending: [0; KEY_WORDS],
            desynced: false,
            key_resync_pending: false,
            report_resync_pending: false,
            resync_time: timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
        }
    }
}
impl RecoveryState {
    pub fn dropped(&mut self) {
        self.desynced = true;
    }
    pub const fn is_desynced(&self) -> bool {
        self.desynced
    }
    pub(crate) fn initialize(&mut self, keys: &[u64]) {
        self.key_pending = [0; KEY_WORDS];
        for i in 0..KEY_WORDS {
            self.key_state[i] = keys.get(i).copied().unwrap_or(0);
        }
        self.desynced = false;
        self.key_resync_pending = false;
        self.report_resync_pending = false;
    }
    pub fn seed(&mut self, keys: &[u64], time: timeval, has_abs: bool) {
        self.key_resync_pending = false;
        for i in 0..KEY_WORDS {
            let next = keys.get(i).copied().unwrap_or(0);
            self.key_pending[i] = self.key_state[i] ^ next;
            if self.key_pending[i] != 0 {
                self.key_resync_pending = true;
            }
            self.key_state[i] = next;
        }
        self.resync_time = time;
        self.report_resync_pending = has_abs;
        self.desynced = false;
    }
    pub fn next_key(&mut self) -> Option<InputEvent> {
        if !self.key_resync_pending {
            return None;
        }
        for i in 0..KEY_WORDS {
            let pending = self.key_pending[i];
            if pending == 0 {
                continue;
            }
            let bit = pending.trailing_zeros() as usize;
            self.key_pending[i] &= !(1u64 << bit);
            let code = i * 64 + bit;
            return Some(InputEvent {
                time: self.resync_time,
                event_type: EV_KEY,
                code: code as u16,
                value: if self.key_state[i] & (1u64 << bit) != 0 {
                    1
                } else {
                    0
                },
            });
        }
        self.key_resync_pending = false;
        None
    }
    pub fn take_report(&mut self) -> Option<InputEvent> {
        if !self.report_resync_pending {
            return None;
        }
        self.report_resync_pending = false;
        Some(InputEvent {
            time: self.resync_time,
            event_type: EV_SYN,
            code: SYN_REPORT,
            value: 0,
        })
    }
    pub const fn has_pending(&self) -> bool {
        self.key_resync_pending || self.report_resync_pending
    }
    pub const fn key_state(&self) -> &[u64; KEY_WORDS] {
        &self.key_state
    }
    fn remember(&mut self, input: InputEvent) {
        if input.event_type != EV_KEY
            || input.code as usize >= KEY_WORDS * 64
            || !(0..=2).contains(&input.value)
        {
            return;
        }
        let word = input.code as usize / 64;
        let mask = 1u64 << (input.code as usize % 64);
        if input.value == 0 {
            self.key_state[word] &= !mask;
        } else {
            self.key_state[word] |= mask;
        }
    }
}

#[derive(Clone, Debug)]
pub struct CacheState {
    pub accel: Abs,
    pub ir: [Abs; 4],
    pub motion_plus: Abs,
    pub nunchuk: [Abs; 2],
    pub classic: [Abs; 3],
    pub balance_board: [Abs; 4],
    pub pro: [Abs; 2],
    pub drums: [Abs; XWII_DRUMS_ABS_NUM],
    pub guitar: [Abs; 3],
}
impl Default for CacheState {
    fn default() -> Self {
        let mut ir = [Abs::default(); 4];
        for p in &mut ir {
            p.x = 1023;
            p.y = 1023;
        }
        Self {
            accel: Abs::default(),
            ir,
            motion_plus: Abs::default(),
            nunchuk: [Abs::default(); 2],
            classic: [Abs::default(); 3],
            balance_board: [Abs::default(); 4],
            pro: [Abs::default(); 2],
            drums: [Abs::default(); XWII_DRUMS_ABS_NUM],
            guitar: [Abs::default(); 3],
        }
    }
}

pub struct Decoder {
    pub interface: InterfaceKind,
    pub cache: CacheState,
    pub recovery: RecoveryState,
    pub motion_plus: MotionPlusNormalizer,
}
impl Decoder {
    pub fn new(interface: InterfaceKind) -> Self {
        Self {
            interface,
            cache: CacheState::default(),
            recovery: RecoveryState::default(),
            motion_plus: MotionPlusNormalizer::new(),
        }
    }
    pub fn set_mp_normalization(&mut self, x: i32, y: i32, z: i32, factor: i32) {
        self.motion_plus.set(x, y, z, factor);
    }
    pub fn mp_normalization(&self) -> (i32, i32, i32, i32) {
        self.motion_plus.values()
    }
    /// Decode one input event.  Unsupported events and intermediate ABS events
    /// return `None`; SYN_REPORT yields the complete cached typed event.
    pub fn push(&mut self, input: InputEvent) -> Option<Event> {
        if self.recovery.is_desynced() {
            if input.event_type == EV_SYN && input.code == SYN_REPORT {
                return None;
            }
            return None;
        }
        if input.event_type == EV_SYN && input.code == SYN_DROPPED {
            self.recovery.dropped();
            return None;
        }
        if input.event_type == EV_KEY {
            self.recovery.remember(input);
            return self.decode_key(input);
        }
        if input.event_type == EV_ABS {
            self.update_abs(input.code, input.value);
            return None;
        }
        if input.event_type == EV_SYN && input.code == SYN_REPORT {
            return self.report(input.time);
        }
        None
    }
    /// Initializes the decoder from the kernel snapshot taken while opening an
    /// evdev node. Unlike recovery, initialization never queues synthetic
    /// transitions or an absolute report.
    pub(crate) fn seed_state(&mut self, keys: &[u64], abs: &[(u16, i32)]) {
        for &(code, value) in abs {
            self.update_abs(code, value);
        }
        self.recovery.initialize(keys);
    }
    /// Seed kernel state after a drop. Synthetic key transitions and a report
    /// can then be consumed in order with [`Self::push_recovered`].
    pub fn recover(&mut self, keys: &[u64], abs: &[(u16, i32)], time: timeval) {
        for &(code, value) in abs {
            self.update_abs(code, value);
        }
        let has_abs = !abs_codes(self.interface).is_empty();
        self.recovery.seed(keys, time, has_abs);
    }
    pub fn push_recovered(&mut self) -> Option<Event> {
        if let Some(input) = self.recovery.next_key() {
            return self.decode_key(input);
        }
        if let Some(input) = self.recovery.take_report() {
            return self.push_report(input);
        }
        None
    }
    fn push_report(&mut self, input: InputEvent) -> Option<Event> {
        self.report(input.time)
    }
    fn report(&mut self, time: timeval) -> Option<Event> {
        let kind = match self.interface {
            InterfaceKind::Accel => EventKind::Accel(self.cache.accel),
            InterfaceKind::Ir => EventKind::Ir(self.cache.ir),
            InterfaceKind::MotionPlus => {
                EventKind::MotionPlus(self.motion_plus.normalize(self.cache.motion_plus))
            }
            InterfaceKind::Nunchuk => EventKind::NunchukMove(self.cache.nunchuk),
            InterfaceKind::Classic => EventKind::ClassicControllerMove(self.cache.classic),
            InterfaceKind::BalanceBoard => EventKind::BalanceBoard(self.cache.balance_board),
            InterfaceKind::Pro => EventKind::ProControllerMove(self.cache.pro),
            InterfaceKind::Drums => EventKind::DrumsMove(self.cache.drums),
            InterfaceKind::Guitar => EventKind::GuitarMove(self.cache.guitar),
            InterfaceKind::Core => return None,
        };
        Some(Event { time, kind })
    }
    fn decode_key(&self, input: InputEvent) -> Option<Event> {
        if !(0..=2).contains(&input.value) {
            return None;
        }
        let code = match self.interface {
            InterfaceKind::Core => map_core_key(input.code),
            InterfaceKind::Nunchuk => map_nunchuk_key(input.code),
            InterfaceKind::Classic => map_classic_key(input.code),
            InterfaceKind::Pro => map_pro_key(input.code),
            InterfaceKind::Drums => map_drums_key(input.code),
            InterfaceKind::Guitar => map_guitar_key(input.code),
            _ => None,
        }?;
        let key = Key {
            code,
            state: input.value as u32,
        };
        let kind = match self.interface {
            InterfaceKind::Core => EventKind::Key(key),
            InterfaceKind::Nunchuk => EventKind::NunchukKey(key),
            InterfaceKind::Classic => EventKind::ClassicControllerKey(key),
            InterfaceKind::Pro => EventKind::ProControllerKey(key),
            InterfaceKind::Drums => EventKind::DrumsKey(key),
            InterfaceKind::Guitar => EventKind::GuitarKey(key),
            _ => return None,
        };
        Some(Event {
            time: input.time,
            kind,
        })
    }
    /// Update one cached axis and report whether the code belongs to this
    /// interface. Unknown ABS codes are ignored exactly like libxwiimote.
    pub fn update_abs_cache(&mut self, code: u16, value: i32) -> bool {
        if !known_abs(self.interface, code) {
            return false;
        }
        self.update_abs(code, value);
        true
    }
    fn update_abs(&mut self, code: u16, value: i32) {
        match self.interface {
            InterfaceKind::Accel => match code {
                ABS_RX => self.cache.accel.x = value,
                ABS_RY => self.cache.accel.y = value,
                ABS_RZ => self.cache.accel.z = value,
                _ => {}
            },
            InterfaceKind::MotionPlus => match code {
                ABS_RX => self.cache.motion_plus.x = value,
                ABS_RY => self.cache.motion_plus.y = value,
                ABS_RZ => self.cache.motion_plus.z = value,
                _ => {}
            },
            InterfaceKind::Ir => {
                let i = match code {
                    ABS_HAT0X => Some((0, true)),
                    ABS_HAT0Y => Some((0, false)),
                    ABS_HAT1X => Some((1, true)),
                    ABS_HAT1Y => Some((1, false)),
                    ABS_HAT2X => Some((2, true)),
                    ABS_HAT2Y => Some((2, false)),
                    ABS_HAT3X => Some((3, true)),
                    ABS_HAT3Y => Some((3, false)),
                    _ => None,
                };
                if let Some((i, x)) = i {
                    if x {
                        self.cache.ir[i].x = value
                    } else {
                        self.cache.ir[i].y = value
                    }
                }
            }
            InterfaceKind::Nunchuk => match code {
                ABS_HAT0X => self.cache.nunchuk[0].x = value,
                ABS_HAT0Y => self.cache.nunchuk[0].y = value,
                ABS_RX => self.cache.nunchuk[1].x = value,
                ABS_RY => self.cache.nunchuk[1].y = value,
                ABS_RZ => self.cache.nunchuk[1].z = value,
                _ => {}
            },
            InterfaceKind::Classic => match code {
                ABS_HAT1X => self.cache.classic[0].x = value,
                ABS_HAT1Y => self.cache.classic[0].y = value,
                ABS_HAT2X => self.cache.classic[1].x = value,
                ABS_HAT2Y => self.cache.classic[1].y = value,
                ABS_HAT3X => self.cache.classic[2].y = value,
                ABS_HAT3Y => self.cache.classic[2].x = value,
                _ => {}
            },
            InterfaceKind::BalanceBoard => match code {
                ABS_HAT0X => self.cache.balance_board[0].x = value,
                ABS_HAT0Y => self.cache.balance_board[1].x = value,
                ABS_HAT1X => self.cache.balance_board[2].x = value,
                ABS_HAT1Y => self.cache.balance_board[3].x = value,
                _ => {}
            },
            InterfaceKind::Pro => match code {
                ABS_X => self.cache.pro[0].x = value,
                ABS_Y => self.cache.pro[0].y = value,
                ABS_RX => self.cache.pro[1].x = value,
                ABS_RY => self.cache.pro[1].y = value,
                _ => {}
            },
            InterfaceKind::Drums => match code {
                ABS_X => self.cache.drums[0].x = value,
                ABS_Y => self.cache.drums[0].y = value,
                ABS_HAT2X => self.cache.drums[1].x = value,
                ABS_HAT2Y => self.cache.drums[2].x = value,
                ABS_HAT0X => self.cache.drums[3].x = value,
                ABS_HAT1X => self.cache.drums[4].x = value,
                ABS_HAT0Y => self.cache.drums[5].x = value,
                ABS_HAT3X => self.cache.drums[6].x = value,
                ABS_HAT3Y => self.cache.drums[7].x = value,
                _ => {}
            },
            InterfaceKind::Guitar => match code {
                ABS_X => self.cache.guitar[0].x = value,
                ABS_Y => self.cache.guitar[0].y = value,
                ABS_HAT1X => self.cache.guitar[1].x = value,
                ABS_HAT0X => self.cache.guitar[2].x = value,
                _ => {}
            },
            InterfaceKind::Core => {}
        }
    }
}

pub fn abs_codes(interface: InterfaceKind) -> &'static [u16] {
    match interface {
        InterfaceKind::Accel | InterfaceKind::MotionPlus => &[ABS_RX, ABS_RY, ABS_RZ],
        InterfaceKind::Ir => &[
            ABS_HAT0X, ABS_HAT0Y, ABS_HAT1X, ABS_HAT1Y, ABS_HAT2X, ABS_HAT2Y, ABS_HAT3X, ABS_HAT3Y,
        ],
        InterfaceKind::Nunchuk => &[ABS_HAT0X, ABS_HAT0Y, ABS_RX, ABS_RY, ABS_RZ],
        InterfaceKind::Classic => &[
            ABS_HAT1X, ABS_HAT1Y, ABS_HAT2X, ABS_HAT2Y, ABS_HAT3X, ABS_HAT3Y,
        ],
        InterfaceKind::BalanceBoard => &[ABS_HAT0X, ABS_HAT0Y, ABS_HAT1X, ABS_HAT1Y],
        InterfaceKind::Pro => &[ABS_X, ABS_Y, ABS_RX, ABS_RY],
        InterfaceKind::Drums => &[
            ABS_X, ABS_Y, ABS_HAT2X, ABS_HAT2Y, ABS_HAT0X, ABS_HAT1X, ABS_HAT0Y, ABS_HAT3X,
            ABS_HAT3Y,
        ],
        InterfaceKind::Guitar => &[ABS_X, ABS_Y, ABS_HAT1X, ABS_HAT0X],
        InterfaceKind::Core => &[],
    }
}

fn known_abs(interface: InterfaceKind, code: u16) -> bool {
    abs_codes(interface).contains(&code)
}

pub fn map_core_key(c: u16) -> Option<u32> {
    Some(match c {
        KEY_LEFT => XWII_KEY_LEFT,
        KEY_RIGHT => XWII_KEY_RIGHT,
        KEY_UP => XWII_KEY_UP,
        KEY_DOWN => XWII_KEY_DOWN,
        KEY_NEXT => XWII_KEY_PLUS,
        KEY_PREVIOUS => XWII_KEY_MINUS,
        BTN_1 => XWII_KEY_ONE,
        BTN_2 => XWII_KEY_TWO,
        BTN_A => XWII_KEY_A,
        BTN_B => XWII_KEY_B,
        BTN_MODE => XWII_KEY_HOME,
        _ => return None,
    })
}
pub fn map_nunchuk_key(c: u16) -> Option<u32> {
    Some(match c {
        BTN_C => XWII_KEY_C,
        BTN_Z => XWII_KEY_Z,
        _ => return None,
    })
}
pub fn map_classic_key(c: u16) -> Option<u32> {
    Some(match c {
        BTN_A => XWII_KEY_A,
        BTN_B => XWII_KEY_B,
        BTN_X => XWII_KEY_X,
        BTN_Y => XWII_KEY_Y,
        KEY_NEXT => XWII_KEY_PLUS,
        KEY_PREVIOUS => XWII_KEY_MINUS,
        BTN_MODE => XWII_KEY_HOME,
        KEY_LEFT => XWII_KEY_LEFT,
        KEY_RIGHT => XWII_KEY_RIGHT,
        KEY_UP => XWII_KEY_UP,
        KEY_DOWN => XWII_KEY_DOWN,
        BTN_TL => XWII_KEY_TL,
        BTN_TR => XWII_KEY_TR,
        BTN_TL2 => XWII_KEY_ZL,
        BTN_TR2 => XWII_KEY_ZR,
        _ => return None,
    })
}
pub fn map_pro_key(c: u16) -> Option<u32> {
    Some(match c {
        BTN_EAST => XWII_KEY_A,
        BTN_SOUTH => XWII_KEY_B,
        BTN_NORTH => XWII_KEY_X,
        BTN_WEST => XWII_KEY_Y,
        BTN_START => XWII_KEY_PLUS,
        BTN_SELECT => XWII_KEY_MINUS,
        BTN_MODE => XWII_KEY_HOME,
        BTN_DPAD_LEFT => XWII_KEY_LEFT,
        BTN_DPAD_RIGHT => XWII_KEY_RIGHT,
        BTN_DPAD_UP => XWII_KEY_UP,
        BTN_DPAD_DOWN => XWII_KEY_DOWN,
        BTN_TL => XWII_KEY_TL,
        BTN_TR => XWII_KEY_TR,
        BTN_TL2 => XWII_KEY_ZL,
        BTN_TR2 => XWII_KEY_ZR,
        BTN_THUMBL => XWII_KEY_THUMBL,
        BTN_THUMBR => XWII_KEY_THUMBR,
        _ => return None,
    })
}
pub fn map_drums_key(c: u16) -> Option<u32> {
    Some(match c {
        BTN_START => XWII_KEY_PLUS,
        BTN_SELECT => XWII_KEY_MINUS,
        _ => return None,
    })
}
pub fn map_guitar_key(c: u16) -> Option<u32> {
    Some(match c {
        BTN_1 => XWII_KEY_FRET_FAR_UP,
        BTN_2 => XWII_KEY_FRET_UP,
        BTN_3 => XWII_KEY_FRET_MID,
        BTN_4 => XWII_KEY_FRET_LOW,
        BTN_5 => XWII_KEY_FRET_FAR_LOW,
        BTN_DPAD_UP => XWII_KEY_STRUM_BAR_UP,
        BTN_DPAD_DOWN => XWII_KEY_STRUM_BAR_DOWN,
        BTN_START => XWII_KEY_PLUS,
        BTN_SELECT => XWII_KEY_MINUS,
        _ => return None,
    })
}
