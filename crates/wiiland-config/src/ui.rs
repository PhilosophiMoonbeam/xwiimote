use std::path::PathBuf;
use std::time::Duration;

use eframe::egui;
use tempfile::NamedTempFile;
use wiiland_core::{
    AimActivation, AimMode, AimSource, DesktopAction, DeviceRuleKind, IrAimMapping, IrTracking,
    Profile, SensorCalibration,
};

use crate::model::{self, ApplyCompletion, CalibrationTransaction, ConfigModel, TransactionKind};
use crate::process::{self, ProcessEvent, ProcessResult, ProcessTask};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Tab {
    Overview,
    Configuration,
    Validation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ValidationKind {
    Trace,
    Calibration,
}

struct ConfigTask {
    transaction: model::Transaction,
    process: ProcessTask,
    _temporary: Option<NamedTempFile>,
    restart_after_save: bool,
}
struct ValidationTask {
    kind: ValidationKind,
    process: ProcessTask,
    calibration: Option<CalibrationOwnership>,
}

struct CalibrationOwnership {
    transaction: CalibrationTransaction,
    device: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ServiceAction {
    Restart,
}

impl ServiceAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Restart => "restart",
        }
    }
}

#[derive(Default)]
struct PendingServiceActions {
    pending: Option<ServiceAction>,
}

impl PendingServiceActions {
    fn request_restart(&mut self, service_active: bool) -> Option<ServiceAction> {
        if service_active {
            self.pending = Some(ServiceAction::Restart);
            None
        } else {
            self.pending = None;
            Some(ServiceAction::Restart)
        }
    }

    fn after_completion(&mut self) -> Option<ServiceAction> {
        self.pending.take()
    }
}

pub struct ControlCenter {
    pub model: ConfigModel,
    config_task: Option<ConfigTask>,
    command_task: Option<(String, ProcessTask)>,
    service_task: Option<(String, ProcessTask)>,
    pending_service_actions: PendingServiceActions,
    service_program: &'static str,
    validation_task: Option<ValidationTask>,
    tab: Tab,
    service_status: String,
    status: String,
    close_confirmation: bool,
    trace_device: String,
    trace_filter: String,
    trace_profile: Option<Profile>,
}

impl ControlCenter {
    pub fn new(model: ConfigModel) -> Self {
        Self {
            model,
            config_task: None,
            command_task: None,
            service_task: None,
            pending_service_actions: PendingServiceActions::default(),
            service_program: "systemctl",
            validation_task: None,
            tab: Tab::Overview,
            service_status: "Checking…".to_owned(),
            status: "Ready".to_owned(),
            close_confirmation: false,
            trace_device: String::new(),
            trace_filter: "all".to_owned(),
            trace_profile: None,
        }
    }

    pub fn initialize(model: ConfigModel) -> Self {
        let mut application = Self::new(model);
        application.begin_load(false);
        application.service_action("is-active");
        application
    }

    pub fn backend_name() -> &'static str {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            "Wayland"
        } else if std::env::var_os("DISPLAY").is_some() {
            "X11"
        } else {
            "unknown"
        }
    }

    pub fn begin_load(&mut self, report_errors: bool) {
        if self.config_task.is_some() {
            return;
        }
        let target = self.model.config_path.clone();
        let mut args = vec!["--dump-config".to_owned()];
        args = process::configured_args(&target, ConfigModel::default_path().as_deref(), args);
        let Some(transaction) = self.model.begin(TransactionKind::Load, Vec::new()) else {
            return;
        };
        let command = self.model.daemon_program().to_owned();
        self.model
            .append_output(&format!("$ {} {}\n", command, shell_args(&args)));
        self.config_task = Some(ConfigTask {
            transaction,
            process: ProcessTask::spawn_capturing_stdout(command, &args),
            _temporary: None,
            restart_after_save: false,
        });
        self.status = "Loading effective configuration".to_owned();
        if report_errors {
            self.status = "Loading effective configuration".to_owned();
        }
    }

    fn begin_save(&mut self, restart: bool) {
        if self.config_task.is_some() || self.model.validate_form().is_err() {
            self.status = "Configuration has validation errors".to_owned();
            return;
        }
        let target = self.model.config_path.clone();
        if target.as_os_str().is_empty() {
            self.status = "Choose a configuration target".to_owned();
            return;
        }
        let bytes = self.model.render();
        let parent = target
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        if let Err(error) = std::fs::create_dir_all(parent) {
            self.status = format!("Cannot prepare config validation: {error}");
            return;
        }
        let mut temporary = match NamedTempFile::new_in(parent) {
            Ok(file) => file,
            Err(error) => {
                self.status = format!("Cannot prepare config validation: {error}");
                return;
            }
        };
        if let Err(error) = std::io::Write::write_all(&mut temporary, &bytes)
            .and_then(|_| temporary.as_file().sync_all())
        {
            self.status = format!("Cannot prepare config validation: {error}");
            return;
        }
        let args = vec![
            "--check-config".to_owned(),
            "--config".to_owned(),
            temporary.path().to_string_lossy().into_owned(),
        ];
        let Some(transaction) = self.model.begin(TransactionKind::Save, bytes) else {
            return;
        };
        let command = self.model.daemon_program().to_owned();
        self.model
            .append_output(&format!("$ {} {}\n", command, shell_args(&args)));
        self.config_task = Some(ConfigTask {
            transaction,
            process: ProcessTask::spawn(command, &args),
            _temporary: Some(temporary),
            restart_after_save: restart,
        });
        self.status = if restart {
            "Validating configuration before save (restart requested)".to_owned()
        } else {
            "Validating configuration before save".to_owned()
        };
    }
    fn poll_config_task(&mut self) {
        let Some(task) = self.config_task.as_ref() else {
            return;
        };
        let result = match poll_process(&task.process, &mut self.model) {
            Some(result) => result,
            None => return,
        };
        let task = self.config_task.take().expect("task exists");
        let restart = task.restart_after_save
            && task.transaction.kind == TransactionKind::Save
            && !self.model.is_explicit_target(&task.transaction.target);
        self.append_result_error(&result);
        let completion = process::transaction_completion(&task.transaction, result);
        let outcome = self.model.finish(&completion);
        match outcome {
            ApplyCompletion::Applied => {
                self.status = match task.transaction.kind {
                    TransactionKind::Load => "Configuration loaded".to_owned(),
                    TransactionKind::Save => "Configuration saved".to_owned(),
                }
            }
            ApplyCompletion::Stale => {
                self.status = "Completion discarded because the form or target changed".to_owned()
            }
            ApplyCompletion::Failed => {
                self.status =
                    "Configuration operation failed; existing data was preserved".to_owned()
            }
        }
        if restart && outcome == ApplyCompletion::Applied {
            self.request_restart_after_save();
        }
    }

    fn append_result_error(&mut self, result: &ProcessResult) {
        if let Some(error) = &result.error {
            self.model
                .append_output(&format!("process error: {error}\n"));
        }
    }

    fn run_command(&mut self, args: Vec<String>, config_sensitive: bool) {
        if self.command_task.is_some() {
            return;
        }
        let args = if config_sensitive {
            process::configured_args(
                &self.model.config_path,
                ConfigModel::default_path().as_deref(),
                args,
            )
        } else {
            args
        };
        let command = self.model.daemon_program().to_owned();
        self.model
            .append_output(&format!("$ {} {}\n", command, shell_args(&args)));
        self.command_task = Some((command.clone(), ProcessTask::spawn(command, &args)));
        self.status = "Command running".to_owned();
    }

    fn poll_command(&mut self) {
        let Some((_, task)) = self.command_task.as_ref() else {
            return;
        };
        let result = match poll_process(task, &mut self.model) {
            Some(result) => result,
            None => return,
        };
        self.command_task = None;
        self.append_result_error(&result);
        self.status = if result.success {
            "Command succeeded".to_owned()
        } else {
            format!(
                "Command failed (exit {})",
                result
                    .code
                    .map_or_else(|| "unknown".to_owned(), |code| code.to_string())
            )
        };
    }

    fn service_action(&mut self, action: &str) {
        if self.service_task.is_some() {
            return;
        }
        self.dispatch_service_action(action);
    }

    fn request_restart_after_save(&mut self) {
        let active = self.service_task.is_some();
        if let Some(action) = self.pending_service_actions.request_restart(active) {
            self.dispatch_service_action(action.as_str());
        } else {
            self.service_status = "Restart queued…".to_owned();
        }
    }

    fn dispatch_service_action(&mut self, action: &str) {
        let args = process::service_args(action);
        self.model.append_output(&format!(
            "$ {} {}\n",
            self.service_program,
            shell_args(&args)
        ));
        self.service_status = format!("{}…", capitalize(action));
        self.service_task = Some((
            action.to_owned(),
            ProcessTask::spawn(self.service_program, &args),
        ));
    }

    fn poll_service(&mut self) {
        let Some((action, task)) = self.service_task.as_ref() else {
            return;
        };
        let result = match poll_process(task, &mut self.model) {
            Some(result) => result,
            None => return,
        };
        let action = action.clone();
        self.service_task = None;
        self.append_result_error(&result);
        if result.success {
            self.service_status = "Action complete".to_owned();
            self.status = format!("Service {action} succeeded");
        } else {
            self.service_status = "Unavailable".to_owned();
            self.status = format!("Service {action} failed");
        }
        if let Some(next) = self.pending_service_actions.after_completion() {
            self.dispatch_service_action(next.as_str());
        }
    }

    fn start_trace(&mut self) {
        if self.validation_task.is_some() {
            self.status = "Another live validation task is already running".to_owned();
            return;
        }
        let mut args = vec![
            "--dry-run".to_owned(),
            format!("--trace-events={}", self.trace_filter),
            "--verbose".to_owned(),
        ];
        if let Some(profile) = self.trace_profile {
            args.extend([
                "--profile".to_owned(),
                profile.as_str().unwrap_or("gamepad").to_owned(),
            ]);
        }
        if !self.trace_device.trim().is_empty() {
            args.extend(["--device".to_owned(), self.trace_device.trim().to_owned()]);
        }
        args = process::configured_args(
            &self.model.config_path,
            ConfigModel::default_path().as_deref(),
            args,
        );
        let command = self.model.daemon_program().to_owned();
        self.model
            .append_output(&format!("$ {} {}\n", command, shell_args(&args)));
        self.validation_task = Some(ValidationTask {
            kind: ValidationKind::Trace,
            process: ProcessTask::spawn(command, &args),
            calibration: None,
        });
        self.status = "Trace running".to_owned();
    }

    fn start_calibration(&mut self) {
        if self.validation_task.is_some() {
            self.status = "Wait for the current trace or calibration capture to finish".to_owned();
            return;
        }
        let device = self.trace_device.trim().to_owned();
        let Some(transaction) = self.model.begin_calibration() else {
            self.status = "Another calibration transaction is already running".to_owned();
            return;
        };
        let mut args = vec![
            "--calibrate-aim".to_owned(),
            "--aim-calibration-duration".to_owned(),
            transaction.captured.aim_calibration_duration.to_string(),
        ];
        if !device.is_empty() {
            args.extend(["--device".to_owned(), device.clone()]);
        }
        args = process::configured_args(
            &transaction.target,
            ConfigModel::default_path().as_deref(),
            args,
        );
        let command = transaction.daemon_program.clone();
        self.model
            .append_output(&format!("$ {} {}\n", command, shell_args(&args)));
        self.validation_task = Some(ValidationTask {
            kind: ValidationKind::Calibration,
            process: ProcessTask::spawn_capturing_stdout(command, &args),
            calibration: Some(CalibrationOwnership {
                transaction,
                device,
            }),
        });
        self.status = "Calibration running".to_owned();
    }

    fn poll_validation(&mut self) {
        let Some(task) = self.validation_task.as_ref() else {
            return;
        };
        let result = match poll_process(&task.process, &mut self.model) {
            Some(result) => result,
            None => return,
        };
        let task = self.validation_task.take().expect("task exists");
        self.append_result_error(&result);
        match task.kind {
            ValidationKind::Trace => {
                self.status = if result.success {
                    "Trace stopped".to_owned()
                } else {
                    "Trace failed".to_owned()
                };
            }
            ValidationKind::Calibration => {
                let ownership = task
                    .calibration
                    .expect("calibration task carries captured ownership");
                self.complete_calibration(ownership, result.success, &result.stdout);
            }
        }
    }

    fn complete_calibration(
        &mut self,
        ownership: CalibrationOwnership,
        success: bool,
        bytes: &[u8],
    ) {
        let model_owned = self.model.finish_calibration(&ownership.transaction);
        let device_owned = self.trace_device.trim() == ownership.device.as_str();
        if !success {
            self.status = "Calibration failed".to_owned();
        } else if !model_owned || !device_owned {
            self.status =
                "Calibration discarded because the form, target, or capture changed".to_owned();
        } else if self.apply_calibration(bytes) {
            self.status = "Calibration values applied; save to persist them".to_owned();
        } else {
            self.status = "Calibration succeeded".to_owned();
        }
    }

    fn apply_calibration(&mut self, bytes: &[u8]) -> bool {
        let Ok(text) = std::str::from_utf8(bytes) else {
            return false;
        };
        let mut next = self.model.config.clone();
        let mut changed = false;
        for line in text.lines() {
            let line = line.trim();
            if (line.starts_with("aim-accel-zero-")
                || line.starts_with("aim-motion-plus-bias-")
                || line.starts_with("aim-calibration-duration="))
                && next.apply_line("calibration-output", 1, line).is_ok()
            {
                changed = true;
            }
        }
        if changed {
            self.model.config = next;
            self.model.mark_dirty();
        }
        changed
    }

    fn draw_overview(&mut self, ui: &mut egui::Ui) {
        ui.heading("Overview");
        ui.label(
            "Configure Wii Remote input, inspect daemon readiness, and capture validation output.",
        );
        egui::Grid::new("paths")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Daemon executable");
                ui.text_edit_singleline(&mut self.model.daemon_path);
                ui.end_row();
                ui.label("Configuration file");
                let mut path = self.model.config_path.to_string_lossy().into_owned();
                if ui.text_edit_singleline(&mut path).changed() {
                    self.model.set_path(PathBuf::from(path));
                }
                ui.end_row();
                ui.label("Configuration scope");
                ui.label(if self.model.is_explicit_path() {
                    "Custom file only — the background service does not load it."
                } else {
                    "Layered defaults — built-in values, system settings, then this user file."
                });
                ui.end_row();
                ui.label("Window system");
                ui.label(Self::backend_name());
                ui.end_row();
            });
        ui.horizontal(|ui| {
            if ui.button("Reload from daemon").clicked() {
                self.begin_load(true);
            }
            if ui.button("Refresh service").clicked() {
                self.service_action("is-active");
            }
            if ui.button("Start").clicked() {
                self.service_action("start");
            }
            if ui.button("Stop").clicked() {
                self.service_action("stop");
            }
            if ui.button("Restart").clicked() {
                self.service_action("restart");
            }
        });
        ui.separator();
        ui.heading("Diagnostics and reference");
        ui.horizontal_wrapped(|ui| {
            if ui.button("Check readiness").clicked() {
                self.run_command(vec!["--doctor".to_owned()], true);
            }
            if ui.button("Validate configuration").clicked() {
                self.run_command(vec!["--check-config".to_owned()], true);
            }
            if ui.button("Show effective config").clicked() {
                self.run_command(vec!["--dump-config".to_owned()], true);
            }
            if ui.button("Find devices").clicked() {
                self.run_command(vec!["--list".to_owned(), "--verbose".to_owned()], false);
            }
            if ui.button("View input map").clicked() {
                self.run_command(vec!["--axis-map".to_owned()], false);
            }
            if ui.button("View test checklist").clicked() {
                self.run_command(vec!["--validation-checklist".to_owned()], false);
            }
        });
        ui.label(format!("Service: {}", self.service_status));
    }

    fn draw_configuration(&mut self, ui: &mut egui::Ui) {
        ui.heading("Configuration");
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.columns(2, |columns| {
                    draw_profile(&mut columns[0], &mut self.model);
                    draw_aim(&mut columns[1], &mut self.model);
                });
                draw_bindings(ui, &mut self.model);
                draw_rules(ui, &mut self.model);
            });
        ui.horizontal(|ui| {
            let busy = self.config_task.is_some();
            if ui
                .add_enabled(!busy, egui::Button::new("Reload from daemon"))
                .clicked()
            {
                self.begin_load(true);
            }
            if ui
                .add_enabled(!busy, egui::Button::new("Validate and save"))
                .clicked()
            {
                self.begin_save(false);
            }
            let restart_enabled = !busy && !self.model.is_explicit_path();
            if ui
                .add_enabled(
                    restart_enabled,
                    egui::Button::new("Save and restart daemon"),
                )
                .clicked()
            {
                self.begin_save(true);
            }
        });
    }

    fn draw_validation(&mut self, ui: &mut egui::Ui) {
        ui.heading("Validation");
        ui.label("Choose one remote for focused output, or leave the device field blank to observe every connected remote.");
        egui::Grid::new("validation").num_columns(2).show(ui, |ui| {
            ui.label("Device");
            ui.text_edit_singleline(&mut self.trace_device);
            ui.end_row();
            ui.label("Event filter");
            combo_token(
                ui,
                "trace-filter",
                &mut self.trace_filter,
                &[
                    ("all", "All events"),
                    ("keys", "Buttons"),
                    ("axes", "Axes"),
                    ("ir", "IR sensor"),
                    ("motion-plus", "MotionPlus"),
                ],
            );
            ui.end_row();
            ui.label("Temporary profile");
            let mut token = self
                .trace_profile
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_owned();
            combo_token(
                ui,
                "trace-profile",
                &mut token,
                &[
                    ("", "Use effective configuration"),
                    ("gamepad", "Temporarily use gamepad"),
                    ("desktop", "Temporarily use desktop"),
                    ("both", "Temporarily use both"),
                ],
            );
            self.trace_profile = Profile::parse(&token);
            ui.end_row();
        });
        let active = self.validation_task.is_some();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!active, egui::Button::new("Start trace"))
                .clicked()
            {
                self.start_trace();
            }
            if ui.add_enabled(active, egui::Button::new("Stop")).clicked() {
                if let Some(task) = &self.validation_task {
                    task.process.terminate();
                }
                self.status = "Stopping trace".to_owned();
            }
            if ui
                .add_enabled(
                    !active,
                    egui::Button::new("Capture flat-surface calibration"),
                )
                .clicked()
            {
                self.start_calibration();
            }
        });
        ui.label("Suggested coverage: original Wii Remote, MotionPlus, Nunchuk, Classic Controller, Wii U Pro Controller, Guitar, Drums, Balance Board, SDL, Wine/Proton, and desktop behavior on Wayland and X11.");
    }

    fn draw_output(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("Command output");
            if ui.button("Copy all").clicked() {
                ui.output_mut(|o| {
                    o.commands.push(egui::output::OutputCommand::CopyText(
                        self.model.output.as_text(),
                    ))
                });
            }
            if ui.button("Clear").clicked() {
                self.model.clear_output();
            }
        });
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.monospace(self.model.output.as_text());
            });
    }
}

impl eframe::App for ControlCenter {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_config_task();
        self.poll_command();
        self.poll_service();
        self.poll_validation();
        if self.model.dirty && ctx.input(|input| input.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_confirmation = true;
        }
        egui::TopBottomPanel::top("title").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("WiiLand Control Center");
                if self.model.dirty {
                    ui.label("(unsaved)");
                }
                ui.separator();
                ui.label(&self.status);
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (tab, name) in [
                    (Tab::Overview, "Overview"),
                    (Tab::Configuration, "Configuration"),
                    (Tab::Validation, "Validation"),
                ] {
                    if ui.selectable_label(self.tab == tab, name).clicked() {
                        self.tab = tab;
                    }
                }
            });
            ui.separator();
            match self.tab {
                Tab::Overview => self.draw_overview(ui),
                Tab::Configuration => self.draw_configuration(ui),
                Tab::Validation => self.draw_validation(ui),
            }
            self.draw_output(ui);
        });
        if self.close_confirmation {
            egui::Window::new("Unsaved configuration")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Your configuration changes have not been saved.");
                    ui.horizontal(|ui| {
                        if ui.button("Discard changes").clicked() {
                            self.close_confirmation = false;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button("Keep editing").clicked() {
                            self.close_confirmation = false;
                        }
                    });
                });
        }
        ctx.request_repaint_after(Duration::from_millis(40));
    }
}

fn draw_profile(ui: &mut egui::Ui, model: &mut ConfigModel) {
    ui.group(|ui| {
        ui.heading("Profiles and pointer feel");
        if combo_profile(ui, "Default profile", &mut model.config.profile) {
            model.mark_dirty();
        }
        if drag(
            ui,
            "D-pad pointer speed",
            &mut model.config.pointer_speed,
            1,
            127,
        ) {
            model.mark_dirty();
        }
        if drag(ui, "IR pointer gain", &mut model.config.ir_speed, 1, 127) {
            model.mark_dirty();
        }
        if drag(
            ui,
            "IR jitter deadzone",
            &mut model.config.ir_deadzone,
            0,
            127,
        ) {
            model.mark_dirty();
        }
        if drag(ui, "IR smoothing %", &mut model.config.ir_smoothing, 0, 95) {
            model.mark_dirty();
        }
        if combo_enum(
            ui,
            "IR tracking",
            &mut model.config.ir_tracking,
            [
                (IrTracking::Dual, "Sensor-bar pair"),
                (IrTracking::Centroid, "Visible-point centroid"),
                (IrTracking::First, "First visible point"),
            ],
        ) {
            model.mark_dirty();
        }
        if combo_enum(
            ui,
            "IR aim mapping",
            &mut model.config.ir_aim_mapping,
            [
                (IrAimMapping::Relative, "Relative movement"),
                (IrAimMapping::Absolute, "Absolute screen position"),
            ],
        ) {
            model.mark_dirty();
        }
        let mut enabled = model.config.ir_screen.is_some();
        if ui
            .checkbox(&mut enabled, "Use screen calibration")
            .changed()
        {
            model.config.ir_screen = enabled.then(model::ir_rect_default);
            model.mark_dirty();
        }
        let mut rect = model
            .config
            .ir_screen
            .unwrap_or_else(model::ir_rect_default);
        for (name, value) in [
            ("IR screen left", &mut rect.left),
            ("IR screen right", &mut rect.right),
            ("IR screen top", &mut rect.top),
            ("IR screen bottom", &mut rect.bottom),
        ] {
            if enabled && drag(ui, name, value, 0, 32767) {
                model.mark_dirty();
            }
        }
        if enabled {
            model.config.ir_screen = Some(rect);
        }
    });
}

fn draw_aim(ui: &mut egui::Ui, model: &mut ConfigModel) {
    ui.group(|ui| {
        ui.heading("Modern motion aiming");
        if combo_enum(
            ui,
            "Output",
            &mut model.config.aim_mode,
            [
                (AimMode::Off, "Off"),
                (AimMode::RightStick, "Right stick"),
                (AimMode::Mouse, "Mouse pointer"),
            ],
        ) {
            model.mark_dirty();
        }
        if combo_enum(
            ui,
            "Best available sensor",
            &mut model.config.aim_source,
            [
                (AimSource::Auto, "Automatic"),
                (AimSource::Ir, "IR sensor"),
                (AimSource::MotionPlus, "MotionPlus"),
                (AimSource::Accelerometer, "Accelerometer"),
            ],
        ) {
            model.mark_dirty();
        }
        if combo_enum(
            ui,
            "Activation",
            &mut model.config.aim_activation,
            [
                (AimActivation::B, "B button"),
                (AimActivation::Always, "Always active"),
                (AimActivation::Z, "Nunchuk Z"),
                (AimActivation::C, "Nunchuk C"),
            ],
        ) {
            model.mark_dirty();
        }
        if drag(ui, "Sensitivity", &mut model.config.aim_sensitivity, 1, 127) {
            model.mark_dirty();
        }
        if drag(ui, "Deadzone", &mut model.config.aim_deadzone, 0, 32767) {
            model.mark_dirty();
        }
        if drag(ui, "Smoothing %", &mut model.config.aim_smoothing, 0, 95) {
            model.mark_dirty();
        }
        if ui
            .checkbox(&mut model.config.aim_invert_x, "Invert X")
            .changed()
        {
            model.mark_dirty();
        }
        if ui
            .checkbox(&mut model.config.aim_invert_y, "Invert Y")
            .changed()
        {
            model.mark_dirty();
        }
        let mut accel = model.config.aim_accel_zero.is_some();
        if ui
            .checkbox(&mut accel, "Use accelerometer calibration")
            .changed()
        {
            model.config.aim_accel_zero = accel.then(model::calibration_default);
            model.mark_dirty();
        }
        let mut motion = model.config.aim_motion_plus_bias.is_some();
        if ui
            .checkbox(&mut motion, "Use MotionPlus calibration")
            .changed()
        {
            model.config.aim_motion_plus_bias = motion.then(model::calibration_default);
            model.mark_dirty();
        }
        if drag(
            ui,
            "Calibration duration",
            &mut model.config.aim_calibration_duration,
            1,
            30,
        ) {
            model.mark_dirty();
        }
        let changed = model
            .config
            .aim_accel_zero
            .as_mut()
            .map(|cal| calibration_fields(ui, "Accelerometer zero", cal))
            .unwrap_or(false);
        if changed {
            model.mark_dirty();
        }
        let changed = model
            .config
            .aim_motion_plus_bias
            .as_mut()
            .map(|cal| calibration_fields(ui, "MotionPlus bias", cal))
            .unwrap_or(false);
        if changed {
            model.mark_dirty();
        }
    });
}

fn calibration_fields(ui: &mut egui::Ui, label: &str, cal: &mut SensorCalibration) -> bool {
    let mut changed = false;
    for (axis, value) in [("X", &mut cal.x), ("Y", &mut cal.y), ("Z", &mut cal.z)] {
        if ui
            .add(
                egui::DragValue::new(value)
                    .range(-32768..=32767)
                    .prefix(format!("{label} {axis}: ")),
            )
            .changed()
        {
            changed = true;
        }
    }
    changed
}

fn draw_bindings(ui: &mut egui::Ui, model: &mut ConfigModel) {
    ui.group(|ui| {
        ui.heading("Desktop button bindings");
        for (name, label) in model::binding_names() {
            let mut value = match name {
                "a" => model.config.desktop_bindings.a,
                "b" => model.config.desktop_bindings.b,
                "plus" => model.config.desktop_bindings.plus,
                "minus" => model.config.desktop_bindings.minus,
                "home" => model.config.desktop_bindings.home,
                "one" => model.config.desktop_bindings.one,
                _ => model.config.desktop_bindings.two,
            };
            let changed = combo_action(ui, label, &mut value);
            match name {
                "a" => model.config.desktop_bindings.a = value,
                "b" => model.config.desktop_bindings.b = value,
                "plus" => model.config.desktop_bindings.plus = value,
                "minus" => model.config.desktop_bindings.minus = value,
                "home" => model.config.desktop_bindings.home = value,
                "one" => model.config.desktop_bindings.one = value,
                _ => model.config.desktop_bindings.two = value,
            }
            if changed {
                model.mark_dirty();
            }
        }
    });
}

fn draw_rules(ui: &mut egui::Ui, model: &mut ConfigModel) {
    ui.group(|ui| {
        ui.heading("Per-device profile rules");
        if ui.button("Add rule").clicked()
            && model.config.device_rules.len() < model::MAX_DEVICE_RULES
        {
            model.config.device_rules.push(model::rule(
                DeviceRuleKind::Devtype,
                String::new(),
                Profile::GAMEPAD,
            ));
            model.mark_dirty();
        }
        let mut remove = None;
        for index in 0..model.config.device_rules.len() {
            let mut rule = model.config.device_rules[index].clone();
            let mut changed = false;
            ui.horizontal(|ui| {
                if combo_enum(
                    ui,
                    "rule-kind",
                    &mut rule.kind,
                    [
                        (DeviceRuleKind::Syspath, "Device path"),
                        (DeviceRuleKind::Devtype, "Device type"),
                    ],
                ) {
                    changed = true;
                }
                if ui.text_edit_singleline(&mut rule.match_text).changed() {
                    changed = true;
                }
                if combo_profile(ui, "rule-profile", &mut rule.profile) {
                    changed = true;
                }
                if ui.small_button("Remove").clicked() {
                    remove = Some(index);
                }
            });
            if changed {
                model.config.device_rules[index] = rule;
                model.mark_dirty();
            }
        }
        if let Some(index) = remove {
            model.config.device_rules.remove(index);
            model.mark_dirty();
        }
    });
}

fn drag(ui: &mut egui::Ui, label: &str, value: &mut i32, min: i32, max: i32) -> bool {
    ui.add(
        egui::DragValue::new(value)
            .range(min..=max)
            .prefix(format!("{label}: ")),
    )
    .changed()
}

fn combo_profile(ui: &mut egui::Ui, label: &str, value: &mut Profile) -> bool {
    let before = *value;
    egui::ComboBox::from_id_salt(("profile", label))
        .selected_text(value.as_str().unwrap_or("unknown"))
        .show_ui(ui, |ui| {
            for (candidate, text) in model::profile_choices() {
                ui.selectable_value(value, candidate, text);
            }
        });
    *value != before
}

fn combo_action(ui: &mut egui::Ui, label: &str, value: &mut DesktopAction) -> bool {
    let before = *value;
    egui::ComboBox::from_id_salt(("action", label))
        .selected_text(value.as_str())
        .show_ui(ui, |ui| {
            for (candidate, text) in model::desktop_actions() {
                ui.selectable_value(value, candidate, text);
            }
        });
    *value != before
}

fn combo_enum<T: Copy + Eq, const N: usize>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    choices: [(T, &str); N],
) -> bool {
    let before = *value;
    let selected = choices
        .iter()
        .find(|(candidate, _)| *candidate == *value)
        .map_or("", |(_, text)| *text);
    egui::ComboBox::from_id_salt(("enum", label))
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (candidate, text) in &choices {
                ui.selectable_value(value, *candidate, *text);
            }
        });
    *value != before
}
fn combo_token(ui: &mut egui::Ui, id: &str, value: &mut String, choices: &[(&str, &str)]) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(if value.is_empty() {
            choices[0].1
        } else {
            choices
                .iter()
                .find(|(token, _)| *token == value)
                .map_or(value.as_str(), |(_, text)| *text)
        })
        .show_ui(ui, |ui| {
            for (token, text) in choices {
                ui.selectable_value(value, (*token).to_owned(), *text);
            }
        });
}
fn poll_process(task: &ProcessTask, model: &mut ConfigModel) -> Option<ProcessResult> {
    loop {
        match task.try_recv() {
            Ok(Some(ProcessEvent::Stdout(bytes) | ProcessEvent::Stderr(bytes))) => {
                model.append_output(&String::from_utf8_lossy(&bytes));
            }
            Ok(Some(ProcessEvent::Finished(result))) => return Some(result),
            Ok(None) => return None,
            Err(_) => {
                return Some(ProcessResult::unavailable("process result channel closed"));
            }
        }
    }
}

fn shell_args(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if arg.contains(char::is_whitespace) {
                format!("'{}'", arg.replace('\'', "'\\''"))
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + chars.as_str()
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::thread;
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn initialization_dispatches_load_and_service_status_together() {
        let mut model = ConfigModel::new(PathBuf::from("/tmp/wiiland-initialization-test.conf"));
        model.daemon_path = "/bin/true".to_owned();
        let application = ControlCenter::initialize(model);

        assert!(application.config_task.is_some());
        assert_eq!(
            application
                .model
                .transaction
                .as_ref()
                .map(|transaction| transaction.kind),
            Some(TransactionKind::Load)
        );
        assert_eq!(
            application
                .service_task
                .as_ref()
                .map(|(action, _)| action.as_str()),
            Some("is-active")
        );
        assert_eq!(application.model.next_transaction, 1);
    }

    #[test]
    fn streamed_process_output_reaches_model_before_completion() {
        let task = ProcessTask::spawn(
            "/bin/sh",
            &[
                "-c".to_owned(),
                "printf visible-early; sleep 0.2; printf visible-late".to_owned(),
            ],
        );
        let mut model = ConfigModel::new(PathBuf::from("/tmp/unused.conf"));
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut completed = false;
        while Instant::now() < deadline && !model.output.as_text().contains("visible-early") {
            completed = poll_process(&task, &mut model).is_some();
            if !completed {
                thread::sleep(Duration::from_millis(5));
            }
        }
        assert!(model.output.as_text().contains("visible-early"));
        assert!(
            !completed,
            "output was only made visible after process exit"
        );
    }
    #[test]
    fn edited_calibration_completion_is_discarded() {
        let mut application = ControlCenter::new(ConfigModel::new(PathBuf::from(
            "/tmp/calibration-edit.conf",
        )));
        let ownership = CalibrationOwnership {
            transaction: application
                .model
                .begin_calibration()
                .expect("calibration starts"),
            device: String::new(),
        };
        application.model.config.pointer_speed += 1;
        application.model.mark_dirty();
        let edited = application.model.config.clone();

        application.complete_calibration(
            ownership,
            true,
            b"aim-accel-zero-x=101\naim-accel-zero-y=102\naim-accel-zero-z=103\n",
        );

        assert_eq!(application.model.config, edited);
        assert!(application.status.contains("discarded"));
    }

    #[test]
    fn retargeted_calibration_completion_is_discarded() {
        let mut application = ControlCenter::new(ConfigModel::new(PathBuf::from(
            "/tmp/calibration-first.conf",
        )));
        let ownership = CalibrationOwnership {
            transaction: application
                .model
                .begin_calibration()
                .expect("calibration starts"),
            device: String::new(),
        };
        application
            .model
            .set_path(PathBuf::from("/tmp/calibration-second.conf"));
        let retargeted = application.model.config.clone();

        application.complete_calibration(
            ownership,
            true,
            b"aim-motion-plus-bias-x=11\naim-motion-plus-bias-y=12\naim-motion-plus-bias-z=13\n",
        );

        assert_eq!(application.model.config, retargeted);
        assert!(application.status.contains("discarded"));
    }

    #[test]
    fn queued_save_restart_runs_after_any_active_service_action() {
        for active_action in ["is-active", "stop"] {
            let mut application =
                ControlCenter::new(ConfigModel::new(PathBuf::from("/tmp/service-queue.conf")));
            application.service_program = "/bin/true";
            application.dispatch_service_action(active_action);
            application.request_restart_after_save();
            assert_eq!(
                application.pending_service_actions.pending,
                Some(ServiceAction::Restart)
            );

            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline
                && application
                    .service_task
                    .as_ref()
                    .is_some_and(|(action, _)| action != "restart")
            {
                application.poll_service();
                thread::sleep(Duration::from_millis(5));
            }

            assert_eq!(
                application
                    .service_task
                    .as_ref()
                    .map(|(action, _)| action.as_str()),
                Some("restart"),
                "restart did not dispatch after {active_action}"
            );
            assert_eq!(application.pending_service_actions.pending, None);
        }
    }
}
