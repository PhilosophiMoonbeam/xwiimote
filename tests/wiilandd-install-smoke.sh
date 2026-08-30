#!/bin/sh
# Verify staged WiiLand install paths for the shared Linux input stack.
# Set WIILAND_INSTALL_BINDIR for a staged bindir override. Set
# WIILAND_BUILD_DIR to a Doxygen-enabled build directory to exercise
# install/uninstall ownership, and WIILAND_DIST_ARCHIVE to inspect an archive.
set -eu

usage='usage: wiilandd-install-smoke.sh <DESTDIR> [prefix] [qt-ui] [sysconfdir] [xwiishow]'
case $# in
1|2|3|4|5) ;;
*)
	printf '%s\n' "$usage" >&2
	exit 1
	;;
esac
stage=$1
prefix=${2:-/usr}
qt_ui=${3:-no}
sysconfdir=${4:-$prefix/etc}
xwiishow_enabled=${5:-yes}
bindir=${WIILAND_INSTALL_BINDIR:-$prefix/bin}

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
case $bindir in
/*) ;;
*)
	printf '%s\n' "bindir must be absolute: $bindir" >&2
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
case $xwiishow_enabled in
yes|no) ;;
*)
	printf '%s\n' "xwiishow must be yes or no: $xwiishow_enabled" >&2
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
bin=$stage$bindir/wiilandd
report=$stage$bindir/wiilandd-hardware-report
man1=$stage$prefix/share/man/man1
man7=$stage$prefix/share/man/man7

for path in "$rules" "$led_rules" "$service" "$xorg" "$doc" "$config" \
	"$system_config" "$bin" "$report" "$man1/wiilandd.1" \
	"$man7/wiiland.7" "$man7/libxwiimote.7"; do
	if [ ! -e "$path" ]; then
		printf '%s\n' "missing staged install artifact: $path" >&2
		exit 1
	fi
done
xwiishow=$stage$bindir/xwiishow
xwiishow_man=$man1/xwiishow.1
if [ "$xwiishow_enabled" = yes ]; then
	for path in "$xwiishow" "$xwiishow_man"; do
		if [ ! -e "$path" ]; then
			printf '%s\n' "missing staged xwiishow artifact: $path" >&2
			exit 1
		fi
	done
	if [ ! -x "$xwiishow" ]; then
		printf '%s\n' "xwiishow is not executable: $xwiishow" >&2
		exit 1
	fi
else
	for path in "$xwiishow" "$xwiishow_man"; do
		if [ -e "$path" ]; then
			printf '%s\n' "unexpected staged xwiishow artifact: $path" >&2
			exit 1
		fi
	done
fi
gui=$stage$bindir/wiiland-config
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
grep -F "ExecStart=$bindir/wiilandd" "$service" >/dev/null
grep -F "ExecCondition=$bindir/wiilandd --check-config" "$service" >/dev/null
grep -F 'WantedBy=default.target' "$service" >/dev/null
grep -F 'backend=uinput' "$config" >/dev/null
grep -F 'profile=gamepad' "$config" >/dev/null
grep -F 'backend=uinput' "$system_config" >/dev/null
grep -F 'profile=gamepad' "$system_config" >/dev/null

smoke_tmp=$(mktemp -d "${TMPDIR:-/tmp}/wiilandd-install-smoke.XXXXXX")
case $smoke_tmp in
/*) ;;
*) smoke_tmp=$(pwd)/$smoke_tmp ;;
esac
contract_fixture_dir=
contract_service_build=
contract_service_backup=
contract_service_existed=no
cleanup() {
	cleanup_status=$?
	trap - EXIT
	set +e
	if [ -n "$contract_fixture_dir" ]; then
		rm -rf "$contract_fixture_dir"
	fi
	if [ -n "$contract_service_build" ]; then
		if [ "$contract_service_existed" = yes ]; then
			cp -p "$contract_service_backup" "$contract_service_build"
		else
			rm -f "$contract_service_build"
		fi
	fi
	rm -rf "$smoke_tmp"
	exit "$cleanup_status"
}
trap cleanup EXIT
trap 'exit 1' INT HUP TERM
report_help=$smoke_tmp/report-help
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
grep -F 'NoNewPrivileges=yes' "$service" >/dev/null
grep -F 'LockPersonality=yes' "$service" >/dev/null
grep -F 'MemoryDenyWriteExecute=yes' "$service" >/dev/null
grep -F 'RestrictRealtime=yes' "$service" >/dev/null
grep -F 'RestrictSUIDSGID=yes' "$service" >/dev/null
grep -F 'SystemCallArchitectures=native' "$service" >/dev/null
grep -F 'UMask=0077' "$service" >/dev/null
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

fake_wiilandd=$smoke_tmp/fake-wiilandd
cat >"$fake_wiilandd" <<'EOF'
#!/bin/sh
case "${1:-}" in
--list)
	printf '%s\n' 'No Wii Remote devices found'
	exit 0
	;;
--dry-run)
	exit 23
	;;
*)
	exit 0
	;;
esac
EOF
chmod +x "$fake_wiilandd"
mkdir "$smoke_tmp/repo"

assert_trace_cleanup() {
	trace_name=$1
	shift
	trace_tmp=$smoke_tmp/trace-$trace_name
	mkdir "$trace_tmp"
	if TMPDIR=$trace_tmp WIILANDD=$fake_wiilandd \
		WIILAND_REPO_DIR=$smoke_tmp/repo \
		"$report" 1 "$@" >"$trace_tmp/output" 2>&1; then
		trace_status=0
	else
		trace_status=$?
	fi
	if [ "$trace_status" -ne 23 ]; then
		printf '%s\n' \
			"hardware report did not preserve daemon status: $trace_status" >&2
		exit 1
	fi
	set -- "$trace_tmp"/wiilandd-hardware-report.*
	if [ -e "$1" ]; then
		printf '%s\n' "hardware report left temporary data: $1" >&2
		exit 1
	fi
}

assert_trace_cleanup default
assert_trace_cleanup selected --trace-events=ir

if [ -n "${WIILAND_DIST_ARCHIVE:-}" ]; then
	archive_list=$smoke_tmp/archive-list
	tar -tf "$WIILAND_DIST_ARCHIVE" >"$archive_list"
	archive_root=$(sed -n '1s,/.*,,p' "$archive_list")
	if [ -z "$archive_root" ]; then
		printf '%s\n' "archive has no package root: $WIILAND_DIST_ARCHIVE" >&2
		exit 1
	fi
	for archive_path in README.md doc/DEVICES doc/PROTOCOL doc/DEV_REMOTE; do
		if ! grep -Fx "$archive_root/$archive_path" "$archive_list" >/dev/null; then
			printf '%s\n' \
				"missing release archive artifact: $archive_path" >&2
			exit 1
		fi
	done
fi

if [ -n "${WIILAND_BUILD_DIR:-}" ]; then
	make_command=${MAKE:-make}
	contract_stage=$smoke_tmp/install-contract
	contract_bindir=/opt/wiiland-smoke/bin
	contract_htmldir=/share/doc
	contract_systemdunitdir=/share/systemd/user
	contract_html=$contract_stage$contract_htmldir/wiiland
	contract_service=$contract_stage$contract_systemdunitdir/wiilandd.service
	sentinel=$contract_stage$contract_htmldir/wiiland-install-smoke.sentinel
	mkdir -p "$contract_stage$contract_htmldir"
	: >"$sentinel"
	contract_service_path=$WIILAND_BUILD_DIR/res/wiilandd.service
	contract_service_backup=$smoke_tmp/wiilandd.service.before
	if [ -e "$contract_service_path" ]; then
		cp -p "$contract_service_path" "$contract_service_backup"
		contract_service_existed=yes
	fi
	contract_service_build=$contract_service_path

	contract_fixture_dir=$(mktemp -d \
		"$WIILAND_BUILD_DIR/doc/html/wiiland-install-smoke.XXXXXX")
	contract_fixture_name=${contract_fixture_dir##*/}
	contract_fixture=$contract_fixture_dir/search/search.js
	contract_installed_fixture=$contract_html/$contract_fixture_name/search/search.js
	mkdir "$contract_fixture_dir/search"
	printf '%s\n' 'nested Doxygen install fixture' >"$contract_fixture"

	"$make_command" -C "$WIILAND_BUILD_DIR" \
		bindir=/usr/bin res/wiilandd.service

	"$make_command" -C "$WIILAND_BUILD_DIR" \
		DESTDIR="$contract_stage" \
		bindir="$contract_bindir" \
		htmldir="$contract_htmldir" \
		systemduserunitdir="$contract_systemdunitdir" install
	test -x "$contract_stage$contract_bindir/wiilandd"
	test -d "$contract_html"
	cmp "$contract_fixture" "$contract_installed_fixture"
	grep -F "ExecStart=$contract_bindir/wiilandd" \
		"$contract_service" >/dev/null
	grep -F "ExecCondition=$contract_bindir/wiilandd --check-config" \
		"$contract_service" >/dev/null

	"$make_command" -C "$WIILAND_BUILD_DIR" \
		DESTDIR="$contract_stage" \
		bindir="$contract_bindir" \
		htmldir="$contract_htmldir" \
		systemduserunitdir="$contract_systemdunitdir" uninstall
	test -e "$sentinel"
	if [ -e "$contract_html" ]; then
		printf '%s\n' \
			"WiiLand HTML remains after uninstall: $contract_html" >&2
		exit 1
	fi
	rm -rf "$contract_fixture_dir"
	contract_fixture_dir=

fi
