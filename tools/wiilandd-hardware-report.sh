#!/bin/sh
# Collect WiiLand diagnostics for real-hardware Wayland and X.org validation.
set -eu

usage() {
	cat <<EOF
Usage:
  wiilandd-hardware-report
  wiilandd-hardware-report <number-or-/sys/path> [extra wiilandd args]

Collect finite WiiLand host, permission, config, doctor, axis-map, device, and
manual Wayland/X.org validation diagnostics. With a device argument, continue
into live dry-run trace capture and pass non-conflicting arguments to wiilandd.
EOF
}

wiilandd=${WIILANDD:-wiilandd}
core_failures=0
device=
case "${1:-}" in
-h|--help)
	usage
	exit 0
	;;
'')
	;;
*)
	device=$1
	shift
	;;
esac
trace_selectors=0
validate_trace_args() {
	while [ "$#" -gt 0 ]; do
		arg=$1
		case "$arg" in
		--trace-events|--trace-events=*)
			trace_selectors=$((trace_selectors + 1))
			shift
			;;
		-d|--device|--device=*|-p|--profile|--profile=*|-n|--dry-run|--dry-run=*|\
		-h|--help|--version|-l|--list|--calibrate-aim|--check-config|\
		--self-test|--axis-map|--validation-checklist|--doctor|--dump-config)
			printf 'wiilandd-hardware-report: conflicting trace argument: %s\n' \
				"$arg" >&2
			usage >&2
			exit 2
			;;
		-c|--config|--backend|--ir-speed|--ir-deadzone|--ir-smoothing|\
		--ir-tracking|--ir-aim-mapping|--pointer-speed|--aim-mode|\
		--aim-source|--aim-activation|--aim-sensitivity|--aim-deadzone|\
		--aim-smoothing|--aim-invert-x|--aim-invert-y|\
		--aim-calibration-duration)
			shift
			if [ "$#" -gt 0 ]; then
				shift
			fi
			;;
		*)
			shift
			;;
		esac
	done

	if [ "$trace_selectors" -gt 1 ]; then
		printf 'wiilandd-hardware-report: exactly one trace selector is allowed\n' >&2
		usage >&2
		exit 2
	fi
}
validate_trace_args "$@"
default_repo_dir=$(CDPATH=; cd -- "$(dirname -- "$0")/.." && pwd)
repo_dir=${WIILAND_REPO_DIR:-$default_repo_dir}
module_dir=${HID_WIIMOTE_MODULE_DIR:-/sys/module/hid_wiimote}
os_release=${OS_RELEASE_PATH:-/etc/os-release}
tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/wiilandd-hardware-report.XXXXXX")
list_file=$tmp_dir/device-list
bt_file=$tmp_dir/bluetooth-controllers
git_status_file=$tmp_dir/git-status
trap 'rm -rf "$tmp_dir"' EXIT INT HUP TERM

section() {
	printf '\n== %s ==\n' "$1"
}

run_optional() {
	printf '$ optional:'
	printf ' %s' "$@"
	printf '\n'
	if command -v "$1" >/dev/null 2>&1; then
		if ! "$@"; then
			printf 'optional failed:'
			printf ' %s' "$@"
			printf '\n'
		fi
	else
		printf 'optional unavailable: %s\n' "$1"
	fi
}
run_pkg_version() {
	printf 'optional.%s.pkg-config.version=' "$1"
	if command -v pkg-config >/dev/null 2>&1; then
		if ! pkg-config --modversion "$1" 2>/dev/null; then
			printf 'unavailable\n'
		fi
	else
		printf 'unavailable\n'
	fi
}

record_core_failure() {
	core_failures=$((core_failures + 1))
	printf 'core.failure.%s=%s\n' "$core_failures" "$1"
}

run_wiilandd_probe() {
	printf '$ %s' "$wiilandd"
	printf ' %s' "$@"
	printf '\n'
	if ! command -v "$wiilandd" >/dev/null 2>&1 || ! "$wiilandd" "$@"; then
		printf 'failed: %s' "$wiilandd"
		printf ' %s' "$@"
		printf '\n'
		return 1
	fi
	return 0
}

run_required_wiilandd_probe() {
	if ! run_wiilandd_probe "$@"; then
		record_core_failure "wiilandd.$1"
	fi
}

read_sysfs_attr() {
	path=$1
	name=$2

	if [ -r "$path/$name" ]; then
		tr -d '\n' <"$path/$name"
	else
		printf 'unavailable'
	fi
}

report_git_commit() {
	if ! command -v git >/dev/null 2>&1; then
		printf 'git.commit=unavailable\n'
		printf 'git.dirty=unavailable\n'
		return 0
	fi

	printf 'git.commit='
	if ! git -C "$repo_dir" rev-parse --short HEAD 2>/dev/null; then
		printf 'unavailable\n'
	fi

	printf 'git.dirty='
	if ! git -C "$repo_dir" status --porcelain --untracked-files=all \
		>"$git_status_file" 2>/dev/null; then
		printf 'unavailable\n'
	elif [ -s "$git_status_file" ]; then
		printf 'yes\n'
	else
		printf 'no\n'
	fi
}

read_battery_attr() {
	path=$1

	for attr in "$path"/power_supply/*/capacity; do
		if [ -r "$attr" ]; then
			tr -d '\n' <"$attr"
			return 0
		fi
	done

	printf 'unavailable'
}

report_path_access() {
	label=$1
	path=$2

	if [ -e "$path" ]; then
		printf '%s.exists=yes\n' "$label"
	else
		printf '%s.exists=no\n' "$label"
	fi
	if [ -r "$path" ]; then
		printf '%s.readable=yes\n' "$label"
	else
		printf '%s.readable=no\n' "$label"
	fi
	if [ -w "$path" ]; then
		printf '%s.writable=yes\n' "$label"
	else
		printf '%s.writable=no\n' "$label"
	fi
	if command -v stat >/dev/null 2>&1 && [ -e "$path" ]; then
		printf '%s.mode=' "$label"
		stat -c %a "$path"
		printf '%s.owner=' "$label"
		stat -c %U "$path"
		printf '%s.group=' "$label"
		stat -c %G "$path"
	fi
}
report_event_nodes() {
	index=$1
	syspath=$2

	for event_dir in "$syspath"/input/input*/event*; do
		if [ ! -d "$event_dir" ]; then
			continue
		fi

		event=${event_dir##*/}
		node=/dev/input/$event
		printf 'device.%s.event.%s.node=%s\n' "$index" "$event" "$node"
		report_path_access "device.$index.event.$event" "$node"
	done
}
capture_bluetooth_controllers() {
	if ! command -v bluetoothctl >/dev/null 2>&1; then
		printf 'optional.bluetoothctl.controllers=unavailable\n'
		return 0
	fi

	printf '$ optional: bluetoothctl list\n'
	if ! bluetoothctl list >"$bt_file"; then
		printf 'optional failed: bluetoothctl list\n'
		return 0
	fi
	cat "$bt_file"

	while read -r kind address rest; do
		if [ "$kind" != Controller ] || [ -z "${address:-}" ]; then
			continue
		fi
		printf '$ optional: bluetoothctl show %s\n' "$address"
		if ! bluetoothctl show "$address"; then
			printf 'optional failed: bluetoothctl show %s\n' "$address"
		fi
	done <"$bt_file"
}
report_module_parameters() {
	if [ ! -d "$module_dir/parameters" ]; then
		printf 'hid-wiimote.parameters=unavailable\n'
		return 0
	fi

	printf 'hid-wiimote.module_dir=%s\n' "$module_dir"
	for attr in "$module_dir"/parameters/*; do
		if [ ! -e "$attr" ]; then
			continue
		fi

		name=${attr##*/}
		printf 'hid-wiimote.parameter.%s=' "$name"
		if [ -r "$attr" ]; then
			tr -d '\n' <"$attr"
		else
			printf 'unreadable'
		fi
		printf '\n'
	done
}
report_os_release() {
	if [ ! -r "$os_release" ]; then
		printf 'os-release=unavailable\n'
		return 0
	fi

	printf 'os-release.path=%s\n' "$os_release"
	while IFS= read -r line; do
		case "$line" in
		NAME=*|PRETTY_NAME=*|ID=*|VERSION_ID=*|VERSION_CODENAME=*)
			printf 'os-release.%s\n' "$line"
			;;
		esac
	done <"$os_release"
}






capture_device_list() {
	printf '$ %s --list\n' "$wiilandd"
	if ! "$wiilandd" --list >"$list_file"; then
		printf 'failed: %s --list\n' "$wiilandd"
		return 1
	fi

	cat "$list_file"
	return 0
}

report_device_uevent() {
	index=$1
	syspath=$2

	if [ ! -r "$syspath/uevent" ]; then
		printf 'device.%s.uevent=unavailable\n' "$index"
		return 0
	fi

	while IFS= read -r line; do
		case "$line" in
		HID_ID=*|HID_NAME=*|HID_PHYS=*|HID_UNIQ=*|MODALIAS=*)
			printf 'device.%s.uevent.%s\n' "$index" "$line"
			;;
		esac
	done <"$syspath/uevent"
}

report_device_attrs() {
	file=$1
	row_number=0

	while read -r index syspath extra ||
	      [ -n "${index:-}${syspath:-}${extra:-}" ]; do
		row_number=$((row_number + 1))
		if [ -z "${index:-}" ] && [ -z "${syspath:-}" ] &&
		   [ -z "${extra:-}" ]; then
			continue
		fi
		if [ "$index ${syspath:-} ${extra:-}" = \
		     'No Wii Remote devices found' ]; then
			continue
		fi

		reason=
		case "$index" in
		''|*[!0-9]*)
			reason='invalid-index'
			;;
		esac
		if [ -z "$reason" ] && [ -z "${syspath:-}" ]; then
			reason='missing-syspath'
		fi
		if [ -z "$reason" ] && [ -n "${extra:-}" ]; then
			reason='extra-fields'
		fi
		if [ -n "$reason" ]; then
			printf 'device-list.row.%s.malformed=%s\n' "$row_number" "$reason"
			printf 'device-list.row.%s.parsed=%s|%s|%s\n' \
				"$row_number" "${index:-}" "${syspath:-}" "${extra:-}"
			record_core_failure "wiilandd.list.malformed-row.$row_number"
			continue
		fi

		printf 'device.%s.syspath=%s\n' "$index" "$syspath"
		printf 'device.%s.devtype=' "$index"
		read_sysfs_attr "$syspath" devtype
		printf '\n'
		printf 'device.%s.extension=' "$index"
		read_sysfs_attr "$syspath" extension
		printf '\n'
		printf 'device.%s.battery=' "$index"
		read_battery_attr "$syspath"
		printf '\n'
		report_device_uevent "$index" "$syspath"
		report_event_nodes "$index" "$syspath"
	done <"$file"
}
report_manual_validation_placeholders() {
	section manual-validation
	printf 'manual.sdl=TODO: validate virtual gamepad in an SDL input tester\n'
	printf 'manual.wine-proton=TODO: validate virtual gamepad in one Wine/Proton game\n'
	printf 'manual.native-wayland-desktop=TODO: validate desktop profile pointer/buttons under a native Wayland compositor\n'
	printf 'manual.native-xorg-desktop=TODO: validate desktop profile pointer/buttons in a native X.org session\n'
	printf 'manual.native-x11-consumer=TODO: validate a native X11 application in a native X.org session\n'
	printf 'manual.xwayland-consumer=TODO: validate an X11 application through XWayland in a Wayland session\n'
	printf 'manual.steam-motion-aim=TODO: validate aim-mode=right-stick in one Steam Input game\n'
	printf 'manual.nonsteam-motion-aim=TODO: validate aim-mode=right-stick in one native or XWayland non-Steam game\n'
	printf 'manual.mouse-motion-aim=TODO: validate aim-mode=mouse in one game that accepts mouse aim\n'
	printf 'manual.motion-aim-calibration=TODO: run wiilandd --device <N> --calibrate-aim on a flat stable surface and paste generated offsets into the test config\n'
	printf 'manual.ir-screen-calibration=optional: for absolute IR aim, record ir-screen-left/right/top/bottom and sensor bar placement used during validation\n'
	printf 'manual.notes=TODO: record pass/fail details, game/app names, display server, and deviations\n'
}

report_timestamp() {
	printf 'report.timestamp.utc='
	if command -v date >/dev/null 2>&1; then
		date -u +%Y-%m-%dT%H:%M:%SZ
	else
		printf 'unavailable\n'
	fi
}




section host
printf 'report.schema.version=2\n'
report_timestamp
run_optional uname -srmo
report_os_release
run_optional bluetoothctl --version
capture_bluetooth_controllers
run_optional modinfo hid-wiimote
report_module_parameters
run_pkg_version libxwiimote
if command -v loginctl >/dev/null 2>&1 && [ -n "${XDG_SESSION_ID:-}" ]; then
	run_optional loginctl show-session "$XDG_SESSION_ID" -p Type -p Desktop -p Name
else
	printf 'optional.session-probe=unavailable\n'
fi
printf 'XDG_CURRENT_DESKTOP=%s\n' "${XDG_CURRENT_DESKTOP:-}"
printf 'XDG_SESSION_DESKTOP=%s\n' "${XDG_SESSION_DESKTOP:-}"
printf 'XDG_SESSION_TYPE=%s\n' "${XDG_SESSION_TYPE:-}"
printf 'DESKTOP_SESSION=%s\n' "${DESKTOP_SESSION:-}"
printf 'GDMSESSION=%s\n' "${GDMSESSION:-}"
printf 'KDE_SESSION_VERSION=%s\n' "${KDE_SESSION_VERSION:-}"
printf 'WAYLAND_DISPLAY=%s\n' "${WAYLAND_DISPLAY:-}"
printf 'DISPLAY=%s\n' "${DISPLAY:-}"
if [ "${XAUTHORITY+x}" = x ]; then
	printf 'XAUTHORITY.set=yes\n'
	if [ -n "$XAUTHORITY" ] && [ -r "$XAUTHORITY" ]; then
		printf 'XAUTHORITY.readable=yes\n'
	else
		printf 'XAUTHORITY.readable=no\n'
	fi
else
	printf 'XAUTHORITY.set=no\n'
	printf 'XAUTHORITY.readable=no\n'
fi
printf 'SWAYSOCK=%s\n' "${SWAYSOCK:-}"
printf 'HYPRLAND_INSTANCE_SIGNATURE=%s\n' "${HYPRLAND_INSTANCE_SIGNATURE:-}"

section permissions
run_optional id
report_path_access dev.uinput /dev/uinput

section wiilandd
if command -v "$wiilandd" >/dev/null 2>&1; then
	printf 'wiilandd.available=yes\n'
else
	printf 'wiilandd.available=no\n'
	record_core_failure wiilandd.missing
fi
if ! run_wiilandd_probe --version; then :; fi
report_git_commit
run_required_wiilandd_probe --check-config
run_required_wiilandd_probe --dump-config
if ! run_wiilandd_probe --axis-map; then :; fi
run_required_wiilandd_probe --validation-checklist
run_required_wiilandd_probe --doctor

section devices
if capture_device_list; then
	report_device_attrs "$list_file"
else
	record_core_failure wiilandd.list
fi
report_manual_validation_placeholders

section result
printf 'report.core-failures=%s\n' "$core_failures"
if [ "$core_failures" -gt 0 ]; then
	printf 'report.status=failed\n'
	exit 1
fi
printf 'report.status=ok\n'

if [ -z "$device" ]; then
	cat <<EOF

Pass a device number or sysfs path to capture live dry-run traces:
  WIILANDD=$wiilandd wiilandd-hardware-report <number-or-/sys/path> [extra wiilandd args]
For focused traces:
  WIILANDD=$wiilandd wiilandd-hardware-report <number-or-/sys/path> --trace-events=motion-plus
  WIILANDD=$wiilandd wiilandd-hardware-report <number-or-/sys/path> --trace-events=ir

During trace capture, exercise every button, stick, trigger, accelerometer,
MotionPlus axis, IR pointer source, and attached extension. Stop with Ctrl-C.
EOF
	exit 0
fi

section trace
printf 'Tracing %s. Stop with Ctrl-C after exercising the hardware matrix.\n' "$device"
if [ "$trace_selectors" -eq 0 ]; then
	rm -rf "$tmp_dir"
	exec "$wiilandd" --dry-run --trace-events --verbose \
		--device "$device" --profile both "$@"
fi
rm -rf "$tmp_dir"
exec "$wiilandd" --dry-run --verbose --device "$device" --profile both "$@"
