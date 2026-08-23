#!/bin/sh
# Build and exercise wiilandd logic without requiring real Wii hardware.
set -eu

cc=${CC:-gcc}
root=$(CDPATH=; cd -- "$(dirname -- "$0")/.." && pwd)
build_dir=${TMPDIR:-/tmp}/wiilandd-smoke.$$
trap 'rm -rf "$build_dir"' EXIT INT HUP TERM
mkdir -p "$build_dir"
bin=$build_dir/wiilandd-smoke

"$cc" -std=gnu99 -Wall -Wextra -Werror -DPACKAGE_VERSION=\"smoke\" \
	-I"$root/lib" "$root/tools/wiilandd.c" "$root/tests/xwii_stubs.c" \
	-o "$bin"

test "$("$bin" --version)" = "wiilandd smoke"
install_stage=$build_dir/install-stage
mkdir -p \
	"$install_stage/usr/bin" \
	"$install_stage/usr/lib/udev/rules.d" \
	"$install_stage/usr/lib/systemd/user" \
	"$install_stage/usr/share/doc/wiiland/examples"
cp "$bin" "$install_stage/usr/bin/wiilandd"
cp "$root/res/70-wiiland-uinput.rules" \
	"$install_stage/usr/lib/udev/rules.d/70-wiiland-uinput.rules"
sed -e 's|@bindir@|/usr/bin|g' "$root/res/wiilandd.service.in" \
	>"$install_stage/usr/lib/systemd/user/wiilandd.service"
cp "$root/res/wiilandd.conf" \
	"$install_stage/usr/share/doc/wiiland/examples/wiilandd.conf"
"$root/tests/wiilandd-install-smoke.sh" "$install_stage" /usr


"$bin" --self-test
"$bin" --config "$root/res/wiilandd.conf" --check-config
"$bin" --config "$root/res/wiilandd.conf" --dump-config >/dev/null
"$bin" --axis-map >"$build_dir/axis-map"
grep -F 'nunchuk.accel.x=ABS_HAT1X' "$build_dir/axis-map" >/dev/null
grep -F 'wiimote.a=BTN_SOUTH' "$build_dir/axis-map" >/dev/null
grep -F 'pro.zl=BTN_TL2' "$build_dir/axis-map" >/dev/null
grep -F 'guitar.stick.x=ABS_X' "$build_dir/axis-map" >/dev/null
grep -F 'classic.zl=BTN_TL2' "$build_dir/axis-map" >/dev/null
grep -F 'guitar.fret.mid=BTN_FRET_MID' "$build_dir/axis-map" >/dev/null
grep -F 'guitar.home=BTN_MODE' "$build_dir/axis-map" >/dev/null
grep -F 'drums.minus=BTN_SELECT' "$build_dir/axis-map" >/dev/null
"$bin" --validation-checklist >"$build_dir/validation-checklist"
grep -F 'motion-plus-external.hotplug=required' "$build_dir/validation-checklist" >/dev/null
grep -F 'wayland.wine-proton=required' "$build_dir/validation-checklist" >/dev/null
cat >"$build_dir/doctor.conf" <<'EOF'
backend=uinput
profile=desktop
EOF
touch "$build_dir/wayland-1"
WAYLAND_DISPLAY=wayland-1 XDG_RUNTIME_DIR=$build_dir \
	"$bin" --config "$build_dir/doctor.conf" --doctor >"$build_dir/doctor"
grep -F 'session.wayland=yes' "$build_dir/doctor" >/dev/null
grep -F "wayland.socket.path=$build_dir/wayland-1" "$build_dir/doctor" >/dev/null
grep -F 'wayland.socket.type=other' "$build_dir/doctor" >/dev/null
grep -F 'wayland.socket.exists=yes' "$build_dir/doctor" >/dev/null
grep -F 'dev.uinput.writable=' "$build_dir/doctor" >/dev/null
grep -F 'backend=uinput' "$build_dir/doctor" >/dev/null
grep -F 'profile=desktop' "$build_dir/doctor" >/dev/null
"$bin" --no-config --trace-events=ir --dump-config >/dev/null
"$bin" --no-config --trace-events=motion-plus --dump-config >/dev/null
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
	grep -F 'wiilandd: cannot open all interfaces for /sys/fake: -6' "$build_dir/open-fail-err" >/dev/null
	grep -F 'wiilandd: cannot add /sys/fake: -6' "$build_dir/open-fail-err" >/dev/null
else
	printf '%s\n' 'wiilandd failed open-failure smoke' >&2
	exit 1
fi
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
	exit 0
	;;
--validation-checklist)
	printf '%s\n' 'wayland.wine-proton=required'
	exit 0
	;;
--doctor)
	printf '%s\n' 'dev.uinput.writable=no'
	exit 0
	;;
--list)
	printf '1\t%s\n' "$FAKE_DEVICE_SYSPATH"
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
chmod +x "$fake_wiilandd"
"$root/tools/wiilandd-hardware-report.sh" --help >"$build_dir/hardware-report-help"
grep -F 'Usage:' "$build_dir/hardware-report-help" >/dev/null
grep -F '<number-or-/sys/path>' "$build_dir/hardware-report-help" >/dev/null
grep -F 'doctor, axis-map' "$build_dir/hardware-report-help" >/dev/null
grep -F 'WantedBy=graphical-session.target' "$root/res/wiilandd.service" >/dev/null
grep -F 'ExecStart=@bindir@/wiilandd' "$root/res/wiilandd.service.in" >/dev/null
if command -v systemd-analyze >/dev/null 2>&1; then
	sed -e 's|@bindir@/wiilandd|/bin/true|g' \
		"$root/res/wiilandd.service.in" >"$build_dir/wiilandd.service"
	systemd-analyze verify --man=no "$build_dir/wiilandd.service"
fi
(cd "$build_dir" && XDG_CURRENT_DESKTOP=TestDesktop SWAYSOCK=/tmp/sway.sock \
	FAKE_DEVICE_SYSPATH=$stub_sys WIILANDD=$fake_wiilandd \
	"$root/tools/wiilandd-hardware-report.sh" \
	7 --trace-events=motion-plus) >"$build_dir/hardware-report"
if git_commit=$(git -C "$root" rev-parse --short HEAD 2>/dev/null); then
	expected_commit=git.commit=$git_commit
else
	expected_commit=git.commit=unavailable
fi
grep -F 'report.schema.version=1' "$build_dir/hardware-report" >/dev/null
grep -F 'report.timestamp.utc=' "$build_dir/hardware-report" >/dev/null
grep -F "$expected_commit" "$build_dir/hardware-report" >/dev/null
grep -F 'git.dirty=' "$build_dir/hardware-report" >/dev/null
grep -F 'XDG_CURRENT_DESKTOP=TestDesktop' "$build_dir/hardware-report" >/dev/null
grep -F 'SWAYSOCK=/tmp/sway.sock' "$build_dir/hardware-report" >/dev/null
grep -F 'manual.sdl=TODO:' "$build_dir/hardware-report" >/dev/null
grep -F 'manual.wine-proton=TODO:' "$build_dir/hardware-report" >/dev/null
grep -F 'manual.native-wayland-desktop=TODO:' "$build_dir/hardware-report" >/dev/null
grep -F '$ '"$fake_wiilandd"' --axis-map' "$build_dir/hardware-report" >/dev/null
grep -F 'device.1.uevent.HID_NAME=Nintendo Wii Remote' "$build_dir/hardware-report" >/dev/null
grep -F 'nunchuk.accel.x=ABS_HAT1X' "$build_dir/hardware-report" >/dev/null
grep -F '$ '"$fake_wiilandd"' --validation-checklist' "$build_dir/hardware-report" >/dev/null
grep -F 'wayland.wine-proton=required' "$build_dir/hardware-report" >/dev/null
grep -F '$ '"$fake_wiilandd"' --doctor' "$build_dir/hardware-report" >/dev/null
grep -F 'dev.uinput.writable=no' "$build_dir/hardware-report" >/dev/null
(cd "$build_dir" && WIILANDD=$fake_wiilandd \
	"$root/tools/wiilandd-hardware-report.sh") >"$build_dir/hardware-report-no-device"
grep -F -- '--trace-events=ir' "$build_dir/hardware-report-no-device" >/dev/null
mkdir -p "$build_dir/nonrepo"
(cd "$build_dir" && WIILAND_REPO_DIR=$build_dir/nonrepo \
	WIILANDD=$fake_wiilandd "$root/tools/wiilandd-hardware-report.sh" \
	7) >"$build_dir/hardware-report-nongit"
grep -F 'git.commit=unavailable' "$build_dir/hardware-report-nongit" >/dev/null
grep -F 'git.dirty=unavailable' "$build_dir/hardware-report-nongit" >/dev/null
grep -F 'fake-wiilandd [--dry-run] [--trace-events] [--verbose] [--device] [7] [--profile] [both] [--trace-events=motion-plus]' \
	"$build_dir/hardware-report" >/dev/null




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
else
	printf '%s\n' 'warning: groff not found; skipping manpage render smoke' >&2
fi
