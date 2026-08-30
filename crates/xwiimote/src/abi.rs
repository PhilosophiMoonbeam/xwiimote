//! Stable C-compatible data types and numeric constants from `lib/xwiimote.h`.
//!
//! C-facing fields intentionally use integers rather than Rust enums.  This
//! keeps values introduced by a newer library representable by older clients.

use core::fmt;

pub const XWII_NAME_CORE: &str = "Nintendo Wii Remote";
pub const XWII_NAME_ACCEL: &str = "Nintendo Wii Remote Accelerometer";
pub const XWII_NAME_IR: &str = "Nintendo Wii Remote IR";
pub const XWII_NAME_MOTION_PLUS: &str = "Nintendo Wii Remote Motion Plus";
pub const XWII_NAME_NUNCHUK: &str = "Nintendo Wii Remote Nunchuk";
pub const XWII_NAME_CLASSIC_CONTROLLER: &str = "Nintendo Wii Remote Classic Controller";
pub const XWII_NAME_BALANCE_BOARD: &str = "Nintendo Wii Remote Balance Board";
pub const XWII_NAME_PRO_CONTROLLER: &str = "Nintendo Wii Remote Pro Controller";
pub const XWII_NAME_DRUMS: &str = "Nintendo Wii Remote Drums";
pub const XWII_NAME_GUITAR: &str = "Nintendo Wii Remote Guitar";

pub const XWII_EVENT_KEY: u32 = 0;
pub const XWII_EVENT_ACCEL: u32 = 1;
pub const XWII_EVENT_IR: u32 = 2;
pub const XWII_EVENT_BALANCE_BOARD: u32 = 3;
pub const XWII_EVENT_MOTION_PLUS: u32 = 4;
pub const XWII_EVENT_PRO_CONTROLLER_KEY: u32 = 5;
pub const XWII_EVENT_PRO_CONTROLLER_MOVE: u32 = 6;
pub const XWII_EVENT_WATCH: u32 = 7;
pub const XWII_EVENT_CLASSIC_CONTROLLER_KEY: u32 = 8;
pub const XWII_EVENT_CLASSIC_CONTROLLER_MOVE: u32 = 9;
pub const XWII_EVENT_NUNCHUK_KEY: u32 = 10;
pub const XWII_EVENT_NUNCHUK_MOVE: u32 = 11;
pub const XWII_EVENT_DRUMS_KEY: u32 = 12;
pub const XWII_EVENT_DRUMS_MOVE: u32 = 13;
pub const XWII_EVENT_GUITAR_KEY: u32 = 14;
pub const XWII_EVENT_GUITAR_MOVE: u32 = 15;
pub const XWII_EVENT_GONE: u32 = 16;
pub const XWII_EVENT_NUM: u32 = 17;

pub const XWII_KEY_LEFT: u32 = 0;
pub const XWII_KEY_RIGHT: u32 = 1;
pub const XWII_KEY_UP: u32 = 2;
pub const XWII_KEY_DOWN: u32 = 3;
pub const XWII_KEY_A: u32 = 4;
pub const XWII_KEY_B: u32 = 5;
pub const XWII_KEY_PLUS: u32 = 6;
pub const XWII_KEY_MINUS: u32 = 7;
pub const XWII_KEY_HOME: u32 = 8;
pub const XWII_KEY_ONE: u32 = 9;
pub const XWII_KEY_TWO: u32 = 10;
pub const XWII_KEY_X: u32 = 11;
pub const XWII_KEY_Y: u32 = 12;
pub const XWII_KEY_TL: u32 = 13;
pub const XWII_KEY_TR: u32 = 14;
pub const XWII_KEY_ZL: u32 = 15;
pub const XWII_KEY_ZR: u32 = 16;
pub const XWII_KEY_THUMBL: u32 = 17;
pub const XWII_KEY_THUMBR: u32 = 18;
pub const XWII_KEY_C: u32 = 19;
pub const XWII_KEY_Z: u32 = 20;
pub const XWII_KEY_STRUM_BAR_UP: u32 = 21;
pub const XWII_KEY_STRUM_BAR_DOWN: u32 = 22;
pub const XWII_KEY_FRET_FAR_UP: u32 = 23;
pub const XWII_KEY_FRET_UP: u32 = 24;
pub const XWII_KEY_FRET_MID: u32 = 25;
pub const XWII_KEY_FRET_LOW: u32 = 26;
pub const XWII_KEY_FRET_FAR_LOW: u32 = 27;
pub const XWII_KEY_NUM: u32 = 28;

pub const XWII_DRUMS_ABS_PAD: usize = 0;
pub const XWII_DRUMS_ABS_CYMBAL_LEFT: usize = 1;
pub const XWII_DRUMS_ABS_CYMBAL_RIGHT: usize = 2;
pub const XWII_DRUMS_ABS_TOM_LEFT: usize = 3;
pub const XWII_DRUMS_ABS_TOM_RIGHT: usize = 4;
pub const XWII_DRUMS_ABS_TOM_FAR_RIGHT: usize = 5;
pub const XWII_DRUMS_ABS_BASS: usize = 6;
pub const XWII_DRUMS_ABS_HI_HAT: usize = 7;
pub const XWII_DRUMS_ABS_NUM: usize = 8;
pub const XWII_ABS_NUM: usize = 8;

pub const XWII_IFACE_CORE: u32 = 0x000001;
pub const XWII_IFACE_ACCEL: u32 = 0x000002;
pub const XWII_IFACE_IR: u32 = 0x000004;
pub const XWII_IFACE_MOTION_PLUS: u32 = 0x000100;
pub const XWII_IFACE_NUNCHUK: u32 = 0x000200;
pub const XWII_IFACE_CLASSIC_CONTROLLER: u32 = 0x000400;
pub const XWII_IFACE_BALANCE_BOARD: u32 = 0x000800;
pub const XWII_IFACE_PRO_CONTROLLER: u32 = 0x001000;
pub const XWII_IFACE_DRUMS: u32 = 0x002000;
pub const XWII_IFACE_GUITAR: u32 = 0x004000;
pub const XWII_IFACE_ALL: u32 = 0x007f07;
pub const XWII_IFACE_WRITABLE: u32 = 0x010000;
pub const XWII_LED1: u32 = 1;
pub const XWII_LED2: u32 = 2;
pub const XWII_LED3: u32 = 3;
pub const XWII_LED4: u32 = 4;

/// Linux evdev event kind values used by the pure decoder.
pub const EV_SYN: u16 = 0;
pub const EV_KEY: u16 = 1;
pub const EV_ABS: u16 = 3;
pub const SYN_REPORT: u16 = 0;
pub const SYN_DROPPED: u16 = 3;

/// C `struct xwii_event_key`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CEventKey {
    pub code: u32,
    pub state: u32,
}

/// C `struct xwii_event_abs`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CEventAbs {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// C `union xwii_event_union`.
#[repr(C)]
#[derive(Clone, Copy)]
pub union EventUnion {
    pub key: CEventKey,
    pub abs: [CEventAbs; XWII_ABS_NUM],
    pub reserved: [u8; 128],
}

impl Default for EventUnion {
    fn default() -> Self {
        Self { reserved: [0; 128] }
    }
}

impl fmt::Debug for EventUnion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventUnion").finish_non_exhaustive()
    }
}

/// C `struct xwii_event`.  `event_type` is the C `type` field.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CEvent {
    pub time: libc::timeval,
    pub event_type: u32,
    pub v: EventUnion,
}

impl Default for CEvent {
    fn default() -> Self {
        Self {
            time: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            event_type: 0,
            v: EventUnion::default(),
        }
    }
}

/// A raw Linux input event, useful to callers feeding [`crate::decode::Decoder`].
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct InputEvent {
    pub time: libc::timeval,
    pub event_type: u16,
    pub code: u16,
    pub value: i32,
}

impl PartialEq for InputEvent {
    fn eq(&self, other: &Self) -> bool {
        self.time.tv_sec == other.time.tv_sec
            && self.time.tv_usec == other.time.tv_usec
            && self.event_type == other.event_type
            && self.code == other.code
            && self.value == other.value
    }
}

impl Eq for InputEvent {}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct InterfaceMask(u32);
impl InterfaceMask {
    pub const CORE: Self = Self(XWII_IFACE_CORE);
    pub const ACCEL: Self = Self(XWII_IFACE_ACCEL);
    pub const IR: Self = Self(XWII_IFACE_IR);
    pub const MOTION_PLUS: Self = Self(XWII_IFACE_MOTION_PLUS);
    pub const NUNCHUK: Self = Self(XWII_IFACE_NUNCHUK);
    pub const CLASSIC_CONTROLLER: Self = Self(XWII_IFACE_CLASSIC_CONTROLLER);
    pub const BALANCE_BOARD: Self = Self(XWII_IFACE_BALANCE_BOARD);
    pub const PRO_CONTROLLER: Self = Self(XWII_IFACE_PRO_CONTROLLER);
    pub const DRUMS: Self = Self(XWII_IFACE_DRUMS);
    pub const GUITAR: Self = Self(XWII_IFACE_GUITAR);
    pub const ALL: Self = Self(XWII_IFACE_ALL);
    pub const WRITABLE: Self = Self(XWII_IFACE_WRITABLE);
    pub const fn empty() -> Self {
        Self(0)
    }
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }
    pub const fn bits(self) -> u32 {
        self.0
    }
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub const fn insert(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub const fn remove(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}
impl core::ops::BitOr for InterfaceMask {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
impl core::ops::BitAnd for InterfaceMask {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}
impl core::ops::Not for InterfaceMask {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

pub type XwiiEvent = CEvent;
pub type XwiiEventAbs = CEventAbs;
pub type XwiiEventKey = CEventKey;

#[inline]
pub const fn xwii_led(num: u32) -> u32 {
    XWII_LED1 + num - 1
}

#[inline]
pub const fn xwii_event_ir_is_valid(abs: &CEventAbs) -> bool {
    abs.x != 1023 || abs.y != 1023
}

// Common evdev key/absolute codes.  Keep these local so decoding remains pure
// and does not depend on libc exposing Linux input headers.
pub const KEY_LEFT: u16 = 105;
pub const KEY_RIGHT: u16 = 106;
pub const KEY_UP: u16 = 103;
pub const KEY_DOWN: u16 = 108;
pub const KEY_NEXT: u16 = 0x197;
pub const KEY_PREVIOUS: u16 = 0x19c;
pub const BTN_1: u16 = 0x101;
pub const BTN_2: u16 = 0x102;
pub const BTN_3: u16 = 0x103;
pub const BTN_4: u16 = 0x104;
pub const BTN_5: u16 = 0x105;
pub const BTN_A: u16 = 0x130;
pub const BTN_B: u16 = 0x131;
pub const BTN_C: u16 = 0x132;
pub const BTN_X: u16 = 0x133;
pub const BTN_Y: u16 = 0x134;
pub const BTN_Z: u16 = 0x135;
pub const BTN_TL: u16 = 0x136;
pub const BTN_TR: u16 = 0x137;
pub const BTN_TL2: u16 = 0x138;
pub const BTN_TR2: u16 = 0x139;
pub const BTN_SELECT: u16 = 0x13a;
pub const BTN_START: u16 = 0x13b;
pub const BTN_MODE: u16 = 0x13c;
pub const BTN_THUMBL: u16 = 0x13d;
pub const BTN_THUMBR: u16 = 0x13e;
pub const BTN_DPAD_UP: u16 = 0x220;
pub const BTN_DPAD_DOWN: u16 = 0x221;
pub const BTN_DPAD_LEFT: u16 = 0x222;
pub const BTN_DPAD_RIGHT: u16 = 0x223;
pub const BTN_EAST: u16 = BTN_B;
pub const BTN_SOUTH: u16 = BTN_A;
pub const BTN_NORTH: u16 = BTN_X;
pub const BTN_WEST: u16 = BTN_Y;
pub const ABS_X: u16 = 0;
pub const ABS_Y: u16 = 1;
pub const ABS_RX: u16 = 3;
pub const ABS_RY: u16 = 4;
pub const ABS_RZ: u16 = 5;
pub const ABS_HAT0X: u16 = 16;
pub const ABS_HAT0Y: u16 = 17;
pub const ABS_HAT1X: u16 = 18;
pub const ABS_HAT1Y: u16 = 19;
pub const ABS_HAT2X: u16 = 20;
pub const ABS_HAT2Y: u16 = 21;
pub const ABS_HAT3X: u16 = 22;
pub const ABS_HAT3Y: u16 = 23;
