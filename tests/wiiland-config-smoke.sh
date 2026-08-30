#!/bin/sh
set -eu

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    printf '%s\n' "usage: $0 <expected-qpa> [binary]" >&2
    exit 2
fi

expected=$1
if [ -z "$expected" ]; then
    printf '%s\n' "wiiland-config smoke: expected QPA must not be empty" >&2
    exit 2
fi

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
binary=${2:-$root/wiiland-config}
if [ ! -x "$binary" ]; then
    printf '%s\n' "wiiland-config smoke: binary is not executable: $binary" >&2
    exit 2
fi

capture=$(mktemp "${TMPDIR:-/tmp}/wiiland-config-smoke.XXXXXX")
errors=$(mktemp "${TMPDIR:-/tmp}/wiiland-config-smoke-errors.XXXXXX")
smoke_home=$(mktemp -d "${TMPDIR:-/tmp}/wiiland-config-smoke-home.XXXXXX")
trap 'rm -rf "$capture" "$errors" "$smoke_home"' EXIT INT HUP TERM

set +e
HOME=$smoke_home XDG_CONFIG_HOME=relative WIILAND_CONFIG_SMOKE_TEST=1 \
    "$binary" >"$capture" 2>"$errors"
status=$?
set -e

if [ "$status" -ne 0 ]; then
    printf '%s\n' "wiiland-config smoke: binary exited with status $status" >&2
    cat "$capture" >&2
    cat "$errors" >&2
    exit 1
fi
if ! {
    printf '%s\n' \
        "qt.platform=$expected" \
        'service.restart.explicit-config=disabled' \
        'calibration.partial-source=isolated' \
        'config.choice-values=canonical' \
        'config.compact-layout=responsive' \
        'config.default-path=absolute' \
        'config.unsaved-state=tracked' \
        'output.actions=available' \
        'output.buffer=bounded' \
        'validation.controls=coordinated' \
        'validation.form=visible'
} | cmp -s - "$capture"; then
    printf '%s\n' \
        "wiiland-config smoke: unexpected platform or UI state" >&2
    printf '%s\n' 'binary standard output:' >&2
    cat "$capture" >&2
    printf '%s\n' 'binary standard error:' >&2
    cat "$errors" >&2
    exit 1
fi
