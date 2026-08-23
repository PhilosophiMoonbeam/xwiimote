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

"$bin" --self-test
"$bin" --config "$root/res/wiilandd.conf" --check-config
"$bin" --config "$root/res/wiilandd.conf" --dump-config >/dev/null
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
XWII_STUB_DEVICES=$stub_sys:$stub_sys_missing "$bin" --list --verbose >"$build_dir/list"
test "$(sed -n '1p' "$build_dir/list")" = "1	$stub_sys"
test "$(sed -n '2p' "$build_dir/list")" = "	devtype=wiimote"
test "$(sed -n '3p' "$build_dir/list")" = "	extension=nunchuk"
test "$(sed -n '4p' "$build_dir/list")" = "2	$stub_sys_missing"
test "$(sed -n '5p' "$build_dir/list")" = "	devtype=unavailable"
test "$(sed -n '6p' "$build_dir/list")" = "	extension=unavailable"
fake_wiilandd=$build_dir/fake-wiilandd
cat >"$fake_wiilandd" <<'EOF'
#!/bin/sh
printf 'fake-wiilandd'
for arg do
	printf ' [%s]' "$arg"
done
printf '\n'
exit 0
EOF
chmod +x "$fake_wiilandd"
(cd "$build_dir" && WIILANDD=$fake_wiilandd "$root/tools/wiilandd-hardware-report.sh" \
	7 --trace-events=motion-plus) >"$build_dir/hardware-report"
if git_commit=$(git -C "$root" rev-parse --short HEAD 2>/dev/null); then
	expected_commit=git.commit=$git_commit
else
	expected_commit=git.commit=unavailable
fi
grep -F "$expected_commit" "$build_dir/hardware-report" >/dev/null
grep -F 'git.dirty=' "$build_dir/hardware-report" >/dev/null
mkdir -p "$build_dir/nonrepo"
(cd "$build_dir" && WIILAND_REPO_DIR=$build_dir/nonrepo \
	WIILANDD=$fake_wiilandd "$root/tools/wiilandd-hardware-report.sh" \
	7) >"$build_dir/hardware-report-nongit"
grep -F 'git.commit=unavailable' "$build_dir/hardware-report-nongit" >/dev/null
grep -F 'git.dirty=unavailable' "$build_dir/hardware-report-nongit" >/dev/null
grep -F 'fake-wiilandd [--dry-run] [--trace-events] [--verbose] [--device] [7] [--profile] [both] [--trace-events=motion-plus]' \
	"$build_dir/hardware-report" >/dev/null




if command -v shellcheck >/dev/null 2>&1; then
	shellcheck "$root/tools/wiilandd-hardware-report.sh" "$root/tests/wiilandd-smoke.sh"
else
	printf '%s\n' 'warning: shellcheck not found; skipping shell smoke' >&2
fi

if command -v groff >/dev/null 2>&1; then
	groff -man -Tascii "$root/doc/wiilandd.1" >/dev/null
else
	printf '%s\n' 'warning: groff not found; skipping wiilandd.1 render smoke' >&2
fi
