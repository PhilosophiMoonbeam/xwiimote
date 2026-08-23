#!/bin/sh
# Collect WiiLand diagnostics for a real-hardware Wayland validation report.
set -eu

wiilandd=${WIILANDD:-wiilandd}
device=${1:-}
list_file=${TMPDIR:-/tmp}/wiilandd-hardware-report-list.$$
trap 'rm -f "$list_file"' EXIT INT HUP TERM

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
		if ! pkg-config --modversion "$1"; then
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

capture_device_list() {
	printf '$ %s --list\n' "$wiilandd"
	if ! "$wiilandd" --list >"$list_file"; then
		printf 'failed: %s --list\n' "$wiilandd"
		return 1
	fi

	cat "$list_file"
	return 0
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
	done <"$file"
}


section host
run_optional uname -srmo
run_optional bluetoothctl --version
run_optional modinfo hid-wiimote
run_pkg_version libxwiimote
if command -v loginctl >/dev/null 2>&1 && [ -n "${XDG_SESSION_ID:-}" ]; then
	loginctl show-session "$XDG_SESSION_ID" -p Type -p Desktop -p Name || true
else
	printf 'session: unavailable\n'
fi
printf 'XDG_CURRENT_DESKTOP=%s\n' "${XDG_CURRENT_DESKTOP:-}"
printf 'WAYLAND_DISPLAY=%s\n' "${WAYLAND_DISPLAY:-}"
printf 'XDG_SESSION_TYPE=%s\n' "${XDG_SESSION_TYPE:-}"

section wiilandd
run_wiilandd_probe --version
run_wiilandd_probe --check-config
run_wiilandd_probe --dump-config

section devices
if capture_device_list; then
	report_device_attrs "$list_file"
fi

if [ -z "$device" ]; then
	cat <<EOF

Pass a device number or sysfs path to capture live dry-run traces:
  WIILANDD=$wiilandd $0 <number-or-/sys/path>

During trace capture, exercise every button, stick, trigger, accelerometer,
MotionPlus axis, IR pointer source, and attached extension. Stop with Ctrl-C.
EOF
	exit 0
fi

section trace
printf 'Tracing %s. Stop with Ctrl-C after exercising the hardware matrix.\n' "$device"
exec "$wiilandd" --dry-run --trace-events --verbose --device "$device" --profile both
