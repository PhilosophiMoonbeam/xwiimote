use crate::backend::{UdevContext, UdevDevice, UdevMonitor};
use crate::sys;
use std::ffi::{CStr, CString};
use std::path::PathBuf;

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
pub struct Monitor {
    udev: UdevContext,
    enumerate: *mut sys::udev_enumerate,
    entry: *mut sys::udev_list_entry,
    initial: Option<Box<Initial>>,
    monitor: Option<UdevMonitor>,
    enumerated: bool,
}
impl Monitor {
    pub fn new(poll: bool, direct: bool) -> Option<Self> {
        let udev = UdevContext::new()?;
        let mut mon = None;
        if poll {
            let channel = CString::new(if direct { "kernel" } else { "udev" }).unwrap();
            let p = unsafe { sys::udev_monitor_new_from_netlink(udev.0, channel.as_ptr()) };
            if p.is_null() {
                return None;
            };
            let m = UdevMonitor(p);
            let hid = CString::new("hid").unwrap();
            if unsafe {
                sys::udev_monitor_filter_add_match_subsystem_devtype(
                    p,
                    hid.as_ptr(),
                    std::ptr::null(),
                )
            } != 0
                || unsafe { sys::udev_monitor_enable_receiving(p) } != 0
            {
                return None;
            };
            mon = Some(m);
        }
        let en = unsafe { sys::udev_enumerate_new(udev.0) };
        if en.is_null() {
            return None;
        };
        let hs = CString::new("hid").unwrap();
        if unsafe { sys::udev_enumerate_add_match_subsystem(en, hs.as_ptr()) } != 0
            || unsafe { sys::udev_enumerate_scan_devices(en) } != 0
        {
            unsafe { sys::udev_enumerate_unref(en) };
            return None;
        };
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
        Some(Self {
            udev,
            enumerate: en,
            entry,
            initial,
            monitor: mon,
            enumerated: false,
        })
    }
    pub fn fd(&mut self, blocking: bool) -> Option<i32> {
        let m = self.monitor.as_ref()?;
        let fd = unsafe { sys::udev_monitor_get_fd(m.0) };
        if fd < 0 {
            return None;
        };
        let mut fl = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if fl < 0 {
            return None;
        };
        if blocking {
            fl &= !libc::O_NONBLOCK
        } else {
            fl |= libc::O_NONBLOCK
        };
        if unsafe { libc::fcntl(fd, libc::F_SETFL, fl) } < 0 {
            return None;
        };
        Some(fd)
    }
    fn free_enum(&mut self) {
        self.initial = None;
        if !self.enumerate.is_null() {
            unsafe { sys::udev_enumerate_unref(self.enumerate) };
            self.enumerate = std::ptr::null_mut();
            self.entry = std::ptr::null_mut();
        }
    }
    fn next_enum(&mut self) -> Option<UdevDevice> {
        while !self.entry.is_null() {
            let e = self.entry;
            self.entry = unsafe { sys::udev_list_entry_get_next(e) };
            let p = unsafe { sys::cstr(sys::udev_list_entry_get_name(e)) }?;
            if let Some(d) = UdevDevice::from_path(&self.udev, p) {
                return Some(d);
            }
        }
        self.enumerated = true;
        if self.monitor.is_none() {
            self.free_enum()
        }
        None
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
        };
        if d.driver().is_none_or(|x| x.to_bytes() != b"wiimote")
            || d.subsystem().is_none_or(|x| x.to_bytes() != b"hid")
        {
            return None;
        };
        Some(PathBuf::from(d.syspath()?.to_string_lossy().into_owned()))
    }
    pub fn poll(&mut self) -> Option<PathBuf> {
        if !self.enumerated {
            loop {
                let d = self.next_enum()?;
                if let Some(p) = self.valid_device(&d) {
                    return Some(p);
                }
            }
        } else {
            let mp = self.monitor.as_ref()?.0;
            loop {
                let p = unsafe { sys::udev_monitor_receive_device(mp) };
                if p.is_null() {
                    self.free_enum();
                    return None;
                };
                let d = UdevDevice(p);
                if self.deduplicate(&d) {
                    continue;
                }
                if let Some(path) = self.valid_device(&d) {
                    return Some(path);
                }
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
    use super::{Initial, deduplicate_initial};

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
}
