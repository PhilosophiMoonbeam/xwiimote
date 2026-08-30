//! Native event types plus Linux evdev constants used by `hid-wiimote`.

pub const EVENT_CODE_KEY: u32 = 0;
pub const EVENT_CODE_ACCEL: u32 = 1;
pub const EVENT_CODE_IR: u32 = 2;
pub const EVENT_CODE_BALANCE_BOARD: u32 = 3;
pub const EVENT_CODE_MOTION_PLUS: u32 = 4;
pub const EVENT_CODE_PRO_CONTROLLER_KEY: u32 = 5;
pub const EVENT_CODE_PRO_CONTROLLER_MOVE: u32 = 6;
pub const EVENT_CODE_WATCH: u32 = 7;
pub const EVENT_CODE_CLASSIC_CONTROLLER_KEY: u32 = 8;
pub const EVENT_CODE_CLASSIC_CONTROLLER_MOVE: u32 = 9;
pub const EVENT_CODE_NUNCHUK_KEY: u32 = 10;
pub const EVENT_CODE_NUNCHUK_MOVE: u32 = 11;
pub const EVENT_CODE_DRUMS_KEY: u32 = 12;
pub const EVENT_CODE_DRUMS_MOVE: u32 = 13;
pub const EVENT_CODE_GUITAR_KEY: u32 = 14;
pub const EVENT_CODE_GUITAR_MOVE: u32 = 15;
pub const EVENT_CODE_GONE: u32 = 16;
pub const EVENT_CODE_COUNT: u32 = 17;

pub const BUTTON_LEFT: u32 = 0;
pub const BUTTON_RIGHT: u32 = 1;
pub const BUTTON_UP: u32 = 2;
pub const BUTTON_DOWN: u32 = 3;
pub const BUTTON_A: u32 = 4;
pub const BUTTON_B: u32 = 5;
pub const BUTTON_PLUS: u32 = 6;
pub const BUTTON_MINUS: u32 = 7;
pub const BUTTON_HOME: u32 = 8;
pub const BUTTON_ONE: u32 = 9;
pub const BUTTON_TWO: u32 = 10;
pub const BUTTON_X: u32 = 11;
pub const BUTTON_Y: u32 = 12;
pub const BUTTON_TL: u32 = 13;
pub const BUTTON_TR: u32 = 14;
pub const BUTTON_ZL: u32 = 15;
pub const BUTTON_ZR: u32 = 16;
pub const BUTTON_THUMBL: u32 = 17;
pub const BUTTON_THUMBR: u32 = 18;
pub const BUTTON_C: u32 = 19;
pub const BUTTON_Z: u32 = 20;
pub const BUTTON_STRUM_BAR_UP: u32 = 21;
pub const BUTTON_STRUM_BAR_DOWN: u32 = 22;
pub const BUTTON_FRET_FAR_UP: u32 = 23;
pub const BUTTON_FRET_UP: u32 = 24;
pub const BUTTON_FRET_MID: u32 = 25;
pub const BUTTON_FRET_LOW: u32 = 26;
pub const BUTTON_FRET_FAR_LOW: u32 = 27;
pub const BUTTON_COUNT: u32 = 28;

pub const DRUM_SLOT_PAD: usize = 0;
pub const DRUM_SLOT_CYMBAL_LEFT: usize = 1;
pub const DRUM_SLOT_CYMBAL_RIGHT: usize = 2;
pub const DRUM_SLOT_TOM_LEFT: usize = 3;
pub const DRUM_SLOT_TOM_RIGHT: usize = 4;
pub const DRUM_SLOT_TOM_FAR_RIGHT: usize = 5;
pub const DRUM_SLOT_BASS: usize = 6;
pub const DRUM_SLOT_HI_HAT: usize = 7;
pub const DRUM_SLOT_COUNT: usize = 8;

/// Linux evdev event kind values used by the pure decoder.
pub const EV_SYN: u16 = 0;
pub const EV_KEY: u16 = 1;
pub const EV_ABS: u16 = 3;
pub const SYN_REPORT: u16 = 0;
pub const SYN_DROPPED: u16 = 3;

/// A decoded Wii Remote button transition.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ButtonEvent {
    pub code: u32,
    pub state: u32,
}

/// A three-axis sample or one logical slot in a multi-axis report.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Axis3 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// A raw Linux input event consumed by the pure decoder.
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
    pub const CORE: Self = Self(0x000001);
    pub const ACCEL: Self = Self(0x000002);
    pub const IR: Self = Self(0x000004);
    pub const MOTION_PLUS: Self = Self(0x000100);
    pub const NUNCHUK: Self = Self(0x000200);
    pub const CLASSIC_CONTROLLER: Self = Self(0x000400);
    pub const BALANCE_BOARD: Self = Self(0x000800);
    pub const PRO_CONTROLLER: Self = Self(0x001000);
    pub const DRUMS: Self = Self(0x002000);
    pub const GUITAR: Self = Self(0x004000);
    pub const ALL: Self = Self(0x007f07);
    pub const WRITABLE: Self = Self(0x010000);
    pub const fn empty() -> Self {
        Self(0)
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
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
impl core::ops::BitOrAssign for InterfaceMask {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
impl core::ops::BitAndAssign for InterfaceMask {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
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

#[inline]
pub const fn is_valid_ir_point(abs: &Axis3) -> bool {
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
