//! Safe Rust API and compatibility ABI for Wii Remote input devices.

pub mod abi;
mod backend;
pub mod decode;

// Device/monitor/FFI implementations are maintained in the runtime leaf.  The
// module declarations are kept here so the crate has one stable public API.
pub mod device;
pub mod ffi;
pub mod monitor;
pub mod sys;

pub use decode::{Event, EventKind, EventType, InterfaceKind, MotionPlusNormalizer, RecoveryState};
pub use device::Interface;

pub type Errno = i32;
