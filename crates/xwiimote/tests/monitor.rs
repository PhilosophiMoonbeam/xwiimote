use xwiimote::monitor::Monitor;

#[test]
fn enumeration_has_a_single_boundary_and_stays_exhausted() {
    let Some(mut monitor) = Monitor::new(false, false) else {
        return;
    };
    assert_eq!(monitor.fd(false), None);

    // Enumeration is finite even when the host happens to expose several HID
    // devices, and each device is observed at most once in the initial snapshot.
    let mut initial = Vec::new();
    while let Some(path) = monitor.poll() {
        assert!(path.is_absolute());
        initial.push(path);
        assert!(
            initial.len() < 4096,
            "udev enumeration did not reach its boundary"
        );
    }
    assert!(monitor.poll().is_none());
    assert!(monitor.poll().is_none());
    assert!(initial.windows(2).all(|pair| pair[0] != pair[1]));
}

#[test]
fn every_monitor_constructor_combination_is_safe_without_hardware() {
    for (poll, direct) in [(false, false), (false, true), (true, false), (true, true)] {
        let Some(mut monitor) = Monitor::new(poll, direct) else {
            continue;
        };
        if poll {
            // fd() switches the netlink endpoint to non-blocking mode, so a
            // no-event fixture can be probed without waiting for hardware.
            let _ = monitor.fd(false);
        } else {
            assert_eq!(monitor.fd(false), None);
        }
        drop(monitor);
    }
}
