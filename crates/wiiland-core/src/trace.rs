use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventType {
    Key,
    Accelerometer,
    Ir,
    BalanceBoard,
    MotionPlus,
    ProKey,
    ProMove,
    Watch,
    ClassicKey,
    ClassicMove,
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
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            0 => Self::Key,
            1 => Self::Accelerometer,
            2 => Self::Ir,
            3 => Self::BalanceBoard,
            4 => Self::MotionPlus,
            5 => Self::ProKey,
            6 => Self::ProMove,
            7 => Self::Watch,
            8 => Self::ClassicKey,
            9 => Self::ClassicMove,
            10 => Self::NunchukKey,
            11 => Self::NunchukMove,
            12 => Self::DrumsKey,
            13 => Self::DrumsMove,
            14 => Self::GuitarKey,
            15 => Self::GuitarMove,
            16 => Self::Gone,
            n => Self::Unknown(n),
        }
    }
    pub const fn raw(self) -> u32 {
        match self {
            Self::Key => 0,
            Self::Accelerometer => 1,
            Self::Ir => 2,
            Self::BalanceBoard => 3,
            Self::MotionPlus => 4,
            Self::ProKey => 5,
            Self::ProMove => 6,
            Self::Watch => 7,
            Self::ClassicKey => 8,
            Self::ClassicMove => 9,
            Self::NunchukKey => 10,
            Self::NunchukMove => 11,
            Self::DrumsKey => 12,
            Self::DrumsMove => 13,
            Self::GuitarKey => 14,
            Self::GuitarMove => 15,
            Self::Gone => 16,
            Self::Unknown(n) => n,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Key => "key",
            Self::Accelerometer => "accelerometer",
            Self::Ir => "ir",
            Self::BalanceBoard => "balance-board",
            Self::MotionPlus => "motion-plus",
            Self::ProKey => "pro-key",
            Self::ProMove => "pro-move",
            Self::Watch => "watch",
            Self::ClassicKey => "classic-key",
            Self::ClassicMove => "classic-move",
            Self::NunchukKey => "nunchuk-key",
            Self::NunchukMove => "nunchuk-move",
            Self::DrumsKey => "drums-key",
            Self::DrumsMove => "drums-move",
            Self::GuitarKey => "guitar-key",
            Self::GuitarMove => "guitar-move",
            Self::Gone => "gone",
            Self::Unknown(_) => "unknown",
        }
    }
    pub const fn is_key(self) -> bool {
        matches!(
            self,
            Self::Key
                | Self::ClassicKey
                | Self::NunchukKey
                | Self::DrumsKey
                | Self::GuitarKey
                | Self::ProKey
        )
    }
    pub const fn is_axes(self) -> bool {
        matches!(
            self,
            Self::Accelerometer
                | Self::Ir
                | Self::BalanceBoard
                | Self::MotionPlus
                | Self::ClassicMove
                | Self::NunchukMove
                | Self::DrumsMove
                | Self::GuitarMove
                | Self::ProMove
        )
    }
}
impl From<u32> for EventType {
    fn from(v: u32) -> Self {
        Self::from_raw(v)
    }
}
impl From<EventType> for u32 {
    fn from(v: EventType) -> u32 {
        v.raw()
    }
}

pub fn event_type_name(raw: u32) -> &'static str {
    EventType::from_raw(raw).name()
}
pub fn is_key_event(raw: u32) -> bool {
    EventType::from_raw(raw).is_key()
}
pub fn is_abs_event(raw: u32) -> bool {
    EventType::from_raw(raw).is_axes()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceFilter {
    All,
    Keys,
    Axes,
    Ir,
    MotionPlus,
}
impl TraceFilter {
    pub fn matches(self, event_type: u32) -> bool {
        match self {
            Self::All => true,
            Self::Keys => is_key_event(event_type),
            Self::Axes => is_abs_event(event_type),
            Self::Ir => event_type == EventType::Ir.raw(),
            Self::MotionPlus => event_type == EventType::MotionPlus.raw(),
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Keys => "keys",
            Self::Axes => "axes",
            Self::Ir => "ir",
            Self::MotionPlus => "motion-plus",
        }
    }
}
impl FromStr for TraceFilter {
    type Err = ();
    fn from_str(v: &str) -> Result<Self, Self::Err> {
        match v {
            "all" => Ok(Self::All),
            "keys" => Ok(Self::Keys),
            "axes" => Ok(Self::Axes),
            "ir" => Ok(Self::Ir),
            "motion-plus" => Ok(Self::MotionPlus),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyPayload {
    pub code: u32,
    pub state: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbsPayload {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum TracePayload {
    #[default]
    None,
    Key(KeyPayload),
    Axes(Vec<AbsPayload>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceEvent {
    pub sequence: u64,
    pub monotonic_us: Option<i64>,
    pub syspath: String,
    pub event_type: u32,
    pub payload: TracePayload,
}
impl TraceEvent {
    pub fn new(
        sequence: u64,
        monotonic_us: Option<i64>,
        syspath: impl Into<String>,
        event_type: u32,
        payload: TracePayload,
    ) -> Self {
        Self {
            sequence,
            monotonic_us,
            syspath: syspath.into(),
            event_type,
            payload,
        }
    }
    pub fn format_line(&self) -> String {
        let mut out = String::new();
        match self.monotonic_us {
            Some(us) => {
                let sec = us.div_euclid(1_000_000);
                let micros = us.rem_euclid(1_000_000);
                out.push_str(&format!("time={sec}.{micros:06} "));
            }
            None => out.push_str("time=unknown "),
        }
        out.push_str(&format!(
            "seq={} {} {} type={}",
            self.sequence,
            self.syspath,
            event_type_name(self.event_type),
            self.event_type
        ));
        match &self.payload {
            TracePayload::Key(k) => out.push_str(&format!(" key={} state={}", k.code, k.state)),
            TracePayload::Axes(values) => {
                for (i, v) in values.iter().enumerate() {
                    out.push_str(&format!(" abs{i}={},{},{}", v.x, v.y, v.z));
                }
            }
            TracePayload::None => {}
        }
        out.push('\n');
        out
    }
}
impl fmt::Display for TraceEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format_line())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TraceConfig {
    pub enabled: bool,
    pub filter: TraceFilter,
}
impl Default for TraceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            filter: TraceFilter::All,
        }
    }
}
impl TraceConfig {
    pub fn enable(&mut self, filter: Option<TraceFilter>) {
        self.enabled = true;
        if let Some(f) = filter {
            self.filter = f
        }
    }
    pub fn matches(&self, event_type: u32) -> bool {
        self.enabled && self.filter.matches(event_type)
    }
}
