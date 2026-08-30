use libc::timeval;

use crate::Axis3;
use crate::decode::{Decoder, EventKind, EventType, InterfaceKind};
use crate::model::{
    ABS_HAT0X, ABS_HAT0Y, ABS_HAT1X, ABS_HAT1Y, ABS_HAT2X, ABS_HAT2Y, ABS_HAT3X, ABS_HAT3Y, ABS_RX,
    ABS_RY, ABS_RZ, ABS_X, ABS_Y, BTN_1, BTN_2, BTN_3, BTN_4, BTN_5, BTN_A, BTN_B, BTN_C,
    BTN_DPAD_DOWN, BTN_DPAD_LEFT, BTN_DPAD_RIGHT, BTN_DPAD_UP, BTN_EAST, BTN_MODE, BTN_NORTH,
    BTN_SELECT, BTN_SOUTH, BTN_START, BTN_THUMBL, BTN_THUMBR, BTN_TL, BTN_TL2, BTN_TR, BTN_TR2,
    BTN_WEST, BTN_X, BTN_Y, BTN_Z, Button, ButtonEvent, ButtonState, EV_ABS, EV_KEY, EV_SYN,
    KEY_DOWN, KEY_LEFT, KEY_NEXT, KEY_PREVIOUS, KEY_RIGHT, KEY_UP, SYN_DROPPED, SYN_REPORT,
    Timestamp,
};

fn time(sec: i64, usec: i64) -> timeval {
    timeval {
        tv_sec: sec,
        tv_usec: usec,
    }
}

fn assert_time_eq(actual: Timestamp, expected: Timestamp) {
    assert_eq!(actual.seconds, expected.seconds);
    assert_eq!(actual.microseconds, expected.microseconds);
}

fn input(at: timeval, event_type: u16, code: u16, value: i32) -> crate::model::InputEvent {
    crate::model::InputEvent {
        time: at,
        event_type,
        code,
        value,
    }
}

#[test]
fn every_event_type_is_distinct_and_unknown_values_are_preserved() {
    let event_types = [
        EventType::Key,
        EventType::Accel,
        EventType::Ir,
        EventType::BalanceBoard,
        EventType::MotionPlus,
        EventType::ProControllerKey,
        EventType::ProControllerMove,
        EventType::Watch,
        EventType::ClassicControllerKey,
        EventType::ClassicControllerMove,
        EventType::NunchukKey,
        EventType::NunchukMove,
        EventType::DrumsKey,
        EventType::DrumsMove,
        EventType::GuitarKey,
        EventType::GuitarMove,
        EventType::Gone,
    ];
    for (index, event_type) in event_types.iter().enumerate() {
        assert!(
            event_types[..index]
                .iter()
                .all(|previous| previous != event_type)
        );
    }
    for value in [17, 0xffff_fffe, u32::MAX] {
        let unknown = EventType::Unknown(value);
        assert!(matches!(unknown, EventType::Unknown(preserved) if preserved == value));
        assert_ne!(
            EventType::Unknown(value),
            EventType::Unknown(value.wrapping_add(1))
        );
    }
}

#[test]
fn every_interface_key_mapping_and_rejected_codes() {
    fn assert_mappings(
        interface: InterfaceKind,
        cases: &[(u16, Button)],
        event_kind: fn(ButtonEvent) -> EventKind,
    ) {
        let mut decoder = Decoder::new(interface);
        for &(code, button) in cases {
            let event = decoder
                .push(input(time(0, 0), EV_KEY, code, 1))
                .expect("mapped key event");
            assert_eq!(
                event.kind,
                event_kind(ButtonEvent {
                    button,
                    state: ButtonState::Pressed,
                })
            );
        }
    }

    assert_mappings(
        InterfaceKind::Core,
        &[
            (KEY_LEFT, Button::Left),
            (KEY_RIGHT, Button::Right),
            (KEY_UP, Button::Up),
            (KEY_DOWN, Button::Down),
            (KEY_NEXT, Button::Plus),
            (KEY_PREVIOUS, Button::Minus),
            (BTN_1, Button::One),
            (BTN_2, Button::Two),
            (BTN_A, Button::A),
            (BTN_B, Button::B),
            (BTN_MODE, Button::Home),
        ],
        EventKind::Key,
    );
    assert_mappings(
        InterfaceKind::Nunchuk,
        &[(BTN_C, Button::C), (BTN_Z, Button::Z)],
        EventKind::NunchukKey,
    );
    assert_mappings(
        InterfaceKind::Classic,
        &[
            (BTN_A, Button::A),
            (BTN_B, Button::B),
            (BTN_X, Button::X),
            (BTN_Y, Button::Y),
            (KEY_NEXT, Button::Plus),
            (KEY_PREVIOUS, Button::Minus),
            (BTN_MODE, Button::Home),
            (KEY_LEFT, Button::Left),
            (KEY_RIGHT, Button::Right),
            (KEY_UP, Button::Up),
            (KEY_DOWN, Button::Down),
            (BTN_TL, Button::ShoulderLeft),
            (BTN_TR, Button::ShoulderRight),
            (BTN_TL2, Button::TriggerLeft),
            (BTN_TR2, Button::TriggerRight),
        ],
        EventKind::ClassicControllerKey,
    );
    assert_mappings(
        InterfaceKind::Pro,
        &[
            (BTN_EAST, Button::A),
            (BTN_SOUTH, Button::B),
            (BTN_NORTH, Button::X),
            (BTN_WEST, Button::Y),
            (BTN_START, Button::Plus),
            (BTN_SELECT, Button::Minus),
            (BTN_MODE, Button::Home),
            (BTN_DPAD_LEFT, Button::Left),
            (BTN_DPAD_RIGHT, Button::Right),
            (BTN_DPAD_UP, Button::Up),
            (BTN_DPAD_DOWN, Button::Down),
            (BTN_TL, Button::ShoulderLeft),
            (BTN_TR, Button::ShoulderRight),
            (BTN_TL2, Button::TriggerLeft),
            (BTN_TR2, Button::TriggerRight),
            (BTN_THUMBL, Button::ThumbLeft),
            (BTN_THUMBR, Button::ThumbRight),
        ],
        EventKind::ProControllerKey,
    );
    assert_mappings(
        InterfaceKind::Drums,
        &[(BTN_START, Button::Plus), (BTN_SELECT, Button::Minus)],
        EventKind::DrumsKey,
    );
    assert_mappings(
        InterfaceKind::Guitar,
        &[
            (BTN_1, Button::FretFarUp),
            (BTN_2, Button::FretUp),
            (BTN_3, Button::FretMid),
            (BTN_4, Button::FretLow),
            (BTN_5, Button::FretFarLow),
            (BTN_DPAD_UP, Button::StrumBarUp),
            (BTN_DPAD_DOWN, Button::StrumBarDown),
            (BTN_START, Button::Plus),
            (BTN_SELECT, Button::Minus),
        ],
        EventKind::GuitarKey,
    );

    for interface in [
        InterfaceKind::Core,
        InterfaceKind::Nunchuk,
        InterfaceKind::Classic,
        InterfaceKind::Pro,
        InterfaceKind::Drums,
        InterfaceKind::Guitar,
    ] {
        assert!(
            Decoder::new(interface)
                .push(input(time(0, 0), EV_KEY, u16::MAX, 1))
                .is_none()
        );
    }
}

#[test]
fn all_absolute_interfaces_update_their_complete_cache() {
    type AbsoluteInterfaceCase<'a> = (InterfaceKind, &'a [(u16, i32)], EventKind);

    let cases: &[AbsoluteInterfaceCase<'_>] = &[
        (
            InterfaceKind::Accel,
            &[(ABS_RX, 1), (ABS_RY, 2), (ABS_RZ, 3)],
            EventKind::Accel(Axis3 { x: 1, y: 2, z: 3 }),
        ),
        (
            InterfaceKind::MotionPlus,
            &[(ABS_RX, 4), (ABS_RY, 5), (ABS_RZ, 6)],
            EventKind::MotionPlus(Axis3 { x: 4, y: 5, z: 6 }),
        ),
        (
            InterfaceKind::Ir,
            &[
                (ABS_HAT0X, 10),
                (ABS_HAT0Y, 11),
                (ABS_HAT1X, 20),
                (ABS_HAT1Y, 21),
                (ABS_HAT2X, 30),
                (ABS_HAT2Y, 31),
                (ABS_HAT3X, 40),
                (ABS_HAT3Y, 41),
            ],
            EventKind::Ir([
                Axis3 { x: 10, y: 11, z: 0 },
                Axis3 { x: 20, y: 21, z: 0 },
                Axis3 { x: 30, y: 31, z: 0 },
                Axis3 { x: 40, y: 41, z: 0 },
            ]),
        ),
        (
            InterfaceKind::Nunchuk,
            &[
                (ABS_HAT0X, 50),
                (ABS_HAT0Y, 51),
                (ABS_RX, 60),
                (ABS_RY, 61),
                (ABS_RZ, 62),
            ],
            EventKind::NunchukMove([
                Axis3 { x: 50, y: 51, z: 0 },
                Axis3 {
                    x: 60,
                    y: 61,
                    z: 62,
                },
            ]),
        ),
        (
            InterfaceKind::Classic,
            &[
                (ABS_HAT1X, 70),
                (ABS_HAT1Y, 71),
                (ABS_HAT2X, 80),
                (ABS_HAT2Y, 81),
                (ABS_HAT3X, 90),
                (ABS_HAT3Y, 91),
            ],
            EventKind::ClassicControllerMove([
                Axis3 { x: 70, y: 71, z: 0 },
                Axis3 { x: 80, y: 81, z: 0 },
                Axis3 { x: 91, y: 90, z: 0 },
            ]),
        ),
        (
            InterfaceKind::BalanceBoard,
            &[
                (ABS_HAT0X, 100),
                (ABS_HAT0Y, 101),
                (ABS_HAT1X, 102),
                (ABS_HAT1Y, 103),
            ],
            EventKind::BalanceBoard([
                Axis3 { x: 100, y: 0, z: 0 },
                Axis3 { x: 101, y: 0, z: 0 },
                Axis3 { x: 102, y: 0, z: 0 },
                Axis3 { x: 103, y: 0, z: 0 },
            ]),
        ),
        (
            InterfaceKind::Pro,
            &[(ABS_X, 110), (ABS_Y, 111), (ABS_RX, 112), (ABS_RY, 113)],
            EventKind::ProControllerMove([
                Axis3 {
                    x: 110,
                    y: 111,
                    z: 0,
                },
                Axis3 {
                    x: 112,
                    y: 113,
                    z: 0,
                },
            ]),
        ),
        (
            InterfaceKind::Drums,
            &[
                (ABS_X, 120),
                (ABS_Y, 121),
                (ABS_HAT2X, 122),
                (ABS_HAT2Y, 123),
                (ABS_HAT0X, 124),
                (ABS_HAT1X, 125),
                (ABS_HAT0Y, 126),
                (ABS_HAT3X, 127),
                (ABS_HAT3Y, 128),
            ],
            EventKind::DrumsMove([
                Axis3 {
                    x: 120,
                    y: 121,
                    z: 0,
                },
                Axis3 { x: 122, y: 0, z: 0 },
                Axis3 { x: 123, y: 0, z: 0 },
                Axis3 { x: 124, y: 0, z: 0 },
                Axis3 { x: 125, y: 0, z: 0 },
                Axis3 { x: 126, y: 0, z: 0 },
                Axis3 { x: 127, y: 0, z: 0 },
                Axis3 { x: 128, y: 0, z: 0 },
            ]),
        ),
        (
            InterfaceKind::Guitar,
            &[
                (ABS_X, 130),
                (ABS_Y, 131),
                (ABS_HAT1X, 132),
                (ABS_HAT0X, 133),
            ],
            EventKind::GuitarMove([
                Axis3 {
                    x: 130,
                    y: 131,
                    z: 0,
                },
                Axis3 { x: 132, y: 0, z: 0 },
                Axis3 { x: 133, y: 0, z: 0 },
            ]),
        ),
    ];
    for &(interface, axes, expected_kind) in cases {
        let mut decoder = Decoder::new(interface);
        for &(code, value) in axes {
            assert!(
                decoder
                    .push(input(time(0, 0), EV_ABS, code, value))
                    .is_none(),
                "{interface:?} unexpectedly reported while updating ABS {code}"
            );
        }
        assert!(
            decoder
                .push(input(time(0, 0), EV_ABS, u16::MAX, -1))
                .is_none()
        );
        let report = decoder
            .push(input(time(9, 8), EV_SYN, SYN_REPORT, 0))
            .expect("SYN_REPORT report");
        assert_time_eq(
            report.time,
            Timestamp {
                seconds: 9,
                microseconds: 8,
            },
        );
        assert_eq!(report.kind, expected_kind);
    }
    assert!(
        Decoder::new(InterfaceKind::Core)
            .push(input(time(0, 0), EV_SYN, SYN_REPORT, 0))
            .is_none()
    );
}

#[test]
fn absolute_cache_values_match_device_slot_order() {
    let mut d = Decoder::new(InterfaceKind::Drums);
    for (code, value) in [
        (ABS_X, -32),
        (ABS_Y, 31),
        (ABS_HAT2X, 1),
        (ABS_HAT2Y, 2),
        (ABS_HAT0X, 3),
        (ABS_HAT1X, 4),
        (ABS_HAT0Y, 5),
        (ABS_HAT3X, 6),
        (ABS_HAT3Y, 7),
    ] {
        assert!(d.push(input(time(0, 0), EV_ABS, code, value)).is_none());
    }
    let report = d
        .push(input(time(0, 0), EV_SYN, SYN_REPORT, 0))
        .expect("drums report");
    assert_eq!(
        report.kind,
        EventKind::DrumsMove([
            Axis3 {
                x: -32,
                y: 31,
                z: 0
            },
            Axis3 { x: 1, y: 0, z: 0 },
            Axis3 { x: 2, y: 0, z: 0 },
            Axis3 { x: 3, y: 0, z: 0 },
            Axis3 { x: 4, y: 0, z: 0 },
            Axis3 { x: 5, y: 0, z: 0 },
            Axis3 { x: 6, y: 0, z: 0 },
            Axis3 { x: 7, y: 0, z: 0 },
        ])
    );

    let mut g = Decoder::new(InterfaceKind::Guitar);
    for (code, value) in [(ABS_X, -32), (ABS_Y, 31), (ABS_HAT1X, -16), (ABS_HAT0X, 31)] {
        assert!(g.push(input(time(0, 0), EV_ABS, code, value)).is_none());
    }
    let report = g
        .push(input(time(0, 0), EV_SYN, SYN_REPORT, 0))
        .expect("guitar report");
    assert_eq!(
        report.kind,
        EventKind::GuitarMove([
            Axis3 {
                x: -32,
                y: 31,
                z: 0
            },
            Axis3 { x: -16, y: 0, z: 0 },
            Axis3 { x: 31, y: 0, z: 0 },
        ])
    );
}

#[test]
fn ir_starts_with_exact_invalid_sentinels_and_accepts_zero() {
    let initial = Decoder::new(InterfaceKind::Ir)
        .push(input(time(0, 0), EV_SYN, SYN_REPORT, 0))
        .expect("initial IR report");
    let EventKind::Ir(points) = initial.kind else {
        panic!("IR decoder did not produce an IR report");
    };
    fn ir_point_is_valid(point: &Axis3) -> bool {
        point.x != 1023 || point.y != 1023
    }

    for point in points {
        assert!(!ir_point_is_valid(&point));
    }
    assert!(!ir_point_is_valid(&Axis3 {
        x: 1023,
        y: 1023,
        z: 42,
    }));
    assert!(ir_point_is_valid(&Axis3 {
        x: 0,
        y: 1023,
        z: 0,
    }));
    assert!(ir_point_is_valid(&Axis3 {
        x: 1023,
        y: 0,
        z: 0,
    }));
}

#[test]
fn motion_plus_normalization_tracks_direction_and_saturates() {
    let mut decoder = Decoder::new(InterfaceKind::MotionPlus);
    assert_eq!(
        decoder
            .push(input(time(0, 0), EV_SYN, SYN_REPORT, 0))
            .unwrap()
            .kind,
        EventKind::MotionPlus(Axis3::default())
    );
    for (code, value) in [(ABS_RX, 5), (ABS_RY, -5)] {
        assert!(
            decoder
                .push(input(time(0, 0), EV_ABS, code, value))
                .is_none()
        );
    }
    assert_eq!(
        decoder
            .push(input(time(0, 0), EV_SYN, SYN_REPORT, 0))
            .unwrap()
            .kind,
        EventKind::MotionPlus(Axis3 { x: 5, y: -5, z: 0 })
    );
    decoder.set_mp_normalization(i32::MAX, i32::MIN, 0, 7);
    let (x, y, z, factor) = decoder.mp_normalization();
    assert_eq!(x, i32::MAX / 100);
    assert_eq!(y, i32::MIN / 100);
    assert_eq!((z, factor), (0, 7));

    let mut positive = Decoder::new(InterfaceKind::MotionPlus);
    positive.set_mp_normalization(0, 0, 0, i32::MAX);
    assert!(
        positive
            .push(input(time(0, 0), EV_ABS, ABS_RX, i32::MAX))
            .is_none()
    );
    assert_eq!(
        positive
            .push(input(time(0, 0), EV_SYN, SYN_REPORT, 0))
            .unwrap()
            .kind,
        EventKind::MotionPlus(Axis3 {
            x: i32::MAX,
            y: 0,
            z: 0,
        })
    );
    assert_eq!(positive.mp_normalization().0, i32::MAX / 100);
}

#[test]
fn syn_dropped_recovery_orders_keys_before_absolute_report_and_preserves_time() {
    let t = time(42, 7);
    let mut d = Decoder::new(InterfaceKind::Nunchuk);
    assert_eq!(
        d.push(input(t, EV_KEY, BTN_Z, 1)).unwrap().kind,
        EventKind::NunchukKey(ButtonEvent {
            button: Button::Z,
            state: ButtonState::Pressed,
        })
    );
    assert!(d.push(input(t, EV_SYN, SYN_DROPPED, 0)).is_none());
    assert!(d.push(input(t, EV_KEY, BTN_C, 1)).is_none());
    assert!(d.push(input(t, EV_SYN, SYN_REPORT, 0)).is_none());
    let mut key_bits = [0u64; BTN_C as usize / 64 + 1];
    key_bits[BTN_C as usize / 64] |= 1u64 << (BTN_C % 64);
    d.recover(
        &key_bits,
        &[
            (ABS_HAT0X, 10),
            (ABS_HAT0Y, 20),
            (ABS_RX, 30),
            (ABS_RY, 40),
            (ABS_RZ, 50),
        ],
        t,
    );
    let first = d.push_recovered().unwrap();
    assert_time_eq(
        first.time,
        Timestamp {
            seconds: 42,
            microseconds: 7,
        },
    );
    assert_eq!(
        first.kind,
        EventKind::NunchukKey(ButtonEvent {
            button: Button::C,
            state: ButtonState::Pressed,
        })
    );
    let second = d.push_recovered().unwrap();
    assert_eq!(
        second.kind,
        EventKind::NunchukKey(ButtonEvent {
            button: Button::Z,
            state: ButtonState::Released,
        })
    );
    let report = d.push_recovered().unwrap();
    assert_time_eq(
        report.time,
        Timestamp {
            seconds: 42,
            microseconds: 7,
        },
    );
    assert_eq!(
        report.kind,
        EventKind::NunchukMove([
            Axis3 { x: 10, y: 20, z: 0 },
            Axis3 {
                x: 30,
                y: 40,
                z: 50,
            },
        ])
    );
    assert!(d.push_recovered().is_none());
}

#[test]
fn syn_dropped_absolute_seed_is_reported_once_and_unchanged_keys_are_silent() {
    let t = time(1, 2);
    let mut d = Decoder::new(InterfaceKind::Nunchuk);
    assert_eq!(
        d.push(input(t, EV_KEY, BTN_Z, 1)).unwrap().kind,
        EventKind::NunchukKey(ButtonEvent {
            button: Button::Z,
            state: ButtonState::Pressed,
        })
    );
    assert!(d.push(input(t, EV_SYN, SYN_DROPPED, 0)).is_none());
    let mut key_bits = [0u64; BTN_Z as usize / 64 + 1];
    key_bits[BTN_Z as usize / 64] |= 1u64 << (BTN_Z % 64);
    d.recover(
        &key_bits,
        &[
            (ABS_HAT0X, 100),
            (ABS_HAT0Y, 200),
            (ABS_RX, 300),
            (ABS_RY, 400),
            (ABS_RZ, 500),
        ],
        t,
    );
    let report = d.push_recovered().unwrap();
    assert_eq!(
        report.kind,
        EventKind::NunchukMove([
            Axis3 {
                x: 100,
                y: 200,
                z: 0,
            },
            Axis3 {
                x: 300,
                y: 400,
                z: 500,
            },
        ])
    );
    assert!(d.push_recovered().is_none());
}

#[test]
fn invalid_key_values_and_unknown_events_are_ignored() {
    let t = time(0, 0);
    let mut d = Decoder::new(InterfaceKind::Core);
    for value in [-1, 3, i32::MAX] {
        assert!(d.push(input(t, EV_KEY, BTN_A, value)).is_none());
    }
    assert!(d.push(input(t, EV_ABS, ABS_X, 1)).is_none());
    assert!(d.push(input(t, 99, 99, 99)).is_none());
}
