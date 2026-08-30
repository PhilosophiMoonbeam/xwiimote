use libc::timeval;
use xwiimote::abi::*;
use xwiimote::decode::{
    Abs, Decoder, EventKind, EventType, InterfaceKind, Key, MotionPlusNormalizer, map_classic_key,
    map_core_key, map_drums_key, map_guitar_key, map_nunchuk_key, map_pro_key,
};

fn time(sec: i64, usec: i64) -> timeval {
    timeval {
        tv_sec: sec,
        tv_usec: usec,
    }
}

fn assert_time_eq(actual: timeval, expected: timeval) {
    assert_eq!(actual.tv_sec, expected.tv_sec);
    assert_eq!(actual.tv_usec, expected.tv_usec);
}

fn input(at: timeval, event_type: u16, code: u16, value: i32) -> InputEvent {
    InputEvent {
        time: at,
        event_type,
        code,
        value,
    }
}

#[test]
fn every_event_discriminant_round_trips_and_unknown_values_are_preserved() {
    for raw in 0..XWII_EVENT_NUM {
        assert_eq!(EventType::from_raw(raw).raw(), raw);
    }
    for raw in [17, 0xffff_fffe, u32::MAX] {
        assert_eq!(EventType::from_raw(raw), EventType::Unknown(raw));
        assert_eq!(EventType::Unknown(raw).raw(), raw);
    }
}

#[test]
fn every_interface_key_mapping_and_rejected_codes() {
    let core = [
        (KEY_LEFT, XWII_KEY_LEFT),
        (KEY_RIGHT, XWII_KEY_RIGHT),
        (KEY_UP, XWII_KEY_UP),
        (KEY_DOWN, XWII_KEY_DOWN),
        (KEY_NEXT, XWII_KEY_PLUS),
        (KEY_PREVIOUS, XWII_KEY_MINUS),
        (BTN_1, XWII_KEY_ONE),
        (BTN_2, XWII_KEY_TWO),
        (BTN_A, XWII_KEY_A),
        (BTN_B, XWII_KEY_B),
        (BTN_MODE, XWII_KEY_HOME),
    ];
    for (code, expected) in core {
        assert_eq!(map_core_key(code), Some(expected));
    }
    let nunchuk = [(BTN_C, XWII_KEY_C), (BTN_Z, XWII_KEY_Z)];
    for (code, expected) in nunchuk {
        assert_eq!(map_nunchuk_key(code), Some(expected));
    }
    let classic = [
        (BTN_A, XWII_KEY_A),
        (BTN_B, XWII_KEY_B),
        (BTN_X, XWII_KEY_X),
        (BTN_Y, XWII_KEY_Y),
        (KEY_NEXT, XWII_KEY_PLUS),
        (KEY_PREVIOUS, XWII_KEY_MINUS),
        (BTN_MODE, XWII_KEY_HOME),
        (KEY_LEFT, XWII_KEY_LEFT),
        (KEY_RIGHT, XWII_KEY_RIGHT),
        (KEY_UP, XWII_KEY_UP),
        (KEY_DOWN, XWII_KEY_DOWN),
        (BTN_TL, XWII_KEY_TL),
        (BTN_TR, XWII_KEY_TR),
        (BTN_TL2, XWII_KEY_ZL),
        (BTN_TR2, XWII_KEY_ZR),
    ];
    for (code, expected) in classic {
        assert_eq!(map_classic_key(code), Some(expected));
    }
    let pro = [
        (BTN_EAST, XWII_KEY_A),
        (BTN_SOUTH, XWII_KEY_B),
        (BTN_NORTH, XWII_KEY_X),
        (BTN_WEST, XWII_KEY_Y),
        (BTN_START, XWII_KEY_PLUS),
        (BTN_SELECT, XWII_KEY_MINUS),
        (BTN_MODE, XWII_KEY_HOME),
        (BTN_DPAD_LEFT, XWII_KEY_LEFT),
        (BTN_DPAD_RIGHT, XWII_KEY_RIGHT),
        (BTN_DPAD_UP, XWII_KEY_UP),
        (BTN_DPAD_DOWN, XWII_KEY_DOWN),
        (BTN_TL, XWII_KEY_TL),
        (BTN_TR, XWII_KEY_TR),
        (BTN_TL2, XWII_KEY_ZL),
        (BTN_TR2, XWII_KEY_ZR),
        (BTN_THUMBL, XWII_KEY_THUMBL),
        (BTN_THUMBR, XWII_KEY_THUMBR),
    ];
    for (code, expected) in pro {
        assert_eq!(map_pro_key(code), Some(expected));
    }
    let drums = [(BTN_START, XWII_KEY_PLUS), (BTN_SELECT, XWII_KEY_MINUS)];
    for (code, expected) in drums {
        assert_eq!(map_drums_key(code), Some(expected));
    }
    let guitar = [
        (BTN_1, XWII_KEY_FRET_FAR_UP),
        (BTN_2, XWII_KEY_FRET_UP),
        (BTN_3, XWII_KEY_FRET_MID),
        (BTN_4, XWII_KEY_FRET_LOW),
        (BTN_5, XWII_KEY_FRET_FAR_LOW),
        (BTN_DPAD_UP, XWII_KEY_STRUM_BAR_UP),
        (BTN_DPAD_DOWN, XWII_KEY_STRUM_BAR_DOWN),
        (BTN_START, XWII_KEY_PLUS),
        (BTN_SELECT, XWII_KEY_MINUS),
    ];
    for (code, expected) in guitar {
        assert_eq!(map_guitar_key(code), Some(expected));
    }
    for f in [
        map_core_key as fn(u16) -> Option<u32>,
        map_nunchuk_key,
        map_classic_key,
        map_pro_key,
        map_drums_key,
        map_guitar_key,
    ] {
        assert_eq!(f(u16::MAX), None);
    }
}

#[test]
fn all_absolute_interfaces_update_their_complete_cache() {
    let cases: &[(InterfaceKind, &[(u16, i32)])] = &[
        (
            InterfaceKind::Accel,
            &[(ABS_RX, 1), (ABS_RY, 2), (ABS_RZ, 3)],
        ),
        (
            InterfaceKind::MotionPlus,
            &[(ABS_RX, 4), (ABS_RY, 5), (ABS_RZ, 6)],
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
        ),
        (
            InterfaceKind::BalanceBoard,
            &[
                (ABS_HAT0X, 100),
                (ABS_HAT0Y, 101),
                (ABS_HAT1X, 102),
                (ABS_HAT1Y, 103),
            ],
        ),
        (
            InterfaceKind::Pro,
            &[(ABS_X, 110), (ABS_Y, 111), (ABS_RX, 112), (ABS_RY, 113)],
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
        ),
        (
            InterfaceKind::Guitar,
            &[
                (ABS_X, 130),
                (ABS_Y, 131),
                (ABS_HAT1X, 132),
                (ABS_HAT0X, 133),
            ],
        ),
    ];
    for &(interface, axes) in cases {
        let mut decoder = Decoder::new(interface);
        for &(code, value) in axes {
            assert!(
                decoder.update_abs_cache(code, value),
                "{interface:?} rejected ABS {code}"
            );
        }
        assert!(!decoder.update_abs_cache(u16::MAX, -1));
        let report = decoder
            .push(input(time(9, 8), EV_SYN, SYN_REPORT, 0))
            .expect("SYN_REPORT report");
        assert_time_eq(report.time, time(9, 8));
        assert_eq!(
            report.kind.raw_type(),
            match interface {
                InterfaceKind::Accel => XWII_EVENT_ACCEL,
                InterfaceKind::Ir => XWII_EVENT_IR,
                InterfaceKind::MotionPlus => XWII_EVENT_MOTION_PLUS,
                InterfaceKind::Nunchuk => XWII_EVENT_NUNCHUK_MOVE,
                InterfaceKind::Classic => XWII_EVENT_CLASSIC_CONTROLLER_MOVE,
                InterfaceKind::BalanceBoard => XWII_EVENT_BALANCE_BOARD,
                InterfaceKind::Pro => XWII_EVENT_PRO_CONTROLLER_MOVE,
                InterfaceKind::Drums => XWII_EVENT_DRUMS_MOVE,
                InterfaceKind::Guitar => XWII_EVENT_GUITAR_MOVE,
                InterfaceKind::Core => unreachable!(),
                unexpected => panic!(
                    "unexpected InterfaceKind variant in absolute-interface cases: {unexpected:?}"
                ),
            }
        );
    }
    assert!(
        Decoder::new(InterfaceKind::Core)
            .push(input(time(0, 0), EV_SYN, SYN_REPORT, 0))
            .is_none()
    );
}

#[test]
fn absolute_cache_values_match_legacy_slot_order() {
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
        assert!(d.update_abs_cache(code, value));
    }
    assert_eq!(
        d.cache.drums[0],
        Abs {
            x: -32,
            y: 31,
            z: 0
        }
    );
    assert_eq!(d.cache.drums[1].x, 1);
    assert_eq!(d.cache.drums[2].x, 2);
    assert_eq!(d.cache.drums[3].x, 3);
    assert_eq!(d.cache.drums[4].x, 4);
    assert_eq!(d.cache.drums[5].x, 5);
    assert_eq!(d.cache.drums[6].x, 6);
    assert_eq!(d.cache.drums[7].x, 7);

    let mut g = Decoder::new(InterfaceKind::Guitar);
    for (code, value) in [(ABS_X, -32), (ABS_Y, 31), (ABS_HAT1X, -16), (ABS_HAT0X, 31)] {
        assert!(g.update_abs_cache(code, value));
    }
    assert_eq!(
        g.cache.guitar[0],
        Abs {
            x: -32,
            y: 31,
            z: 0
        }
    );
    assert_eq!(g.cache.guitar[1].x, -16);
    assert_eq!(g.cache.guitar[2].x, 31);
}

#[test]
fn ir_starts_with_exact_invalid_sentinels_and_accepts_zero() {
    let d = Decoder::new(InterfaceKind::Ir);
    for point in d.cache.ir {
        assert!(!xwii_event_ir_is_valid(&point));
    }
    assert!(!xwii_event_ir_is_valid(&CEventAbs {
        x: 1023,
        y: 1023,
        z: 42
    }));
    assert!(xwii_event_ir_is_valid(&CEventAbs {
        x: 0,
        y: 1023,
        z: 0
    }));
    assert!(xwii_event_ir_is_valid(&CEventAbs {
        x: 1023,
        y: 0,
        z: 0
    }));
}

#[test]
fn motion_plus_normalization_tracks_direction_and_saturates() {
    let mut n = MotionPlusNormalizer::new();
    assert_eq!(n.normalize(Abs { x: 0, y: 0, z: 0 }), Abs::default());
    assert_eq!(
        n.normalize(Abs { x: 5, y: -5, z: 0 }),
        Abs { x: 5, y: -5, z: 0 }
    );
    n.set(i32::MAX, i32::MIN, 0, 7);
    let (x, y, z, factor) = n.values();
    assert_eq!(x, i32::MAX / 100);
    assert_eq!(y, i32::MIN / 100);
    assert_eq!((z, factor), (0, 7));
    let mut positive = MotionPlusNormalizer::new();
    positive.set(0, 0, 0, i32::MAX);
    positive.normalize(Abs {
        x: i32::MAX,
        y: 0,
        z: 0,
    });
    assert_eq!(positive.values().0, i32::MAX / 100);
}

#[test]
fn syn_dropped_recovery_orders_keys_before_absolute_report_and_preserves_time() {
    let t = time(42, 7);
    let mut d = Decoder::new(InterfaceKind::Nunchuk);
    assert_eq!(
        d.push(input(t, EV_KEY, BTN_Z, 1)).unwrap().kind,
        EventKind::NunchukKey(Key {
            code: XWII_KEY_Z,
            state: 1
        })
    );
    assert!(d.push(input(t, EV_SYN, SYN_DROPPED, 0)).is_none());
    assert!(d.recovery.is_desynced());
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
    assert_time_eq(first.time, t);
    assert_eq!(
        first.kind,
        EventKind::NunchukKey(Key {
            code: XWII_KEY_C,
            state: 1
        })
    );
    let second = d.push_recovered().unwrap();
    assert_eq!(
        second.kind,
        EventKind::NunchukKey(Key {
            code: XWII_KEY_Z,
            state: 0
        })
    );
    let report = d.push_recovered().unwrap();
    assert_time_eq(report.time, t);
    assert_eq!(
        report.kind,
        EventKind::NunchukMove([
            Abs { x: 10, y: 20, z: 0 },
            Abs {
                x: 30,
                y: 40,
                z: 50
            },
        ])
    );
    assert!(d.push_recovered().is_none());
}

#[test]
fn syn_dropped_absolute_seed_is_reported_once_and_key_state_is_stable() {
    let t = time(1, 2);
    let mut d = Decoder::new(InterfaceKind::Accel);
    d.recover(&[], &[(ABS_RX, 100), (ABS_RY, 200), (ABS_RZ, 300)], t);
    let report = d.push_recovered().unwrap();
    assert_eq!(
        report.kind,
        EventKind::Accel(Abs {
            x: 100,
            y: 200,
            z: 300
        })
    );
    assert!(d.push_recovered().is_none());
    assert_eq!(d.recovery.key_state(), &[0; 12]);
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
