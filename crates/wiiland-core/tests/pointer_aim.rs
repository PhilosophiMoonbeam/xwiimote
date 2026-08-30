use wiiland_core::aim::{AimConfig, AimResult, AimState, AimVector, scale_ir_absolute_axis};
use wiiland_core::config::{
    AimActivation, AimMode, AimSource, IrAimMapping, IrRectangle, IrTracking,
};
use wiiland_core::pointer::{
    IrFrame, IrPoint, POINTER_DOWN, POINTER_LEFT, POINTER_RIGHT, POINTER_UP, PointerDelta,
    PointerState, apply_deadzone, scaled_delta, smooth_axis,
};

fn point(x: i32, y: i32) -> IrPoint {
    IrPoint { valid: true, x, y }
}
fn aim_config() -> AimConfig {
    AimConfig {
        output: AimMode::RightStick,
        source: AimSource::Auto,
        activation: AimActivation::Always,
        sensitivity: 16,
        deadzone: 4,
        smoothing: 0,
        invert_x: false,
        invert_y: false,
        ir_mapping: IrAimMapping::Relative,
        ir_screen: None,
        accel_calibration: None,
        motion_plus_calibration: None,
    }
}

#[test]
fn dpad_pointer_velocity_is_sticky_and_opposites_cancel() {
    let mut state = PointerState::default();
    assert_eq!(state.velocity(), PointerDelta { dx: 0, dy: 0 });
    assert_eq!(
        state.update_key(POINTER_LEFT, true),
        PointerDelta { dx: -16, dy: 0 }
    );
    assert_eq!(
        state.update_key(POINTER_UP, true),
        PointerDelta { dx: -16, dy: -16 }
    );
    assert_eq!(state.pointer_keys(), POINTER_LEFT | POINTER_UP);
    assert_eq!(state.tick(), PointerDelta { dx: -16, dy: -16 });
    assert_eq!(
        state.update_key(POINTER_RIGHT, true),
        PointerDelta { dx: 0, dy: -16 }
    );
    assert_eq!(
        state.update_key(POINTER_LEFT, false),
        PointerDelta { dx: 16, dy: -16 }
    );
    assert_eq!(
        state.update_key(POINTER_UP, false),
        PointerDelta { dx: 16, dy: 0 }
    );
    assert_eq!(
        state.update_key(POINTER_RIGHT, false),
        PointerDelta::default()
    );
    state.set_speed(31);
    assert_eq!(
        state.update_key(POINTER_DOWN, true),
        PointerDelta { dx: 0, dy: 31 }
    );
    state.reset();
    assert_eq!(state.pointer_keys(), 0);
    assert_eq!(state.velocity(), PointerDelta::default());
}

#[test]
fn pointer_scaling_deadzone_smoothing_and_ir_tracking_preserve_integer_rules() {
    assert_eq!(scaled_delta(200, 280, 8), 10);
    assert_eq!(scaled_delta(200, 280, 16), 20);
    assert_eq!(scaled_delta(280, 200, 8), -10);
    assert_eq!(apply_deadzone(5, 6), 0);
    assert_eq!(apply_deadzone(-5, 6), 0);
    assert_eq!(apply_deadzone(6, 6), 6);
    assert_eq!(apply_deadzone(0, 0), 0);
    assert_eq!(smooth_axis(200, 280, 50), 240);
    assert_eq!(smooth_axis(0, 1, 33), 0);
    assert_eq!(smooth_axis(200, 280, 0), 280);

    let mut state = PointerState::new(16, 8, 0, 0, IrTracking::First);
    assert_eq!(
        state.update_ir(Some(point(200, 300))),
        PointerDelta::default()
    );
    assert!(state.ir_active());
    assert_eq!(
        state.update_ir(Some(point(280, 260))),
        PointerDelta { dx: 10, dy: -5 }
    );
    state.set_ir_options(8, 6, 0, IrTracking::First);
    assert_eq!(
        state.update_ir(Some(point(360, 220))),
        PointerDelta { dx: 10, dy: 0 }
    );
    state.set_ir_options(8, 0, 50, IrTracking::First);
    assert_eq!(
        state.update_ir(Some(point(440, 180))),
        PointerDelta { dx: 5, dy: -2 }
    );
    assert_eq!(state.update_ir(None), PointerDelta::default());
    assert!(!state.ir_active());
    assert_eq!(
        state.update_ir(Some(point(440, 180))),
        PointerDelta::default()
    );
}

#[test]
fn ir_tracking_first_centroid_and_dual_choose_expected_points() {
    let mut frame = IrFrame::default();
    frame.points[0] = point(1023, 1023);
    frame.points[1] = point(200, 300);
    frame.points[2] = IrPoint {
        valid: false,
        x: 500,
        y: 500,
    };
    let state = PointerState::new(16, 8, 0, 0, IrTracking::First);
    assert_eq!(state.select_ir(&frame), Some(point(1023, 1023)));

    let centroid = PointerState::new(16, 8, 0, 0, IrTracking::Centroid);
    assert_eq!(centroid.select_ir(&frame), Some(point(611, 661)));

    let mut dual_frame = IrFrame::default();
    dual_frame.points[0] = point(100, 300);
    dual_frame.points[1] = point(500, 300);
    dual_frame.points[2] = point(260, 340);
    let mut dual = PointerState::new(16, 8, 0, 0, IrTracking::Dual);
    assert_eq!(dual.select_ir(&dual_frame), Some(point(300, 300)));
    assert_eq!(dual.update_ir_frame(&dual_frame), PointerDelta::default());
    dual_frame.points[0] = point(520, 300);
    dual_frame.points[1] = point(120, 300);
    dual_frame.points[2] = point(280, 340);
    assert_eq!(dual.select_ir(&dual_frame), Some(point(320, 300)));
    assert_eq!(
        dual.update_ir_frame(&dual_frame),
        PointerDelta { dx: 2, dy: 0 }
    );
    assert_eq!(dual.select_ir(&IrFrame::default()), None);
}

#[test]
fn aim_activation_and_reset_follow_mode_and_key_contracts() {
    let mut config = aim_config();
    config.activation = AimActivation::B;
    let mut state = AimState::new(config);
    assert!(state.enabled());
    assert!(!state.is_active());
    assert_eq!(state.activation_key(19, true), AimResult::default());
    assert!(!state.held);
    assert_eq!(state.activation_key(5, true), AimResult::default());
    assert!(state.held && state.is_active());
    let result = state.process_motion_plus([20, -12, 0]);
    assert_eq!(result.output, Some(AimVector { x: 16, y: -8 }));
    assert!(!result.reset);
    let result = state.activation_key(5, false);
    assert!(result.reset);
    assert_eq!(result.output, Some(AimVector { x: 0, y: 0 }));
    assert!(!state.held && state.active_source.is_none());

    state.config.activation = AimActivation::Always;
    assert!(state.is_active());
    assert_eq!(state.activation_key(999, false), AimResult::default());
    state.config.output = AimMode::Off;
    assert!(!state.enabled());
    assert_eq!(state.activation_key(5, true), AimResult::default());
}

#[test]
fn aim_sources_are_explicit_or_sticky_auto_and_loss_resets() {
    let mut config = aim_config();
    config.deadzone = 0;
    let mut state = AimState::new(config);
    assert_eq!(
        state.process_motion_plus([20, -12, 0]).output,
        Some(AimVector { x: 20, y: -12 })
    );
    assert_eq!(state.active_source, Some(AimSource::MotionPlus));
    assert_eq!(
        state.process_accelerometer([100, 200, 0]),
        AimResult::default()
    );
    assert_eq!(
        state.process_ir(Some(point(400, 300))).output,
        Some(AimVector::default())
    );
    assert_eq!(state.active_source, Some(AimSource::Ir));
    assert!(state.process_motion_plus([20, 20, 0]).output.is_none());
    let reset = state.process_ir(None);
    assert!(reset.reset && reset.output == Some(AimVector::default()));
    assert!(state.active_source.is_none());

    let mut explicit = aim_config();
    explicit.source = AimSource::MotionPlus;
    explicit.deadzone = 0;
    let mut state = AimState::new(explicit);
    assert_eq!(state.process_ir(Some(point(2, 3))), AimResult::default());
    assert_eq!(
        state.process_motion_plus([20, -12, 0]).output,
        Some(AimVector { x: 20, y: -12 })
    );
}

#[test]
fn aim_calibration_absolute_ir_inversion_and_smoothing_are_integer_exact() {
    assert_eq!(scale_ir_absolute_axis(100, 100, 900), -32_767);
    assert_eq!(scale_ir_absolute_axis(500, 100, 900), 0);
    assert_eq!(scale_ir_absolute_axis(900, 100, 900), 32_767);
    assert_eq!(scale_ir_absolute_axis(0, 100, 900), -32_767);
    assert_eq!(scale_ir_absolute_axis(10, 1, 1), 0);

    let mut config = aim_config();
    config.deadzone = 0;
    config.ir_mapping = IrAimMapping::Absolute;
    config.ir_screen = Some(IrRectangle {
        left: 100,
        right: 900,
        top: 100,
        bottom: 700,
    });
    config.invert_x = true;
    config.invert_y = true;
    config.smoothing = 0;
    let mut state = AimState::new(config);
    assert_eq!(
        state.process_ir(Some(point(900, 400))).output,
        Some(AimVector { x: -32_767, y: 0 })
    );

    let mut config = aim_config();
    config.deadzone = 0;
    config.smoothing = 50;
    let mut state = AimState::new(config);
    assert_eq!(
        state.process_motion_plus([100, 0, 0]).output,
        Some(AimVector { x: 50, y: 0 })
    );
    assert_eq!(
        state.process_motion_plus([200, 0, 0]).output,
        Some(AimVector { x: 125, y: 0 })
    );
    assert_eq!(
        state.process_motion_plus([0, 0, 0]).output,
        Some(AimVector { x: 62, y: 0 })
    );

    let mut config = aim_config();
    config.deadzone = 0;
    config.motion_plus_calibration = Some(wiiland_core::SensorCalibration {
        x: 4,
        y: -4,
        z: 0,
        axes: wiiland_core::SensorCalibration::ALL,
    });
    let mut state = AimState::new(config);
    assert_eq!(
        state.process_motion_plus([20, -12, 0]).output,
        Some(AimVector { x: 16, y: -8 })
    );

    let mut config = aim_config();
    config.deadzone = 0;
    config.accel_calibration = Some(wiiland_core::SensorCalibration {
        x: 100,
        y: 200,
        z: 0,
        axes: wiiland_core::SensorCalibration::ALL,
    });
    let mut state = AimState::new(config);
    let result = state.process_accelerometer([120, 188, 0]);
    assert_eq!(result.output, Some(AimVector { x: 20, y: -12 }));
    assert!(state.accel_zeroed);
}
