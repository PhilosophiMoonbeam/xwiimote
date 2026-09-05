//! The logo's pearl, sea-glass and evergreen palette, adapted for readable controls.
use eframe::egui::{self, Color32, CornerRadius, FontId, Frame, Margin, RichText, Stroke};

pub const ICON: &[u8] = include_bytes!("../../../res/wiiland-icon.png");

#[derive(Clone, Copy)]
pub struct Palette {
    pub canvas: Color32,
    pub surface: Color32,
    pub mist: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub border: Color32,
    pub warning: Color32,
}

impl Palette {
    pub fn for_ui(ui: &egui::Ui) -> Self {
        Self::new(ui.visuals().dark_mode)
    }

    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                canvas: Color32::from_rgb(22, 36, 39),
                surface: Color32::from_rgb(30, 47, 49),
                mist: Color32::from_rgb(41, 66, 64),
                ink: Color32::from_rgb(232, 242, 232),
                muted: Color32::from_rgb(165, 189, 180),
                accent: Color32::from_rgb(153, 213, 193),
                on_accent: Color32::from_rgb(23, 55, 52),
                border: Color32::from_rgb(61, 84, 80),
                warning: Color32::from_rgb(238, 198, 133),
            }
        } else {
            Self {
                canvas: Color32::from_rgb(241, 245, 240),
                surface: Color32::from_rgb(252, 253, 248),
                mist: Color32::from_rgb(225, 239, 229),
                ink: Color32::from_rgb(38, 73, 76),
                muted: Color32::from_rgb(87, 112, 105),
                accent: Color32::from_rgb(43, 110, 99),
                on_accent: Color32::from_rgb(255, 255, 250),
                border: Color32::from_rgb(204, 220, 209),
                warning: Color32::from_rgb(141, 91, 29),
            }
        }
    }
}

pub fn install(ctx: &egui::Context) {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let dark = theme == egui::Theme::Dark;
        let p = Palette::new(dark);
        let mut style = egui::Style {
            text_styles: [
                (egui::TextStyle::Small, FontId::proportional(12.0)),
                (egui::TextStyle::Body, FontId::proportional(15.0)),
                (egui::TextStyle::Button, FontId::proportional(14.0)),
                (egui::TextStyle::Heading, FontId::proportional(21.0)),
                (egui::TextStyle::Monospace, FontId::monospace(12.0)),
            ]
            .into(),
            ..Default::default()
        };
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(14.0, 8.0);
        style.spacing.interact_size = egui::vec2(40.0, 34.0);
        style.spacing.combo_width = 190.0;
        style.spacing.text_edit_width = 240.0;
        let v = &mut style.visuals;
        *v = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        v.panel_fill = p.canvas;
        v.window_fill = p.surface;
        v.extreme_bg_color = p.surface;
        v.text_edit_bg_color = Some(p.surface);
        v.faint_bg_color = p.mist;
        v.code_bg_color = p.mist;
        v.override_text_color = None;
        v.weak_text_color = Some(p.muted);
        v.hyperlink_color = p.accent;
        v.warn_fg_color = p.warning;
        v.selection.bg_fill = p.mist;
        v.selection.stroke = Stroke::new(1.5_f32, p.accent);
        v.window_stroke = Stroke::new(1.0_f32, p.border);
        v.window_corner_radius = CornerRadius::same(16);
        v.menu_corner_radius = CornerRadius::same(10);
        v.text_cursor.stroke.color = p.accent;
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.ink);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
        v.widgets.noninteractive.corner_radius = CornerRadius::same(12);
        for widget in [
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            widget.corner_radius = CornerRadius::same(8);
            widget.bg_fill = p.surface;
            widget.weak_bg_fill = p.surface;
            widget.bg_stroke = Stroke::new(1.0_f32, p.border);
            widget.fg_stroke = Stroke::new(1.3_f32, p.ink);
        }
        v.widgets.hovered.bg_fill = p.mist;
        v.widgets.hovered.weak_bg_fill = p.mist;
        v.widgets.hovered.bg_stroke = Stroke::new(1.3_f32, p.accent);
        v.widgets.active.bg_fill = p.mist;
        v.widgets.active.weak_bg_fill = p.mist;
        v.widgets.active.bg_stroke = Stroke::new(1.5_f32, p.accent);
        v.widgets.open.bg_fill = p.mist;
        ctx.set_style_of(theme, style);
    }
    ctx.options_mut(|options| options.fallback_theme = egui::Theme::Light);
}

pub fn card(ui: &egui::Ui) -> Frame {
    let p = Palette::for_ui(ui);
    Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(14)
        .inner_margin(22)
}

pub fn panel(fill: Color32, margin: i8) -> Frame {
    Frame::new().fill(fill).inner_margin(Margin::same(margin))
}

pub fn heading(ui: &mut egui::Ui, title: &str, description: &str) {
    let p = Palette::for_ui(ui);
    ui.label(RichText::new(title).size(29.0).color(p.ink));
    ui.label(RichText::new(description).color(p.muted));
    ui.add_space(12.0);
}

pub fn primary(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
    let p = Palette::for_ui(ui);
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).color(p.on_accent))
            .fill(p.accent)
            .stroke(Stroke::NONE),
    )
}

pub fn note(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).color(Palette::for_ui(ui).muted));
}

pub fn badge(ui: &mut egui::Ui, text: &str, warning: bool) {
    let p = Palette::for_ui(ui);
    Frame::new()
        .fill(p.mist)
        .corner_radius(20)
        .inner_margin(Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.0).color(if warning {
                p.warning
            } else {
                p.accent
            }));
        });
}
