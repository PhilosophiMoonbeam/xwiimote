<p align="center">
  <img src="res/wiiland-logo.png" width="800" alt="WiiLand — Native Wii input for Linux">
</p>

<p align="center">
  Turn Nintendo Wii-family controllers into first-class Linux gamepads, pointers, and keyboards.
</p>

<p align="center">
  <a href="#quick-start"><strong>Quick start</strong></a> ·
  <a href="#configure">Configure</a> ·
  <a href="#diagnose">Diagnose</a> ·
  <a href="#documentation">Documentation</a>
</p>

---

WiiLand is a display-neutral Linux input stack for Wii Remotes, extensions,
Balance Boards, Wii U Pro Controllers, and compatible devices. It uses the
kernel's `hid-wiimote` driver and emits ordinary `uinput`/`evdev` devices—no
compositor API, XWayland dependency, or application-specific driver.

```text
Wii hardware → hid-wiimote → libxwiimote → wiilandd → uinput/evdev
                                                    ├─ Wayland / libinput
                                                    ├─ X.org
                                                    └─ SDL, Steam, Wine/Proton, native apps
```

| | |
|---|---|
| **Sessions** | Native Wayland and X.org |
| **Outputs** | `WiiLand Virtual Controller`, `WiiLand Virtual Desktop` |
| **Profiles** | `gamepad`, `desktop`, `both` |
| **Frontends** | Qt 6 control center, optional ncurses monitor |
| **Driver path** | Linux `hid-wiimote` → `libxwiimote` → `wiilandd` |

## Quick start

### 1. Build and install

Required: a C compiler, C++17 compiler, GNU Make, `pkg-config`, libudev headers,
and Linux uinput headers. Autotools and Libtool are required for a checkout.
Qt 6 Widgets and ncurses are optional.

```sh
./autogen.sh --prefix=/usr --sysconfdir=/etc \
  --enable-qt-ui=auto --enable-xwiishow=auto
make -j"$(nproc)"
sudo make install
```

A release archive already contains `configure`; use it instead of `autogen.sh`.
After installing udev rules, reload them and reconnect the controller:

```sh
sudo udevadm control --reload-rules
sudo udevadm trigger
```

### 2. Pair a controller

The desktop Bluetooth panel is the shortest route. For `bluetoothctl`, press the
controller's sync button, then:

```text
power on
agent on
default-agent
scan on
pair XX:XX:XX:XX:XX:XX
trust XX:XX:XX:XX:XX:XX
connect XX:XX:XX:XX:XX:XX
scan off
quit
```

Replace the address with the device shown by `scan on`.

### 3. Check the host

```sh
wiilandd --doctor
wiilandd --list
```

`--doctor` checks session detection, Bluetooth, configuration, and input/uinput
permissions. Fix reported permission failures before starting the daemon.

### 4. Run WiiLand

Choose one mode—do not run the foreground daemon and user service together.

```sh
# Fast foreground test
wiilandd --profile gamepad

# Normal user service
systemctl --user daemon-reload
systemctl --user enable --now wiilandd.service
```

With the Qt frontend installed:

```sh
wiiland-config
```

The control center edits and validates configuration, manages the user service,
and runs device traces and calibration capture.

## Configure

WiiLand loads configuration in this order:

1. `${sysconfdir}/wiiland/wiilandd.conf`—normally `/etc/wiiland/wiilandd.conf`
2. `$XDG_CONFIG_HOME/wiiland/wiilandd.conf` when `XDG_CONFIG_HOME` is absolute;
   otherwise `$HOME/.config/wiiland/wiilandd.conf`

Later values win. `--config PATH` selects one file instead; `--no-config` uses
built-in defaults. The complete annotated sample is
[`res/wiilandd.conf`](res/wiilandd.conf).

Create a small user configuration:

```sh
config_dir="$HOME/.config/wiiland"
mkdir -p "$config_dir"
printf '%s\n' \
  'profile=both' \
  'pointer-speed=24' \
  'ir-speed=10' \
  > "$config_dir/wiilandd.conf"

wiilandd --check-config
systemctl --user restart wiilandd.service
```

| Profile | Result |
|---|---|
| `gamepad` | One virtual controller per connected remote |
| `desktop` | Virtual pointer and keyboard controls |
| `both` | Gamepad and desktop devices together |

The sample configuration also covers IR tracking, screen calibration, desktop
button mapping, per-device rules, and motion aiming. Capture optional flat-surface
motion calibration with:

```sh
wiilandd --device N --calibrate-aim
```

Copy the emitted key/value lines into the user configuration. Each enabled
sensor requires its complete X/Y/Z triple.

## Diagnose

| Goal | Command |
|---|---|
| Host readiness | `wiilandd --doctor` |
| Connected remotes | `wiilandd --list` |
| Validate configuration | `wiilandd --check-config` |
| Show effective configuration | `wiilandd --dump-config` |
| Show virtual axis mapping | `wiilandd --axis-map` |
| Print hardware test matrix | `wiilandd --validation-checklist` |
| Run hardware-free checks | `wiilandd --self-test` |
| Gather a support report | `wiilandd-hardware-report` |

Trace a connected remote without creating virtual devices:

```sh
wiilandd --dry-run --trace-events --verbose --device N --profile both
```

Use a number from `wiilandd --list` or a `/sys` device path. For a shareable
report with a live trace:

```sh
wiilandd-hardware-report N
```

### Common failures

| Symptom | Action |
|---|---|
| No remote listed | Pair, trust, and connect it; confirm `hid-wiimote` is loaded |
| Input/uinput denied | Install the udev rules, reconnect, then rerun `--doctor` |
| Service will not start | Run `--check-config`, then inspect `journalctl --user -u wiilandd.service` |
| Qt window will not open | Install the Qt `wayland` plugin or `xcb` plugin for the active session |
| Duplicate raw pointer on X.org | Install the packaged `50-xorg-fix-wiiland.conf` policy |

## Build options

| Option | Values | Purpose |
|---|---|---|
| `--enable-qt-ui` | `auto`, `yes`, `no` | Qt 6 control center |
| `--enable-xwiishow` | `auto`, `yes`, `no` | ncurses monitor |
| `--enable-debug` | flag | Debug build |
| `--with-udev-rules-dir` | `auto`, `no`, absolute path | Device permissions |
| `--with-systemd-user-unit-dir` | `auto`, `no`, absolute path | User service |
| `--with-xorg-conf-dir` | `auto`, `no`, absolute path | Raw-device X.org policy |

`yes` makes a missing optional dependency a configure error. Integration paths
use pkg-config locations when available and sensible prefix-based fallbacks
otherwise.

## Development

```sh
make -j"$(nproc)" check
make distcheck
```

`xwiidump` is a non-installed EEPROM diagnostic requiring kernel `debugfs`.
Hardware-free smoke checks live in [`tests/`](tests); real-device reports should
include kernel, distribution, Bluetooth adapter, device type, session, and
consumer results.

## Documentation

- [`doc/WIILAND`](doc/WIILAND) — architecture, configuration, and operations
- [`doc/wiilandd.1`](doc/wiilandd.1) — daemon and command reference
- [`doc/wiiland-config.1`](doc/wiiland-config.1) — Qt control center
- [`doc/wiiland.7`](doc/wiiland.7) — installed overview
- [`doc/DEVICES`](doc/DEVICES) and [`doc/PROTOCOL`](doc/PROTOCOL) — hardware model and protocol
- [`DEV`](DEV) — contributor build notes

Questions, bugs, and hardware reports belong in the
[issue tracker](https://github.com/PhilosophiMoonbeam/wiiland/issues).

## License and lineage

WiiLand is open source under the permissive xwiimote license; see
[`LICENSE`](LICENSE) and [`COPYING`](COPYING). It is derived from xwiimote by
David Herrmann and retains its authorship and contributor history.
