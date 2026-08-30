use crate::Interface;
use crate::abi::{self, CEvent};
use crate::backend;
use crate::device::RawEvent;
use crate::monitor::Monitor;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::ptr;

#[inline]
fn guard<T: Copy>(fallback: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(fallback)
}
#[inline]
fn status(result: Result<(), i32>) -> c_int {
    match result {
        Ok(()) => 0,
        Err(errno) => errno,
    }
}
unsafe fn iface<'a>(p: *mut Interface) -> Option<&'a mut Interface> {
    unsafe { p.as_mut() }
}
unsafe fn monitor<'a>(p: *mut Monitor) -> Option<&'a mut Monitor> {
    unsafe { p.as_mut() }
}

/// Returns the static name for an interface bit.
///
/// # Safety
///
/// This function has no caller-side safety requirements. A non-null return
/// value points to immutable static NUL-terminated storage and must not be freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xwii_get_iface_name(iface: u32) -> *const c_char {
    guard(ptr::null(), || {
        let i = match iface {
            abi::XWII_IFACE_CORE => 0,
            abi::XWII_IFACE_ACCEL => 1,
            abi::XWII_IFACE_IR => 2,
            abi::XWII_IFACE_MOTION_PLUS => 3,
            abi::XWII_IFACE_NUNCHUK => 4,
            abi::XWII_IFACE_CLASSIC_CONTROLLER => 5,
            abi::XWII_IFACE_BALANCE_BOARD => 6,
            abi::XWII_IFACE_PRO_CONTROLLER => 7,
            abi::XWII_IFACE_DRUMS => 8,
            abi::XWII_IFACE_GUITAR => 9,
            _ => return ptr::null(),
        };
        backend::IF_NAMES[i].as_ptr().cast()
    })
}

#[unsafe(no_mangle)]
/// Creates an interface for `syspath` and stores its owned handle in `out`.
///
/// # Safety
///
/// If non-null, `syspath` must point to a readable NUL-terminated string for
/// the duration of the call. If non-null, `out` must be aligned and writable
/// for one `*mut Interface`. On success, the stored handle owns one reference
/// that the caller must eventually pass to [`xwii_iface_unref`].
pub unsafe extern "C" fn xwii_iface_new(out: *mut *mut Interface, syspath: *const c_char) -> c_int {
    guard(-libc::EINVAL, || {
        if out.is_null() || syspath.is_null() {
            return -libc::EINVAL;
        };
        let path = match unsafe { CStr::from_ptr(syspath) }.to_str() {
            Ok(x) => Path::new(x),
            Err(_) => return -libc::EINVAL,
        };
        match Interface::new(path) {
            Ok(d) => {
                unsafe { *out = Box::into_raw(Box::new(d)) };
                0
            }
            Err(e) => e,
        }
    })
}
#[unsafe(no_mangle)]
/// Adds one reference to an interface handle.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call.
pub unsafe extern "C" fn xwii_iface_ref(p: *mut Interface) {
    guard((), || {
        if let Some(d) = unsafe { iface(p) }
            && d.refcount > 0
        {
            d.refcount += 1;
        }
    })
}
#[unsafe(no_mangle)]
/// Releases one owned reference to an interface handle.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`], the caller must own one reference to it, and it must be
/// exclusively accessible for the duration of the call. If this releases the
/// final reference, `p` becomes invalid immediately and must not be used again.
pub unsafe extern "C" fn xwii_iface_unref(p: *mut Interface) {
    guard((), || {
        if p.is_null() {
            return;
        }
        let d = unsafe { &mut *p };
        if d.refcount == 0 {
            return;
        }
        d.refcount -= 1;
        if d.refcount == 0 {
            drop(unsafe { Box::from_raw(p) });
        }
    })
}
#[unsafe(no_mangle)]
/// Returns the interface's borrowed system-path string.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call. A non-null return value is borrowed from the interface, must not
/// be freed, and remains valid only while the interface remains live.
pub unsafe extern "C" fn xwii_iface_get_syspath(p: *mut Interface) -> *const c_char {
    guard(ptr::null(), || {
        let Some(d) = (unsafe { iface(p) }) else {
            return ptr::null();
        };
        d.syspath_c.as_ptr()
    })
}
#[unsafe(no_mangle)]
/// Returns the interface's file descriptor.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call.
pub unsafe extern "C" fn xwii_iface_get_fd(p: *mut Interface) -> c_int {
    guard(-1, || unsafe { iface(p) }.map_or(-1, |d| d.fd()))
}
#[unsafe(no_mangle)]
/// Enables or disables interface-change monitoring.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call. `watch` must have a valid C boolean representation.
pub unsafe extern "C" fn xwii_iface_watch(p: *mut Interface, watch: bool) -> c_int {
    guard(-libc::EINVAL, || {
        unsafe { iface(p) }.map_or(-libc::EINVAL, |d| status(d.watch(watch)))
    })
}
#[unsafe(no_mangle)]
/// Opens the selected interface bits.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call.
pub unsafe extern "C" fn xwii_iface_open(p: *mut Interface, ifaces: u32) -> c_int {
    guard(-libc::EINVAL, || {
        unsafe { iface(p) }.map_or(-libc::EINVAL, |d| status(d.open(ifaces)))
    })
}
#[unsafe(no_mangle)]
/// Closes the selected interface bits.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call.
pub unsafe extern "C" fn xwii_iface_close(p: *mut Interface, ifaces: u32) {
    guard((), || {
        if let Some(d) = unsafe { iface(p) } {
            d.close(ifaces)
        }
    })
}
#[unsafe(no_mangle)]
/// Returns the currently open interface bits.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call.
pub unsafe extern "C" fn xwii_iface_opened(p: *mut Interface) -> u32 {
    guard(0, || unsafe { iface(p) }.map_or(0, |d| d.opened()))
}
#[unsafe(no_mangle)]
/// Returns the currently available interface bits.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call.
pub unsafe extern "C" fn xwii_iface_available(p: *mut Interface) -> u32 {
    guard(0, || unsafe { iface(p) }.map_or(0, |d| d.available()))
}

fn copy_raw(raw: &RawEvent, out: &mut CEvent) {
    out.time = raw.time;
    out.event_type = raw.kind;
    unsafe { ptr::copy_nonoverlapping(raw.payload.as_ptr(), out.v.reserved.as_mut_ptr(), 128) }
}

fn dispatch_writes_output(out: *mut CEvent, size: usize) -> bool {
    !out.is_null() && size != 0
}
#[unsafe(no_mangle)]
/// Dispatches one event, copying at most `size` bytes to `out`.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call. If non-null, `out` must be writable for at least
/// `min(size, size_of::<CEvent>())` bytes.
/// A null `out` or zero `size` is a no-op and does not consume queued input.
pub unsafe extern "C" fn xwii_iface_dispatch(
    p: *mut Interface,
    out: *mut CEvent,
    size: usize,
) -> c_int {
    if p.is_null() {
        return -libc::EFAULT;
    }
    if !dispatch_writes_output(out, size) {
        return 0;
    }
    guard(-libc::EINVAL, || {
        let Some(device) = (unsafe { iface(p) }) else {
            return -libc::EFAULT;
        };
        let mut raw = RawEvent::default();
        match device.dispatch(Some(&mut raw)) {
            Ok(()) => {
                let n = size.min(std::mem::size_of::<CEvent>());
                let mut full = CEvent::default();
                copy_raw(&raw, &mut full);
                unsafe {
                    ptr::copy_nonoverlapping((&full as *const CEvent).cast::<u8>(), out.cast(), n);
                }
                0
            }
            Err(errno) => errno,
        }
    })
}
#[unsafe(no_mangle)]
/// Polls one complete event into `out`.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call. If non-null, `out` must be writable for one [`CEvent`].
pub unsafe extern "C" fn xwii_iface_poll(p: *mut Interface, out: *mut CEvent) -> c_int {
    unsafe { xwii_iface_dispatch(p, out, std::mem::size_of::<CEvent>()) }
}
#[unsafe(no_mangle)]
/// Changes the interface's rumble state.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call. `on` must have a valid C boolean representation.
pub unsafe extern "C" fn xwii_iface_rumble(p: *mut Interface, on: bool) -> c_int {
    guard(-libc::EINVAL, || {
        unsafe { iface(p) }.map_or(-libc::EINVAL, |d| status(d.rumble(on)))
    })
}
#[unsafe(no_mangle)]
/// Writes the selected LED state to `state`.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call. If non-null, `state` must be aligned and writable for one `bool`.
pub unsafe extern "C" fn xwii_iface_get_led(
    p: *mut Interface,
    led: u32,
    state: *mut bool,
) -> c_int {
    guard(-libc::EINVAL, || {
        if state.is_null() {
            return -libc::EINVAL;
        };
        let Some(d) = (unsafe { iface(p) }) else {
            return -libc::EINVAL;
        };
        match d.get_led(led.wrapping_sub(1) as usize) {
            Ok(v) => {
                unsafe { *state = v };
                0
            }
            Err(e) => e,
        }
    })
}
#[unsafe(no_mangle)]
/// Changes the selected LED state.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call. `state` must have a valid C boolean representation.
pub unsafe extern "C" fn xwii_iface_set_led(p: *mut Interface, led: u32, state: bool) -> c_int {
    guard(-libc::EINVAL, || {
        unsafe { iface(p) }.map_or(-libc::EINVAL, |d| {
            status(d.set_led(led.wrapping_sub(1) as usize, state))
        })
    })
}
fn attr_out(p: *mut Interface, name: &str, out: *mut *mut c_char) -> c_int {
    if out.is_null() {
        return -libc::EINVAL;
    };
    let Some(d) = (unsafe { iface(p) }) else {
        return -libc::EINVAL;
    };
    match d.attr(name) {
        Ok(v) => {
            let q = crate::sys::alloc_c_string(&v);
            if q.is_null() {
                -libc::ENOMEM
            } else {
                unsafe { *out = q };
                0
            }
        }
        Err(e) => e,
    }
}
#[unsafe(no_mangle)]
/// Writes the interface's battery capacity to `capacity`.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call. If non-null, `capacity` must be writable for one `u8`.
pub unsafe extern "C" fn xwii_iface_get_battery(p: *mut Interface, capacity: *mut u8) -> c_int {
    guard(-libc::EINVAL, || {
        if capacity.is_null() {
            return -libc::EINVAL;
        };
        match unsafe { iface(p) }
            .ok_or(-libc::EINVAL)
            .and_then(|d| d.battery())
        {
            Ok(v) => {
                unsafe { *capacity = v };
                0
            }
            Err(e) => e,
        }
    })
}
#[unsafe(no_mangle)]
/// Allocates and returns the interface's device-type string.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call. If non-null, `out` must be aligned and writable for one
/// `*mut c_char`. On success, the stored string is allocated with `malloc` and
/// must be released by the caller with `free`.
pub unsafe extern "C" fn xwii_iface_get_devtype(p: *mut Interface, out: *mut *mut c_char) -> c_int {
    guard(-libc::EINVAL, || attr_out(p, "devtype", out))
}
#[unsafe(no_mangle)]
/// Allocates and returns the interface's extension string.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call. If non-null, `out` must be aligned and writable for one
/// `*mut c_char`. On success, the stored string is allocated with `malloc` and
/// must be released by the caller with `free`.
pub unsafe extern "C" fn xwii_iface_get_extension(
    p: *mut Interface,
    out: *mut *mut c_char,
) -> c_int {
    guard(-libc::EINVAL, || attr_out(p, "extension", out))
}
#[unsafe(no_mangle)]
/// Sets the Motion Plus normalization values.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call.
pub unsafe extern "C" fn xwii_iface_set_mp_normalization(
    p: *mut Interface,
    x: i32,
    y: i32,
    z: i32,
    f: i32,
) {
    guard((), || {
        if let Some(d) = unsafe { iface(p) } {
            d.set_mp_normalization(x, y, z, f)
        }
    })
}
#[unsafe(no_mangle)]
/// Writes the Motion Plus normalization values to the provided outputs.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_iface_new`] and must be exclusively accessible for the duration of
/// the call. Each non-null output pointer must be aligned and writable for one
/// `i32`.
pub unsafe extern "C" fn xwii_iface_get_mp_normalization(
    p: *mut Interface,
    x: *mut i32,
    y: *mut i32,
    z: *mut i32,
    f: *mut i32,
) {
    guard((), || {
        let (v, k) = unsafe { iface(p) }
            .map(|device| device.mp_normalization())
            .unwrap_or(([0; 3], 0));
        if !x.is_null() {
            unsafe { *x = v[0] }
        }
        if !y.is_null() {
            unsafe { *y = v[1] }
        }
        if !z.is_null() {
            unsafe { *z = v[2] }
        }
        if !f.is_null() {
            unsafe { *f = k }
        }
    })
}

#[unsafe(no_mangle)]
/// Creates a monitor and returns its owned handle.
///
/// # Safety
///
/// `poll` and `direct` must have valid C boolean representations. A non-null
/// return value owns one reference that the caller must eventually pass to
/// [`xwii_monitor_unref`].
pub unsafe extern "C" fn xwii_monitor_new(poll: bool, direct: bool) -> *mut Monitor {
    guard(ptr::null_mut(), || {
        Monitor::new(poll, direct)
            .map(|m| Box::into_raw(Box::new(m)))
            .unwrap_or(ptr::null_mut())
    })
}
#[unsafe(no_mangle)]
/// Adds one reference to a monitor handle.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_monitor_new`] and must be exclusively accessible for the duration of
/// the call.
pub unsafe extern "C" fn xwii_monitor_ref(p: *mut Monitor) {
    guard((), || {
        if let Some(m) = unsafe { monitor(p) }
            && m.refcount > 0
        {
            m.refcount += 1
        }
    })
}
#[unsafe(no_mangle)]
/// Releases one owned reference to a monitor handle.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_monitor_new`], the caller must own one reference to it, and it must
/// be exclusively accessible for the duration of the call. If this releases
/// the final reference, `p` becomes invalid immediately and must not be used
/// again.
pub unsafe extern "C" fn xwii_monitor_unref(p: *mut Monitor) {
    guard((), || {
        if p.is_null() {
            return;
        };
        let m = unsafe { &mut *p };
        if m.refcount == 0 {
            return;
        };
        m.refcount -= 1;
        if m.refcount == 0 {
            drop(unsafe { Box::from_raw(p) });
        }
    })
}
#[unsafe(no_mangle)]
/// Returns one of the monitor's file descriptors.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_monitor_new`] and must be exclusively accessible for the duration of
/// the call. `blocking` must have a valid C boolean representation.
pub unsafe extern "C" fn xwii_monitor_get_fd(p: *mut Monitor, blocking: bool) -> c_int {
    guard(-1, || {
        unsafe { monitor(p) }
            .and_then(|m| m.fd(blocking))
            .unwrap_or(-1)
    })
}
#[unsafe(no_mangle)]
/// Polls the monitor and returns an allocated system-path string.
///
/// # Safety
///
/// `p` may be null. Otherwise, it must be a live handle returned by
/// [`xwii_monitor_new`] and must be exclusively accessible for the duration of
/// the call. A non-null return value is allocated with `malloc` and must be
/// released by the caller with `free`.
pub unsafe extern "C" fn xwii_monitor_poll(p: *mut Monitor) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let Some(m) = (unsafe { monitor(p) }) else {
            return ptr::null_mut();
        };
        m.poll()
            .map(|x| crate::sys::alloc_c_string(x.as_os_str().as_encoded_bytes()))
            .unwrap_or(ptr::null_mut())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_or_zero_sized_output_skips_dispatch() {
        let output = std::ptr::NonNull::<CEvent>::dangling().as_ptr();
        assert!(!dispatch_writes_output(ptr::null_mut(), 0));
        assert!(!dispatch_writes_output(ptr::null_mut(), 1));
        assert!(!dispatch_writes_output(output, 0));
        assert!(dispatch_writes_output(output, 1));
    }
}
