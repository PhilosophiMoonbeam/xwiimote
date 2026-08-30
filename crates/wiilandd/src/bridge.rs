//! Per-device event bridge and output lifecycle.
use crate::uinput::{Backend, VirtualDevice, VirtualKind};
use std::cell::Cell;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use wiiland_core::aim::{AimConfig, AimState};
use wiiland_core::mapping::{self, Abs3, MotionKind};
use wiiland_core::pointer::{
    IrFrame, IrPoint, POINTER_DOWN, POINTER_LEFT, POINTER_RIGHT, POINTER_UP, PointerState,
};
use wiiland_core::{
    AbsPayload, Config, KeyPayload, Profile, TraceEvent, TraceFilter, TracePayload,
};
use wiiland_hid::{
    Axis3, Button, ButtonEvent, ButtonState, Event, EventKind, Interface, InterfaceMask,
};

pub const MAX_EVENTS_PER_DRAIN: usize = 256;
pub const PROFILE_GAMEPAD: u8 = Profile::GAMEPAD.bits();
pub const PROFILE_DESKTOP: u8 = Profile::DESKTOP.bits();

fn io_errno(error: &io::Error) -> i32 {
    -error.raw_os_error().unwrap_or(libc::EIO)
}

fn button_code(button: Button) -> Option<u32> {
    Some(match button {
        Button::Left => 0,
        Button::Right => 1,
        Button::Up => 2,
        Button::Down => 3,
        Button::Plus => 6,
        Button::Minus => 7,
        Button::One => 9,
        Button::Two => 10,
        Button::A => 4,
        Button::B => 5,
        Button::Home => 8,
        Button::C => 19,
        Button::Z => 20,
        Button::X => 11,
        Button::Y => 12,
        Button::ShoulderLeft => 13,
        Button::ShoulderRight => 14,
        Button::TriggerLeft => 15,
        Button::TriggerRight => 16,
        Button::ThumbLeft => 17,
        Button::ThumbRight => 18,
        Button::StrumBarUp => 21,
        Button::StrumBarDown => 22,
        Button::FretFarUp => 23,
        Button::FretUp => 24,
        Button::FretMid => 25,
        Button::FretLow => 26,
        Button::FretFarLow => 27,
        _ => return None,
    })
}

fn button_state(state: ButtonState) -> Option<u32> {
    match state {
        ButtonState::Released => Some(0),
        ButtonState::Pressed => Some(1),
        ButtonState::Repeated => Some(2),
        _ => None,
    }
}

fn event_type_code(kind: EventKind) -> u32 {
    match kind {
        EventKind::Key(_) => 0,
        EventKind::Accel(_) => 1,
        EventKind::Ir(_) => 2,
        EventKind::BalanceBoard(_) => 3,
        EventKind::MotionPlus(_) => 4,
        EventKind::ProControllerKey(_) => 5,
        EventKind::ProControllerMove(_) => 6,
        EventKind::Watch => 7,
        EventKind::ClassicControllerKey(_) => 8,
        EventKind::ClassicControllerMove(_) => 9,
        EventKind::NunchukKey(_) => 10,
        EventKind::NunchukMove(_) => 11,
        EventKind::DrumsKey(_) => 12,
        EventKind::DrumsMove(_) => 13,
        EventKind::GuitarKey(_) => 14,
        EventKind::GuitarMove(_) => 15,
        EventKind::Gone => 16,
        EventKind::Unknown(value) => value,
        _ => u32::MAX,
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BridgeAction {
    Continue,
    Gone,
}

type TraceSink = Box<dyn FnMut(&str)>;
type TraceClock = Box<dyn FnMut() -> i64>;

struct TraceContext {
    filter: TraceFilter,
    sequence: Rc<Cell<u64>>,
    clock: TraceClock,
    sink: TraceSink,
}

impl TraceContext {
    fn new<F: FnMut(&str) + 'static>(
        filter: TraceFilter,
        sequence: Rc<Cell<u64>>,
        sink: F,
    ) -> Self {
        Self::with_clock(filter, sequence, monotonic_time_us, sink)
    }

    fn with_clock<C: FnMut() -> i64 + 'static, F: FnMut(&str) + 'static>(
        filter: TraceFilter,
        sequence: Rc<Cell<u64>>,
        clock: C,
        sink: F,
    ) -> Self {
        Self {
            filter,
            sequence,
            clock: Box::new(clock),
            sink: Box::new(sink),
        }
    }

    fn emit(&mut self, syspath: &Path, event: &Event) {
        if !self.filter.matches(event_type_code(event.kind)) {
            return;
        }
        let sequence = self.sequence.get() + 1;
        self.sequence.set(sequence);
        let monotonic_us = (self.clock)();
        let line = format_trace_line(sequence, monotonic_us, syspath, event);
        (self.sink)(&line);
    }
}

pub struct BridgeDevice<B: Backend + Clone = crate::uinput::SystemBackend> {
    pub syspath: PathBuf,
    pub profile: Profile,
    pub iface: Interface,
    pub gamepad: Option<VirtualDevice<B>>,
    pub desktop: Option<VirtualDevice<B>>,
    pub pointer: PointerState,
    pub aim: AimState,
    pub opened_ifaces: InterfaceMask,
    pub pending_ifaces: InterfaceMask,
    trace: Option<TraceContext>,
    config: Config,
    backend: B,
    outputs_enabled: bool,
}
impl BridgeDevice<crate::uinput::SystemBackend> {
    pub fn new(path: impl AsRef<Path>, config: &Config) -> Result<Self, i32> {
        Self::with_backend(path, config, crate::uinput::SystemBackend)
    }
}
impl<B: Backend + Clone> BridgeDevice<B> {
    pub fn with_backend(path: impl AsRef<Path>, config: &Config, backend: B) -> Result<Self, i32> {
        Self::with_backend_outputs(path, config, backend, true)
    }
    pub fn with_backend_outputs(
        path: impl AsRef<Path>,
        config: &Config,
        backend: B,
        outputs_enabled: bool,
    ) -> Result<Self, i32> {
        let iface = Interface::new(path.as_ref()).map_err(|error| io_errno(&error))?;
        let syspath = iface.syspath().to_path_buf();
        let devtype = iface.attr("devtype").ok();
        let profile = profile_for_device(config, &syspath, devtype.as_deref());
        let mut iface = iface;
        iface.watch(true).map_err(|error| io_errno(&error))?;
        let requested = requested_interfaces(profile, config);
        let mut pending = requested;
        let opened_result = iface.open(requested);
        let opened = match &opened_result {
            Ok(mask) => *mask,
            Err(error) => error.opened(),
        };
        if opened.is_empty()
            && let Err(error) = opened_result
        {
            return Err(io_errno(error.source()));
        }
        pending &= !opened;
        let (gamepad, desktop) = create_outputs(profile, config, &backend, outputs_enabled)?;
        Ok(Self {
            syspath,
            profile,
            iface,
            gamepad,
            desktop,
            pointer: PointerState::new(
                config.pointer_speed,
                config.ir_speed,
                config.ir_deadzone,
                config.ir_smoothing,
                config.ir_tracking,
            ),
            aim: AimState::new(AimConfig::from_config(config)),
            opened_ifaces: opened,
            pending_ifaces: pending,
            trace: None,
            config: config.clone(),
            backend,
            outputs_enabled,
        })
    }
    pub fn set_trace_sink<F: FnMut(&str) + 'static>(&mut self, filter: TraceFilter, sink: F) {
        self.set_trace_sink_with_sequence(filter, Rc::new(Cell::new(0)), sink);
    }
    pub(crate) fn set_trace_sink_with_sequence<F: FnMut(&str) + 'static>(
        &mut self,
        filter: TraceFilter,
        sequence: Rc<Cell<u64>>,
        sink: F,
    ) {
        self.trace = Some(TraceContext::new(filter, sequence, sink));
    }
    pub fn path(&self) -> &Path {
        &self.syspath
    }
    pub fn retry_open(&mut self) -> Result<(), i32> {
        if self.pending_ifaces.is_empty() {
            return Ok(());
        }
        let result = self.iface.open(self.pending_ifaces);
        let opened = match &result {
            Ok(mask) => *mask,
            Err(error) => error.opened(),
        };
        self.opened_ifaces = opened;
        self.pending_ifaces &= !self.opened_ifaces;
        if !(self.opened_ifaces & requested_interfaces(self.profile, &self.config)).is_empty() {
            if result.is_err() && !self.pending_ifaces.is_empty() {
                return Ok(());
            }
            return Ok(());
        }
        match result {
            Ok(_) => Err(-libc::ENODEV),
            Err(error) => Err(io_errno(error.source())),
        }
    }
    pub fn handle_watch(&mut self) -> Result<(), i32> {
        let opened = self.iface.opened();
        let lost = self.opened_ifaces & !opened;
        if !lost.is_empty() {
            self.gamepad.take();
            self.desktop.take();
            self.pointer.reset();
            self.aim.reset();
            self.recreate_outputs()?;
        }
        self.opened_ifaces = opened;
        self.pending_ifaces = requested_interfaces(self.profile, &self.config) & !opened;
        self.retry_open()
    }
    fn recreate_outputs(&mut self) -> Result<(), i32> {
        let (gamepad, desktop) = create_outputs(
            self.profile,
            &self.config,
            &self.backend,
            self.outputs_enabled,
        )?;
        self.gamepad = gamepad;
        self.desktop = desktop;
        Ok(())
    }
    pub fn drain(&mut self) -> Result<BridgeAction, i32> {
        self.drain_with(|_, _| {})
    }
    pub fn drain_with<F>(&mut self, mut observer: F) -> Result<BridgeAction, i32>
    where
        F: FnMut(&Path, &Event),
    {
        for _ in 0..MAX_EVENTS_PER_DRAIN {
            match self.iface.dispatch() {
                Ok(event) => {
                    self.trace(&event);
                    observer(&self.syspath, &event);
                    match self.handle_event(&event)? {
                        BridgeAction::Continue => {}
                        BridgeAction::Gone => return Ok(BridgeAction::Gone),
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return Ok(BridgeAction::Continue);
                }
                Err(e) => return Err(io_errno(&e)),
            }
        }
        Ok(BridgeAction::Continue)
    }
    fn trace(&mut self, event: &Event) {
        if let Some(trace) = self.trace.as_mut() {
            trace.emit(&self.syspath, event);
        }
    }
    pub fn handle_event(&mut self, event: &Event) -> Result<BridgeAction, i32> {
        match event.kind {
            EventKind::Gone => return Ok(BridgeAction::Gone),
            EventKind::Watch => {
                self.handle_watch()?;
            }
            EventKind::Key(key)
            | EventKind::NunchukKey(key)
            | EventKind::ClassicControllerKey(key)
            | EventKind::ProControllerKey(key)
            | EventKind::GuitarKey(key)
            | EventKind::DrumsKey(key) => {
                self.handle_key(key)?;
            }
            EventKind::Accel(value) => {
                self.handle_motion(MotionKind::Accel, &[value])?;
                let result = self.aim.process_accelerometer([value.x, value.y, value.z]);
                self.emit_aim(result)?;
            }
            EventKind::MotionPlus(value) => {
                self.handle_motion(MotionKind::MotionPlus, &[value])?;
                let result = self.aim.process_motion_plus([value.x, value.y, value.z]);
                self.emit_aim(result)?;
            }
            EventKind::NunchukMove(values) => {
                self.handle_motion(MotionKind::Nunchuk, &values)?;
            }
            EventKind::ClassicControllerMove(values) => {
                self.handle_motion(MotionKind::Classic, &values)?;
            }
            EventKind::ProControllerMove(values) => {
                self.handle_motion(MotionKind::Pro, &values)?;
            }
            EventKind::GuitarMove(values) => {
                self.handle_motion(MotionKind::Guitar, &values)?;
            }
            EventKind::DrumsMove(values) => {
                self.handle_motion(MotionKind::Drums, &values)?;
            }
            EventKind::BalanceBoard(values) => {
                self.handle_motion(MotionKind::Balance, &values)?;
            }
            EventKind::Ir(values) => {
                let mut frame = IrFrame::default();
                for (point, value) in frame.points.iter_mut().zip(values) {
                    *point = IrPoint {
                        valid: valid_ir_point(&value),
                        x: value.x,
                        y: value.y,
                    };
                }
                let point = self.pointer.select_ir(&frame);
                if has_desktop_profile(self.profile) {
                    let delta = self.pointer.update_ir_frame(&frame);
                    self.emit_pointer(delta.dx, delta.dy)?;
                }
                let result = self.aim.process_ir(point);
                self.emit_aim(result)?;
            }
            _ => {}
        }
        Ok(BridgeAction::Continue)
    }

    fn handle_key(&mut self, key: ButtonEvent) -> Result<(), i32> {
        let Some(state) = button_state(key.state) else {
            return Ok(());
        };
        let Some(code) = button_code(key.button) else {
            return Ok(());
        };
        if needs_gamepad(self.profile, &self.config)
            && self.outputs_enabled
            && let Some(mapped) = mapping::map_key(code)
            && let Some(out) = self.gamepad.as_mut()
        {
            out.emit_key(mapped, state)?;
        }
        if has_desktop_profile(self.profile) {
            self.desktop_key(code, state)?;
            self.pointer_key(code, state)?;
        }
        let result = self.aim.activation_key(code, state != 0);
        self.emit_aim(result)
    }

    fn handle_motion(&mut self, kind: MotionKind, values: &[Axis3]) -> Result<(), i32> {
        if !needs_gamepad(self.profile, &self.config) || !self.outputs_enabled {
            return Ok(());
        }
        let mut axes = [Abs3 { x: 0, y: 0, z: 0 }; 8];
        for (target, source) in axes.iter_mut().zip(values) {
            *target = Abs3 {
                x: source.x,
                y: source.y,
                z: source.z,
            };
        }
        let mapped = mapping::map_motion(kind, axes);
        if let Some(out) = self.gamepad.as_mut() {
            for axis in mapped.axes.iter().take(mapped.count) {
                out.emit_abs(axis.code, axis.value)?;
            }
            out.syn()?;
        }
        Ok(())
    }
    fn desktop_key(&mut self, code: u32, state: u32) -> Result<(), i32> {
        if !self.outputs_enabled {
            return Ok(());
        }
        let action = match code {
            4 => self.config.desktop_bindings.a,
            5 => self.config.desktop_bindings.b,
            6 => self.config.desktop_bindings.plus,
            7 => self.config.desktop_bindings.minus,
            8 => self.config.desktop_bindings.home,
            9 => self.config.desktop_bindings.one,
            10 => self.config.desktop_bindings.two,
            _ => wiiland_core::DesktopAction::Disabled,
        };
        let code = match action {
            wiiland_core::DesktopAction::LeftClick => 0x110,
            wiiland_core::DesktopAction::RightClick => 0x111,
            wiiland_core::DesktopAction::Enter => 28,
            wiiland_core::DesktopAction::Escape => 1,
            wiiland_core::DesktopAction::Overview => 125,
            wiiland_core::DesktopAction::PageUp => 104,
            wiiland_core::DesktopAction::PageDown => 109,
            wiiland_core::DesktopAction::Disabled => return Ok(()),
        };
        if let Some(out) = self.desktop.as_mut() {
            out.emit_key(code, state)
        } else {
            Ok(())
        }
    }
    fn pointer_key(&mut self, code: u32, state: u32) -> Result<(), i32> {
        let bit = match code {
            0 => POINTER_LEFT,
            1 => POINTER_RIGHT,
            2 => POINTER_UP,
            3 => POINTER_DOWN,
            _ => return Ok(()),
        };
        let d = self.pointer.update_key(bit, state != 0);
        self.emit_pointer(d.dx, d.dy)
    }
    fn emit_pointer(&mut self, dx: i32, dy: i32) -> Result<(), i32> {
        if !self.outputs_enabled {
            return Ok(());
        }
        if let Some(out) = self.desktop.as_mut() {
            out.emit_rel(0, dx)?;
            out.emit_rel(1, dy)?;
            if dx != 0 || dy != 0 {
                out.syn()?;
            }
        }
        Ok(())
    }
    fn emit_aim(&mut self, r: wiiland_core::aim::AimResult) -> Result<(), i32> {
        if !self.outputs_enabled {
            return Ok(());
        }
        let Some(v) = r.output else { return Ok(()) };
        match self.aim.config.output {
            wiiland_core::AimMode::Mouse => {
                if let Some(out) = self.desktop.as_mut() {
                    out.emit_rel(0, v.x)?;
                    out.emit_rel(1, v.y)?;
                    if v.x != 0 || v.y != 0 {
                        out.syn()?;
                    }
                }
            }
            wiiland_core::AimMode::RightStick => {
                if let Some(out) = self.gamepad.as_mut() {
                    out.emit_abs(mapping::ABS_RX, v.x.clamp(-32768, 32767))?;
                    out.emit_abs(mapping::ABS_RY, v.y.clamp(-32768, 32767))?;
                    out.syn()?;
                }
            }
            wiiland_core::AimMode::Off => {}
        }
        Ok(())
    }
    pub fn tick_pointer(&mut self) -> Result<(), i32> {
        let d = self.pointer.tick();
        self.emit_pointer(d.dx, d.dy)
    }
}
impl<B: Backend + Clone> Drop for BridgeDevice<B> {
    fn drop(&mut self) {
        self.gamepad.take();
        self.desktop.take();
    }
}
fn needs_gamepad(p: Profile, c: &Config) -> bool {
    p.contains(Profile::GAMEPAD) || c.aim_mode == wiiland_core::AimMode::RightStick
}
fn needs_desktop(p: Profile, c: &Config) -> bool {
    p.contains(Profile::DESKTOP) || c.aim_mode == wiiland_core::AimMode::Mouse
}
fn has_desktop_profile(p: Profile) -> bool {
    p.contains(Profile::DESKTOP)
}
fn profile_for_device(config: &Config, syspath: &Path, devtype: Option<&[u8]>) -> Profile {
    let syspath = syspath.to_string_lossy();
    let devtype = devtype
        .and_then(|value| std::str::from_utf8(value).ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    config.profile_for_device(Some(&syspath), devtype)
}
type OutputPair<B> = (Option<VirtualDevice<B>>, Option<VirtualDevice<B>>);
fn create_outputs<B: Backend + Clone>(
    profile: Profile,
    config: &Config,
    backend: &B,
    outputs_enabled: bool,
) -> Result<OutputPair<B>, i32> {
    if !outputs_enabled {
        return Ok((None, None));
    }
    let gamepad = if needs_gamepad(profile, config) {
        Some(VirtualDevice::with_backend(
            crate::uinput::UINPUT_PATH,
            VirtualKind::Controller,
            backend.clone(),
        )?)
    } else {
        None
    };
    let desktop = if needs_desktop(profile, config) {
        Some(VirtualDevice::with_backend(
            crate::uinput::UINPUT_PATH,
            VirtualKind::Desktop,
            backend.clone(),
        )?)
    } else {
        None
    };
    Ok((gamepad, desktop))
}
pub fn requested_interfaces(p: Profile, c: &Config) -> InterfaceMask {
    let mut interfaces = InterfaceMask::empty();
    if p.contains(Profile::GAMEPAD) {
        interfaces = InterfaceMask::ALL & !InterfaceMask::IR;
    }
    if p.contains(Profile::DESKTOP) {
        interfaces |= InterfaceMask::CORE | InterfaceMask::IR;
    }
    if c.aim_mode != wiiland_core::AimMode::Off {
        interfaces |= match c.aim_source {
            wiiland_core::AimSource::Ir => InterfaceMask::IR,
            wiiland_core::AimSource::MotionPlus => InterfaceMask::MOTION_PLUS,
            wiiland_core::AimSource::Accelerometer => InterfaceMask::ACCEL,
            wiiland_core::AimSource::Auto => {
                InterfaceMask::IR | InterfaceMask::MOTION_PLUS | InterfaceMask::ACCEL
            }
        };
        if matches!(
            c.aim_activation,
            wiiland_core::AimActivation::Z | wiiland_core::AimActivation::C
        ) {
            interfaces |= InterfaceMask::NUNCHUK;
        }
    }
    interfaces
}
fn valid_ir_point(abs: &Axis3) -> bool {
    abs.x != 1023 || abs.y != 1023
}
fn key_event(kind: EventKind) -> Option<(u32, u32)> {
    let key = match kind {
        EventKind::Key(key)
        | EventKind::NunchukKey(key)
        | EventKind::ClassicControllerKey(key)
        | EventKind::ProControllerKey(key)
        | EventKind::GuitarKey(key)
        | EventKind::DrumsKey(key) => key,
        _ => return None,
    };
    Some((button_code(key.button)?, button_state(key.state)?))
}

fn axis_events(kind: &EventKind) -> &[Axis3] {
    match kind {
        EventKind::Accel(value) | EventKind::MotionPlus(value) => std::slice::from_ref(value),
        EventKind::Ir(values) | EventKind::BalanceBoard(values) => values,
        EventKind::NunchukMove(values) | EventKind::ProControllerMove(values) => values,
        EventKind::ClassicControllerMove(values) | EventKind::GuitarMove(values) => values,
        EventKind::DrumsMove(values) => values,
        _ => &[],
    }
}

fn monotonic_time_us() -> i64 {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) } != 0 {
        return 0;
    }
    now.tv_sec
        .saturating_mul(1_000_000)
        .saturating_add(now.tv_nsec / 1_000)
}

fn format_trace_line(sequence: u64, monotonic_us: i64, syspath: &Path, event: &Event) -> String {
    let payload = if let Some((code, state)) = key_event(event.kind) {
        TracePayload::Key(KeyPayload { code, state })
    } else {
        let axes = axis_events(&event.kind);
        if axes.is_empty() {
            TracePayload::None
        } else {
            TracePayload::Axes(
                axes.iter()
                    .map(|value| AbsPayload {
                        x: value.x,
                        y: value.y,
                        z: value.z,
                    })
                    .collect(),
            )
        }
    };

    let mut line = TraceEvent::new(
        sequence,
        Some(monotonic_us),
        syspath.to_string_lossy(),
        event_type_code(event.kind),
        payload,
    )
    .format_line();
    if line.ends_with('\n') {
        line.pop();
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uinput::{RecordingBackend, RecordingOp};
    use std::cell::RefCell;
    use wiiland_core::{AimActivation, AimMode, AimSource, DeviceRule, DeviceRuleKind};

    #[test]
    fn button_states_preserve_linux_input_values() {
        assert_eq!(button_state(ButtonState::Released), Some(0));
        assert_eq!(button_state(ButtonState::Pressed), Some(1));
        assert_eq!(button_state(ButtonState::Repeated), Some(2));
    }

    #[test]
    fn disabled_outputs_remain_absent_when_recreated_after_watch() {
        let backend = RecordingBackend::new();
        let config = Config {
            profile: Profile::BOTH,
            ..Config::default()
        };

        let initial = create_outputs(config.profile, &config, &backend, false).unwrap();
        assert!(initial.0.is_none());
        assert!(initial.1.is_none());
        let refreshed = create_outputs(config.profile, &config, &backend, false).unwrap();
        assert!(refreshed.0.is_none());
        assert!(refreshed.1.is_none());
        assert!(backend.operations().is_empty());
    }

    #[test]
    fn enabled_outputs_use_the_recording_backend() {
        let backend = RecordingBackend::new();
        let config = Config {
            profile: Profile::BOTH,
            ..Config::default()
        };

        let outputs = create_outputs(config.profile, &config, &backend, true).unwrap();
        assert!(outputs.0.is_some());
        assert!(outputs.1.is_some());
        assert_eq!(
            backend
                .operations()
                .iter()
                .filter(|op| matches!(op, RecordingOp::Open(_)))
                .count(),
            2
        );
    }

    #[test]
    fn profile_selection_uses_syspath_and_trimmed_devtype() {
        let config = Config {
            profile: Profile::GAMEPAD,
            device_rules: vec![
                DeviceRule {
                    kind: DeviceRuleKind::Syspath,
                    match_text: "wii-red".into(),
                    profile: Profile::DESKTOP,
                },
                DeviceRule {
                    kind: DeviceRuleKind::Devtype,
                    match_text: "balanceboard".into(),
                    profile: Profile::BOTH,
                },
            ],
            ..Config::default()
        };

        assert_eq!(
            profile_for_device(
                &config,
                Path::new("/sys/devices/wii-red"),
                Some(b"balanceboard\n")
            ),
            Profile::BOTH
        );
    }

    #[test]
    fn requested_interfaces_include_nunchuk_for_z_and_c_activation() {
        for activation in [AimActivation::Z, AimActivation::C] {
            let config = Config {
                profile: Profile::DESKTOP,
                aim_mode: AimMode::Mouse,
                aim_source: AimSource::Accelerometer,
                aim_activation: activation,
                ..Config::default()
            };
            assert!(requested_interfaces(config.profile, &config).contains(InterfaceMask::NUNCHUK));
        }

        let config = Config {
            profile: Profile::DESKTOP,
            aim_mode: AimMode::Mouse,
            aim_source: AimSource::Accelerometer,
            aim_activation: AimActivation::B,
            ..Config::default()
        };

        assert!(!requested_interfaces(config.profile, &config).contains(InterfaceMask::NUNCHUK));
    }

    #[test]
    fn gamepad_forwarding_predicate_includes_right_stick_aim_output() {
        let config = Config {
            profile: Profile::DESKTOP,
            aim_mode: AimMode::RightStick,
            ..Config::default()
        };
        assert!(needs_gamepad(config.profile, &config));
        assert!(!config.profile.contains(Profile::GAMEPAD));
    }

    #[test]
    fn desktop_pointer_controls_require_the_desktop_profile_not_only_an_aim_output() {
        let config = Config {
            profile: Profile::GAMEPAD,
            aim_mode: AimMode::Mouse,
            ..Config::default()
        };
        assert!(needs_desktop(config.profile, &config));
        assert!(!has_desktop_profile(config.profile));
    }

    #[test]
    fn trace_lines_use_emission_time_and_include_name_type_and_key_payload() {
        let event = Event {
            time: wiiland_hid::Timestamp {
                seconds: 99,
                microseconds: 999,
            },
            kind: EventKind::NunchukKey(ButtonEvent {
                button: Button::Z,
                state: ButtonState::Pressed,
            }),
        };

        assert_eq!(
            format_trace_line(7, 12_000_034, Path::new("/sys/wii0"), &event),
            "time=12.000034 seq=7 /sys/wii0 nunchuk-key type=10 key=20 state=1"
        );
    }

    #[test]
    fn trace_lines_include_all_eight_absolute_payloads() {
        let event = Event {
            time: wiiland_hid::Timestamp {
                seconds: 0,
                microseconds: 0,
            },
            kind: EventKind::DrumsMove(std::array::from_fn(|i| Axis3 {
                x: i as i32,
                y: -(i as i32),
                z: (i * 10) as i32,
            })),
        };

        assert_eq!(
            format_trace_line(8, 1_000_002, Path::new("/sys/wii1"), &event),
            concat!(
                "time=1.000002 seq=8 /sys/wii1 drums-move type=13",
                " abs0=0,0,0 abs1=1,-1,10 abs2=2,-2,20 abs3=3,-3,30",
                " abs4=4,-4,40 abs5=5,-5,50 abs6=6,-6,60 abs7=7,-7,70"
            )
        );
    }

    #[test]
    fn trace_contexts_share_sequence_and_timestamp_lifecycle_events_at_emission() {
        let sequence = Rc::new(Cell::new(0));
        let now = Rc::new(Cell::new(0));
        let lines = Rc::new(RefCell::new(Vec::new()));

        let mut first = TraceContext::with_clock(
            TraceFilter::All,
            Rc::clone(&sequence),
            {
                let now = Rc::clone(&now);
                move || now.get()
            },
            {
                let lines = Rc::clone(&lines);
                move |line| lines.borrow_mut().push(line.to_owned())
            },
        );
        let mut second = TraceContext::with_clock(
            TraceFilter::All,
            Rc::clone(&sequence),
            {
                let now = Rc::clone(&now);
                move || now.get()
            },
            {
                let lines = Rc::clone(&lines);
                move |line| lines.borrow_mut().push(line.to_owned())
            },
        );
        let watch = Event {
            time: wiiland_hid::Timestamp {
                seconds: 0,
                microseconds: 0,
            },
            kind: EventKind::Watch,
        };
        let gone = Event {
            time: wiiland_hid::Timestamp {
                seconds: 0,
                microseconds: 0,
            },
            kind: EventKind::Gone,
        };

        now.set(41_000_007);
        first.emit(Path::new("/sys/wii0"), &watch);
        now.set(42_000_008);
        second.emit(Path::new("/sys/wii1"), &gone);

        assert_eq!(sequence.get(), 2);
        assert_eq!(
            &*lines.borrow(),
            &[
                "time=41.000007 seq=1 /sys/wii0 watch type=7",
                "time=42.000008 seq=2 /sys/wii1 gone type=16",
            ]
        );
    }
}
