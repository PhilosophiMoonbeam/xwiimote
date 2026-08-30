use std::fmt::Write as _;

use ratatui::Frame;
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::{App, ViewMode, button_at, key_name};

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let body = match app.mode {
        ViewMode::Error => error_view(area.width, area.height),
        ViewMode::Basic => basic_view(app),
        ViewMode::Extended => extended_view(app),
    };
    let paragraph = Paragraph::new(Text::from(body));
    if matches!(app.mode, ViewMode::Basic) {
        frame.render_widget(paragraph, area);
        return;
    }

    let title = match app.mode {
        ViewMode::Extended => "WIILAND SHOW [EXTENDED]",
        _ => "WIILAND SHOW",
    };
    frame.render_widget(
        paragraph
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn error_view(width: u16, height: u16) -> String {
    format!(
        "Error: Screen smaller than 80x24; no view\nCurrent size: {width}x{height}\nResize the terminal to continue."
    )
}

fn basic_view(app: &App) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "+- Keys ----------+ +------+ +---------------------------------+---------------+"
    );
    let _ = writeln!(
        out,
        "|       | |       | |      |  Accel x:{:>6} y:{:>6} z:{:>6} | WIILAND SHOW  |",
        app.accel.x, app.accel.y, app.accel.z
    );
    let _ = writeln!(
        out,
        "|       | |       | +------+ +---------------------------------+---------------+"
    );
    let _ = writeln!(
        out,
        "|     +-+ +-+     | IR: {}  {}  {}  {} |",
        ir_value(app, 0),
        ir_value(app, 1),
        ir_value(app, 2),
        ir_value(app, 3)
    );
    let _ = writeln!(
        out,
        "|     |     |     | +--------------------------------+-------------------------+"
    );
    let _ = writeln!(
        out,
        "|     +-+ +-+     | MP x:{:>7} y:{:>7} z:{:>7} | LED {} {} {} {} |",
        app.motion_plus.x,
        app.motion_plus.y,
        app.motion_plus.z,
        led(app, 0),
        led(app, 1),
        led(app, 2),
        led(app, 3)
    );
    let _ = writeln!(
        out,
        "|       | |       | +--------------------------+-----+----------------------+--+"
    );
    let _ = writeln!(
        out,
        "|       +-+       | Battery: {:>3}% | Rumble:{:<3} | Ext: {:<20} |  |",
        app.battery.map_or(String::from("N/A"), |v| v.to_string()),
        if app.rumble { "ON" } else { "OFF" },
        app.extension
    );
    let _ = writeln!(out, "|                 | Device: {:<51}|", app.device_type);
    let _ = writeln!(out, "|   +-+     +-+   | Opened: {}", open_names(app));
    let _ = writeln!(
        out,
        "|   | |     | |   | Keys: {}",
        key_list(&app.key_state)
    );
    let _ = writeln!(
        out,
        "|   +-+     +-+   | Nunchuk: {}  Classic: {}  Pro: {}",
        enabled(app.nunchuk_enabled),
        enabled(app.classic_enabled),
        enabled(app.pro_enabled)
    );
    let _ = writeln!(
        out,
        "|                 | Balance: {}  Guitar: {}  Drums: {}",
        enabled(app.balance_enabled),
        enabled(app.guitar_enabled),
        enabled(app.drums_enabled)
    );
    let _ = writeln!(
        out,
        "| ( ) |     | ( ) | Status: {}",
        app.status.back().map(String::as_str).unwrap_or("Ready")
    );
    let _ = writeln!(
        out,
        "|                 +----------------------------------------------------------+"
    );
    let _ = writeln!(
        out,
        "|      +++++      | Commands: q quit  f freeze  s refresh/cal  k/a/i/m toggles |"
    );
    let _ = writeln!(
        out,
        "|      +   +      +----------------------------------------------------------+"
    );
    let _ = writeln!(
        out,
        "|      +++++      | 1-4 LEDs  r rumble  N/c extensions  b/p/g/d controllers |"
    );
    let _ = writeln!(
        out,
        "+-----------------+----------------------------------------------------------+"
    );
    out
}

fn extended_view(app: &App) -> String {
    let mut out = basic_view(app);
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "+- Accelerometer --------------------------------+ +- IR camera ------------------------------+"
    );
    let _ = writeln!(
        out,
        "| X {:>8}  Y {:>8}  Z {:>8}                  | x:{}  +:{}  *:{}  -:{}                    |",
        app.accel.x,
        app.accel.y,
        app.accel.z,
        ir_value(app, 0),
        ir_value(app, 1),
        ir_value(app, 2),
        ir_value(app, 3)
    );
    let _ = writeln!(
        out,
        "|       <---------##--------->                  | +------------------+----------------------+"
    );
    let _ = writeln!(
        out,
        "|                                               | |                  |                      |"
    );
    let _ = writeln!(
        out,
        "+- Balance Board --------------------------------+ |                  |                      |"
    );
    let _ = writeln!(
        out,
        "| Sum:{:>7}  #1:{:>7} #2:{:>7} #3:{:>7} #4:{:>7}     | |                  |                      |",
        app.balance.iter().map(|p| p.x).sum::<i32>(),
        app.balance[0].x,
        app.balance[1].x,
        app.balance[2].x,
        app.balance[3].x
    );
    let _ = writeln!(
        out,
        "+------------------------------------------------+ +------------------+----------------------+"
    );
    let _ = writeln!(
        out,
        "+- Motion Plus ----------------------------------+ +- Nunchuk -------------------------------+"
    );
    let _ = writeln!(
        out,
        "| x:{:>7} y:{:>7} z:{:>7} norm:{}               | Stick x:{:>6} y:{:>6} accel {:>5},{:>5},{:>5} |",
        app.motion_plus.x,
        app.motion_plus.y,
        app.motion_plus.z,
        enabled(app.motion_plus_enabled),
        app.nunchuk[0].x,
        app.nunchuk[0].y,
        app.nunchuk[1].x,
        app.nunchuk[1].y,
        app.nunchuk[1].z
    );
    let _ = writeln!(
        out,
        "| position ({:>5},{:>5})                         | Buttons C:{} Z:{}                         |",
        app.mp_position[0],
        app.mp_position[1],
        button(&app.nunchuk_keys, 19),
        button(&app.nunchuk_keys, 20)
    );
    let _ = writeln!(
        out,
        "+------------------------------------------------+ +------------------------------------------+"
    );
    let _ = writeln!(
        out,
        "+- Classic Controller ---------------------------+ +- Pro Controller -------------------------+"
    );
    let _ = writeln!(
        out,
        "| LX:{:>5} LY:{:>5} RX:{:>5} RY:{:>5} LT:{:>3} RT:{:>3} | LX:{:>5} LY:{:>5} RX:{:>5} RY:{:>5}        |",
        app.classic[0].x,
        app.classic[0].y,
        app.classic[1].x,
        app.classic[1].y,
        app.classic[2].x,
        app.classic[2].y,
        app.pro[0].x,
        app.pro[0].y,
        app.pro[1].x,
        app.pro[1].y
    );
    let _ = writeln!(
        out,
        "| Keys: {:<30} | Keys: {:<34} |",
        key_list(&app.classic_keys),
        key_list(&app.pro_keys)
    );
    let _ = writeln!(
        out,
        "+------------------------------------------------+ +------------------------------------------+"
    );
    let _ = writeln!(
        out,
        "+- Guitar --------------------------------------+ +- Drums ---------------------------------+"
    );
    let _ = writeln!(
        out,
        "| Stick X:{:>6} Stick Y:{:>6}                 | Pad:{:>5} CymL:{:>5} CymR:{:>5} Bass:{:>5} |",
        app.guitar[0].x,
        app.guitar[0].y,
        app.drums[0].x,
        app.drums[1].x,
        app.drums[2].x,
        app.drums[6].x
    );
    let _ = writeln!(
        out,
        "| Whammy:{:>6} Fret-board:{:>6}               | TomL:{:>5} TomR:{:>5} TomFR:{:>5} Hat:{:>5} |",
        app.guitar[1].x,
        app.guitar[2].x,
        app.drums[3].x,
        app.drums[4].x,
        app.drums[5].x,
        app.drums[7].x
    );
    let _ = writeln!(
        out,
        "| Keys: {:<37} | Keys: {:<34} |",
        key_list(&app.guitar_keys),
        key_list(&app.drums_keys)
    );
    let _ = writeln!(
        out,
        "+-----------------------------------------------+ +------------------------------------------+"
    );
    let _ = writeln!(
        out,
        "ASCII view: [D-pad]   (A) (B)   [PLUS/MINUS]   [Nunchuk stick]   [Classic/Pro pads]"
    );
    out
}

fn enabled(value: bool) -> &'static str {
    if value { "ON" } else { "OFF" }
}
fn led(app: &App, n: usize) -> &'static str {
    if !app.led_writable[n] {
        "N/A"
    } else if app.leds[n] {
        "ON"
    } else {
        "off"
    }
}
fn button(keys: &[bool; 28], code: usize) -> &'static str {
    if keys.get(code).copied().unwrap_or(false) {
        "X"
    } else {
        " "
    }
}
fn ir_value(app: &App, n: usize) -> String {
    let p = app.ir[n];
    if p.x >= 1023 || p.y >= 1023 {
        String::from("N/A")
    } else {
        format!("{:04},{:04}", p.x, p.y)
    }
}
fn open_names(app: &App) -> String {
    let mut names = Vec::new();
    if app.keys_enabled {
        names.push("core")
    }
    if app.accel_enabled {
        names.push("accel")
    }
    if app.ir_enabled {
        names.push("ir")
    }
    if app.motion_plus_enabled {
        names.push("mp")
    }
    if app.nunchuk_enabled {
        names.push("nunchuk")
    }
    if app.classic_enabled {
        names.push("classic")
    }
    if app.balance_enabled {
        names.push("balance")
    }
    if app.pro_enabled {
        names.push("pro")
    }
    if app.drums_enabled {
        names.push("drums")
    }
    if app.guitar_enabled {
        names.push("guitar")
    }
    if names.is_empty() {
        String::from("none")
    } else {
        names.join(",")
    }
}
fn key_list(keys: &[bool; 28]) -> String {
    let mut names = Vec::new();
    for (index, pressed) in keys.iter().enumerate() {
        if *pressed && let Some(button) = button_at(index) {
            names.push(key_name(button));
        }
    }
    if names.is_empty() {
        String::from("none")
    } else {
        names.join(",")
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    #[test]
    fn basic_layout_fits_exactly_80_by_24() {
        let mut app = App::default();
        app.resize(80, 24);
        app.status
            .push_back(String::from("Screen smaller than 160x48; limited view"));

        let expected = basic_view(&app);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("create test terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render basic layout");
        let buffer = terminal.backend().buffer();

        let mut rendered_lines = Vec::new();
        for y in 0..buffer.area.height {
            let mut line = String::new();
            for x in 0..buffer.area.width {
                line.push_str(buffer[(x, y)].symbol());
            }
            rendered_lines.push(line);
        }

        let expected_lines: Vec<_> = expected.lines().collect();
        assert_eq!(expected_lines.len(), 19);
        for (line_number, expected_line) in expected_lines.iter().enumerate() {
            assert!(
                expected_line.len() <= 80,
                "basic row {} exceeds the minimum width: {expected_line:?}",
                line_number + 1
            );
            assert_eq!(
                rendered_lines[line_number],
                format!("{expected_line:<80}"),
                "basic row {} wrapped or was clipped",
                line_number + 1
            );
        }
        assert!(
            rendered_lines[expected_lines.len()..]
                .iter()
                .all(|line| line.trim().is_empty()),
            "basic layout spilled into unused rows"
        );
    }

    #[test]
    fn extended_guitar_and_drums_labels_use_every_payload_slot() {
        let mut app = App::default();
        app.guitar[0].x = 101;
        app.guitar[0].y = 102;
        app.guitar[1].x = 201;
        app.guitar[2].x = 301;
        for (index, value) in app.drums.iter_mut().enumerate() {
            value.x = 1000 + index as i32;
        }

        let view = extended_view(&app);
        assert!(view.contains("Stick X:   101 Stick Y:   102"));
        assert!(view.contains("Whammy:   201 Fret-board:   301"));
        assert!(!view.contains("Tilt:"));
        assert!(!view.contains(" Bar:"));
        assert!(view.contains("Pad: 1000 CymL: 1001 CymR: 1002 Bass: 1006"));
        assert!(view.contains("TomL: 1003 TomR: 1004 TomFR: 1005 Hat: 1007"));
    }

    #[test]
    fn extended_layout_fits_exactly_160_by_48() {
        let mut app = App::default();
        app.resize(160, 48);
        let expected = extended_view(&app);
        let expected_lines: Vec<_> = expected.lines().collect();
        assert!(
            expected_lines.len() <= 46,
            "extended content exceeds the bordered viewport height"
        );

        let backend = TestBackend::new(160, 48);
        let mut terminal = Terminal::new(backend).expect("create test terminal");
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render extended layout");
        let buffer = terminal.backend().buffer();

        for (line_number, expected_line) in expected_lines.iter().enumerate() {
            assert!(
                expected_line.len() <= 158,
                "extended row {} exceeds the bordered viewport width: {expected_line:?}",
                line_number + 1
            );
            let mut rendered = String::new();
            for x in 1..159 {
                rendered.push_str(buffer[(x, line_number as u16 + 1)].symbol());
            }
            assert_eq!(
                rendered,
                format!("{expected_line:<158}"),
                "extended row {} wrapped or was clipped",
                line_number + 1
            );
        }
        for y in (expected_lines.len() as u16 + 1)..47 {
            let mut rendered = String::new();
            for x in 1..159 {
                rendered.push_str(buffer[(x, y)].symbol());
            }
            assert!(
                rendered.trim().is_empty(),
                "extended layout spilled into unused row {y}"
            );
        }
    }
}
