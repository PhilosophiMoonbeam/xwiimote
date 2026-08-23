#!/bin/sh
# Collect WiiLand diagnostics for a real-hardware Wayland validation report.
set -eu

wiilandd=${WIILANDD:-wiilandd}
device=${1:-}

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
"$wiilandd" --version
"$wiilandd" --check-config
"$wiilandd" --dump-config

section devices
"$wiilandd" --list

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
