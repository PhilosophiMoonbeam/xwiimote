#!/bin/sh
# Verify staged WiiLand install paths that matter for Wayland/uinput packaging.
set -eu

stage=${1:?usage: wiilandd-install-smoke.sh <DESTDIR> [prefix]}
prefix=${2:-/usr}

case $prefix in
/*) ;;
*)
	printf '%s\n' "prefix must be absolute: $prefix" >&2
	exit 1
	;;
esac

rules=$stage$prefix/lib/udev/rules.d/70-wiiland-uinput.rules
service=$stage$prefix/lib/systemd/user/wiilandd.service
config=$stage$prefix/share/doc/wiiland/examples/wiilandd.conf
bin=$stage$prefix/bin/wiilandd

for path in "$rules" "$service" "$config" "$bin"; do
	if [ ! -e "$path" ]; then
		printf '%s\n' "missing staged install artifact: $path" >&2
		exit 1
	fi
done

[ -x "$bin" ] || {
	printf '%s\n' "installed wiilandd is not executable: $bin" >&2
	exit 1
}

grep -F 'KERNEL=="uinput"' "$rules" >/dev/null
grep -F 'TAG+="uaccess"' "$rules" >/dev/null
grep -F "ExecStart=$prefix/bin/wiilandd" "$service" >/dev/null
grep -F "ExecStartPre=$prefix/bin/wiilandd --check-config" "$service" >/dev/null
grep -F 'WantedBy=graphical-session.target' "$service" >/dev/null
grep -F 'backend=uinput' "$config" >/dev/null
grep -F 'profile=gamepad' "$config" >/dev/null
