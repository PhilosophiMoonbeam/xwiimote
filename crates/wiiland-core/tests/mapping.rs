use wiiland_core::mapping::*;

fn values() -> [Abs3; 8] {
    [Abs3 { x: 0, y: 0, z: 0 }; 8]
}

#[test]
fn linux_uinput_codes_match_kernel_abi_values() {
    assert_eq!(
        [
            BTN_SOUTH,
            BTN_EAST,
            BTN_NORTH,
            BTN_WEST,
            BTN_TL,
            BTN_TR,
            BTN_TL2,
            BTN_TR2,
            BTN_SELECT,
            BTN_START,
            BTN_MODE,
            BTN_THUMBL,
            BTN_THUMBR,
            BTN_DPAD_LEFT,
            BTN_DPAD_RIGHT,
            BTN_DPAD_UP,
            BTN_DPAD_DOWN,
            BTN_1,
            BTN_2,
            BTN_C,
            BTN_Z,
            BTN_STRUM_BAR_UP,
            BTN_STRUM_BAR_DOWN,
            BTN_FRET_FAR_UP,
            BTN_FRET_UP,
            BTN_FRET_MID,
            BTN_FRET_LOW,
            BTN_FRET_FAR_LOW,
        ],
        [
            0x130, 0x131, 0x133, 0x134, 0x136, 0x137, 0x138, 0x139, 0x13a, 0x13b, 0x13c, 0x13d,
            0x13e, 0x222, 0x223, 0x220, 0x221, 0x101, 0x102, 0x132, 0x135, 0x229, 0x22a, 0x224,
            0x225, 0x226, 0x227, 0x228,
        ]
    );
    assert_eq!(
        [
            ABS_X,
            ABS_Y,
            ABS_Z,
            ABS_RX,
            ABS_RY,
            ABS_RZ,
            ABS_THROTTLE,
            ABS_RUDDER,
            ABS_WHEEL,
            ABS_GAS,
            ABS_BRAKE,
            ABS_HAT0X,
            ABS_HAT1X,
            ABS_HAT1Y,
            ABS_HAT2X,
            ABS_HAT3X,
            ABS_HAT3Y,
            ABS_MISC,
            ABS_PRESSURE,
            ABS_DISTANCE,
            ABS_TILT_X,
            ABS_TILT_Y,
        ],
        [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x10, 0x12, 0x13,
            0x14, 0x16, 0x17, 0x28, 0x18, 0x19, 0x1a, 0x1b,
        ]
    );
    assert_eq!(DESKTOP_KEYS, [0x110, 0x111, 28, 1, 125, 104, 109]);
    assert_eq!(DESKTOP_RELS, [0x00, 0x01]);
}

#[test]
fn guitar_virtual_codes_are_exact_and_do_not_collide_with_core_controls() {
    let guitar_codes = [
        BTN_FRET_FAR_UP,
        BTN_FRET_UP,
        BTN_FRET_MID,
        BTN_FRET_LOW,
        BTN_FRET_FAR_LOW,
        BTN_STRUM_BAR_UP,
        BTN_STRUM_BAR_DOWN,
    ];
    assert_eq!(
        guitar_codes,
        [0x224, 0x225, 0x226, 0x227, 0x228, 0x229, 0x22a]
    );

    let non_guitar_core_controls = [
        BTN_DPAD_LEFT,
        BTN_DPAD_RIGHT,
        BTN_DPAD_UP,
        BTN_DPAD_DOWN,
        BTN_SOUTH,
        BTN_EAST,
        BTN_NORTH,
        BTN_WEST,
        BTN_START,
        BTN_SELECT,
        BTN_MODE,
        BTN_1,
        BTN_2,
        BTN_TL,
        BTN_TR,
        BTN_TL2,
        BTN_TR2,
        BTN_THUMBL,
        BTN_THUMBR,
        BTN_C,
        BTN_Z,
    ];
    for (index, code) in guitar_codes.into_iter().enumerate() {
        assert!(
            !non_guitar_core_controls.contains(&code),
            "guitar code {code:#x} collides with a core control"
        );
        assert!(
            !guitar_codes[..index].contains(&code),
            "guitar code {code:#x} is not unique"
        );
    }
}

#[test]
fn public_wii_guitar_keys_emit_dedicated_virtual_codes() {
    let public_key_mappings = [
        (23, BTN_FRET_FAR_UP),
        (24, BTN_FRET_UP),
        (25, BTN_FRET_MID),
        (26, BTN_FRET_LOW),
        (27, BTN_FRET_FAR_LOW),
        (21, BTN_STRUM_BAR_UP),
        (22, BTN_STRUM_BAR_DOWN),
    ];
    for (public_key, expected_virtual_code) in public_key_mappings {
        assert_eq!(
            map_key(public_key),
            Some(expected_virtual_code),
            "public Wii guitar key {public_key}"
        );
    }
}

#[test]
fn key_mapping_covers_every_internal_button_and_rejects_unknown() {
    let code_to_key = [
        BTN_DPAD_LEFT,
        BTN_DPAD_RIGHT,
        BTN_DPAD_UP,
        BTN_DPAD_DOWN,
        BTN_SOUTH,
        BTN_EAST,
        BTN_START,
        BTN_SELECT,
        BTN_MODE,
        BTN_1,
        BTN_2,
        BTN_NORTH,
        BTN_WEST,
        BTN_TL,
        BTN_TR,
        BTN_TL2,
        BTN_TR2,
        BTN_THUMBL,
        BTN_THUMBR,
        BTN_C,
        BTN_Z,
        BTN_STRUM_BAR_UP,
        BTN_STRUM_BAR_DOWN,
        BTN_FRET_FAR_UP,
        BTN_FRET_UP,
        BTN_FRET_MID,
        BTN_FRET_LOW,
        BTN_FRET_FAR_LOW,
    ];
    for (code, expected_code) in code_to_key.into_iter().enumerate() {
        assert_eq!(map_key(code as u32), Some(expected_code), "key {code}");
    }
    assert_eq!(map_key(28), None);
    assert_eq!(map_key(u32::MAX), None);

    let expected_capability_order = [
        BTN_DPAD_LEFT,
        BTN_DPAD_RIGHT,
        BTN_DPAD_UP,
        BTN_DPAD_DOWN,
        BTN_SOUTH,
        BTN_EAST,
        BTN_NORTH,
        BTN_WEST,
        BTN_START,
        BTN_SELECT,
        BTN_MODE,
        BTN_1,
        BTN_2,
        BTN_TL,
        BTN_TR,
        BTN_TL2,
        BTN_TR2,
        BTN_THUMBL,
        BTN_THUMBR,
        BTN_C,
        BTN_Z,
        BTN_STRUM_BAR_UP,
        BTN_STRUM_BAR_DOWN,
        BTN_FRET_FAR_UP,
        BTN_FRET_UP,
        BTN_FRET_MID,
        BTN_FRET_LOW,
        BTN_FRET_FAR_LOW,
    ];
    assert_eq!(CORE_KEYS.len(), 28);
    assert_eq!(CORE_KEYS, expected_capability_order);
}

#[test]
fn sensor_and_extension_axis_maps_are_ordered_and_bounded() {
    assert_eq!(
        [accel_abs_code(0), accel_abs_code(1), accel_abs_code(2)],
        [Some(ABS_THROTTLE), Some(ABS_RUDDER), Some(ABS_WHEEL)]
    );
    assert_eq!(
        [
            nunchuk_accel_abs_code(0),
            nunchuk_accel_abs_code(1),
            nunchuk_accel_abs_code(2)
        ],
        [Some(ABS_HAT1X), Some(ABS_HAT1Y), Some(ABS_HAT2X)]
    );
    assert_eq!(
        [
            motion_plus_abs_code(0),
            motion_plus_abs_code(1),
            motion_plus_abs_code(2)
        ],
        [Some(ABS_GAS), Some(ABS_BRAKE), Some(ABS_HAT0X)]
    );
    assert_eq!(
        [
            balance_abs_code(0),
            balance_abs_code(1),
            balance_abs_code(2),
            balance_abs_code(3)
        ],
        [
            Some(ABS_PRESSURE),
            Some(ABS_DISTANCE),
            Some(ABS_TILT_X),
            Some(ABS_TILT_Y)
        ]
    );
    assert_eq!(
        [
            drums_abs_code(0),
            drums_abs_code(1),
            drums_abs_code(2),
            drums_abs_code(3),
            drums_abs_code(4),
            drums_abs_code(5),
            drums_abs_code(6),
            drums_abs_code(7)
        ],
        [
            Some(ABS_X),
            Some(ABS_RX),
            Some(ABS_RY),
            Some(ABS_Z),
            Some(ABS_RZ),
            Some(ABS_HAT3X),
            Some(ABS_HAT3Y),
            Some(ABS_MISC)
        ]
    );
    assert_eq!(accel_abs_code(3), None);
    assert_eq!(nunchuk_accel_abs_code(3), None);
    assert_eq!(motion_plus_abs_code(3), None);
    assert_eq!(balance_abs_code(4), None);
    assert_eq!(drums_abs_code(8), None);
}

#[test]
fn virtual_capability_sets_match_gamepad_and_desktop_contracts() {
    assert_eq!(CONTROLLER_CAPABILITIES.keys, CORE_KEYS.as_slice());
    assert_eq!(CONTROLLER_CAPABILITIES.axes, CORE_AXES.as_slice());
    assert!(CONTROLLER_CAPABILITIES.rels.is_empty());
    assert_eq!(DESKTOP_CAPABILITIES.keys, DESKTOP_KEYS.as_slice());
    assert!(DESKTOP_CAPABILITIES.axes.is_empty());
    assert_eq!(DESKTOP_CAPABILITIES.rels, &[0, 1]);
    assert_eq!(DESKTOP_KEYS, [0x110, 0x111, 28, 1, 125, 104, 109]);
}

#[test]
fn signed_and_unsigned_scaling_preserves_extrema_and_integer_truncation() {
    assert_eq!(scale_signed_axis(-120, 120, 120), VIRTUAL_AXIS_MIN);
    assert_eq!(scale_signed_axis(120, 120, 120), VIRTUAL_AXIS_MAX);
    assert_eq!(scale_signed_axis(-60, 120, 120), -16_384);
    assert_eq!(scale_signed_axis(60, 120, 120), 16_383);
    assert_eq!(scale_signed_axis(-1, 32, 31), -1024);
    assert_eq!(scale_signed_axis(1, 32, 31), 1057);
    assert_eq!(scale_signed_axis(-999, 0, 1), 0);
    assert_eq!(scale_signed_axis(999, 1, 0), 0);
    assert_eq!(scale_unsigned_axis(0, 62, 1023), 0);
    assert_eq!(scale_unsigned_axis(31, 62, 1023), 511);
    assert_eq!(scale_unsigned_axis(62, 62, 1023), 1023);
    assert_eq!(scale_unsigned_axis(-1, 62, 1023), 0);
    assert_eq!(scale_unsigned_axis(999, 0, 1023), 0);
    assert_eq!(scale_unsigned_axis(2, 3, 1), 0);
    assert_eq!(scale_unsigned_axis(2, 3, -1), 0);
    assert_eq!(clamp_axis_value(-40_000), VIRTUAL_AXIS_MIN);
    assert_eq!(clamp_axis_value(40_000), VIRTUAL_AXIS_MAX);
}

#[test]
fn axis_info_matches_virtual_ranges_and_omits_unknown_codes() {
    for code in CORE_AXES {
        let info = axis_info(code).expect("core axis metadata");
        assert_eq!(
            info.fuzz,
            if matches!(
                code,
                ABS_Z
                    | ABS_RZ
                    | ABS_HAT3Y
                    | ABS_MISC
                    | ABS_PRESSURE
                    | ABS_DISTANCE
                    | ABS_TILT_X
                    | ABS_TILT_Y
            ) {
                4
            } else {
                16
            }
        );
        if matches!(code, ABS_Z | ABS_RZ | ABS_HAT3Y | ABS_MISC) {
            assert_eq!(
                info,
                AxisInfo {
                    minimum: 0,
                    maximum: VIRTUAL_TRIGGER_MAX,
                    flat: 0,
                    fuzz: 4
                }
            );
        } else if matches!(code, ABS_PRESSURE | ABS_DISTANCE | ABS_TILT_X | ABS_TILT_Y) {
            assert_eq!(
                info,
                AxisInfo {
                    minimum: 0,
                    maximum: 65_535,
                    flat: 0,
                    fuzz: 4
                }
            );
        } else {
            assert_eq!(
                info,
                AxisInfo {
                    minimum: VIRTUAL_AXIS_MIN,
                    maximum: VIRTUAL_AXIS_MAX,
                    flat: 256,
                    fuzz: 16
                }
            );
        }
    }
    assert_eq!(axis_info(u16::MAX), None);
}
#[test]
fn motion_mapping_preserves_every_device_shape_and_order() {
    let mut v = values();
    v[0] = Abs3 {
        x: -500,
        y: 0,
        z: 500,
    };
    let mapped = map_motion(MotionKind::Accel, v);
    assert_eq!(mapped.count, 3);
    assert_eq!(
        mapped.axes[..3],
        [
            MappedAxis {
                code: ABS_THROTTLE,
                value: VIRTUAL_AXIS_MIN
            },
            MappedAxis {
                code: ABS_RUDDER,
                value: 0
            },
            MappedAxis {
                code: ABS_WHEEL,
                value: VIRTUAL_AXIS_MAX
            },
        ]
    );

    v = values();
    v[0] = Abs3 {
        x: -120,
        y: 120,
        z: 0,
    };
    v[1] = Abs3 {
        x: -500,
        y: 0,
        z: 500,
    };
    let mapped = map_motion(MotionKind::Nunchuk, v);
    assert_eq!(mapped.count, 5);
    assert_eq!(
        mapped.axes[..5],
        [
            MappedAxis {
                code: ABS_X,
                value: VIRTUAL_AXIS_MIN
            },
            MappedAxis {
                code: ABS_Y,
                value: VIRTUAL_AXIS_MAX
            },
            MappedAxis {
                code: ABS_HAT1X,
                value: VIRTUAL_AXIS_MIN
            },
            MappedAxis {
                code: ABS_HAT1Y,
                value: 0
            },
            MappedAxis {
                code: ABS_HAT2X,
                value: VIRTUAL_AXIS_MAX
            },
        ]
    );

    v = values();
    v[0] = Abs3 {
        x: -30,
        y: 30,
        z: 0,
    };
    v[1] = Abs3 {
        x: 30,
        y: -30,
        z: 0,
    };
    v[2] = Abs3 { x: 62, y: 0, z: 0 };
    let mapped = map_motion(MotionKind::Classic, v);
    assert_eq!(mapped.count, 6);
    assert_eq!(
        mapped.axes[..6],
        [
            MappedAxis {
                code: ABS_X,
                value: VIRTUAL_AXIS_MIN
            },
            MappedAxis {
                code: ABS_Y,
                value: VIRTUAL_AXIS_MAX
            },
            MappedAxis {
                code: ABS_RX,
                value: VIRTUAL_AXIS_MAX
            },
            MappedAxis {
                code: ABS_RY,
                value: VIRTUAL_AXIS_MIN
            },
            MappedAxis {
                code: ABS_Z,
                value: VIRTUAL_TRIGGER_MAX
            },
            MappedAxis {
                code: ABS_RZ,
                value: 0
            },
        ]
    );

    v[0] = Abs3 {
        x: -1024,
        y: 1024,
        z: 0,
    };
    v[1] = Abs3 {
        x: 1024,
        y: -1024,
        z: 0,
    };
    let mapped = map_motion(MotionKind::Pro, v);
    assert_eq!(mapped.count, 4);
    assert_eq!(
        mapped.axes[..4],
        [
            MappedAxis {
                code: ABS_X,
                value: VIRTUAL_AXIS_MIN
            },
            MappedAxis {
                code: ABS_Y,
                value: VIRTUAL_AXIS_MAX
            },
            MappedAxis {
                code: ABS_RX,
                value: VIRTUAL_AXIS_MAX
            },
            MappedAxis {
                code: ABS_RY,
                value: VIRTUAL_AXIS_MIN
            },
        ]
    );

    v = values();
    v[0] = Abs3 {
        x: -32,
        y: 31,
        z: 0,
    };
    v[1].x = -16;
    v[2].x = 31;
    let mapped = map_motion(MotionKind::Guitar, v);
    assert_eq!(mapped.count, 4);
    assert_eq!(
        mapped.axes[..4],
        [
            MappedAxis {
                code: ABS_X,
                value: VIRTUAL_AXIS_MIN
            },
            MappedAxis {
                code: ABS_Y,
                value: VIRTUAL_AXIS_MAX
            },
            MappedAxis {
                code: ABS_HAT3X,
                value: VIRTUAL_AXIS_MIN
            },
            MappedAxis {
                code: ABS_HAT3Y,
                value: VIRTUAL_TRIGGER_MAX
            },
        ]
    );

    v = values();
    v[0].x = 100;
    v[1].x = 200;
    v[2].x = 300;
    v[3].x = 400;
    let mapped = map_motion(MotionKind::Balance, v);
    assert_eq!(mapped.count, 4);
    assert_eq!(
        mapped.axes[..4],
        [
            MappedAxis {
                code: ABS_PRESSURE,
                value: 100
            },
            MappedAxis {
                code: ABS_DISTANCE,
                value: 200
            },
            MappedAxis {
                code: ABS_TILT_X,
                value: 300
            },
            MappedAxis {
                code: ABS_TILT_Y,
                value: 400
            },
        ]
    );

    v = values();
    v[0] = Abs3 {
        x: -16_000,
        y: 0,
        z: 16_000,
    };
    let mapped = map_motion(MotionKind::MotionPlus, v);
    assert_eq!(mapped.count, 3);
    assert_eq!(
        mapped.axes[..3],
        [
            MappedAxis {
                code: ABS_GAS,
                value: VIRTUAL_AXIS_MIN
            },
            MappedAxis {
                code: ABS_BRAKE,
                value: 0
            },
            MappedAxis {
                code: ABS_HAT0X,
                value: VIRTUAL_AXIS_MAX
            },
        ]
    );

    v = values();
    v[0] = Abs3 {
        x: -32,
        y: 31,
        z: 0,
    };
    for value in v.iter_mut().skip(1) {
        value.x = DRUM_PRESSURE_MAX;
    }
    let mapped = map_motion(MotionKind::Drums, v);
    assert_eq!(mapped.count, 9);
    assert_eq!(
        mapped.axes[0],
        MappedAxis {
            code: ABS_X,
            value: VIRTUAL_AXIS_MIN
        }
    );
    assert_eq!(
        mapped.axes[1],
        MappedAxis {
            code: ABS_Y,
            value: VIRTUAL_AXIS_MAX
        }
    );
    for (i, code) in [
        ABS_RX, ABS_RY, ABS_Z, ABS_RZ, ABS_HAT3X, ABS_HAT3Y, ABS_MISC,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            mapped.axes[i + 2],
            MappedAxis {
                code,
                value: if matches!(i, 0 | 1 | 4) {
                    VIRTUAL_AXIS_MAX
                } else {
                    VIRTUAL_TRIGGER_MAX
                }
            }
        );
    }
}

#[test]
fn drum_pressure_uses_signed_axis_ranges_and_trigger_ranges() {
    assert_eq!(
        scale_drums_pressure(0, DRUM_PRESSURE_MAX),
        VIRTUAL_TRIGGER_MAX
    );
    assert_eq!(scale_drums_pressure(1, DRUM_PRESSURE_MAX), VIRTUAL_AXIS_MAX);
    assert_eq!(scale_drums_pressure(2, DRUM_PRESSURE_MAX), VIRTUAL_AXIS_MAX);
    assert_eq!(scale_drums_pressure(5, DRUM_PRESSURE_MAX), VIRTUAL_AXIS_MAX);
    assert_eq!(
        scale_drums_pressure(3, DRUM_PRESSURE_MAX),
        VIRTUAL_TRIGGER_MAX
    );
    assert_eq!(scale_drums_pressure(7, -1), 0);
    assert_eq!(
        scale_drums_pressure(7, DRUM_PRESSURE_MAX + 1),
        VIRTUAL_TRIGGER_MAX
    );
}
