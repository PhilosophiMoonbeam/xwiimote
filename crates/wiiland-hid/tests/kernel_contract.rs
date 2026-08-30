use wiiland_hid::decode::{EventKind, EventType};
use wiiland_hid::model::*;

#[test]
fn public_interface_is_the_owning_device_and_kind_is_the_decoder_discriminator() {
    let constructor: fn(&std::path::Path) -> Result<wiiland_hid::Interface, wiiland_hid::Errno> =
        wiiland_hid::Interface::new;
    let kind = wiiland_hid::InterfaceKind::Core;
    assert_eq!(kind, wiiland_hid::decode::InterfaceKind::Core);
    let _ = constructor;
}

#[test]
fn event_button_and_slot_identifiers_are_complete_and_contiguous() {
    let events = [
        EVENT_CODE_KEY,
        EVENT_CODE_ACCEL,
        EVENT_CODE_IR,
        EVENT_CODE_BALANCE_BOARD,
        EVENT_CODE_MOTION_PLUS,
        EVENT_CODE_PRO_CONTROLLER_KEY,
        EVENT_CODE_PRO_CONTROLLER_MOVE,
        EVENT_CODE_WATCH,
        EVENT_CODE_CLASSIC_CONTROLLER_KEY,
        EVENT_CODE_CLASSIC_CONTROLLER_MOVE,
        EVENT_CODE_NUNCHUK_KEY,
        EVENT_CODE_NUNCHUK_MOVE,
        EVENT_CODE_DRUMS_KEY,
        EVENT_CODE_DRUMS_MOVE,
        EVENT_CODE_GUITAR_KEY,
        EVENT_CODE_GUITAR_MOVE,
        EVENT_CODE_GONE,
    ];
    assert_eq!(
        events.as_slice(),
        (0..EVENT_CODE_COUNT).collect::<Vec<_>>().as_slice()
    );
    assert_eq!(EVENT_CODE_COUNT, 17);

    let keys = [
        BUTTON_LEFT,
        BUTTON_RIGHT,
        BUTTON_UP,
        BUTTON_DOWN,
        BUTTON_A,
        BUTTON_B,
        BUTTON_PLUS,
        BUTTON_MINUS,
        BUTTON_HOME,
        BUTTON_ONE,
        BUTTON_TWO,
        BUTTON_X,
        BUTTON_Y,
        BUTTON_TL,
        BUTTON_TR,
        BUTTON_ZL,
        BUTTON_ZR,
        BUTTON_THUMBL,
        BUTTON_THUMBR,
        BUTTON_C,
        BUTTON_Z,
        BUTTON_STRUM_BAR_UP,
        BUTTON_STRUM_BAR_DOWN,
        BUTTON_FRET_FAR_UP,
        BUTTON_FRET_UP,
        BUTTON_FRET_MID,
        BUTTON_FRET_LOW,
        BUTTON_FRET_FAR_LOW,
    ];
    assert_eq!(
        keys.as_slice(),
        (0..BUTTON_COUNT).collect::<Vec<_>>().as_slice()
    );
    assert_eq!(BUTTON_COUNT, 28);
    assert_eq!(
        [
            DRUM_SLOT_PAD,
            DRUM_SLOT_CYMBAL_LEFT,
            DRUM_SLOT_CYMBAL_RIGHT,
            DRUM_SLOT_TOM_LEFT,
            DRUM_SLOT_TOM_RIGHT,
            DRUM_SLOT_TOM_FAR_RIGHT,
            DRUM_SLOT_BASS,
            DRUM_SLOT_HI_HAT,
        ],
        [0, 1, 2, 3, 4, 5, 6, 7]
    );
    assert_eq!(DRUM_SLOT_COUNT, 8);

    assert_eq!(InterfaceMask::ALL.bits(), 0x007f07);
    assert_eq!(InterfaceMask::WRITABLE.bits(), 0x010000);
    assert_eq!((EV_SYN, EV_KEY, EV_ABS), (0, 1, 3));
    assert_eq!((SYN_REPORT, SYN_DROPPED), (0, 3));
}

#[test]
fn evdev_codes_match_linux_input_event_codes_h() {
    assert_eq!(
        (KEY_LEFT, KEY_RIGHT, KEY_UP, KEY_DOWN),
        (105, 106, 103, 108)
    );
    assert_eq!((KEY_NEXT, KEY_PREVIOUS), (0x197, 0x19c));
    assert_eq!(
        (BTN_1, BTN_2, BTN_3, BTN_4, BTN_5),
        (0x101, 0x102, 0x103, 0x104, 0x105)
    );
    assert_eq!(
        (BTN_A, BTN_B, BTN_C, BTN_X, BTN_Y, BTN_Z),
        (0x130, 0x131, 0x132, 0x133, 0x134, 0x135)
    );
    assert_eq!(
        (BTN_TL, BTN_TR, BTN_TL2, BTN_TR2),
        (0x136, 0x137, 0x138, 0x139)
    );
    assert_eq!(
        (BTN_SELECT, BTN_START, BTN_MODE, BTN_THUMBL, BTN_THUMBR),
        (0x13a, 0x13b, 0x13c, 0x13d, 0x13e)
    );
    assert_eq!(
        (BTN_DPAD_UP, BTN_DPAD_DOWN, BTN_DPAD_LEFT, BTN_DPAD_RIGHT),
        (0x220, 0x221, 0x222, 0x223)
    );
    assert_eq!(
        (ABS_X, ABS_Y, ABS_RX, ABS_RY, ABS_RZ),
        (0x00, 0x01, 0x03, 0x04, 0x05)
    );
    assert_eq!(
        (
            ABS_HAT0X, ABS_HAT0Y, ABS_HAT1X, ABS_HAT1Y, ABS_HAT2X, ABS_HAT2Y, ABS_HAT3X, ABS_HAT3Y,
        ),
        (0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17)
    );
}

#[test]
fn interface_masks_preserve_kernel_interface_sets() {
    let masks = [
        InterfaceMask::CORE,
        InterfaceMask::ACCEL,
        InterfaceMask::IR,
        InterfaceMask::MOTION_PLUS,
        InterfaceMask::NUNCHUK,
        InterfaceMask::CLASSIC_CONTROLLER,
        InterfaceMask::BALANCE_BOARD,
        InterfaceMask::PRO_CONTROLLER,
        InterfaceMask::DRUMS,
        InterfaceMask::GUITAR,
    ];
    let mut all = InterfaceMask::empty();
    for mask in masks {
        assert!(!all.contains(mask));
        all = all.insert(mask);
        assert!(all.contains(mask));
    }
    assert_eq!(all, InterfaceMask::ALL);
    assert!(all.contains(InterfaceMask::ALL));
    assert_eq!((all | InterfaceMask::WRITABLE).bits(), 0x017f07);
    assert_eq!((all & InterfaceMask::ACCEL).bits(), 0x000002);
    assert_eq!(all.remove(InterfaceMask::IR).bits(), 0x007f03);
    assert_eq!(
        (!InterfaceMask::empty() & InterfaceMask::ALL),
        InterfaceMask::ALL
    );
}

#[test]
fn typed_events_expose_stable_trace_discriminants() {
    let kinds = [EventKind::Gone, EventKind::Unknown(0xdead_beef)];
    assert_eq!(kinds[0].raw_type(), EVENT_CODE_GONE);
    assert_eq!(kinds[1].raw_type(), 0xdead_beef);
    for raw in 0..EVENT_CODE_COUNT {
        assert_eq!(EventType::from_raw(raw).raw(), raw);
    }
}
