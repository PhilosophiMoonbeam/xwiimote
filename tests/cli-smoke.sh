#!/bin/sh
# Exercise the hardware-free command-line contracts of xwiishow and xwiidump.
set -eu

LC_ALL=C
export LC_ALL

root=$(CDPATH=; cd -- "$(dirname -- "$0")/.." && pwd)
xwiishow=${XWIISHOW:-$root/xwiishow}
xwiidump=${XWIIDUMP:-$root/xwiidump}
cc=${CC:-cc}
tmp=${TMPDIR:-/tmp}/wiiland-cli-smoke.$$
trap 'rm -rf "$tmp"' EXIT INT HUP TERM
mkdir -p "$tmp"

fail() {
	printf '%s\n' "$1" >&2
	exit 1
}

[ -x "$xwiishow" ] || fail "xwiishow is not executable: $xwiishow"
[ -x "$xwiidump" ] || fail "xwiidump is not executable: $xwiidump"

"$xwiishow" --help >"$tmp/xwiishow-help" 2>"$tmp/xwiishow-help-err"
[ ! -s "$tmp/xwiishow-help-err" ] || fail 'xwiishow --help wrote to stderr'
grep -F 'xwiishow <positive-ordinal>' "$tmp/xwiishow-help" >/dev/null
grep -F 'xwiishow /sys/path/to/device' "$tmp/xwiishow-help" >/dev/null
grep -F 'q: Quit application' "$tmp/xwiishow-help" >/dev/null

"$xwiishow" list >"$tmp/xwiishow-list" 2>"$tmp/xwiishow-list-err"
[ ! -s "$tmp/xwiishow-list-err" ] || fail 'xwiishow list wrote to stderr'
awk -F '\t' '
	NF && (NF != 2 || $1 !~ /^[1-9][0-9]*$/ || $2 !~ /^\/sys\//) {
		exit 1
	}
' "$tmp/xwiishow-list" || fail 'xwiishow list emitted a malformed row'

if "$xwiishow" 0 >"$tmp/xwiishow-invalid-out" 2>"$tmp/xwiishow-invalid-err"; then
	fail 'xwiishow accepted an invalid selector'
fi
[ ! -s "$tmp/xwiishow-invalid-out" ] || fail 'xwiishow invalid selector wrote to stdout'
grep -F 'selector must be a positive ordinal or an absolute /sys path: 0' \
	"$tmp/xwiishow-invalid-err" >/dev/null

if "$xwiishow" >"$tmp/xwiishow-missing-out" 2>"$tmp/xwiishow-missing-err"; then
	fail 'xwiishow accepted a missing selector'
fi
[ ! -s "$tmp/xwiishow-missing-out" ] || fail 'xwiishow missing selector wrote to stdout'
grep -F 'xwiishow: expected exactly one selector' \
	"$tmp/xwiishow-missing-err" >/dev/null
grep -F 'xwiishow list' "$tmp/xwiishow-missing-err" >/dev/null

if "$xwiishow" 1 2 >"$tmp/xwiishow-surplus-out" \
	2>"$tmp/xwiishow-surplus-err"; then
	fail 'xwiishow accepted surplus selectors'
fi
[ ! -s "$tmp/xwiishow-surplus-out" ] || fail 'xwiishow surplus selectors wrote to stdout'
grep -F 'xwiishow: expected exactly one selector' \
	"$tmp/xwiishow-surplus-err" >/dev/null

# Interpose only the constructor path so the real xwiishow reaches its terminal
# guard without requiring a physical Wii Remote.
cat >"$tmp/xwii-nontty-shim.c" <<'EOF'
struct xwii_iface;

int xwii_iface_new(struct xwii_iface **iface, const char *syspath)
{
	static int fake_iface;
	(void)syspath;
	*iface = (struct xwii_iface *)&fake_iface;
	return 0;
}

const char *xwii_iface_get_syspath(struct xwii_iface *iface)
{
	(void)iface;
	return "/sys/fake-wii-remote";
}

void xwii_iface_unref(struct xwii_iface *iface)
{
	(void)iface;
}
EOF
"$cc" -shared -fPIC "$tmp/xwii-nontty-shim.c" -o "$tmp/xwii-nontty-shim.so"
if LD_PRELOAD="$tmp/xwii-nontty-shim.so" "$xwiishow" /sys/fake-wii-remote \
	</dev/null >"$tmp/xwiishow-nontty-out" 2>"$tmp/xwiishow-nontty-err"; then
	fail 'xwiishow accepted non-terminal stdin'
fi
printf '%s\n' 'Using Wii Remote: /sys/fake-wii-remote' \
	>"$tmp/xwiishow-nontty-expected"
cmp "$tmp/xwiishow-nontty-expected" "$tmp/xwiishow-nontty-out"
grep -F 'interactive UI requires a terminal on stdin' \
	"$tmp/xwiishow-nontty-err" >/dev/null

"$xwiidump" --help >"$tmp/xwiidump-help" 2>"$tmp/xwiidump-help-err"
[ ! -s "$tmp/xwiidump-help-err" ] || fail 'xwiidump --help wrote to stderr'
grep -F 'Usage:' "$tmp/xwiidump-help" >/dev/null
grep -F 'Read a Wii Remote EEPROM file and write its contents to stdout.' \
	"$tmp/xwiidump-help" >/dev/null

if "$xwiidump" >"$tmp/xwiidump-missing-out" 2>"$tmp/xwiidump-missing-err"; then
	fail 'xwiidump accepted a missing EEPROM file'
fi
[ ! -s "$tmp/xwiidump-missing-out" ] || fail 'xwiidump missing argument wrote to stdout'
grep -F 'Usage:' "$tmp/xwiidump-missing-err" >/dev/null

if "$xwiidump" one two >"$tmp/xwiidump-surplus-out" \
	2>"$tmp/xwiidump-surplus-err"; then
	fail 'xwiidump accepted surplus arguments'
fi
[ ! -s "$tmp/xwiidump-surplus-out" ] || fail 'xwiidump surplus arguments wrote to stdout'
grep -F 'Usage:' "$tmp/xwiidump-surplus-err" >/dev/null

missing=$tmp/no-such-eeprom
if "$xwiidump" "$missing" >"$tmp/xwiidump-nonexistent-out" \
	2>"$tmp/xwiidump-nonexistent-err"; then
	fail 'xwiidump accepted a nonexistent EEPROM file'
fi
[ ! -s "$tmp/xwiidump-nonexistent-out" ] || fail 'xwiidump nonexistent file wrote to stdout'
grep -F "Cannot open eeprom file '$missing':" \
	"$tmp/xwiidump-nonexistent-err" >/dev/null

printf '\000\001\002\177\200\376\377\125' >"$tmp/eeprom-complete"
"$xwiidump" "$tmp/eeprom-complete" >"$tmp/xwiidump-complete-out" \
	2>"$tmp/xwiidump-complete-err"
[ ! -s "$tmp/xwiidump-complete-err" ] || fail 'xwiidump complete fixture wrote to stderr'
printf '0x00000000: 0x00 0x01 0x02 0x7f 0x80 0xfe 0xff 0x55\n0x00000008: (eof)' \
	>"$tmp/xwiidump-complete-expected"
cmp "$tmp/xwiidump-complete-expected" "$tmp/xwiidump-complete-out"

printf '\020\040\377' >"$tmp/eeprom-partial"
if "$xwiidump" "$tmp/eeprom-partial" >"$tmp/xwiidump-partial-out" \
	2>"$tmp/xwiidump-partial-err"; then
	fail 'xwiidump accepted a partial EEPROM record'
fi
printf '0x00000000: 0x10 0x20 0xff (eof)' >"$tmp/xwiidump-partial-expected"
cmp "$tmp/xwiidump-partial-expected" "$tmp/xwiidump-partial-out"
grep -F "Unexpected end of eeprom file '$tmp/eeprom-partial' at offset 0x00000003" \
	"$tmp/xwiidump-partial-err" >/dev/null
