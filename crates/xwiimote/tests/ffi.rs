use std::ffi::{CStr, CString};
use std::mem::size_of;
use std::ptr;

use xwiimote::abi::*;
use xwiimote::ffi::*;

#[test]
fn interface_names_cover_all_public_flags_and_reject_combinations() {
    let names = [
        (XWII_IFACE_CORE, XWII_NAME_CORE),
        (XWII_IFACE_ACCEL, XWII_NAME_ACCEL),
        (XWII_IFACE_IR, XWII_NAME_IR),
        (XWII_IFACE_MOTION_PLUS, XWII_NAME_MOTION_PLUS),
        (XWII_IFACE_NUNCHUK, XWII_NAME_NUNCHUK),
        (XWII_IFACE_CLASSIC_CONTROLLER, XWII_NAME_CLASSIC_CONTROLLER),
        (XWII_IFACE_BALANCE_BOARD, XWII_NAME_BALANCE_BOARD),
        (XWII_IFACE_PRO_CONTROLLER, XWII_NAME_PRO_CONTROLLER),
        (XWII_IFACE_DRUMS, XWII_NAME_DRUMS),
        (XWII_IFACE_GUITAR, XWII_NAME_GUITAR),
    ];
    for (flag, expected) in names {
        let actual = unsafe { xwii_get_iface_name(flag) };
        assert!(!actual.is_null());
        assert_eq!(
            unsafe { CStr::from_ptr(actual) }.to_str().unwrap(),
            expected
        );
    }
    for invalid in [
        0,
        XWII_IFACE_CORE | XWII_IFACE_ACCEL,
        XWII_IFACE_WRITABLE,
        u32::MAX,
    ] {
        assert!(unsafe { xwii_get_iface_name(invalid) }.is_null());
    }
}

#[test]
fn null_handles_return_documented_errors_without_unwinding() {
    let null_iface: *mut xwiimote::Interface = ptr::null_mut();
    let null_monitor: *mut xwiimote::monitor::Monitor = ptr::null_mut();
    let mut event = CEvent::default();
    let mut capacity = 0_u8;
    let mut state = false;
    let mut text: *mut libc::c_char = ptr::null_mut();
    let mut x = 11_i32;
    let mut y = 22_i32;
    let mut z = 33_i32;
    let mut factor = 44_i32;

    unsafe {
        assert_eq!(xwii_iface_new(ptr::null_mut(), ptr::null()), -libc::EINVAL);
        let mut out_iface = null_iface;
        assert_eq!(xwii_iface_new(&mut out_iface, ptr::null()), -libc::EINVAL);
        assert_eq!(xwii_iface_get_fd(null_iface), -1);
        assert_eq!(xwii_iface_watch(null_iface, true), -libc::EINVAL);
        assert_eq!(xwii_iface_open(null_iface, XWII_IFACE_CORE), -libc::EINVAL);
        xwii_iface_close(null_iface, XWII_IFACE_ALL);
        assert_eq!(xwii_iface_opened(null_iface), 0);
        assert_eq!(xwii_iface_available(null_iface), 0);
        assert_eq!(
            xwii_iface_dispatch(null_iface, &mut event, size_of::<CEvent>()),
            -libc::EFAULT
        );
        assert_eq!(
            xwii_iface_dispatch(null_iface, ptr::null_mut(), 0),
            -libc::EFAULT
        );
        assert_eq!(xwii_iface_poll(null_iface, &mut event), -libc::EFAULT);
        assert_eq!(xwii_iface_rumble(null_iface, true), -libc::EINVAL);
        assert_eq!(
            xwii_iface_get_led(null_iface, XWII_LED1, &mut state),
            -libc::EINVAL
        );
        assert_eq!(
            xwii_iface_get_led(null_iface, XWII_LED1, ptr::null_mut()),
            -libc::EINVAL
        );
        assert_eq!(
            xwii_iface_set_led(null_iface, XWII_LED1, true),
            -libc::EINVAL
        );
        assert_eq!(
            xwii_iface_get_battery(null_iface, &mut capacity),
            -libc::EINVAL
        );
        assert_eq!(
            xwii_iface_get_battery(null_iface, ptr::null_mut()),
            -libc::EINVAL
        );
        assert_eq!(xwii_iface_get_devtype(null_iface, &mut text), -libc::EINVAL);
        assert_eq!(
            xwii_iface_get_devtype(null_iface, ptr::null_mut()),
            -libc::EINVAL
        );
        assert_eq!(
            xwii_iface_get_extension(null_iface, &mut text),
            -libc::EINVAL
        );
        assert_eq!(
            xwii_iface_get_extension(null_iface, ptr::null_mut()),
            -libc::EINVAL
        );
        xwii_iface_set_mp_normalization(null_iface, 1, 2, 3, 4);
        xwii_iface_get_mp_normalization(null_iface, &mut x, &mut y, &mut z, &mut factor);
        assert_eq!((x, y, z, factor), (0, 0, 0, 0));
        xwii_iface_get_mp_normalization(
            null_iface,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        );

        xwii_monitor_ref(null_monitor);
        xwii_monitor_unref(null_monitor);
        assert_eq!(xwii_monitor_get_fd(null_monitor, false), -1);
        assert!(xwii_monitor_poll(null_monitor).is_null());
    }
}

#[test]
fn null_interface_zeros_each_requested_motion_plus_output_independently() {
    let null_iface = ptr::null_mut();
    unsafe {
        let mut x = 1;
        xwii_iface_get_mp_normalization(
            null_iface,
            &mut x,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        );
        assert_eq!(x, 0);

        let mut y = 2;
        xwii_iface_get_mp_normalization(
            null_iface,
            ptr::null_mut(),
            &mut y,
            ptr::null_mut(),
            ptr::null_mut(),
        );
        assert_eq!(y, 0);

        let mut z = 3;
        xwii_iface_get_mp_normalization(
            null_iface,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut z,
            ptr::null_mut(),
        );
        assert_eq!(z, 0);

        let mut factor = 4;
        xwii_iface_get_mp_normalization(
            null_iface,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut factor,
        );
        assert_eq!(factor, 0);
    }
}

#[test]
fn invalid_paths_use_negative_errno_and_do_not_allocate_handles() {
    let path = CString::new("/definitely-not-a-wiimote-device").unwrap();
    let mut iface = ptr::null_mut();
    let result = unsafe { xwii_iface_new(&mut iface, path.as_ptr()) };
    assert!(result < 0);
    assert!(iface.is_null());

    let embedded = CString::new("/sys\0not-a-path");
    assert!(embedded.is_err());
}

#[test]
fn monitor_reference_and_c_allocator_ownership_are_safe() {
    // Enumeration-only monitors do not require a real Wii device.  The test
    // accepts a missing libudev environment, but exercises every ownership
    // transition whenever the current API can construct the monitor.
    let monitor = unsafe { xwii_monitor_new(false, false) };
    let Some(monitor) = (!monitor.is_null()).then_some(monitor) else {
        return;
    };
    unsafe {
        xwii_monitor_ref(monitor);
        assert!(xwii_monitor_get_fd(monitor, false) == -1);
        let path = xwii_monitor_poll(monitor);
        if !path.is_null() {
            assert!(!CStr::from_ptr(path).to_bytes().is_empty());
            libc::free(path.cast());
        }
        xwii_monitor_unref(monitor);
        xwii_monitor_unref(monitor);
    }
}

#[test]
fn dispatch_size_contract_has_safe_zero_and_partial_destinations() {
    // A null handle is rejected before output validation for every size, and
    // no partial destination bytes are touched.
    let mut bytes = [0xa5_u8; size_of::<CEvent>()];
    for size in [0, 1, size_of::<CEvent>() / 2, size_of::<CEvent>()] {
        let result = unsafe {
            xwii_iface_dispatch(ptr::null_mut(), bytes.as_mut_ptr().cast::<CEvent>(), size)
        };
        assert_eq!(result, -libc::EFAULT);
        assert!(bytes.iter().all(|&byte| byte == 0xa5));
    }
}
