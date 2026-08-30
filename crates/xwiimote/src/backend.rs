use crate::sys;
use std::ffi::{CStr, CString};
use std::os::fd::RawFd;
use std::path::{Path, PathBuf};

pub const IF_NAMES: [&CStr; 10] = [
    c"Nintendo Wii Remote",
    c"Nintendo Wii Remote Accelerometer",
    c"Nintendo Wii Remote IR",
    c"Nintendo Wii Remote Motion Plus",
    c"Nintendo Wii Remote Nunchuk",
    c"Nintendo Wii Remote Classic Controller",
    c"Nintendo Wii Remote Balance Board",
    c"Nintendo Wii Remote Pro Controller",
    c"Nintendo Wii Remote Drums",
    c"Nintendo Wii Remote Guitar",
];
pub const IF_BITS: [u32; 10] = [
    0x1, 0x2, 0x4, 0x100, 0x200, 0x400, 0x800, 0x1000, 0x2000, 0x4000,
];
pub const ALL_INTERFACES: u32 = 0x7f07;
pub const WRITABLE: u32 = 0x10000;
pub struct UdevContext(pub(crate) *mut sys::udev);
impl UdevContext {
    pub fn new() -> Option<Self> {
        unsafe {
            let p = sys::udev_new();
            (!p.is_null()).then_some(Self(p))
        }
    }
}
impl Drop for UdevContext {
    fn drop(&mut self) {
        unsafe {
            sys::udev_unref(self.0);
        }
    }
}

pub struct UdevDevice(pub(crate) *mut sys::udev_device);
impl UdevDevice {
    pub fn from_path(u: &UdevContext, path: &CStr) -> Option<Self> {
        unsafe {
            let p = sys::udev_device_new_from_syspath(u.0, path.as_ptr());
            (!p.is_null()).then_some(Self(p))
        }
    }
    pub fn syspath(&self) -> Option<&CStr> {
        unsafe { sys::cstr(sys::udev_device_get_syspath(self.0)) }
    }
    pub fn subsystem(&self) -> Option<&CStr> {
        unsafe { sys::cstr(sys::udev_device_get_subsystem(self.0)) }
    }
    pub fn driver(&self) -> Option<&CStr> {
        unsafe { sys::cstr(sys::udev_device_get_driver(self.0)) }
    }
    pub fn sysname(&self) -> Option<&CStr> {
        unsafe { sys::cstr(sys::udev_device_get_sysname(self.0)) }
    }
    pub fn devnode(&self) -> Option<&CStr> {
        unsafe { sys::cstr(sys::udev_device_get_devnode(self.0)) }
    }
    pub fn action(&self) -> Option<&CStr> {
        unsafe { sys::cstr(sys::udev_device_get_action(self.0)) }
    }
    pub fn attr(&self, name: &CStr) -> Option<&CStr> {
        unsafe { sys::cstr(sys::udev_device_get_sysattr_value(self.0, name.as_ptr())) }
    }
}
impl Drop for UdevDevice {
    fn drop(&mut self) {
        unsafe {
            sys::udev_device_unref(self.0);
        }
    }
}

pub struct UdevMonitor(pub(crate) *mut sys::udev_monitor);
impl Drop for UdevMonitor {
    fn drop(&mut self) {
        unsafe {
            sys::udev_monitor_unref(self.0);
        }
    }
}

pub struct Nodes {
    pub nodes: [Option<PathBuf>; 10],
    pub available: u32,
    pub leds: [Option<PathBuf>; 4],
    pub battery: Option<PathBuf>,
}
impl Default for Nodes {
    fn default() -> Self {
        Self {
            nodes: std::array::from_fn(|_| None),
            available: 0,
            leds: std::array::from_fn(|_| None),
            battery: None,
        }
    }
}

pub fn interface_by_name(name: &CStr) -> Option<usize> {
    IF_NAMES
        .iter()
        .position(|x| name.to_bytes() == x.to_bytes())
}

pub fn refresh_nodes(ctx: &UdevContext, dev: &UdevDevice, old: &mut Nodes) -> Result<(), i32> {
    let e = unsafe { sys::udev_enumerate_new(ctx.0) };
    if e.is_null() {
        return Err(-libc::ENOMEM);
    }
    let _enum_guard = EnumGuard(e);
    let input = CString::new("input").unwrap();
    let leds = CString::new("leds").unwrap();
    let power = CString::new("power_supply").unwrap();
    let rc = unsafe {
        sys::udev_enumerate_add_match_subsystem(e, input.as_ptr())
            + sys::udev_enumerate_add_match_subsystem(e, leds.as_ptr())
            + sys::udev_enumerate_add_match_subsystem(e, power.as_ptr())
            + sys::udev_enumerate_add_match_parent(e, dev.0)
    };
    if rc != 0 {
        return Err(-libc::ENOMEM);
    }
    let rc = unsafe { sys::udev_enumerate_scan_devices(e) };
    if rc != 0 {
        return Err(rc);
    }
    old.available = 0;
    let mut prev: Option<usize> = None;
    let mut ent = unsafe { sys::udev_enumerate_get_list_entry(e) };
    while !ent.is_null() {
        let path = unsafe { sys::cstr(sys::udev_list_entry_get_name(ent)) };
        if let Some(path) = path
            && let Some(child) = UdevDevice::from_path(ctx, path)
        {
            if child.subsystem().is_some_and(|s| s.to_bytes() == b"input") {
                if let Some(sysname) = child.sysname() {
                    if sysname.to_bytes().starts_with(b"input") {
                        prev = child
                            .attr(CString::new("name").unwrap().as_c_str())
                            .and_then(interface_by_name);
                    } else if sysname.to_bytes().starts_with(b"event") {
                        if let (Some(i), Some(node)) = (prev.take(), child.devnode()) {
                            old.nodes[i] = Some(PathBuf::from(node.to_string_lossy().into_owned()));
                            old.available |= IF_BITS[i];
                        }
                    } else {
                        prev = None;
                    }
                }
            } else if child.subsystem().is_some_and(|s| s.to_bytes() == b"leds") {
                if let Some(n) = path
                    .to_str()
                    .ok()
                    .and_then(|s| s.as_bytes().last())
                    .copied()
                    .and_then(|x| (x as char).to_digit(10))
                    && n < 4
                {
                    old.leds[n as usize] =
                        Some(PathBuf::from(path.to_string_lossy().into_owned()).join("brightness"));
                }
            } else if child
                .subsystem()
                .is_some_and(|s| s.to_bytes() == b"power_supply")
            {
                old.battery =
                    Some(PathBuf::from(path.to_string_lossy().into_owned()).join("capacity"));
            }
        }
        ent = unsafe { sys::udev_list_entry_get_next(ent) };
    }
    Ok(())
}
struct EnumGuard(*mut sys::udev_enumerate);
impl Drop for EnumGuard {
    fn drop(&mut self) {
        unsafe {
            sys::udev_enumerate_unref(self.0);
        }
    }
}

pub fn open_node(path: &Path, write: bool) -> Result<RawFd, i32> {
    let c = CString::new(path.as_os_str().as_encoded_bytes()).map_err(|_| -libc::EINVAL)?;
    let flags =
        if write { libc::O_RDWR } else { libc::O_RDONLY } | libc::O_NONBLOCK | libc::O_CLOEXEC;
    let fd = unsafe { libc::open(c.as_ptr(), flags) };
    if fd < 0 { Err(-sys::errno()) } else { Ok(fd) }
}

pub fn read_attr(path: &Path) -> Result<Vec<u8>, i32> {
    let c = CString::new(path.as_os_str().as_encoded_bytes()).map_err(|_| -libc::EINVAL)?;
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(-sys::errno());
    }
    let mut out = Vec::new();
    let mut buf = [0u8; 256];
    loop {
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
        if n > 0 {
            out.extend_from_slice(&buf[..n as usize]);
            if n < buf.len() as isize {
                break;
            }
        } else if n == 0 {
            break;
        } else if sys::errno() == libc::EINTR {
            continue;
        } else {
            let e = -sys::errno();
            unsafe {
                libc::close(fd);
            }
            return Err(e);
        }
    }
    unsafe {
        libc::close(fd);
    }
    while out.last().is_some_and(|b| b.is_ascii_whitespace()) {
        out.pop();
    }
    Ok(out)
}

pub fn write_attr(path: &Path, value: &[u8]) -> Result<(), i32> {
    let c = CString::new(path.as_os_str().as_encoded_bytes()).map_err(|_| -libc::EINVAL)?;
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(-sys::errno());
    }
    let mut off = 0;
    while off < value.len() {
        let n = unsafe { libc::write(fd, value[off..].as_ptr().cast(), value.len() - off) };
        if n > 0 {
            off += n as usize;
        } else if n < 0 && sys::errno() == libc::EINTR {
            continue;
        } else {
            let e = -sys::errno();
            unsafe {
                libc::close(fd);
            }
            return Err(e);
        }
    }
    unsafe {
        libc::close(fd);
    }
    Ok(())
}
