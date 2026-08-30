//! Native Rust support for Wii Remote devices exposed by Linux `hid-wiimote`.

mod backend;
pub mod decode;
pub mod model;

// Device and monitor implementations own the kernel-facing runtime state.
mod device;
mod monitor;
mod sys;

pub use decode::{Event, EventKind, EventType, InterfaceKind, MotionPlusNormalizer, RecoveryState};
pub use device::Interface;
pub use model::{Axis3, ButtonEvent, InterfaceMask};
pub use monitor::Monitor;

pub type Errno = i32;
