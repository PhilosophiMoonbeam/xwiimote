use std::mem::{align_of, offset_of, size_of};
use xwiimote::abi::*;
use xwiimote::decode::{EventKind, EventType};

#[test]
fn public_interface_is_the_owning_device_and_kind_is_the_decoder_discriminator() {
    let constructor: fn(&std::path::Path) -> Result<xwiimote::Interface, xwiimote::Errno> =
        xwiimote::Interface::new;
    let kind = xwiimote::InterfaceKind::Core;
    assert_eq!(kind, xwiimote::decode::InterfaceKind::Core);
    let _ = constructor;
}
#[test]
fn c_event_layout_matches_the_published_header() {
    assert_eq!(size_of::<CEventKey>(), 8);
    assert_eq!(align_of::<CEventKey>(), 4);
    assert_eq!(offset_of!(CEventKey, code), 0);
    assert_eq!(offset_of!(CEventKey, state), 4);

    assert_eq!(size_of::<CEventAbs>(), 12);
    assert_eq!(align_of::<CEventAbs>(), 4);
    assert_eq!(offset_of!(CEventAbs, x), 0);
    assert_eq!(offset_of!(CEventAbs, y), 4);
    assert_eq!(offset_of!(CEventAbs, z), 8);

    assert_eq!(size_of::<EventUnion>(), 128);
    assert_eq!(align_of::<EventUnion>(), 4);
    let payload_offset = size_of::<libc::timeval>() + size_of::<u32>();
    let payload_end = payload_offset + size_of::<EventUnion>();
    let struct_alignment = align_of::<CEvent>();
    let expected_size = payload_end.div_ceil(struct_alignment) * struct_alignment;
    assert_eq!(size_of::<CEvent>(), expected_size);
    assert_eq!(align_of::<CEvent>(), align_of::<libc::timeval>());
    assert_eq!(offset_of!(CEvent, time), 0);
    assert_eq!(offset_of!(CEvent, event_type), size_of::<libc::timeval>());
    assert_eq!(offset_of!(CEvent, v), payload_offset);
    assert_eq!(size_of::<InputEvent>(), size_of::<libc::timeval>() + 8);
    assert_eq!(
        offset_of!(InputEvent, event_type),
        size_of::<libc::timeval>()
    );
    assert_eq!(offset_of!(InputEvent, code), size_of::<libc::timeval>() + 2);
    assert_eq!(
        offset_of!(InputEvent, value),
        size_of::<libc::timeval>() + 4
    );
}

#[test]
fn published_event_and_key_constants_are_complete_and_contiguous() {
    let events = [
        XWII_EVENT_KEY,
        XWII_EVENT_ACCEL,
        XWII_EVENT_IR,
        XWII_EVENT_BALANCE_BOARD,
        XWII_EVENT_MOTION_PLUS,
        XWII_EVENT_PRO_CONTROLLER_KEY,
        XWII_EVENT_PRO_CONTROLLER_MOVE,
        XWII_EVENT_WATCH,
        XWII_EVENT_CLASSIC_CONTROLLER_KEY,
        XWII_EVENT_CLASSIC_CONTROLLER_MOVE,
        XWII_EVENT_NUNCHUK_KEY,
        XWII_EVENT_NUNCHUK_MOVE,
        XWII_EVENT_DRUMS_KEY,
        XWII_EVENT_DRUMS_MOVE,
        XWII_EVENT_GUITAR_KEY,
        XWII_EVENT_GUITAR_MOVE,
        XWII_EVENT_GONE,
    ];
    assert_eq!(
        events.as_slice(),
        (0..XWII_EVENT_NUM).collect::<Vec<_>>().as_slice()
    );
    assert_eq!(XWII_EVENT_NUM, 17);

    let keys = [
        XWII_KEY_LEFT,
        XWII_KEY_RIGHT,
        XWII_KEY_UP,
        XWII_KEY_DOWN,
        XWII_KEY_A,
        XWII_KEY_B,
        XWII_KEY_PLUS,
        XWII_KEY_MINUS,
        XWII_KEY_HOME,
        XWII_KEY_ONE,
        XWII_KEY_TWO,
        XWII_KEY_X,
        XWII_KEY_Y,
        XWII_KEY_TL,
        XWII_KEY_TR,
        XWII_KEY_ZL,
        XWII_KEY_ZR,
        XWII_KEY_THUMBL,
        XWII_KEY_THUMBR,
        XWII_KEY_C,
        XWII_KEY_Z,
        XWII_KEY_STRUM_BAR_UP,
        XWII_KEY_STRUM_BAR_DOWN,
        XWII_KEY_FRET_FAR_UP,
        XWII_KEY_FRET_UP,
        XWII_KEY_FRET_MID,
        XWII_KEY_FRET_LOW,
        XWII_KEY_FRET_FAR_LOW,
    ];
    assert_eq!(
        keys.as_slice(),
        (0..XWII_KEY_NUM).collect::<Vec<_>>().as_slice()
    );
    assert_eq!(XWII_KEY_NUM, 28);
    assert_eq!(
        [
            XWII_DRUMS_ABS_PAD,
            XWII_DRUMS_ABS_CYMBAL_LEFT,
            XWII_DRUMS_ABS_CYMBAL_RIGHT,
            XWII_DRUMS_ABS_TOM_LEFT,
            XWII_DRUMS_ABS_TOM_RIGHT,
            XWII_DRUMS_ABS_TOM_FAR_RIGHT,
            XWII_DRUMS_ABS_BASS,
            XWII_DRUMS_ABS_HI_HAT,
        ],
        [0, 1, 2, 3, 4, 5, 6, 7]
    );
    assert_eq!(XWII_DRUMS_ABS_NUM, 8);
    assert_eq!(XWII_ABS_NUM, 8);

    assert_eq!(XWII_IFACE_ALL, 0x007f07);
    assert_eq!(XWII_IFACE_WRITABLE, 0x010000);
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
fn interface_masks_and_led_helpers_preserve_raw_values() {
    let bits = [
        (InterfaceMask::CORE, XWII_IFACE_CORE),
        (InterfaceMask::ACCEL, XWII_IFACE_ACCEL),
        (InterfaceMask::IR, XWII_IFACE_IR),
        (InterfaceMask::MOTION_PLUS, XWII_IFACE_MOTION_PLUS),
        (InterfaceMask::NUNCHUK, XWII_IFACE_NUNCHUK),
        (
            InterfaceMask::CLASSIC_CONTROLLER,
            XWII_IFACE_CLASSIC_CONTROLLER,
        ),
        (InterfaceMask::BALANCE_BOARD, XWII_IFACE_BALANCE_BOARD),
        (InterfaceMask::PRO_CONTROLLER, XWII_IFACE_PRO_CONTROLLER),
        (InterfaceMask::DRUMS, XWII_IFACE_DRUMS),
        (InterfaceMask::GUITAR, XWII_IFACE_GUITAR),
    ];
    let mut all = InterfaceMask::empty();
    for (mask, raw) in bits {
        assert_eq!(mask.bits(), raw);
        assert!(!all.contains(mask));
        all = all.insert(mask);
        assert!(all.contains(mask));
    }
    assert_eq!(all, InterfaceMask::ALL);
    assert!(all.contains(InterfaceMask::ALL));
    assert_eq!(
        (all | InterfaceMask::WRITABLE).bits(),
        XWII_IFACE_ALL | XWII_IFACE_WRITABLE
    );
    assert_eq!((all & InterfaceMask::ACCEL).bits(), XWII_IFACE_ACCEL);
    assert_eq!(
        all.remove(InterfaceMask::IR).bits(),
        XWII_IFACE_ALL & !XWII_IFACE_IR
    );
    assert_eq!(
        (!InterfaceMask::empty() & InterfaceMask::ALL),
        InterfaceMask::ALL
    );
    assert_eq!(xwii_led(1), XWII_LED1);
    assert_eq!(xwii_led(4), XWII_LED4);
}

#[test]
fn typed_events_expose_the_same_discriminants_as_c_events() {
    let kinds = [EventKind::Gone, EventKind::Unknown(0xdead_beef)];
    assert_eq!(kinds[0].raw_type(), XWII_EVENT_GONE);
    assert_eq!(kinds[1].raw_type(), 0xdead_beef);
    for raw in 0..XWII_EVENT_NUM {
        assert_eq!(EventType::from_raw(raw).raw(), raw);
    }
}
