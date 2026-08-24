#!/bin/sh
# Collect WiiLand diagnostics for a real-hardware Wayland validation report.
set -eu

usage() {
	cat <<EOF
Usage:
  $0
  $0 <number-or-/sys/path> [extra wiilandd args]

Collect finite WiiLand host, permission, config, doctor, axis-map, device, and
manual validation diagnostics. With a device argument, continue into live
dry-run trace capture and pass remaining arguments to wiilandd.
EOF
}

wiilandd=${WIILANDD:-wiilandd}
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
	if command -v "$1" >/dev/null 2>&1; then
		if ! "$@"; then
			printf 'failed:'
			printf ' %s' "$@"
			printf '\n'
		fi
	else
		printf 'unavailable: %s\n' "$1"
	fi
}
run_pkg_version() {
	printf '%s pkg-config version: ' "$1"
	if command -v pkg-config >/dev/null 2>&1; then
		if ! pkg-config --modversion "$1" 2>/dev/null; then
			printf 'unavailable\n'
		fi
	else
		printf 'unavailable\n'
	fi
}

run_wiilandd_probe() {
	printf '$ %s' "$wiilandd"
	printf ' %s' "$@"
	printf '\n'
	if ! "$wiilandd" "$@"; then
		printf 'failed: %s' "$wiilandd"
		printf ' %s' "$@"
		printf '\n'
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
		printf 'bluetoothctl controllers: unavailable\n'
		return 0
	fi

	printf '$ bluetoothctl list\n'
	if ! bluetoothctl list >"$bt_file"; then
		printf 'failed: bluetoothctl list\n'
		return 0
	fi
	cat "$bt_file"

	while read -r kind address rest; do
		if [ "$kind" != Controller ] || [ -z "${address:-}" ]; then
			continue
		fi
		printf '$ bluetoothctl show %s\n' "$address"
		if ! bluetoothctl show "$address"; then
			printf 'failed: bluetoothctl show %s\n' "$address"
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

	while read -r index syspath; do
		case "$index" in
		''|*[!0-9]*)
			continue
			;;
		esac
		if [ -z "${syspath:-}" ]; then
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
	printf 'manual.native-wayland-desktop=TODO: validate desktop profile pointer/buttons under the target compositor\n'
	printf 'manual.steam-motion-aim=TODO: validate aim-mode=right-stick in one Steam Input game\n'
	printf 'manual.nonsteam-motion-aim=TODO: validate aim-mode=right-stick in one native or XWayland non-Steam game\n'
	printf 'manual.mouse-motion-aim=TODO: validate aim-mode=mouse in one game that accepts mouse aim\n'
	printf 'manual.motion-aim-calibration=TODO: run wiilandd --device <N> --calibrate-aim on a flat stable surface and paste generated offsets into the test config\n'
	printf 'manual.ir-screen-calibration=optional: for absolute IR aim, record ir-screen-left/right/top/bottom and sensor bar placement used during validation\n'
	printf 'manual.notes=TODO: record pass/fail details, game/app names, and deviations\n'
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
printf 'report.schema.version=1\n'
report_timestamp
run_optional uname -srmo
report_os_release
run_optional bluetoothctl --version
capture_bluetooth_controllers
run_optional modinfo hid-wiimote
report_module_parameters
run_pkg_version libxwiimote
if command -v loginctl >/dev/null 2>&1 && [ -n "${XDG_SESSION_ID:-}" ]; then
	loginctl show-session "$XDG_SESSION_ID" -p Type -p Desktop -p Name || true
else
	printf 'session: unavailable\n'
fi
printf 'XDG_CURRENT_DESKTOP=%s\n' "${XDG_CURRENT_DESKTOP:-}"
printf 'XDG_SESSION_DESKTOP=%s\n' "${XDG_SESSION_DESKTOP:-}"
printf 'XDG_SESSION_TYPE=%s\n' "${XDG_SESSION_TYPE:-}"
printf 'DESKTOP_SESSION=%s\n' "${DESKTOP_SESSION:-}"
printf 'GDMSESSION=%s\n' "${GDMSESSION:-}"
printf 'KDE_SESSION_VERSION=%s\n' "${KDE_SESSION_VERSION:-}"
printf 'WAYLAND_DISPLAY=%s\n' "${WAYLAND_DISPLAY:-}"
printf 'SWAYSOCK=%s\n' "${SWAYSOCK:-}"
printf 'HYPRLAND_INSTANCE_SIGNATURE=%s\n' "${HYPRLAND_INSTANCE_SIGNATURE:-}"

section permissions
run_optional id
report_path_access dev.uinput /dev/uinput

section wiilandd
run_wiilandd_probe --version
report_git_commit
run_wiilandd_probe --check-config
run_wiilandd_probe --dump-config
run_wiilandd_probe --axis-map
run_wiilandd_probe --validation-checklist
run_wiilandd_probe --doctor

section devices
if capture_device_list; then
	report_device_attrs "$list_file"
fi
report_manual_validation_placeholders

if [ -z "$device" ]; then
	cat <<EOF

Pass a device number or sysfs path to capture live dry-run traces:
  WIILANDD=$wiilandd $0 <number-or-/sys/path> [extra wiilandd args]
For focused traces:
  WIILANDD=$wiilandd $0 <number-or-/sys/path> --trace-events=motion-plus
  WIILANDD=$wiilandd $0 <number-or-/sys/path> --trace-events=ir

During trace capture, exercise every button, stick, trigger, accelerometer,
MotionPlus axis, IR pointer source, and attached extension. Stop with Ctrl-C.
EOF
	exit 0
fi

section trace
printf 'Tracing %s. Stop with Ctrl-C after exercising the hardware matrix.\n' "$device"
exec "$wiilandd" --dry-run --trace-events --verbose --device "$device" --profile both "$@"
