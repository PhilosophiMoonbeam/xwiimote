#!/bin/sh
# Build and exercise wiilandd logic without requiring real Wii hardware.
set -eu

cc=${CC:-gcc}
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
build_dir=${TMPDIR:-/tmp}/wiilandd-smoke.$$
trap 'rm -rf "$build_dir"' EXIT INT HUP TERM
mkdir -p "$build_dir"
bin=$build_dir/wiilandd-smoke

"$cc" -std=gnu99 -Wall -Wextra -Werror -DPACKAGE_VERSION=\"smoke\" \
	-I"$root/lib" "$root/tools/wiilandd.c" "$root/tests/xwii_stubs.c" \
	-o "$bin"

test "$("$bin" --version)" = "wiilandd smoke"

"$bin" --self-test
"$bin" --config "$root/res/wiilandd.conf" --check-config
"$bin" --config "$root/res/wiilandd.conf" --dump-config >/dev/null
"$bin" --no-config --trace-events=motion-plus --dump-config >/dev/null
if "$bin" --no-config --trace-events=bad --dump-config >/dev/null 2>&1; then
	printf '%s\n' 'wiilandd accepted invalid trace event filter' >&2
	exit 1
fi
stub_sys=$build_dir/sys/devices/wiimote0
mkdir -p "$stub_sys"
printf '%s\n' wiimote >"$stub_sys/devtype"
printf '%s\n' nunchuk >"$stub_sys/extension"
XWII_STUB_DEVICES=$stub_sys "$bin" --list --verbose >"$build_dir/list"
test "$(sed -n '1p' "$build_dir/list")" = "1	$stub_sys"
test "$(sed -n '2p' "$build_dir/list")" = "	devtype=wiimote"
test "$(sed -n '3p' "$build_dir/list")" = "	extension=nunchuk"



if command -v groff >/dev/null 2>&1; then
	groff -man -Tascii "$root/doc/wiilandd.1" >/dev/null
else
	printf '%s\n' 'warning: groff not found; skipping wiilandd.1 render smoke' >&2
fi
