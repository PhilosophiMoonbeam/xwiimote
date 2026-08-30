#!/bin/sh
# Build and exercise wiilandd logic without requiring real Wii hardware.
set -eu

cc=${CC:-gcc}
root=$(CDPATH=; cd -- "$(dirname -- "$0")/.." && pwd)
build_dir=${TMPDIR:-/tmp}/wiilandd-smoke.$$
trap 'rm -rf "$build_dir"' EXIT INT HUP TERM
mkdir -p "$build_dir"
bin=$build_dir/wiilandd-smoke
system_config=$build_dir/system/wiilandd.conf

"$cc" -std=gnu99 -Wall -Wextra -Werror -DPACKAGE_VERSION=\"smoke\" \
	"-DWIILAND_SYSTEM_CONFIG_PATH=\"$system_config\"" \
	-I"$root/lib" "$root/tools/wiilandd.c" "$root/tests/xwii_stubs.c" \
	-o "$bin"

test "$("$bin" --version)" = "wiilandd smoke"
mkdir -p "$(dirname "$system_config")" "$build_dir/home"
printf '%s\n' 'profile=desktop' >"$system_config"
HOME=$build_dir/home XDG_CONFIG_HOME='' \
	"$bin" --dump-config >"$build_dir/system-config-dump"
grep -F 'profile=desktop' "$build_dir/system-config-dump" >/dev/null
HOME=$build_dir/home XDG_CONFIG_HOME='' \
	"$bin" --doctor >"$build_dir/system-config-doctor"
grep -F "config.system.path=$system_config" \
	"$build_dir/system-config-doctor" >/dev/null
grep -F 'config.system.exists=yes' "$build_dir/system-config-doctor" >/dev/null
rm -f "$system_config"

mkdir -p "$build_dir/relative/wiiland" "$build_dir/home/.config/wiiland"
printf '%s\n' 'profile=desktop' \
	>"$build_dir/relative/wiiland/wiilandd.conf"
printf '%s\n' 'profile=both' \
	>"$build_dir/home/.config/wiiland/wiilandd.conf"
(cd "$build_dir" && HOME=$build_dir/home XDG_CONFIG_HOME=relative \
	"$bin" --dump-config) >"$build_dir/relative-xdg-dump"
grep -F 'profile=both' "$build_dir/relative-xdg-dump" >/dev/null
rm -rf "$build_dir/relative" "$build_dir/home/.config"

install_stage=$build_dir/install-stage
mkdir -p \
	"$install_stage/etc/wiiland" \
	"$install_stage/usr/bin" \
	"$install_stage/usr/lib/udev/rules.d" \
	"$install_stage/usr/lib/systemd/user" \
	"$install_stage/usr/share/X11/xorg.conf.d" \
	"$install_stage/usr/share/doc/wiiland" \
	"$install_stage/usr/share/doc/wiiland/examples" \
	"$install_stage/usr/share/man/man1" \
	"$install_stage/usr/share/man/man7"
cp "$bin" "$install_stage/usr/bin/wiilandd"
cp "$root/res/70-wiiland-uinput.rules" \
	"$install_stage/usr/lib/udev/rules.d/70-wiiland-uinput.rules"
cp "$root/res/70-udev-wiiland.rules" \
	"$install_stage/usr/lib/udev/rules.d/70-udev-wiiland.rules"
cp "$root/res/50-xorg-fix-wiiland.conf" \
	"$install_stage/usr/share/X11/xorg.conf.d/50-xorg-fix-wiiland.conf"
sed -e 's|@bindir@|/usr/bin|g' "$root/res/wiilandd.service.in" \
	>"$install_stage/usr/lib/systemd/user/wiilandd.service"
cp "$root/res/wiilandd.conf" \
	"$install_stage/usr/share/doc/wiiland/examples/wiilandd.conf"
cp "$root/res/wiilandd.conf" \
	"$install_stage/etc/wiiland/wiilandd.conf"
cp "$root/tools/wiilandd-hardware-report.sh" \
	"$install_stage/usr/bin/wiilandd-hardware-report"
chmod +x "$install_stage/usr/bin/wiilandd-hardware-report"
cp "$root/doc/WIILAND" "$install_stage/usr/share/doc/wiiland/WIILAND"
cp "$root/doc/wiilandd.1" "$install_stage/usr/share/man/man1/"
cp "$root/doc/wiiland.7" "$root/doc/libxwiimote.7" \
	"$install_stage/usr/share/man/man7/"
"$root/tests/wiilandd-install-smoke.sh" "$install_stage" /usr no /etc no


"$bin" --self-test
"$bin" --config "$root/res/wiilandd.conf" --check-config
"$bin" --config "$root/res/wiilandd.conf" --dump-config >/dev/null
cat >"$build_dir/invalid-screen.conf" <<'EOF'
ir-aim-mapping=absolute
ir-screen-left=900
ir-screen-right=100
EOF
if "$bin" --config "$build_dir/invalid-screen.conf" --check-config \
	>"$build_dir/invalid-screen-out" 2>"$build_dir/invalid-screen-err"; then
	printf '%s\n' 'wiilandd accepted inverted IR screen calibration' >&2
	exit 1
fi
grep -F 'IR screen calibration requires right > left and bottom > top' \
	"$build_dir/invalid-screen-err" >/dev/null
cat >"$build_dir/partial-accel.conf" <<'EOF'
aim-accel-zero-x=10
EOF
if "$bin" --config "$build_dir/partial-accel.conf" --check-config \
	>"$build_dir/partial-accel-out" 2>"$build_dir/partial-accel-err"; then
	printf '%s\n' 'wiilandd accepted partial accelerometer calibration' >&2
	exit 1
fi
grep -F 'accelerometer calibration requires complete x, y, and z values' \
	"$build_dir/partial-accel-err" >/dev/null
cat >"$build_dir/partial-motion-plus.conf" <<'EOF'
aim-motion-plus-bias-y=10
EOF
if "$bin" --config "$build_dir/partial-motion-plus.conf" --check-config \
	>"$build_dir/partial-motion-plus-out" \
	2>"$build_dir/partial-motion-plus-err"; then
	printf '%s\n' 'wiilandd accepted partial MotionPlus calibration' >&2
	exit 1
fi
grep -F 'MotionPlus calibration requires complete x, y, and z values' \
	"$build_dir/partial-motion-plus-err" >/dev/null
cat >"$build_dir/complete-accel.conf" <<'EOF'
aim-accel-zero-x=10
aim-accel-zero-y=11
aim-accel-zero-z=12
EOF
"$bin" --config "$build_dir/complete-accel.conf" --check-config

if "$bin" --no-config --config "$root/res/wiilandd.conf" --dump-config \
	>"$build_dir/config-conflict-out" 2>"$build_dir/config-conflict-err"; then
	printf '%s\n' 'wiilandd accepted conflicting config selectors' >&2
	exit 1
fi
grep -F -- '--no-config cannot be combined with --config' \
	"$build_dir/config-conflict-err" >/dev/null
reject_invalid_command() {
	if "$bin" "$@" >"$build_dir/action-order-out" \
		2>"$build_dir/action-order-err"; then
		printf '%s\n' 'wiilandd ignored an invalid argument' >&2
		exit 1
	fi
}
reject_invalid_command --axis-map --definitely-invalid
reject_invalid_command --definitely-invalid --axis-map
if "$bin" --doctor --dump-config >"$build_dir/action-conflict-out" \
	2>"$build_dir/action-conflict-err"; then
	printf '%s\n' 'wiilandd accepted conflicting primary actions' >&2
	exit 1
fi
grep -F 'conflicting actions: --doctor and --dump-config' \
	"$build_dir/action-conflict-err" >/dev/null
"$bin" --axis-map >"$build_dir/axis-map"
grep -F 'range.signed=-32768:32767' "$build_dir/axis-map" >/dev/null
grep -F 'range.trigger=0:1023' "$build_dir/axis-map" >/dev/null
grep -F 'range.balance=0:65535' "$build_dir/axis-map" >/dev/null
grep -F 'nunchuk.accel.x=ABS_HAT1X' "$build_dir/axis-map" >/dev/null
grep -F 'wiimote.a=BTN_SOUTH' "$build_dir/axis-map" >/dev/null
grep -F 'pro.zl=BTN_TL2' "$build_dir/axis-map" >/dev/null
grep -F 'guitar.stick.x=ABS_X' "$build_dir/axis-map" >/dev/null
grep -F 'guitar.whammy=ABS_HAT3X' "$build_dir/axis-map" >/dev/null
grep -F 'guitar.fret-board=ABS_HAT3Y' "$build_dir/axis-map" >/dev/null
grep -F 'drums.tom.far-right=ABS_HAT3X' "$build_dir/axis-map" >/dev/null
grep -F 'drums.bass=ABS_HAT3Y' "$build_dir/axis-map" >/dev/null
grep -F 'classic.zl=BTN_TL2' "$build_dir/axis-map" >/dev/null
grep -F 'guitar.fret.mid=BTN_FRET_MID' "$build_dir/axis-map" >/dev/null
grep -F 'guitar.minus=BTN_SELECT' "$build_dir/axis-map" >/dev/null
grep -F 'drums.minus=BTN_SELECT' "$build_dir/axis-map" >/dev/null
grep -F 'aim.right-stick.x=ABS_RX' "$build_dir/axis-map" >/dev/null
grep -F 'aim.mouse.x=REL_X' "$build_dir/axis-map" >/dev/null
"$bin" --validation-checklist >"$build_dir/validation-checklist"
grep -F 'motion-plus-external.hotplug=required' "$build_dir/validation-checklist" >/dev/null
grep -F 'wayland.wine-proton=required' "$build_dir/validation-checklist" >/dev/null
grep -F 'steam.motion-aim-right-stick=required' "$build_dir/validation-checklist" >/dev/null
cat >"$build_dir/doctor.conf" <<'EOF'
backend=uinput
profile=desktop
EOF
touch "$build_dir/wayland-1"
DISPLAY='' WAYLAND_DISPLAY=wayland-1 XDG_RUNTIME_DIR=$build_dir \
	XDG_SESSION_TYPE=wayland \
	"$bin" --config "$build_dir/doctor.conf" --doctor >"$build_dir/doctor-wayland"
grep -F 'session.display-server=wayland' "$build_dir/doctor-wayland" >/dev/null
grep -F 'session.wayland=yes' "$build_dir/doctor-wayland" >/dev/null
grep -F 'session.x11=no' "$build_dir/doctor-wayland" >/dev/null
grep -F 'session.xwayland.available=no' "$build_dir/doctor-wayland" >/dev/null
grep -F "wayland.socket.path=$build_dir/wayland-1" "$build_dir/doctor-wayland" >/dev/null
grep -F 'wayland.socket.type=other' "$build_dir/doctor-wayland" >/dev/null
grep -F 'wayland.socket.exists=yes' "$build_dir/doctor-wayland" >/dev/null
grep -F 'x11.display=unknown' "$build_dir/doctor-wayland" >/dev/null
grep -F 'dev.uinput.writable=' "$build_dir/doctor-wayland" >/dev/null
grep -F 'backend=uinput' "$build_dir/doctor-wayland" >/dev/null
grep -F 'profile=desktop' "$build_dir/doctor-wayland" >/dev/null
grep -F "config.system.path=$system_config" "$build_dir/doctor-wayland" >/dev/null
grep -F 'config.system.exists=no' "$build_dir/doctor-wayland" >/dev/null
grep -F 'aim.mode=off' "$build_dir/doctor-wayland" >/dev/null

DISPLAY=:4242 WAYLAND_DISPLAY='' XDG_SESSION_TYPE=x11 \
	"$bin" --no-config --doctor >"$build_dir/doctor-x11"
grep -F 'session.display-server=x11' "$build_dir/doctor-x11" >/dev/null
grep -F 'session.wayland=no' "$build_dir/doctor-x11" >/dev/null
grep -F 'session.x11=yes' "$build_dir/doctor-x11" >/dev/null
grep -F 'session.xwayland.available=not-applicable' \
	"$build_dir/doctor-x11" >/dev/null
grep -F 'x11.display=:4242' "$build_dir/doctor-x11" >/dev/null
grep -F 'x11.socket.path=/tmp/.X11-unix/X4242' "$build_dir/doctor-x11" >/dev/null
grep -F 'x11.socket.exists=' "$build_dir/doctor-x11" >/dev/null

DISPLAY=unix/:4243.1 WAYLAND_DISPLAY=wayland-1 XDG_RUNTIME_DIR=$build_dir \
	XDG_SESSION_TYPE=wayland \
	"$bin" --no-config --doctor >"$build_dir/doctor-xwayland"
grep -F 'session.display-server=wayland' "$build_dir/doctor-xwayland" >/dev/null
grep -F 'session.wayland=yes' "$build_dir/doctor-xwayland" >/dev/null
grep -F 'session.x11=yes' "$build_dir/doctor-xwayland" >/dev/null
grep -F 'session.xwayland.available=no' "$build_dir/doctor-xwayland" >/dev/null
grep -F 'x11.socket.path=/tmp/.X11-unix/X4243' "$build_dir/doctor-xwayland" >/dev/null
grep -F 'x11.socket.exists=no' "$build_dir/doctor-xwayland" >/dev/null

DISPLAY=remote.example:7.0 WAYLAND_DISPLAY=wayland-1 XDG_RUNTIME_DIR=$build_dir \
	XDG_SESSION_TYPE=wayland \
	"$bin" --no-config --doctor >"$build_dir/doctor-wayland-remote-x11"
grep -F 'session.display-server=wayland' \
	"$build_dir/doctor-wayland-remote-x11" >/dev/null
grep -F 'session.xwayland.available=unknown' \
	"$build_dir/doctor-wayland-remote-x11" >/dev/null

DISPLAY=remote.example:7.0 WAYLAND_DISPLAY='' XDG_SESSION_TYPE=x11 \
	"$bin" --no-config --doctor >"$build_dir/doctor-x11-remote"
grep -F 'session.display-server=x11' "$build_dir/doctor-x11-remote" >/dev/null
grep -F 'x11.display=remote.example:7.0' "$build_dir/doctor-x11-remote" >/dev/null
grep -F 'x11.socket.path=unknown' "$build_dir/doctor-x11-remote" >/dev/null
grep -F 'x11.socket.exists=unknown' "$build_dir/doctor-x11-remote" >/dev/null

DISPLAY='' WAYLAND_DISPLAY='' XDG_SESSION_TYPE='' \
	"$bin" --no-config --doctor >"$build_dir/doctor-headless"
grep -F 'session.display-server=headless' "$build_dir/doctor-headless" >/dev/null
grep -F 'session.wayland=no' "$build_dir/doctor-headless" >/dev/null
grep -F 'session.x11=no' "$build_dir/doctor-headless" >/dev/null
grep -F 'wayland.display=unknown' "$build_dir/doctor-headless" >/dev/null
grep -F 'x11.display=unknown' "$build_dir/doctor-headless" >/dev/null

"$bin" --help >"$build_dir/help"
grep -F 'Linux uinput virtual input devices' "$build_dir/help" >/dev/null
grep -F 'WiiLand Virtual Controller' "$build_dir/help" >/dev/null
grep -F 'WiiLand Virtual Desktop' "$build_dir/help" >/dev/null
"$bin" --no-config --trace-events=ir --dump-config >/dev/null
"$bin" --no-config --trace-events=motion-plus --dump-config >/dev/null
"$bin" --no-config --aim-mode=right-stick --aim-source=motion-plus \
	--aim-activation=z --aim-sensitivity=24 --aim-deadzone=12 \
	--aim-smoothing=40 --aim-invert-y=yes --aim-calibration-duration=12 \
	--ir-tracking=centroid --ir-aim-mapping=absolute \
	--dump-config >"$build_dir/aim-dump"
grep -F 'aim-mode=right-stick' "$build_dir/aim-dump" >/dev/null
grep -F 'aim-source=motion-plus' "$build_dir/aim-dump" >/dev/null
grep -F 'aim-activation=z' "$build_dir/aim-dump" >/dev/null
grep -F 'aim-sensitivity=24' "$build_dir/aim-dump" >/dev/null
grep -F 'aim-deadzone=12' "$build_dir/aim-dump" >/dev/null
grep -F 'aim-smoothing=40' "$build_dir/aim-dump" >/dev/null
grep -F 'aim-invert-y=yes' "$build_dir/aim-dump" >/dev/null
grep -F 'aim-calibration-duration=12' "$build_dir/aim-dump" >/dev/null
grep -F 'ir-tracking=centroid' "$build_dir/aim-dump" >/dev/null
grep -F 'ir-aim-mapping=absolute' "$build_dir/aim-dump" >/dev/null
if "$bin" --no-config --trace-events=bad --dump-config >/dev/null 2>&1; then
	printf '%s\n' 'wiilandd accepted invalid trace event filter' >&2
	exit 1
fi
stub_sys=$build_dir/sys/devices/wiimote0
stub_sys_missing=$build_dir/sys/devices/wiimote1
mkdir -p "$stub_sys" "$stub_sys_missing"
printf '%s\n' wiimote >"$stub_sys/devtype"
printf '%s\n' nunchuk >"$stub_sys/extension"
printf '%s\n' 'HID_NAME=Nintendo Wii Remote' >"$stub_sys/uevent"
XWII_STUB_DEVICES=$stub_sys:$stub_sys_missing "$bin" --list --verbose >"$build_dir/list"
test "$(sed -n '1p' "$build_dir/list")" = "1	$stub_sys"
if XWII_STUB_DEVICES=/sys/fake "$bin" --no-config --dry-run \
	>"$build_dir/add-fail-out" 2>"$build_dir/add-fail-err"; then
	grep -F 'wiilandd: cannot add /sys/fake: -19' "$build_dir/add-fail-err" >/dev/null
else
	printf '%s\n' 'wiilandd failed monitor add-failure smoke' >&2
	exit 1
fi
if XWII_STUB_DEVICES=/sys/fake XWII_STUB_IFACE_NEW_OK=1 XWII_STUB_WATCH_RET=-5 \
	"$bin" --no-config --dry-run >"$build_dir/watch-fail-out" \
	2>"$build_dir/watch-fail-err"; then
	grep -F 'wiilandd: cannot watch /sys/fake: -5' "$build_dir/watch-fail-err" >/dev/null
	grep -F 'wiilandd: cannot add /sys/fake: -5' "$build_dir/watch-fail-err" >/dev/null
else
	printf '%s\n' 'wiilandd failed watch-failure smoke' >&2
	exit 1
fi
if XWII_STUB_DEVICES=/sys/fake XWII_STUB_IFACE_NEW_OK=1 XWII_STUB_OPEN_RET=-6 \
	"$bin" --no-config --dry-run >"$build_dir/open-fail-out" \
	2>"$build_dir/open-fail-err"; then
	grep -F 'wiilandd: cannot open required interfaces for /sys/fake: -6' "$build_dir/open-fail-err" >/dev/null
	grep -F 'wiilandd: cannot add /sys/fake: -6' "$build_dir/open-fail-err" >/dev/null
else
	printf '%s\n' 'wiilandd failed required-interface open-failure smoke' >&2
	exit 1
fi
XWII_STUB_DEVICES=/sys/fake XWII_STUB_IFACE_NEW_OK=1 XWII_STUB_OPEN_RET=-6 \
	XWII_STUB_OPENED=1 "$bin" --no-config --dry-run \
	>"$build_dir/open-partial-out" 2>"$build_dir/open-partial-err"
grep -F 'wiilandd: cannot open some interfaces for /sys/fake: -6' \
	"$build_dir/open-partial-err" >/dev/null
XWII_STUB_DEVICES=/sys/retry XWII_STUB_IFACE_NEW_OK=1 \
	XWII_STUB_IFACE_NEW_FAILS=1 "$bin" --no-config --dry-run --verbose \
	>"$build_dir/retry-out" 2>"$build_dir/retry-err"
grep -F 'wiilandd: cannot add /sys/retry: -19' "$build_dir/retry-err" >/dev/null
test "$(grep -c 'bridging /sys/retry' "$build_dir/retry-err")" = 1
XWII_STUB_DEVICES=/sys/simultaneous-old XWII_STUB_SIMULTANEOUS_READY=1 \
	"$bin" --no-config --dry-run --verbose \
	>"$build_dir/simultaneous-out" 2>"$build_dir/simultaneous-err"
grep -F 'xwii stub: simultaneous rebuilt active-bridges=1 stale-dispatches=0' \
	"$build_dir/simultaneous-err" >/dev/null
test "$(grep -c 'bridging /sys/simultaneous-' \
	"$build_dir/simultaneous-err")" = 2
grep -F 'bridging /sys/simultaneous-old' \
	"$build_dir/simultaneous-err" >/dev/null
grep -F 'bridging /sys/simultaneous-new' \
	"$build_dir/simultaneous-err" >/dev/null
if grep -F 'stale simultaneous owner dispatch' \
	"$build_dir/simultaneous-err" >/dev/null; then
	printf '%s\n' 'wiilandd dispatched a stale simultaneous owner' >&2
	exit 1
fi


XWII_STUB_CALIBRATION_SOURCE=accel XWII_STUB_AVAILABLE=2 \
	XWII_STUB_OPEN_RET=-6 XWII_STUB_OPENED=2 XWII_STUB_EXPECT_OPEN=2 \
	"$bin" --no-config --device /sys/fake --aim-calibration-duration=1 \
	--calibrate-aim >"$build_dir/calibrate-accel" \
	2>"$build_dir/calibrate-accel-err"
grep -F 'aim-accel-zero-x=10' "$build_dir/calibrate-accel" >/dev/null
grep -F '# warning: MotionPlus calibration unavailable or unstable' \
	"$build_dir/calibrate-accel" >/dev/null

XWII_STUB_CALIBRATION_SOURCE=motion-plus XWII_STUB_AVAILABLE=256 \
	XWII_STUB_OPEN_RET=-6 XWII_STUB_OPENED=256 XWII_STUB_EXPECT_OPEN=256 \
	"$bin" --no-config --device /sys/fake --aim-calibration-duration=1 \
	--calibrate-aim >"$build_dir/calibrate-motion" \
	2>"$build_dir/calibrate-motion-err"
grep -F 'aim-motion-plus-bias-x=10' "$build_dir/calibrate-motion" >/dev/null
grep -F '# warning: accelerometer calibration unavailable or unstable' \
	"$build_dir/calibrate-motion" >/dev/null
test "$(sed -n '2p' "$build_dir/list")" = "	devtype=wiimote"
test "$(sed -n '3p' "$build_dir/list")" = "	extension=nunchuk"
test "$(sed -n '4p' "$build_dir/list")" = "2	$stub_sys_missing"
test "$(sed -n '5p' "$build_dir/list")" = "	devtype=unavailable"
test "$(sed -n '6p' "$build_dir/list")" = "	extension=unavailable"
fake_wiilandd=$build_dir/fake-wiilandd
cat >"$fake_wiilandd" <<'EOF'
#!/bin/sh
case "$1" in
--axis-map)
	printf '%s\n' 'nunchuk.accel.x=ABS_HAT1X'
	printf '%s\n' 'aim.right-stick.x=ABS_RX'
	exit 0
	;;
--validation-checklist)
	printf '%s\n' 'wayland.wine-proton=required'
	printf '%s\n' 'steam.motion-aim-right-stick=required'
	exit 0
	;;
--doctor)
	printf '%s\n' 'dev.uinput.writable=no'
	printf '%s\n' 'aim.mode=right-stick'
	exit 0
	;;
--list)
	mode=${FAKE_LIST_MODE:-}
	if [ -z "$mode" ]; then
		if [ -n "${FAKE_DEVICE_SYSPATH:-}" ]; then
			mode=device
		else
			mode=zero
		fi
	fi
	case "$mode" in
	zero)
		printf '%s\n' 'No Wii Remote devices found'
		;;
	device)
		printf '1\t%s\n' "$FAKE_DEVICE_SYSPATH"
		;;
	malformed)
		printf '%s\n' 'one /sys/fake unexpected'
		;;
	*)
		printf '%s\n' "unknown FAKE_LIST_MODE: $FAKE_LIST_MODE" >&2
		exit 1
		;;
	esac
	exit 0
	;;
esac
printf 'fake-wiilandd'
for arg do
	printf ' [%s]' "$arg"
done
printf '\n'
exit 0
EOF

assert_manual_report_rows() {
	report_file=$1
	for row in \
		'manual.sdl=TODO:' \
		'manual.wine-proton=TODO:' \
		'manual.native-wayland-desktop=TODO:' \
		'manual.native-xorg-desktop=TODO:' \
		'manual.native-x11-consumer=TODO:' \
		'manual.xwayland-consumer=TODO:' \
		'manual.steam-motion-aim=TODO:' \
		'manual.nonsteam-motion-aim=TODO:' \
		'manual.mouse-motion-aim=TODO:' \
		'manual.motion-aim-calibration=TODO:' \
		'manual.ir-screen-calibration=optional:' \
		'manual.notes=TODO:'
	do
		grep -F "$row" "$report_file" >/dev/null
	done
}
chmod +x "$fake_wiilandd"
"$root/tools/wiilandd-hardware-report.sh" --help >"$build_dir/hardware-report-help"
grep -F 'Usage:' "$build_dir/hardware-report-help" >/dev/null
grep -F '<number-or-/sys/path>' "$build_dir/hardware-report-help" >/dev/null
grep -F 'doctor, axis-map' "$build_dir/hardware-report-help" >/dev/null
grep -F 'WantedBy=default.target' \
	"$install_stage/usr/lib/systemd/user/wiilandd.service" >/dev/null
grep -F 'ExecStart=@bindir@/wiilandd' "$root/res/wiilandd.service.in" >/dev/null
grep -F 'NoNewPrivileges=yes' "$root/res/wiilandd.service.in" >/dev/null
grep -F 'LockPersonality=yes' "$root/res/wiilandd.service.in" >/dev/null
grep -F 'MemoryDenyWriteExecute=yes' "$root/res/wiilandd.service.in" >/dev/null
grep -F 'RestrictRealtime=yes' "$root/res/wiilandd.service.in" >/dev/null
grep -F 'RestrictSUIDSGID=yes' "$root/res/wiilandd.service.in" >/dev/null
grep -F 'SystemCallArchitectures=native' \
	"$root/res/wiilandd.service.in" >/dev/null
grep -F 'UMask=0077' "$root/res/wiilandd.service.in" >/dev/null
if command -v systemd-analyze >/dev/null 2>&1; then
	sed -e 's|@bindir@/wiilandd|/bin/true|g' \
		"$root/res/wiilandd.service.in" >"$build_dir/wiilandd.service"
	systemd-analyze verify --man=no "$build_dir/wiilandd.service"
fi
touch "$build_dir/xauthority"
(cd "$build_dir" && XDG_CURRENT_DESKTOP=TestDesktop XDG_SESSION_TYPE=x11 \
	WAYLAND_DISPLAY='' DISPLAY=:77 XAUTHORITY=$build_dir/xauthority \
	SWAYSOCK=/tmp/sway.sock \
	FAKE_DEVICE_SYSPATH=$stub_sys WIILANDD=$fake_wiilandd \
	"$root/tools/wiilandd-hardware-report.sh" \
	7 --trace-events=motion-plus) >"$build_dir/hardware-report"
if git_commit=$(git -C "$root" rev-parse --short HEAD 2>/dev/null); then
	expected_commit=git.commit=$git_commit
else
	expected_commit=git.commit=unavailable
fi
grep -F 'report.schema.version=2' "$build_dir/hardware-report" >/dev/null
grep -F 'report.timestamp.utc=' "$build_dir/hardware-report" >/dev/null
grep -F "$expected_commit" "$build_dir/hardware-report" >/dev/null
grep -F 'git.dirty=' "$build_dir/hardware-report" >/dev/null
grep -F 'XDG_CURRENT_DESKTOP=TestDesktop' "$build_dir/hardware-report" >/dev/null
grep -F 'XDG_SESSION_TYPE=x11' "$build_dir/hardware-report" >/dev/null
grep -F 'WAYLAND_DISPLAY=' "$build_dir/hardware-report" >/dev/null
grep -F 'SWAYSOCK=/tmp/sway.sock' "$build_dir/hardware-report" >/dev/null
grep -F 'DISPLAY=:77' "$build_dir/hardware-report" >/dev/null
grep -F 'XAUTHORITY.set=yes' "$build_dir/hardware-report" >/dev/null
grep -F 'XAUTHORITY.readable=yes' "$build_dir/hardware-report" >/dev/null
assert_manual_report_rows "$build_dir/hardware-report"
grep -F '$ '"$fake_wiilandd"' --axis-map' "$build_dir/hardware-report" >/dev/null
grep -F 'device.1.uevent.HID_NAME=Nintendo Wii Remote' "$build_dir/hardware-report" >/dev/null
grep -F 'nunchuk.accel.x=ABS_HAT1X' "$build_dir/hardware-report" >/dev/null
grep -F 'aim.right-stick.x=ABS_RX' "$build_dir/hardware-report" >/dev/null
grep -F '$ '"$fake_wiilandd"' --validation-checklist' "$build_dir/hardware-report" >/dev/null
grep -F 'wayland.wine-proton=required' "$build_dir/hardware-report" >/dev/null
grep -F 'steam.motion-aim-right-stick=required' "$build_dir/hardware-report" >/dev/null
grep -F '$ '"$fake_wiilandd"' --doctor' "$build_dir/hardware-report" >/dev/null
grep -F 'dev.uinput.writable=no' "$build_dir/hardware-report" >/dev/null
grep -F 'aim.mode=right-stick' "$build_dir/hardware-report" >/dev/null
(cd "$build_dir" && XDG_CURRENT_DESKTOP=WaylandDesktop \
	XDG_SESSION_TYPE=wayland WAYLAND_DISPLAY=wayland-9 DISPLAY=:88 \
	XAUTHORITY=$build_dir/xauthority FAKE_LIST_MODE=zero \
	WIILANDD=$fake_wiilandd \
	"$root/tools/wiilandd-hardware-report.sh") \
	>"$build_dir/hardware-report-wayland"
grep -F 'XDG_CURRENT_DESKTOP=WaylandDesktop' \
	"$build_dir/hardware-report-wayland" >/dev/null
grep -F 'XDG_SESSION_TYPE=wayland' \
	"$build_dir/hardware-report-wayland" >/dev/null
grep -F 'WAYLAND_DISPLAY=wayland-9' \
	"$build_dir/hardware-report-wayland" >/dev/null
grep -F 'DISPLAY=:88' "$build_dir/hardware-report-wayland" >/dev/null
grep -F 'XAUTHORITY.set=yes' "$build_dir/hardware-report-wayland" >/dev/null
grep -F 'XAUTHORITY.readable=yes' "$build_dir/hardware-report-wayland" >/dev/null
grep -F 'No Wii Remote devices found' \
	"$build_dir/hardware-report-wayland" >/dev/null
grep -F 'report.core-failures=0' \
	"$build_dir/hardware-report-wayland" >/dev/null
grep -F 'report.status=ok' "$build_dir/hardware-report-wayland" >/dev/null
if grep -F 'device.1.syspath=' "$build_dir/hardware-report-wayland" >/dev/null; then
	printf '%s\n' 'hardware report invented a device for a valid empty list' >&2
	exit 1
fi
assert_manual_report_rows "$build_dir/hardware-report-wayland"
grep -F -- '--trace-events=ir' "$build_dir/hardware-report-wayland" >/dev/null

if (cd "$build_dir" && FAKE_LIST_MODE=malformed WIILANDD=$fake_wiilandd \
	"$root/tools/wiilandd-hardware-report.sh") \
	>"$build_dir/hardware-report-malformed" \
	2>"$build_dir/hardware-report-malformed-err"; then
	printf '%s\n' 'hardware report accepted a malformed device list' >&2
	exit 1
fi
grep -F 'device-list.row.1.malformed=invalid-index' \
	"$build_dir/hardware-report-malformed" >/dev/null
grep -F 'core.failure.1=wiilandd.list.malformed-row.1' \
	"$build_dir/hardware-report-malformed" >/dev/null
grep -F 'report.status=failed' "$build_dir/hardware-report-malformed" >/dev/null

if (cd "$build_dir" && WIILANDD=$build_dir/missing-wiilandd \
	"$root/tools/wiilandd-hardware-report.sh") \
	>"$build_dir/hardware-report-missing-daemon" \
	2>"$build_dir/hardware-report-missing-daemon-err"; then
	printf '%s\n' 'hardware report succeeded without wiilandd' >&2
	exit 1
fi
grep -F 'wiilandd.available=no' \
	"$build_dir/hardware-report-missing-daemon" >/dev/null
grep -F 'core.failure.1=wiilandd.missing' \
	"$build_dir/hardware-report-missing-daemon" >/dev/null
grep -F 'report.status=failed' \
	"$build_dir/hardware-report-missing-daemon" >/dev/null

if "$root/tools/wiilandd-hardware-report.sh" 7 --device 8 \
	>"$build_dir/hardware-report-conflict-device-out" \
	2>"$build_dir/hardware-report-conflict-device-err"; then
	printf '%s\n' 'hardware report accepted a conflicting device selector' >&2
	exit 1
fi
grep -F 'conflicting trace argument: --device' \
	"$build_dir/hardware-report-conflict-device-err" >/dev/null
if "$root/tools/wiilandd-hardware-report.sh" 7 --profile desktop \
	>"$build_dir/hardware-report-conflict-profile-out" \
	2>"$build_dir/hardware-report-conflict-profile-err"; then
	printf '%s\n' 'hardware report accepted a conflicting profile' >&2
	exit 1
fi
grep -F 'conflicting trace argument: --profile' \
	"$build_dir/hardware-report-conflict-profile-err" >/dev/null
if "$root/tools/wiilandd-hardware-report.sh" 7 --dry-run \
	>"$build_dir/hardware-report-conflict-dry-run-out" \
	2>"$build_dir/hardware-report-conflict-dry-run-err"; then
	printf '%s\n' 'hardware report accepted a conflicting dry-run flag' >&2
	exit 1
fi
grep -F 'conflicting trace argument: --dry-run' \
	"$build_dir/hardware-report-conflict-dry-run-err" >/dev/null
for action in \
	-h --help --version -l --list --calibrate-aim --check-config \
	--self-test --axis-map --validation-checklist --doctor --dump-config
do
	if "$root/tools/wiilandd-hardware-report.sh" 7 "$action" \
		>"$build_dir/hardware-report-conflict-action-out" \
		2>"$build_dir/hardware-report-conflict-action-err"; then
		printf '%s\n' \
			"hardware report accepted a conflicting daemon action: $action" >&2
		exit 1
	fi
	grep -F "conflicting trace argument: $action" \
		"$build_dir/hardware-report-conflict-action-err" >/dev/null
done
if "$root/tools/wiilandd-hardware-report.sh" 7 \
	--trace-events=ir --trace-events=motion-plus \
	>"$build_dir/hardware-report-conflict-trace-out" \
	2>"$build_dir/hardware-report-conflict-trace-err"; then
	printf '%s\n' 'hardware report accepted multiple trace selectors' >&2
	exit 1
fi
grep -F 'exactly one trace selector is allowed' \
	"$build_dir/hardware-report-conflict-trace-err" >/dev/null
mkdir -p "$build_dir/nonrepo"
(cd "$build_dir" && WIILAND_REPO_DIR=$build_dir/nonrepo \
	WIILANDD=$fake_wiilandd "$root/tools/wiilandd-hardware-report.sh" \
	7) >"$build_dir/hardware-report-nongit"
grep -F 'git.commit=unavailable' "$build_dir/hardware-report-nongit" >/dev/null
grep -F 'git.dirty=unavailable' "$build_dir/hardware-report-nongit" >/dev/null
test "$(sed -n '$p' "$build_dir/hardware-report")" = \
	"fake-wiilandd [--dry-run] [--verbose] [--device] [7] [--profile] [both] [--trace-events=motion-plus]"
grep -F 'PKG_CHECK_MODULES([QT6_WIDGETS], [Qt6Widgets]' "$root/configure.ac" >/dev/null
grep -F 'bin_PROGRAMS += wiiland-config' "$root/Makefile.am" >/dev/null
grep -F 'QApplication app(argc, argv);' "$root/tools/wiiland-config.cpp" >/dev/null
grep -F 'QTabWidget' "$root/tools/wiiland-config.cpp" >/dev/null
grep -F 'qOverload<int, QProcess::ExitStatus>(&QProcess::finished)' "$root/tools/wiiland-config.cpp" >/dev/null
grep -F 'process->start(program, arguments);' "$root/tools/wiiland-config.cpp" >/dev/null
grep -F 'desktopBindingNames()' "$root/tools/wiiland-config.cpp" >/dev/null
grep -F 'aim-mode=right-stick' "$root/res/wiilandd.conf" >/dev/null
grep -F 'aim-mode=right-stick' "$root/doc/wiilandd.1" >/dev/null
grep -F 'aimMode->addItems' "$root/tools/wiiland-config.cpp" >/dev/null
grep -F 'aim-calibration-duration=8' "$root/res/wiilandd.conf" >/dev/null
grep -F 'ir-tracking=dual' "$root/res/wiilandd.conf" >/dev/null
grep -F 'ir-aim-mapping' "$root/doc/wiilandd.1" >/dev/null
grep -F 'irTracking->addItems' "$root/tools/wiiland-config.cpp" >/dev/null
grep -F 'aim-accel-zero-x' "$root/doc/wiilandd.1" >/dev/null
grep -F 'aimCalibrationDuration' "$root/tools/wiiland-config.cpp" >/dev/null
grep -F -- '--calibrate-aim' "$root/tools/wiiland-config.cpp" >/dev/null




if command -v shellcheck >/dev/null 2>&1; then
	shellcheck "$root/tools/wiilandd-hardware-report.sh" \
		"$root/tests/wiilandd-install-smoke.sh" \
		"$root/tests/wiilandd-smoke.sh"
else
	printf '%s\n' 'warning: shellcheck not found; skipping shell smoke' >&2
fi

if command -v groff >/dev/null 2>&1; then
	groff -man -Tascii "$root/doc/wiilandd.1" >/dev/null
	groff -man -Tascii "$root/doc/wiiland.7" >/dev/null
	groff -man -Tascii "$root/doc/libxwiimote.7" >/dev/null
	groff -man -Tascii "$root/doc/xwiishow.1" >/dev/null
else
	printf '%s\n' 'warning: groff not found; skipping manpage render smoke' >&2
fi
