//! Narrow raw bindings used by the Linux `hid-wiimote` device implementation.
//! The rest of the crate never owns a libudev object without one of the RAII
//! wrappers in this module.
#![allow(non_camel_case_types, dead_code)]

use std::os::fd::RawFd;
use std::os::raw::{c_char, c_int, c_void};

#[repr(C)]
pub struct udev {
    _private: [u8; 0],
}
#[repr(C)]
pub struct udev_device {
    _private: [u8; 0],
}
#[repr(C)]
pub struct udev_enumerate {
    _private: [u8; 0],
}
#[repr(C)]
pub struct udev_list_entry {
    _private: [u8; 0],
}
#[repr(C)]
pub struct udev_monitor {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct InputEvent {
    pub time: libc::timeval,
    pub type_: u16,
    pub code: u16,
    pub value: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct InputAbsInfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

#[cfg_attr(
    any(target_arch = "x86_64", all(target_arch = "x86", target_env = "gnu")),
    repr(C, packed)
)]
#[cfg_attr(
    not(any(target_arch = "x86_64", all(target_arch = "x86", target_env = "gnu"))),
    repr(C)
)]
#[derive(Clone, Copy, Default)]
pub struct EpollEvent {
    pub events: u32,
    pub data: u64,
}

const _: () = {
    assert!(std::mem::size_of::<EpollEvent>() == std::mem::size_of::<libc::epoll_event>());
    assert!(std::mem::align_of::<EpollEvent>() == std::mem::align_of::<libc::epoll_event>());
};

#[cfg(any(target_arch = "x86_64", all(target_arch = "x86", target_env = "gnu")))]
const _: () = {
    assert!(std::mem::size_of::<EpollEvent>() == 12);
    assert!(std::mem::offset_of!(EpollEvent, data) == 4);
};

#[link(name = "udev")]
unsafe extern "C" {
    pub fn udev_new() -> *mut udev;
    pub fn udev_ref(u: *mut udev) -> *mut udev;
    pub fn udev_unref(u: *mut udev) -> *mut udev;
    pub fn udev_device_new_from_syspath(u: *mut udev, path: *const c_char) -> *mut udev_device;
    pub fn udev_device_ref(d: *mut udev_device) -> *mut udev_device;
    pub fn udev_device_unref(d: *mut udev_device) -> *mut udev_device;
    pub fn udev_device_get_syspath(d: *mut udev_device) -> *const c_char;
    pub fn udev_device_get_subsystem(d: *mut udev_device) -> *const c_char;
    pub fn udev_device_get_driver(d: *mut udev_device) -> *const c_char;
    pub fn udev_device_get_sysname(d: *mut udev_device) -> *const c_char;
    pub fn udev_device_get_devnode(d: *mut udev_device) -> *const c_char;
    pub fn udev_device_get_action(d: *mut udev_device) -> *const c_char;
    pub fn udev_device_get_sysattr_value(d: *mut udev_device, name: *const c_char)
    -> *const c_char;
    pub fn udev_device_get_parent(d: *mut udev_device) -> *mut udev_device;

    pub fn udev_enumerate_new(u: *mut udev) -> *mut udev_enumerate;
    pub fn udev_enumerate_ref(e: *mut udev_enumerate) -> *mut udev_enumerate;
    pub fn udev_enumerate_unref(e: *mut udev_enumerate) -> *mut udev_enumerate;
    pub fn udev_enumerate_add_match_subsystem(e: *mut udev_enumerate, s: *const c_char) -> c_int;
    pub fn udev_enumerate_add_match_parent(e: *mut udev_enumerate, d: *mut udev_device) -> c_int;
    pub fn udev_enumerate_scan_devices(e: *mut udev_enumerate) -> c_int;
    pub fn udev_enumerate_get_list_entry(e: *mut udev_enumerate) -> *mut udev_list_entry;
    pub fn udev_list_entry_get_next(e: *mut udev_list_entry) -> *mut udev_list_entry;
    pub fn udev_list_entry_get_name(e: *mut udev_list_entry) -> *const c_char;

    pub fn udev_monitor_new_from_netlink(u: *mut udev, name: *const c_char) -> *mut udev_monitor;
    pub fn udev_monitor_ref(m: *mut udev_monitor) -> *mut udev_monitor;
    pub fn udev_monitor_unref(m: *mut udev_monitor) -> *mut udev_monitor;
    pub fn udev_monitor_filter_add_match_subsystem_devtype(
        m: *mut udev_monitor,
        s: *const c_char,
        t: *const c_char,
    ) -> c_int;
    pub fn udev_monitor_enable_receiving(m: *mut udev_monitor) -> c_int;
    pub fn udev_monitor_get_fd(m: *mut udev_monitor) -> c_int;
    pub fn udev_monitor_receive_device(m: *mut udev_monitor) -> *mut udev_device;
}

pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_ABS: u16 = 0x03;
pub const EV_FF: u16 = 0x15;
pub const SYN_REPORT: u16 = 0;
pub const SYN_DROPPED: u16 = 3;
pub const FF_RUMBLE: u16 = 0x50;
pub const FF_GAIN: u16 = 0x60;
pub const EVIOCGKEY: libc::c_ulong = 0x80404518;
pub const EVIOCGABS_BASE: libc::c_ulong = 0x80184540;
pub const EVIOCGABS: libc::c_ulong = EVIOCGABS_BASE;
pub const EVIOCSFF: libc::c_ulong = 0x40304580;
pub const EVIOCRMFF: libc::c_ulong = 0x40044581;

#[inline]
pub fn evio_cgabs(code: u16) -> libc::c_ulong {
    EVIOCGABS | ((code as libc::c_ulong) << 8)
}
#[inline]
pub fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

/// Borrows a C string from `p`, returning `None` for a null pointer.
///
/// # Safety
///
/// A non-null `p` must point to a valid NUL-terminated byte sequence. The
/// sequence must remain readable and immutable for the entire lifetime `'a`.
pub unsafe fn cstr<'a>(p: *const c_char) -> Option<&'a std::ffi::CStr> {
    if p.is_null() {
        None
    } else {
        Some(unsafe { std::ffi::CStr::from_ptr(p) })
    }
}

/// Creates a close-on-exec epoll file descriptor.
///
/// # Safety
///
/// This function has no caller-side safety requirements. A nonnegative return
/// value is an owned file descriptor that the caller must eventually close.
pub unsafe fn epoll_create() -> RawFd {
    unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) }
}
/// Applies an operation to an epoll interest list.
///
/// # Safety
///
/// `fd` and `target` must remain open and must not be concurrently closed or
/// reused for the duration of the call. For operations that consume an event,
/// `event` must be non-null, aligned, and readable as an [`EpollEvent`] for the
/// duration of the call; for operations that ignore it, `event` may be null.
pub unsafe fn epoll_ctl(fd: RawFd, op: c_int, target: RawFd, event: *mut EpollEvent) -> c_int {
    unsafe { libc::epoll_ctl(fd, op, target, event.cast::<libc::epoll_event>()) }
}
/// Waits for events and writes them into `events`.
///
/// # Safety
///
/// `fd` must remain open, refer to an epoll instance, and not be concurrently
/// closed or reused for the duration of the call. The caller must uphold any
/// kernel-level synchronization requirements for the registrations represented
/// by returned events.
pub unsafe fn epoll_wait(fd: RawFd, events: &mut [EpollEvent], timeout: c_int) -> c_int {
    unsafe {
        libc::epoll_wait(
            fd,
            events.as_mut_ptr().cast::<libc::epoll_event>(),
            events.len() as c_int,
            timeout,
        )
    }
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn ioctl<T>(fd: RawFd, request: libc::c_ulong, arg: *mut T) -> c_int {
    unsafe { libc::ioctl(fd, request, arg) }
}

pub fn close_fd(fd: &mut RawFd) {
    if *fd >= 0 {
        unsafe {
            libc::close(*fd);
        }
        *fd = -1;
    }
}

pub fn alloc_c_string(s: &[u8]) -> *mut c_char {
    let mut bytes = s.to_vec();
    if bytes.last().copied() != Some(0) {
        bytes.push(0);
    }
    let p = unsafe { libc::malloc(bytes.len()) as *mut c_char };
    if p.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), p, bytes.len());
    }
    p
}

pub fn c_void_ptr<T>(p: *mut T) -> *mut c_void {
    p.cast()
}

#[cfg(test)]
mod tests {
    use super::EpollEvent;
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn epoll_event_matches_libc_layout_and_array_stride() {
        assert_eq!(size_of::<EpollEvent>(), size_of::<libc::epoll_event>());
        assert_eq!(align_of::<EpollEvent>(), align_of::<libc::epoll_event>());

        let events = [EpollEvent::default(); 2];
        let first = events.as_ptr() as usize;
        let second = unsafe { events.as_ptr().add(1) } as usize;
        assert_eq!(second - first, size_of::<libc::epoll_event>());
    }

    #[cfg(any(target_arch = "x86_64", all(target_arch = "x86", target_env = "gnu")))]
    #[test]
    fn x86_epoll_event_is_kernel_packed() {
        assert_eq!(size_of::<EpollEvent>(), 12);
        assert_eq!(offset_of!(EpollEvent, data), 4);
    }
}
