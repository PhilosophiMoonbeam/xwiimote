use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use wiiland_core::{
    AimActivation, AimMode, AimSource, Backend, Config, IrAimMapping, IrTracking, TraceConfig,
    TraceFilter,
};

/// Controls whether the daemon publishes its IPC server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IpcMode {
    Auto,
    Disabled,
    Path(PathBuf),
}

/// The mutually-exclusive top-level operations accepted by `wiilandd`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Run,
    Help,
    Version,
    List,
    AxisMap,
    ValidationChecklist,
    Doctor,
    DumpConfig,
    CheckConfig,
    SelfTest,
    CalibrateAim,
}
impl Action {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Help => "--help",
            Self::Version => "--version",
            Self::List => "--list",
            Self::AxisMap => "--axis-map",
            Self::ValidationChecklist => "--validation-checklist",
            Self::Doctor => "--doctor",
            Self::DumpConfig => "--dump-config",
            Self::CheckConfig => "--check-config",
            Self::SelfTest => "--self-test",
            Self::CalibrateAim => "--calibrate-aim",
        }
    }
    fn diagnostic_without_config(self) -> bool {
        matches!(
            self,
            Self::Help | Self::Version | Self::List | Self::AxisMap | Self::ValidationChecklist
        )
    }
}

/// Results of the first pass. This pass discovers config layering, IPC mode,
/// and operations; all other value options are deliberately deferred until
/// after config.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pass1 {
    pub config_path: Option<PathBuf>,
    pub explicit_config: bool,
    pub no_config: bool,
    pub ipc: IpcMode,
    pub action: Action,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cli {
    pub action: Action,
    pub config: Config,
    pub config_path: Option<PathBuf>,
    pub explicit_config: bool,
    pub no_config: bool,
    pub ipc: IpcMode,
    pub device: Option<String>,
    pub dry_run: bool,
    pub verbose: bool,
    pub trace: TraceConfig,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CliError {
    pub code: i32,
    pub message: String,
    pub usage: bool,
}
impl CliError {
    fn syntax(message: impl Into<String>) -> Self {
        Self {
            code: libc::EINVAL,
            message: message.into(),
            usage: true,
        }
    }
    fn conflict(message: impl Into<String>) -> Self {
        Self {
            code: libc::EINVAL,
            message: message.into(),
            usage: false,
        }
    }
}
impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for CliError {}

fn arg_strings<I, S>(args: I) -> Result<Vec<String>, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    args.into_iter()
        .map(|a| {
            a.into()
                .into_string()
                .map_err(|_| CliError::syntax("arguments must be UTF-8"))
        })
        .collect()
}

fn requested_action(
    selected: &mut Action,
    requested: Action,
    option: &str,
) -> Result<(), CliError> {
    if *selected != Action::Run && *selected != requested {
        return Err(CliError::conflict(format!(
            "wiilandd: conflicting actions: {} and {}",
            selected.label(),
            option
        )));
    }
    *selected = requested;
    Ok(())
}

fn ipc_path(value: &str, option: &str) -> Result<PathBuf, CliError> {
    if value.is_empty() {
        return Err(CliError::syntax(format!(
            "wiilandd: {} requires a path",
            option
        )));
    }
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(CliError::syntax(format!(
            "wiilandd: {} requires an absolute path",
            option
        )));
    }
    Ok(path)
}

impl Cli {
    /// Parse the config selectors, IPC mode, and action hints without
    /// applying any value. Value options are deliberately deferred until
    /// after config.
    pub fn parse_pass1(args: &[String]) -> Result<Pass1, CliError> {
        let mut config_path = None;
        let mut explicit_config = false;
        let mut no_config = false;
        let mut ipc = IpcMode::Auto;
        let mut action = Action::Run;
        let mut i = 0;
        while i < args.len() {
            let arg = args[i].as_str();
            if let Some(path) = arg.strip_prefix("--config=") {
                if path.is_empty() {
                    return Err(CliError::syntax("wiilandd: --config requires a path"));
                }
                config_path = Some(PathBuf::from(path));
                explicit_config = true;
            } else if arg == "-c" || arg == "--config" {
                i += 1;
                let Some(path) = args.get(i) else {
                    return Err(CliError::syntax("wiilandd: --config requires a path"));
                };
                config_path = Some(PathBuf::from(path));
                explicit_config = true;
            } else if arg == "--no-config" {
                no_config = true;
            } else if arg == "--no-ipc" {
                if matches!(&ipc, IpcMode::Path(_)) {
                    return Err(CliError::conflict(
                        "wiilandd: --no-ipc cannot be combined with --ipc-socket",
                    ));
                }
                ipc = IpcMode::Disabled;
            } else if let Some(path) = arg.strip_prefix("--ipc-socket=") {
                if matches!(&ipc, IpcMode::Disabled) {
                    return Err(CliError::conflict(
                        "wiilandd: --no-ipc cannot be combined with --ipc-socket",
                    ));
                }
                ipc = IpcMode::Path(ipc_path(path, "--ipc-socket")?);
            } else if arg == "--ipc-socket" {
                i += 1;
                let Some(path) = args.get(i) else {
                    return Err(CliError::syntax("wiilandd: --ipc-socket requires a path"));
                };
                if matches!(&ipc, IpcMode::Disabled) {
                    return Err(CliError::conflict(
                        "wiilandd: --no-ipc cannot be combined with --ipc-socket",
                    ));
                }
                ipc = IpcMode::Path(ipc_path(path, "--ipc-socket")?);
            } else if arg == "-h" || arg == "--help" {
                requested_action(&mut action, Action::Help, arg)?;
            } else if arg == "--version" {
                requested_action(&mut action, Action::Version, arg)?;
            } else if arg == "-l" || arg == "--list" {
                requested_action(&mut action, Action::List, arg)?;
            } else if arg == "--axis-map" {
                requested_action(&mut action, Action::AxisMap, arg)?;
            } else if arg == "--validation-checklist" {
                requested_action(&mut action, Action::ValidationChecklist, arg)?;
            } else if arg == "--doctor" {
                requested_action(&mut action, Action::Doctor, arg)?;
            } else if arg == "--dump-config" {
                requested_action(&mut action, Action::DumpConfig, arg)?;
            } else if arg == "--check-config" {
                requested_action(&mut action, Action::CheckConfig, arg)?;
            } else if arg == "--self-test" {
                requested_action(&mut action, Action::SelfTest, arg)?;
            } else if arg == "--calibrate-aim" {
                requested_action(&mut action, Action::CalibrateAim, arg)?;
            }
            i += 1;
        }
        if no_config && explicit_config {
            return Err(CliError::conflict(
                "wiilandd: --no-config cannot be combined with --config",
            ));
        }
        Ok(Pass1 {
            config_path,
            explicit_config,
            no_config,
            ipc,
            action,
        })
    }

    /// Parse all options over an already-loaded configuration. Since this is
    /// always the second pass, command-line values necessarily win.
    pub fn parse_pass2(args: &[String], config: Config, pass1: Pass1) -> Result<Self, CliError> {
        let mut out = Self {
            action: Action::Run,
            config: {
                config
                    .validate()
                    .map_err(|e| CliError::syntax(e.to_string()))?;
                config
            },
            config_path: pass1.config_path.clone(),
            explicit_config: pass1.explicit_config,
            no_config: pass1.no_config,
            ipc: pass1.ipc.clone(),
            device: None,
            dry_run: false,
            verbose: false,
            trace: TraceConfig::default(),
        };
        let mut i = 0;
        while i < args.len() {
            let arg = args[i].as_str();
            macro_rules! value {
                () => {{
                    i += 1;
                    args.get(i).ok_or_else(|| {
                        CliError::syntax(format!("wiilandd: {} requires a value", arg))
                    })?
                }};
            }
            macro_rules! set_int {
                ($field:ident, $min:expr, $max:expr) => {{
                    let v = value!()
                        .parse::<i64>()
                        .ok()
                        .filter(|v| ($min..=$max).contains(v))
                        .ok_or_else(|| {
                            CliError::syntax(format!("wiilandd: invalid value for {}", arg))
                        })?;
                    out.config.$field = v as i32;
                }};
            }
            if arg == "-h" || arg == "--help" {
                requested_action(&mut out.action, Action::Help, arg)?;
            } else if arg == "--version" {
                requested_action(&mut out.action, Action::Version, arg)?;
            } else if arg == "-l" || arg == "--list" {
                requested_action(&mut out.action, Action::List, arg)?;
            } else if arg == "--axis-map" {
                requested_action(&mut out.action, Action::AxisMap, arg)?;
            } else if arg == "--validation-checklist" {
                requested_action(&mut out.action, Action::ValidationChecklist, arg)?;
            } else if arg == "--doctor" {
                requested_action(&mut out.action, Action::Doctor, arg)?;
            } else if arg == "--dump-config" {
                requested_action(&mut out.action, Action::DumpConfig, arg)?;
            } else if arg == "--check-config" {
                requested_action(&mut out.action, Action::CheckConfig, arg)?;
            } else if arg == "--self-test" {
                requested_action(&mut out.action, Action::SelfTest, arg)?;
            } else if arg == "--calibrate-aim" {
                requested_action(&mut out.action, Action::CalibrateAim, arg)?;
            } else if arg == "--no-config" {
            } else if arg == "--config" {
                i += 1;
            } else if arg.starts_with("--config=")
                || arg == "--no-ipc"
                || arg.starts_with("--ipc-socket=")
            {
            } else if arg == "--ipc-socket" {
                i += 1;
            } else if arg == "-v" || arg == "--verbose" {
                out.verbose = true;
            } else if arg == "-n" || arg == "--dry-run" {
                out.dry_run = true;
            } else if arg == "-d" || arg == "--device" {
                out.device = Some(value!().to_owned());
            } else if let Some(v) = arg.strip_prefix("--device=") {
                if v.is_empty() {
                    return Err(CliError::syntax("wiilandd: --device requires a value"));
                }
                out.device = Some(v.to_owned());
            } else if arg == "-p" || arg == "--profile" {
                let v = value!();
                out.config.profile = wiiland_core::Profile::parse(v).ok_or_else(|| {
                    CliError::syntax(format!("wiilandd: invalid value for {}", arg))
                })?;
            } else if let Some(v) = arg.strip_prefix("--profile=") {
                out.config.profile = wiiland_core::Profile::parse(v).ok_or_else(|| {
                    CliError::syntax(format!("wiilandd: invalid value for {}", arg))
                })?;
            } else if arg == "--backend" {
                let v = value!();
                if v != "uinput" {
                    return Err(CliError::syntax("wiilandd: invalid value for --backend"));
                }
                out.config.backend = Backend::Uinput;
            } else if let Some(v) = arg.strip_prefix("--backend=") {
                if v != "uinput" {
                    return Err(CliError::syntax("wiilandd: invalid value for --backend"));
                }
                out.config.backend = Backend::Uinput;
            } else if let Some(v) = arg.strip_prefix("--trace-events=") {
                out.trace.enabled = true;
                out.trace.filter = v
                    .parse()
                    .map_err(|_| CliError::syntax("wiilandd: invalid value for --trace-events"))?;
            } else if arg == "--trace-events" {
                out.trace.enabled = true;
                out.trace.filter = TraceFilter::All;
            } else if arg == "--pointer-speed" {
                set_int!(pointer_speed, 1i64, 127i64);
            } else if let Some(v) = arg.strip_prefix("--pointer-speed=") {
                out.config.pointer_speed = v
                    .parse::<i64>()
                    .ok()
                    .filter(|v| (1..=127).contains(v))
                    .ok_or_else(|| {
                    CliError::syntax("wiilandd: invalid value for --pointer-speed")
                })? as i32;
            } else if arg == "--ir-speed" {
                set_int!(ir_speed, 1i64, 127i64);
            } else if let Some(v) = arg.strip_prefix("--ir-speed=") {
                out.config.ir_speed = v
                    .parse::<i64>()
                    .ok()
                    .filter(|v| (1..=127).contains(v))
                    .ok_or_else(|| CliError::syntax("wiilandd: invalid value for --ir-speed"))?
                    as i32;
            } else if arg == "--ir-deadzone" {
                set_int!(ir_deadzone, 0i64, 127i64);
            } else if let Some(v) = arg.strip_prefix("--ir-deadzone=") {
                out.config.ir_deadzone = v
                    .parse::<i64>()
                    .ok()
                    .filter(|v| (0..=127).contains(v))
                    .ok_or_else(|| CliError::syntax("wiilandd: invalid value for --ir-deadzone"))?
                    as i32;
            } else if arg == "--ir-smoothing" {
                set_int!(ir_smoothing, 0i64, 95i64);
            } else if let Some(v) = arg.strip_prefix("--ir-smoothing=") {
                out.config.ir_smoothing = v
                    .parse::<i64>()
                    .ok()
                    .filter(|v| (0..=95).contains(v))
                    .ok_or_else(|| CliError::syntax("wiilandd: invalid value for --ir-smoothing"))?
                    as i32;
            } else if arg == "--ir-tracking" {
                out.config.ir_tracking = value!().parse_choice(IrTracking::parse, arg)?;
            } else if let Some(v) = arg.strip_prefix("--ir-tracking=") {
                out.config.ir_tracking = parse_choice(v, IrTracking::parse, arg)?;
            } else if arg == "--ir-aim-mapping" {
                out.config.ir_aim_mapping = value!().parse_choice(IrAimMapping::parse, arg)?;
            } else if let Some(v) = arg.strip_prefix("--ir-aim-mapping=") {
                out.config.ir_aim_mapping = parse_choice(v, IrAimMapping::parse, arg)?;
            } else if arg == "--aim-mode" {
                out.config.aim_mode = value!().parse_choice(AimMode::parse, arg)?;
            } else if let Some(v) = arg.strip_prefix("--aim-mode=") {
                out.config.aim_mode = parse_choice(v, AimMode::parse, arg)?;
            } else if arg == "--aim-source" {
                out.config.aim_source = value!().parse_choice(AimSource::parse, arg)?;
            } else if let Some(v) = arg.strip_prefix("--aim-source=") {
                out.config.aim_source = parse_choice(v, AimSource::parse, arg)?;
            } else if arg == "--aim-activation" {
                out.config.aim_activation = value!().parse_choice(AimActivation::parse, arg)?;
            } else if let Some(v) = arg.strip_prefix("--aim-activation=") {
                out.config.aim_activation = parse_choice(v, AimActivation::parse, arg)?;
            } else if arg == "--aim-sensitivity" {
                set_int!(aim_sensitivity, 1i64, 127i64);
            } else if let Some(v) = arg.strip_prefix("--aim-sensitivity=") {
                out.config.aim_sensitivity = int_value(v, 1, 127, arg)?;
            } else if arg == "--aim-deadzone" {
                set_int!(aim_deadzone, 0i64, 32767i64);
            } else if let Some(v) = arg.strip_prefix("--aim-deadzone=") {
                out.config.aim_deadzone = int_value(v, 0, 32767, arg)?;
            } else if arg == "--aim-smoothing" {
                set_int!(aim_smoothing, 0i64, 95i64);
            } else if let Some(v) = arg.strip_prefix("--aim-smoothing=") {
                out.config.aim_smoothing = int_value(v, 0, 95, arg)?;
            } else if arg == "--aim-invert-x" {
                out.config.aim_invert_x = bool_value(value!(), arg)?;
            } else if let Some(v) = arg.strip_prefix("--aim-invert-x=") {
                out.config.aim_invert_x = bool_value(v, arg)?;
            } else if arg == "--aim-invert-y" {
                out.config.aim_invert_y = bool_value(value!(), arg)?;
            } else if let Some(v) = arg.strip_prefix("--aim-invert-y=") {
                out.config.aim_invert_y = bool_value(v, arg)?;
            } else if arg == "--aim-calibration-duration" {
                set_int!(aim_calibration_duration, 1i64, 30i64);
            } else if let Some(v) = arg.strip_prefix("--aim-calibration-duration=") {
                out.config.aim_calibration_duration = int_value(v, 1, 30, arg)?;
            } else if arg == "--config" || arg == "-c" {
                i += 1;
            } else {
                return Err(CliError::syntax(format!(
                    "wiilandd: unrecognized option '{}'",
                    arg
                )));
            }
            i += 1;
        }
        out.config
            .validate()
            .map_err(|e| CliError::syntax(e.to_string()))?;
        Ok(out)
    }

    pub fn parse<I, S>(args: I) -> Result<Self, CliError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut args = arg_strings(args)?;
        if !args.is_empty() {
            args.remove(0);
        }
        let pass1 = Self::parse_pass1(&args)?;
        let config = if pass1.no_config || pass1.action.diagnostic_without_config() {
            Config::default()
        } else if let Some(path) = pass1.config_path.as_ref() {
            Config::load_file(path).map_err(|e| CliError::syntax(e.to_string()))?
        } else {
            Config::load_default_layers().map_err(|e| CliError::syntax(e.to_string()))?
        };
        Self::parse_pass2(&args, config, pass1)
    }
}

fn parse_choice<T>(
    value: &str,
    parser: impl Fn(&str) -> Option<T>,
    option: &str,
) -> Result<T, CliError> {
    parser(value).ok_or_else(|| CliError::syntax(format!("wiilandd: invalid value for {}", option)))
}
trait ParseChoice {
    fn parse_choice<T>(
        &self,
        parser: impl Fn(&str) -> Option<T>,
        option: &str,
    ) -> Result<T, CliError>;
}
impl ParseChoice for str {
    fn parse_choice<T>(
        &self,
        parser: impl Fn(&str) -> Option<T>,
        option: &str,
    ) -> Result<T, CliError> {
        parse_choice(self, parser, option)
    }
}
fn int_value(value: &str, min: i64, max: i64, option: &str) -> Result<i32, CliError> {
    value
        .parse::<i64>()
        .ok()
        .filter(|v| (min..=max).contains(v))
        .map(|v| v as i32)
        .ok_or_else(|| CliError::syntax(format!("wiilandd: invalid value for {}", option)))
}
fn bool_value(value: &str, option: &str) -> Result<bool, CliError> {
    match value {
        "yes" | "true" | "1" => Ok(true),
        "no" | "false" | "0" => Ok(false),
        _ => Err(CliError::syntax(format!(
            "wiilandd: invalid value for {}",
            option
        ))),
    }
}

pub fn usage() -> &'static str {
    "Usage:\n\twiilandd [OPTIONS]\n\twiilandd --device <number|/sys/path> [OPTIONS]\n\nOptions:\n\t-h, --help       Show this help\n\t    --version    Show version\n\t-l, --list       List connected Wii Remote devices and exit\n\t                 Combine with --verbose for devtype/extension\n\t-d, --device     Bridge one device instead of monitoring all devices\n\t-p, --profile    gamepad, desktop, or both (default: gamepad)\n\t    --backend <uinput>       Input backend (default: uinput)\n\t    --ir-speed <1-127>       IR pointer gain (default: 8)\n\t    --ir-deadzone <0-127>   IR jitter deadzone (default: 0)\n\t    --ir-smoothing <0-95>   IR smoothing percent (default: 0)\n\t    --ir-tracking <dual|centroid|first>\n\t    --ir-aim-mapping <relative|absolute>\n\t    --pointer-speed <1-127>  Desktop pointer step (default: 16)\n\t    --aim-mode <off|mouse|right-stick>\n\t    --aim-source <auto|ir|motion-plus|accelerometer>\n\t    --aim-activation <always|b|z|c>\n\t    --aim-sensitivity <1-127>\n\t    --aim-deadzone <0-32767>\n\t    --aim-smoothing <0-95>\n\t    --aim-invert-x <yes|no>\n\t    --aim-invert-y <yes|no>\n\t    --calibrate-aim\n\t    --aim-calibration-duration <1-30>\n\t-c, --config     Load key=value config file\n\t    --no-config   Do not load the default config file\n\t    --no-ipc      Disable the IPC server\n\t    --ipc-socket PATH\n\t                 Bind the IPC server to an explicit absolute path\n\t-n, --dry-run    Do not create /dev/uinput devices or emit input\n\t    --check-config  Validate configuration and exit\n\t    --self-test  Run deterministic self tests and exit\n\t    --trace-events[=all|keys|axes|ir|motion-plus]\n\t    --axis-map   Print virtual gamepad axis mapping and exit\n\t    --validation-checklist  Print required hardware validation matrix\n\t    --doctor    Print runtime readiness diagnostics and exit\n\t    --dump-config  Print resolved configuration and exit\n\t-v, --verbose    Print device lifecycle details\n\nwiilandd creates Linux uinput virtual input devices named\n\"WiiLand Virtual Controller\" and \"WiiLand Virtual Desktop\"\nthrough the common evdev/libinput input stack.\n"
}

/// Process the actual command line and return a Unix exit status.
pub fn run() -> i32 {
    let args: Vec<OsString> = std::env::args_os().collect();
    let cli = match Cli::parse(args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", e);
            if e.usage {
                eprint!("{}", usage());
            }
            return e.code;
        }
    };
    match crate::commands::execute(&cli) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{}", e);
            e.code()
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn pass(args: &[&str]) -> Pass1 {
        let args = args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
        Cli::parse_pass1(&args).unwrap()
    }

    #[test]
    fn ipc_defaults_to_auto_and_is_independent_of_config() {
        let first = pass(&[]);
        let second = pass(&["--no-config"]);
        assert_eq!(first.ipc, IpcMode::Auto);
        assert_eq!(second.ipc, IpcMode::Auto);
        assert_ne!(first.no_config, second.no_config);
    }

    #[test]
    fn ipc_accepts_both_path_forms() {
        assert_eq!(pass(&["--no-ipc"]).ipc, IpcMode::Disabled);
        assert_eq!(
            pass(&["--ipc-socket", "/tmp/wiilandd.sock"]).ipc,
            IpcMode::Path(PathBuf::from("/tmp/wiilandd.sock"))
        );
        assert_eq!(
            pass(&["--ipc-socket=/tmp/wiilandd.sock"]).ipc,
            IpcMode::Path(PathBuf::from("/tmp/wiilandd.sock"))
        );
    }

    #[test]
    fn ipc_rejects_empty_and_relative_paths() {
        for args in [
            vec!["--ipc-socket", ""],
            vec!["--ipc-socket="],
            vec!["--ipc-socket", "wiilandd.sock"],
            vec!["--ipc-socket=wiilandd.sock"],
        ] {
            assert!(
                Cli::parse_pass1(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err()
            );
        }
    }

    #[test]
    fn ipc_rejects_no_ipc_conflicts() {
        assert!(
            Cli::parse_pass1(
                &["--no-ipc", "--ipc-socket", "/tmp/wiilandd.sock"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            )
            .is_err()
        );
        assert!(
            Cli::parse_pass1(
                &["--ipc-socket=/tmp/wiilandd.sock", "--no-ipc"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            )
            .is_err()
        );
    }

    #[test]
    fn ipc_socket_value_is_consumed_before_action_parsing() {
        let parsed = pass(&["--ipc-socket", "/tmp/--help", "--help"]);
        assert_eq!(parsed.action, Action::Help);
        assert_eq!(parsed.ipc, IpcMode::Path(PathBuf::from("/tmp/--help")));
    }

    #[test]
    fn pass2_preserves_ipc_without_touching_config() {
        let pass1 = pass(&["--ipc-socket=/tmp/wiilandd.sock"]);
        let config = Config::default();
        let parsed = Cli::parse_pass2(&[], config.clone(), pass1).unwrap();
        assert_eq!(
            parsed.ipc,
            IpcMode::Path(PathBuf::from("/tmp/wiilandd.sock"))
        );
        assert_eq!(parsed.config, config);
    }
}
