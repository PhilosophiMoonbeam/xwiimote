#!/bin/sh
# Verify staged WiiLand install paths for the shared Linux input stack.
set -eu

usage='usage: wiilandd-install-smoke.sh <DESTDIR> [prefix] [qt-ui] [sysconfdir]'
case $# in
1|2|3|4) ;;
*)
	printf '%s\n' "$usage" >&2
	exit 1
	;;
esac
stage=$1
prefix=${2:-/usr}
qt_ui=${3:-no}
sysconfdir=${4:-$prefix/etc}

case $stage in
/*) ;;
*)
	printf '%s\n' "DESTDIR must be absolute: $stage" >&2
	exit 1
	;;
esac

case $prefix in
/*) ;;
*)
	printf '%s\n' "prefix must be absolute: $prefix" >&2
	exit 1
	;;
esac
case $sysconfdir in
/*) ;;
*)
	printf '%s\n' "sysconfdir must be absolute: $sysconfdir" >&2
	exit 1
	;;
esac
case $qt_ui in
yes|no) ;;
*)
	printf '%s\n' "qt-ui must be yes or no: $qt_ui" >&2
	exit 1
	;;
esac

rules=$stage$prefix/lib/udev/rules.d/70-wiiland-uinput.rules
led_rules=$stage$prefix/lib/udev/rules.d/70-udev-wiiland.rules
service=$stage$prefix/lib/systemd/user/wiilandd.service
xorg=$stage$prefix/share/X11/xorg.conf.d/50-xorg-fix-wiiland.conf
doc=$stage$prefix/share/doc/wiiland/WIILAND
config=$stage$prefix/share/doc/wiiland/examples/wiilandd.conf
system_config=$stage$sysconfdir/wiiland/wiilandd.conf
bin=$stage$prefix/bin/wiilandd
report=$stage$prefix/bin/wiilandd-hardware-report
man1=$stage$prefix/share/man/man1
man7=$stage$prefix/share/man/man7

for path in "$rules" "$led_rules" "$service" "$xorg" "$doc" "$config" \
	"$system_config" "$bin" "$report" \
	"$man1/xwiishow.1" "$man1/wiilandd.1" \
	"$man7/wiiland.7" "$man7/libxwiimote.7"; do
	if [ ! -e "$path" ]; then
		printf '%s\n' "missing staged install artifact: $path" >&2
		exit 1
	fi
done
gui=$stage$prefix/bin/wiiland-config
desktop=$stage$prefix/share/applications/io.github.philosophimoonbeam.wiiland-config.desktop
icon=$stage$prefix/share/icons/hicolor/scalable/apps/io.github.philosophimoonbeam.wiiland.svg
gui_man=$man1/wiiland-config.1
if [ "$qt_ui" = yes ]; then
	for path in "$gui" "$desktop" "$icon" "$gui_man"; do
		if [ ! -e "$path" ]; then
			printf '%s\n' "missing staged Qt artifact: $path" >&2
			exit 1
		fi
	done
	if [ ! -x "$gui" ]; then
		printf '%s\n' "Qt frontend is not executable: $gui" >&2
		exit 1
	fi
else
	for path in "$gui" "$desktop" "$icon" "$gui_man"; do
		if [ -e "$path" ]; then
			printf '%s\n' "unexpected staged Qt artifact: $path" >&2
			exit 1
		fi
	done
fi

[ -x "$bin" ] || {
	printf '%s\n' "installed wiilandd is not executable: $bin" >&2
	exit 1
}
[ -x "$report" ] || {
	printf '%s\n' "installed hardware report is not executable: $report" >&2
	exit 1
}

grep -F 'SUBSYSTEM=="input"' "$rules" >/dev/null
grep -F 'DRIVERS=="wiimote"' "$rules" >/dev/null
grep -F 'KERNEL=="uinput"' "$rules" >/dev/null
grep -F 'SUBSYSTEM=="misc"' "$rules" >/dev/null
grep -F 'OPTIONS+="static_node=uinput"' "$rules" >/dev/null
grep -F 'TAG+="uaccess"' "$rules" >/dev/null
grep -F 'GROUP="input"' "$rules" >/dev/null
grep -F 'MODE="0660"' "$rules" >/dev/null
grep -F 'SUBSYSTEM=="leds"' "$led_rules" >/dev/null
grep -F 'brightness' "$led_rules" >/dev/null
grep -F 'MatchProduct "Nintendo Wii Remote"' "$xorg" >/dev/null
grep -F 'Option "Ignore" "on"' "$xorg" >/dev/null
grep -F "ExecStart=$prefix/bin/wiilandd" "$service" >/dev/null
grep -F "ExecCondition=$prefix/bin/wiilandd --check-config" "$service" >/dev/null
grep -F 'WantedBy=default.target' "$service" >/dev/null
grep -F 'backend=uinput' "$config" >/dev/null
grep -F 'profile=gamepad' "$config" >/dev/null
grep -F 'backend=uinput' "$system_config" >/dev/null
grep -F 'profile=gamepad' "$system_config" >/dev/null

report_help=${TMPDIR:-/tmp}/wiilandd-report-help.$$
trap 'rm -f "$report_help"' EXIT INT HUP TERM
"$report" --help >"$report_help"
grep -F 'wiilandd-hardware-report <number-or-/sys/path>' "$report_help" >/dev/null

grep -F 'ACTION=="add"' "$led_rules" >/dev/null
grep -F 'SUBSYSTEM=="leds"' "$led_rules" >/dev/null
grep -F 'DRIVERS=="wiimote"' "$led_rules" >/dev/null
grep -F 'RUN{program}+="/usr/bin/chgrp input /sys%p/brightness"' \
	"$led_rules" >/dev/null
grep -F 'RUN{program}+="/usr/bin/chmod g+w /sys%p/brightness"' \
	"$led_rules" >/dev/null
grep -F 'Section "InputClass"' "$xorg" >/dev/null
grep -F 'Identifier "Nintendo Wii Remote Raw Input Blacklist"' "$xorg" >/dev/null
grep -F 'Identifier "Nintendo Wii Remote Classic Controller Whitelist"' \
	"$xorg" >/dev/null
grep -F 'Identifier "Nintendo Wii Remote Pro Controller Whitelist"' \
	"$xorg" >/dev/null
grep -F 'MatchDevicePath "/dev/input/event*"' "$xorg" >/dev/null

grep -F '[Unit]' "$service" >/dev/null
grep -F '[Service]' "$service" >/dev/null
grep -F 'Type=simple' "$service" >/dev/null
grep -F 'Restart=on-failure' "$service" >/dev/null
grep -F '[Install]' "$service" >/dev/null
if grep -F '@bindir@' "$service" >/dev/null; then
	printf '%s\n' "unsubstituted bindir in staged service: $service" >&2
	exit 1
fi

grep -F 'aim-mode=right-stick' "$config" >/dev/null
grep -F 'aim-mode=right-stick' "$system_config" >/dev/null
cmp "$config" "$system_config"

if [ "$qt_ui" = yes ]; then
	grep -F 'Type=Application' "$desktop" >/dev/null
	grep -F 'Exec=wiiland-config' "$desktop" >/dev/null
	grep -F 'TryExec=wiiland-config' "$desktop" >/dev/null
	grep -F 'Icon=io.github.philosophimoonbeam.wiiland' "$desktop" >/dev/null
	grep -F '<svg' "$icon" >/dev/null
	grep -F 'wiiland-config' "$gui_man" >/dev/null
fi
