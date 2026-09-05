# WiiLand Control Center

The control center uses the README logo's pearl surfaces, sea-glass accents,
and evergreen text. The interface is implemented in egui/eframe, with native
Wayland and X11 backends.

## Visual language

- Keep decorative glass and reflections in the emblem. Controls use opaque
  surfaces, clear boundaries, and visible focus states.
- Use the shared palette in `crates/wiiland-config/src/theme.rs`. Pearl and dusk
  have the same layout and hierarchy; appearance follows the system by default.
- Use whitespace to separate tasks, and use accent color for the primary action
  and current selection. Warnings always include explanatory text.
- Keep form labels visible and aligned. Attach each field's label to its
  accessibility response; never rely on placeholder text alone.

## Workflows

Overview answers where configuration is saved, whether the service is running,
and how to check device discovery. Configure separates profile/pointer tuning,
motion aiming, button bindings, and ordered device rules. Test & calibrate
separates live input capture, flat-surface calibration, and saved-file checks.

Save controls remain outside the scrolling page. Unsaved edits remain visible
across navigation; reload and close use a modal discard confirmation. Ctrl+S
validates and saves without restarting. Custom files cannot restart the service.

The activity drawer opens for diagnostic commands and captures, and can be
resized or hidden. A running capture always has a Stop capture action in the
status strip, even after navigating away. Service status reflects a status
query rather than the success of a preceding service command. Cancelled captures
release their ownership without applying partial calibration values.

The minimum viewport is 760 × 600. Below 1000 pixels wide, navigation moves from
the sidebar into the header and overview cards stack. Save controls and capture
cancellation remain reachable when the log is open.

## Assets and verification

`res/io.github.philosophimoonbeam.wiiland.svg` is the transparent emblem extracted
from `res/wiiland-logo.svg`. `res/wiiland-icon.png` is its 512 × 512 raster export,
embedded in the binary for the native window icon and overview illustration.
No runtime asset downloads or font dependencies are needed.

`cargo test --locked -p wiiland-config` exercises the model, asynchronous tasks,
and real egui pointer interactions. The UI tests cover compact navigation,
labelled bindings, independent rule dropdowns, invalid rules, discard cancellation,
capture cancellation, aligned form columns, and persistent save controls at both
viewport sizes in both palettes.
