#![cfg(target_os = "linux")]

use wiilandd::uinput::{UINPUT_PATH, VirtualDevice, VirtualKind};

#[test]
#[ignore = "requires explicit access to the host /dev/uinput device"]
fn real_uinput_creates_emits_and_destroys_both_device_kinds() {
    assert_eq!(
        std::env::var("WIILAND_REAL_UINPUT").as_deref(),
        Ok("1"),
        "refusing real uinput test without WIILAND_REAL_UINPUT=1"
    );

    {
        let kind = VirtualKind::Controller;
        let capabilities = kind.capabilities();
        let mut device = VirtualDevice::new(UINPUT_PATH, kind).unwrap_or_else(|error| {
            panic!("failed to create controller through /dev/uinput: {error}")
        });

        device
            .emit_key(capabilities.keys[0], 1)
            .expect("failed to emit controller key press");
        device
            .emit_abs(capabilities.axes[0], 0)
            .expect("failed to emit controller axis");
        device.syn().expect("failed to sync controller input");
        device
            .emit_key(capabilities.keys[0], 0)
            .expect("failed to emit controller key release");
    }

    {
        let kind = VirtualKind::Desktop;
        let capabilities = kind.capabilities();
        let mut device = VirtualDevice::new(UINPUT_PATH, kind).unwrap_or_else(|error| {
            panic!("failed to create desktop through /dev/uinput: {error}")
        });

        device
            .emit_key(capabilities.keys[0], 1)
            .expect("failed to emit desktop key press");
        device
            .emit_rel(capabilities.rels[0], 1)
            .expect("failed to emit desktop relative motion");
        device.syn().expect("failed to sync desktop input");
        device
            .emit_key(capabilities.keys[0], 0)
            .expect("failed to emit desktop key release");
    }
}
