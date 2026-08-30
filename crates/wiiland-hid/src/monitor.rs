use crate::backend::{UdevContext, UdevDevice, UdevMonitor};
use crate::sys;
use std::ffi::{CStr, CString};
use std::io;
use std::os::fd::{BorrowedFd, RawFd};
use std::path::PathBuf;

/// Selects whether a monitor enumerates existing devices or watches udev.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MonitorMode {
    Enumerate,
    Watch,
}

struct Initial {
    path: Vec<u8>,
    next: Option<Box<Initial>>,
}

fn deduplicate_initial(initial: &mut Option<Box<Initial>>, action: &[u8], path: &[u8]) -> bool {
    if action != b"add" && action != b"remove" {
        return false;
    }
    let mut link = initial;
    while link.is_some() {
        if let Some(mut item) = link.take_if(|item| item.path == path) {
            let duplicate = action == b"add";
            *link = item.next.take();
            return duplicate;
        }
        link = &mut link.as_mut().unwrap().next;
    }
    false
}

fn null_receive<T>(errno: i32) -> io::Result<Option<T>> {
    if errno == 0 || errno == libc::EAGAIN || errno == libc::EWOULDBLOCK {
        Ok(None)
    } else {
        Err(io::Error::from_raw_os_error(errno))
    }
}

fn classify_enum_device_errno(errno: i32) -> io::Result<()> {
    if errno == 0 || errno == libc::ENOENT || errno == libc::ENODEV {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(errno))
    }
}

pub struct Monitor {
    udev: UdevContext,
    enumerate: *mut sys::udev_enumerate,
    entry: *mut sys::udev_list_entry,
    initial: Option<Box<Initial>>,
    monitor: Option<UdevMonitor>,
    enumerated: bool,
}

impl Monitor {
    pub fn new(mode: MonitorMode) -> io::Result<Self> {
        let udev = UdevContext::new().ok_or_else(|| io::Error::from_raw_os_error(libc::ENOMEM))?;
        let mut mon = None;
        if mode == MonitorMode::Watch {
            // Watch mode deliberately always uses the udev netlink channel.
            let channel = CString::new("udev").expect("static channel has no NUL");
            let p = unsafe { sys::udev_monitor_new_from_netlink(udev.0, channel.as_ptr()) };
            if p.is_null() {
                return Err(io::Error::from_raw_os_error(libc::ENOMEM));
            }
            let m = UdevMonitor(p);
            let hid = CString::new("hid").expect("static subsystem has no NUL");
            if unsafe {
                sys::udev_monitor_filter_add_match_subsystem_devtype(
                    p,
                    hid.as_ptr(),
                    std::ptr::null(),
                )
            } != 0
                || unsafe { sys::udev_monitor_enable_receiving(p) } != 0
            {
                return Err(io::Error::from_raw_os_error(libc::ENOMEM));
            }
            mon = Some(m);
        }
        let en = unsafe { sys::udev_enumerate_new(udev.0) };
        if en.is_null() {
            return Err(io::Error::from_raw_os_error(libc::ENOMEM));
        }
        let hs = CString::new("hid").expect("static subsystem has no NUL");
        if unsafe { sys::udev_enumerate_add_match_subsystem(en, hs.as_ptr()) } != 0
            || unsafe { sys::udev_enumerate_scan_devices(en) } != 0
        {
            unsafe { sys::udev_enumerate_unref(en) };
            return Err(io::Error::from_raw_os_error(libc::ENOMEM));
        }
        let entry = unsafe { sys::udev_enumerate_get_list_entry(en) };
        let mut initial = None;
        if mon.is_some() {
            let mut it = entry;
            while !it.is_null() {
                let p = unsafe { sys::cstr(sys::udev_list_entry_get_name(it)) };
                if let Some(p) = p {
                    initial = Some(Box::new(Initial {
                        path: p.to_bytes().to_vec(),
                        next: initial,
                    }));
                }
                it = unsafe { sys::udev_list_entry_get_next(it) };
            }
        }
        Ok(Self {
            udev,
            enumerate: en,
            entry,
            initial,
            monitor: mon,
            enumerated: false,
        })
    }

    pub fn fd(&self) -> Option<BorrowedFd<'_>> {
        let monitor = self.monitor.as_ref()?;
        let fd = unsafe { sys::udev_monitor_get_fd(monitor.0) };
        (fd >= 0).then(|| unsafe { BorrowedFd::borrow_raw(fd as RawFd) })
    }

    fn free_enum(&mut self) {
        self.initial = None;
        if !self.enumerate.is_null() {
            unsafe { sys::udev_enumerate_unref(self.enumerate) };
            self.enumerate = std::ptr::null_mut();
            self.entry = std::ptr::null_mut();
        }
    }
    fn next_enum(&mut self) -> io::Result<Option<UdevDevice>> {
        while !self.entry.is_null() {
            let e = self.entry;
            self.entry = unsafe { sys::udev_list_entry_get_next(e) };
            let Some(path) = (unsafe { sys::cstr(sys::udev_list_entry_get_name(e)) }) else {
                return Ok(None);
            };
            let (device, errno) = unsafe {
                *libc::__errno_location() = 0;
                let device = sys::udev_device_new_from_syspath(self.udev.0, path.as_ptr());
                (device, sys::errno())
            };
            if !device.is_null() {
                return Ok(Some(UdevDevice(device)));
            }
            classify_enum_device_errno(errno)?;
        }
        self.enumerated = true;
        if self.monitor.is_none() {
            self.free_enum()
        }
        Ok(None)
    }
    fn deduplicate(&mut self, d: &UdevDevice) -> bool {
        let Some(action) = d.action().map(CStr::to_bytes) else {
            return false;
        };
        if action != b"add" && action != b"remove" {
            return false;
        }
        let Some(path) = d.syspath().map(CStr::to_bytes) else {
            return false;
        };
        deduplicate_initial(&mut self.initial, action, path)
    }
    fn valid_device(&self, d: &UdevDevice) -> Option<PathBuf> {
        if d.action().is_some_and(|x| x.to_bytes() != b"add") {
            return None;
        }
        if d.driver().is_none_or(|x| x.to_bytes() != b"wiimote")
            || d.subsystem().is_none_or(|x| x.to_bytes() != b"hid")
        {
            return None;
        }
        Some(PathBuf::from(d.syspath()?.to_string_lossy().into_owned()))
    }

    pub fn poll(&mut self) -> io::Result<Option<PathBuf>> {
        if !self.enumerated {
            loop {
                let Some(d) = self.next_enum()? else {
                    return Ok(None);
                };
                if let Some(p) = self.valid_device(&d) {
                    return Ok(Some(p));
                }
            }
        }
        let Some(mp) = self.monitor.as_ref().map(|m| m.0) else {
            return Ok(None);
        };
        loop {
            let (p, errno) = unsafe {
                *libc::__errno_location() = 0;
                let p = sys::udev_monitor_receive_device(mp);
                (p, sys::errno())
            };
            if p.is_null() {
                let empty = null_receive(errno)?;
                self.free_enum();
                return Ok(empty);
            }
            let d = UdevDevice(p);
            if self.deduplicate(&d) {
                continue;
            }
            if let Some(path) = self.valid_device(&d) {
                return Ok(Some(path));
            }
        }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.free_enum();
        self.monitor.take();
    }
}

#[cfg(test)]
mod tests {
    use super::{Initial, classify_enum_device_errno, deduplicate_initial, null_receive};

    fn initial(paths: &[&[u8]]) -> Option<Box<Initial>> {
        paths.iter().rev().fold(None, |next, path| {
            Some(Box::new(Initial {
                path: path.to_vec(),
                next,
            }))
        })
    }
    #[test]
    fn initial_add_is_suppressed_once_then_readd_passes() {
        let mut initial = initial(&[b"/sys/device"]);
        assert!(deduplicate_initial(&mut initial, b"add", b"/sys/device"));
        assert!(!deduplicate_initial(&mut initial, b"add", b"/sys/device"));
    }
    #[test]
    fn initial_remove_is_consumed_then_readd_passes() {
        let mut initial = initial(&[b"/sys/device"]);
        assert!(!deduplicate_initial(
            &mut initial,
            b"remove",
            b"/sys/device"
        ));
        assert!(!deduplicate_initial(&mut initial, b"add", b"/sys/device"));
    }
    #[test]
    fn unrelated_path_does_not_consume_initial_path() {
        let mut initial = initial(&[b"/sys/device"]);
        assert!(!deduplicate_initial(&mut initial, b"add", b"/sys/other"));
        assert!(deduplicate_initial(&mut initial, b"add", b"/sys/device"));
    }
    #[test]
    fn unsupported_action_does_not_change_initial_state() {
        let mut initial = initial(&[b"/sys/device"]);
        assert!(!deduplicate_initial(
            &mut initial,
            b"change",
            b"/sys/device"
        ));
        assert!(deduplicate_initial(&mut initial, b"add", b"/sys/device"));
    }
    #[test]
    fn null_receive_without_errno_is_empty() {
        assert_eq!(null_receive::<()>(0).unwrap(), None);
    }
    #[test]
    fn null_receive_with_would_block_is_empty() {
        for errno in [libc::EAGAIN, libc::EWOULDBLOCK] {
            assert_eq!(null_receive::<()>(errno).unwrap(), None);
        }
    }
    #[test]
    fn null_receive_with_real_errno_is_error() {
        let error = null_receive::<()>(libc::ENODEV).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(libc::ENODEV));
    }
    #[test]
    fn vanished_enumerated_device_is_skipped() {
        for errno in [0, libc::ENOENT, libc::ENODEV] {
            classify_enum_device_errno(errno).unwrap();
        }
    }
    #[test]
    fn enumerated_device_creation_failure_is_error() {
        for errno in [libc::EACCES, libc::ENOMEM] {
            let error = classify_enum_device_errno(errno).unwrap_err();
            assert_eq!(error.raw_os_error(), Some(errno));
        }
    }
}
