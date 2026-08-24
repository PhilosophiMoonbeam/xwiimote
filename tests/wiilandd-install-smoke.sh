#!/bin/sh
# Verify staged WiiLand install paths that matter for Wayland/uinput packaging.
set -eu

stage=${1:?usage: wiilandd-install-smoke.sh <DESTDIR> [prefix] [qt-ui]}
prefix=${2:-/usr}
qt_ui=${3:-no}

case $prefix in
/*) ;;
*)
	printf '%s\n' "prefix must be absolute: $prefix" >&2
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
service=$stage$prefix/lib/systemd/user/wiilandd.service
doc=$stage$prefix/share/doc/wiiland/WIILAND
config=$stage$prefix/share/doc/wiiland/examples/wiilandd.conf
bin=$stage$prefix/bin/wiilandd
man1=$stage$prefix/share/man/man1
man7=$stage$prefix/share/man/man7

for path in "$rules" "$service" "$doc" "$config" "$bin" \
	"$man1/xwiishow.1" "$man1/wiilandd.1" \
	"$man7/wiiland.7" "$man7/libxwiimote.7"; do
	if [ ! -e "$path" ]; then
		printf '%s\n' "missing staged install artifact: $path" >&2
		exit 1
	fi
done
if [ "$qt_ui" = yes ]; then
	gui=$stage$prefix/bin/wiiland-config
	if [ ! -x "$gui" ]; then
		printf '%s\n' "missing executable Qt frontend: $gui" >&2
		exit 1
	fi
fi

[ -x "$bin" ] || {
	printf '%s\n' "installed wiilandd is not executable: $bin" >&2
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
grep -F "ExecStart=$prefix/bin/wiilandd" "$service" >/dev/null
grep -F "ExecCondition=$prefix/bin/wiilandd --check-config" "$service" >/dev/null
grep -F 'WantedBy=graphical-session.target' "$service" >/dev/null
grep -F 'backend=uinput' "$config" >/dev/null
grep -F 'profile=gamepad' "$config" >/dev/null
