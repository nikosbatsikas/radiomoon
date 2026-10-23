//! Small drawing helpers: colours, vector icons and flat buttons for the RadioMoon look.
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui};
use std::f32::consts::PI;

// Palette of FM Analyzer: dark instrument look
pub const BG: Color32 = Color32::from_rgb(0x0b, 0x0e, 0x12);
pub const PANEL: Color32 = Color32::from_rgb(0x11, 0x15, 0x1b);
pub const CARD: Color32 = Color32::from_rgb(0x17, 0x1c, 0x24);
pub const BORDER: Color32 = Color32::from_rgb(0x26, 0x2d, 0x38);
pub const INSET: Color32 = Color32::from_rgb(0x08, 0x0a, 0x0d);
pub const SIDEBAR: Color32 = Color32::from_rgb(0x08, 0x0a, 0x0d);
pub const CONTROL: Color32 = Color32::from_rgb(0x20, 0x27, 0x31);
pub const CONTROL_HOVER: Color32 = Color32::from_rgb(0x29, 0x32, 0x3e);
pub const CONTROL_ACTIVE: Color32 = Color32::from_rgb(0x31, 0x3c, 0x4a);
pub const TEXT: Color32 = Color32::from_rgb(0xd6, 0xdc, 0xe4);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x7a, 0x85, 0x93);
pub const ACCENT: Color32 = Color32::from_rgb(0x38, 0xb6, 0xff);
pub const GOOD: Color32 = Color32::from_rgb(0x34, 0xd3, 0x86);
pub const WARN: Color32 = Color32::from_rgb(0xf5, 0xb0, 0x3a);
pub const BAD: Color32 = Color32::from_rgb(0xff, 0x52, 0x5a);

// Names used all over the program (button fills carry white text, so they are the deeper shades)
pub const GRAY: Color32 = TEXT;
pub const BLUE: Color32 = Color32::from_rgb(0x1f, 0x7f, 0xc4);
pub const GREEN: Color32 = Color32::from_rgb(0x1f, 0x9d, 0x62);
pub const RED: Color32 = Color32::from_rgb(0xc4, 0x36, 0x40);
pub const DARK: Color32 = CONTROL;

pub fn visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.dark_mode = true;
    v.override_text_color = Some(TEXT);
    v.panel_fill = PANEL;
    v.window_fill = CARD;
    v.extreme_bg_color = INSET;
    v.faint_bg_color = Color32::from_rgb(0x1b, 0x21, 0x2a);
    v.code_bg_color = INSET;
    v.window_rounding = 6.0.into();
    v.menu_rounding = 6.0.into();
    v.window_stroke = Stroke::new(1.0_f32, BORDER);
    v.window_shadow = egui::epaint::Shadow { offset: vec2(0.0, 10.0), blur: 28.0, spread: 0.0, color: Color32::from_black_alpha(170) };
    v.popup_shadow = egui::epaint::Shadow { offset: vec2(0.0, 6.0), blur: 16.0, spread: 0.0, color: Color32::from_black_alpha(140) };
    v.hyperlink_color = ACCENT;
    v.warn_fg_color = WARN;
    v.error_fg_color = BAD;
    v.selection.bg_fill = ACCENT.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    let r: egui::Rounding = 4.0.into();
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = CARD;
    w.noninteractive.weak_bg_fill = CARD;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    w.noninteractive.rounding = r;
    w.inactive.bg_fill = CONTROL;
    w.inactive.weak_bg_fill = CONTROL;
    w.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    w.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    w.inactive.rounding = r;
    w.hovered.bg_fill = CONTROL_HOVER;
    w.hovered.weak_bg_fill = CONTROL_HOVER;
    w.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.6));
    w.hovered.fg_stroke = Stroke::new(1.5_f32, TEXT);
    w.hovered.rounding = r;
    w.hovered.expansion = 0.0;
    w.active.bg_fill = CONTROL_ACTIVE;
    w.active.weak_bg_fill = CONTROL_ACTIVE;
    w.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    w.active.fg_stroke = Stroke::new(1.5_f32, TEXT);
    w.active.rounding = r;
    w.active.expansion = 0.0;
    w.open = w.active;
    v
}

/// Same fonts as FM Analyzer (egui's built-in ones), with an embedded DejaVu Sans as the fallback for Greek.
fn install_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions, FontFamily};
    let mut fd = FontDefinitions::default();
    fd.font_data.insert("dejavu".into(), FontData::from_static(include_bytes!("../assets/fonts/DejaVuSans.ttf")));
    fd.font_data.insert("dejavu-bold".into(), FontData::from_static(include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf")));
    fd.families.entry(FontFamily::Proportional).or_default().push("dejavu".into());
    fd.families.entry(FontFamily::Monospace).or_default().push("dejavu".into());
    ctx.set_fonts(fd);
}

/// Fonts sizes, spacing and colours of the whole program.
pub fn apply_style(ctx: &egui::Context) {
    use egui::{FontFamily, FontId, TextStyle};
    ctx.set_theme(egui::Theme::Dark); // never follow the system's light theme
    install_fonts(ctx);
    let mut style = (*ctx.style()).clone();
    style.text_styles = std::collections::BTreeMap::from([
        (TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.5, FontFamily::Monospace)),
        (TextStyle::Button, FontId::new(14.5, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
    ]);
    style.spacing.item_spacing = vec2(8.0, 6.0);
    style.spacing.button_padding = vec2(10.0, 4.0);
    style.spacing.interact_size.y = 26.0;
    style.spacing.slider_width = 140.0;
    style.spacing.window_margin = egui::Margin::same(14.0);
    style.visuals = visuals();
    ctx.set_style(style);
}

/// A rounded panel that groups related settings.
pub fn card<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::none()
        .fill(CARD)
        .rounding(8.0)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .inner_margin(14.0)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width()); // every card spans the whole page
            if !title.is_empty() {
                ui.label(egui::RichText::new(crate::i18n::tr(title)).strong().size(14.0).color(ACCENT));
                ui.add_space(2.0);
            }
            add(ui)
        })
        .inner
}

pub fn page_title(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.label(egui::RichText::new(crate::i18n::tr(title)).size(20.0).strong());
    if !subtitle.is_empty() {
        ui.label(egui::RichText::new(crate::i18n::tr(subtitle)).color(GRAY));
    }
    ui.add_space(6.0);
}

pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Button::new(egui::RichText::new(crate::i18n::tr(text)).color(Color32::WHITE).strong()).fill(ACCENT.gamma_multiply(0.85)).rounding(6.0))
}

pub fn danger_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Button::new(egui::RichText::new(crate::i18n::tr(text)).color(Color32::WHITE)).fill(Color32::from_rgb(160, 45, 50)).rounding(6.0))
}

pub fn lighten(c: Color32, a: u8) -> Color32 {
    Color32::from_rgb(c.r().saturating_add(a), c.g().saturating_add(a), c.b().saturating_add(a))
}

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Play,
    Pause,
    Stop,
    Next,
    Eject,
    Restart,
    Loop,
    Trash,
    Headphones,
    Up,
    Down,
    Top,
    Bottom,
    MoveUp,
    MoveDown,
    Plus,
    Insert,
    Clear,
}

fn arc(p: &Painter, c: Pos2, r: f32, a0: f32, a1: f32, st: Stroke) -> (Pos2, f32) {
    let n = 18;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    let end = pts[pts.len() - 1];
    p.add(Shape::line(pts, st));
    (end, a1 + PI / 2.0)
}

fn head(p: &Painter, tip: Pos2, dir: f32, size: f32, col: Color32) {
    let a = pos2(tip.x - size * (dir + 0.5).cos(), tip.y - size * (dir + 0.5).sin());
    let b = pos2(tip.x - size * (dir - 0.5).cos(), tip.y - size * (dir - 0.5).sin());
    p.add(Shape::convex_polygon(vec![tip, a, b], col, Stroke::NONE));
}

pub fn draw_icon(p: &Painter, icon: Icon, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new((s * 0.18).max(1.5), col);
    match icon {
        Icon::Play => {
            p.add(Shape::convex_polygon(
                vec![pos2(c.x - s * 0.6, c.y - s * 0.8), pos2(c.x - s * 0.6, c.y + s * 0.8), pos2(c.x + s * 0.8, c.y)],
                col,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            for dx in [-0.5f32, 0.5] {
                p.rect_filled(Rect::from_center_size(pos2(c.x + dx * s, c.y), vec2(s * 0.55, s * 1.6)), 1.0, col);
            }
        }
        Icon::Stop => {
            p.rect_filled(Rect::from_center_size(c, vec2(s * 1.5, s * 1.5)), 2.0, col);
        }
        Icon::Next => {
            p.add(Shape::convex_polygon(
                vec![pos2(c.x - s * 0.8, c.y - s * 0.8), pos2(c.x - s * 0.8, c.y + s * 0.8), pos2(c.x + s * 0.5, c.y)],
                col,
                Stroke::NONE,
            ));
            p.rect_filled(Rect::from_center_size(pos2(c.x + s * 0.8, c.y), vec2(s * 0.3, s * 1.6)), 0.0, col);
        }
        Icon::Eject => {
            p.add(Shape::convex_polygon(
                vec![pos2(c.x - s * 0.9, c.y + s * 0.2), pos2(c.x + s * 0.9, c.y + s * 0.2), pos2(c.x, c.y - s * 0.8)],
                col,
                Stroke::NONE,
            ));
            p.rect_filled(Rect::from_center_size(pos2(c.x, c.y + s * 0.75), vec2(s * 1.8, s * 0.35)), 0.0, col);
        }
        Icon::Up => {
            p.add(Shape::line(vec![pos2(c.x - s * 0.9, c.y + s * 0.4), pos2(c.x, c.y - s * 0.5), pos2(c.x + s * 0.9, c.y + s * 0.4)], st));
        }
        Icon::Down => {
            p.add(Shape::line(vec![pos2(c.x - s * 0.9, c.y - s * 0.4), pos2(c.x, c.y + s * 0.5), pos2(c.x + s * 0.9, c.y - s * 0.4)], st));
        }
        Icon::Top => {
            p.line_segment([pos2(c.x - s * 0.9, c.y - s * 0.8), pos2(c.x + s * 0.9, c.y - s * 0.8)], st);
            p.add(Shape::line(vec![pos2(c.x - s * 0.9, c.y + s * 0.7), pos2(c.x, c.y - s * 0.2), pos2(c.x + s * 0.9, c.y + s * 0.7)], st));
        }
        Icon::Bottom => {
            p.line_segment([pos2(c.x - s * 0.9, c.y + s * 0.8), pos2(c.x + s * 0.9, c.y + s * 0.8)], st);
            p.add(Shape::line(vec![pos2(c.x - s * 0.9, c.y - s * 0.7), pos2(c.x, c.y + s * 0.2), pos2(c.x + s * 0.9, c.y - s * 0.7)], st));
        }
        Icon::MoveUp => {
            p.add(Shape::convex_polygon(
                vec![pos2(c.x - s * 0.8, c.y + s * 0.5), pos2(c.x + s * 0.8, c.y + s * 0.5), pos2(c.x, c.y - s * 0.6)],
                col,
                Stroke::NONE,
            ));
        }
        Icon::MoveDown => {
            p.add(Shape::convex_polygon(
                vec![pos2(c.x - s * 0.8, c.y - s * 0.5), pos2(c.x + s * 0.8, c.y - s * 0.5), pos2(c.x, c.y + s * 0.6)],
                col,
                Stroke::NONE,
            ));
        }
        Icon::Plus => {
            p.line_segment([pos2(c.x - s, c.y), pos2(c.x + s, c.y)], Stroke::new(s * 0.3, col));
            p.line_segment([pos2(c.x, c.y - s), pos2(c.x, c.y + s)], Stroke::new(s * 0.3, col));
        }
        Icon::Insert => {
            p.line_segment([pos2(c.x + s * 0.9, c.y), pos2(c.x - s * 0.5, c.y)], st);
            head(p, pos2(c.x - s * 0.95, c.y), PI, s * 0.9, col);
        }
        Icon::Clear => {
            p.line_segment([pos2(c.x - s * 0.7, c.y - s * 0.7), pos2(c.x + s * 0.7, c.y + s * 0.7)], st);
            p.line_segment([pos2(c.x - s * 0.7, c.y + s * 0.7), pos2(c.x + s * 0.7, c.y - s * 0.7)], st);
        }
        Icon::Trash => {
            p.line_segment([pos2(c.x - s * 0.8, c.y - s * 0.6), pos2(c.x + s * 0.8, c.y - s * 0.6)], st);
            p.line_segment([pos2(c.x - s * 0.3, c.y - s * 0.85), pos2(c.x + s * 0.3, c.y - s * 0.85)], st);
            p.rect_stroke(Rect::from_min_max(pos2(c.x - s * 0.6, c.y - s * 0.45), pos2(c.x + s * 0.6, c.y + s * 0.9)), 2.0, st);
            p.line_segment([pos2(c.x - s * 0.2, c.y - s * 0.2), pos2(c.x - s * 0.2, c.y + s * 0.65)], Stroke::new(1.2_f32, col));
            p.line_segment([pos2(c.x + s * 0.2, c.y - s * 0.2), pos2(c.x + s * 0.2, c.y + s * 0.65)], Stroke::new(1.2_f32, col));
        }
        Icon::Headphones => {
            let cy = c.y + s * 0.15;
            let r = s * 0.9;
            arc(p, pos2(c.x, cy), r, PI, 2.0 * PI, st);
            p.rect_filled(Rect::from_center_size(pos2(c.x - r, cy + s * 0.3), vec2(s * 0.5, s * 0.9)), 2.0, col);
            p.rect_filled(Rect::from_center_size(pos2(c.x + r, cy + s * 0.3), vec2(s * 0.5, s * 0.9)), 2.0, col);
        }
        Icon::Restart => {
            let (end, dir) = arc(p, c, s * 0.8, -PI * 0.3, PI * 1.5, st);
            head(p, end, dir, s * 0.7, col);
        }
        Icon::Loop => {
            let (e1, d1) = arc(p, c, s * 0.8, PI * 1.1, PI * 1.95, st);
            head(p, e1, d1, s * 0.6, col);
            let (e2, d2) = arc(p, c, s * 0.8, PI * 0.1, PI * 0.95, st);
            head(p, e2, d2, s * 0.6, col);
        }
    }
}

/// Width of a button that fits its (translated) label with some space around it.
pub fn label_w(ui: &Ui, label: &str, min: f32) -> f32 {
    let t = crate::i18n::tr(label);
    let w = ui.fonts(|f| f.layout_no_wrap(t, FontId::proportional(14.0), Color32::WHITE).size().x);
    (w + 30.0).max(min)
}

/// A flat button drawn into a fixed rectangle. Icon above, label below (or either alone).
pub fn button_at(ui: &mut Ui, rect: Rect, id: &str, icon: Option<Icon>, label: &str, fill: Color32, fg: Color32) -> Response {
    let label = &crate::i18n::tr(label);
    let resp = ui.interact(rect, ui.id().with(id), Sense::click());
    let p = ui.painter_at(rect);
    let f = if resp.is_pointer_button_down_on() {
        fill.gamma_multiply(0.7)
    } else if resp.hovered() {
        lighten(fill, 22)
    } else {
        fill
    };
    p.rect_filled(rect, 5.0, f);
    let c = rect.center();
    match (icon, label.is_empty()) {
        (Some(i), false) => {
            draw_icon(&p, i, pos2(c.x, rect.top() + rect.height() * 0.38), (rect.height() * 0.16).clamp(6.0, 13.0), fg);
            p.text(pos2(c.x, rect.bottom() - 11.0), Align2::CENTER_CENTER, label, FontId::proportional(11.5), fg);
        }
        (Some(i), true) => draw_icon(&p, i, c, (rect.height() * 0.22).clamp(6.0, 14.0), fg),
        (None, _) => {
            p.text(c, Align2::CENTER_CENTER, label, FontId::proportional(14.0), fg);
        }
    }
    resp
}

/// Text clipped to a column [x0, x1] of a row.
pub fn cell_text(ui: &Ui, row: Rect, x0: f32, x1: f32, text: &str, size: f32, color: Color32) {
    let clip = Rect::from_min_max(pos2(x0, row.top()), pos2(x1, row.bottom()));
    ui.painter_at(clip).text(pos2(x0 + 4.0, row.center().y), Align2::LEFT_CENTER, text, FontId::proportional(size), color);
}

/// Crescent moon used as the RadioMoon logo.
pub fn draw_moon(p: &Painter, c: Pos2, r: f32, col: Color32, bg: Color32) {
    p.circle_filled(c, r, col);
    p.circle_filled(pos2(c.x + r * 0.45, c.y - r * 0.2), r * 0.85, bg);
}

/// Text sizes: smaller while the Control Panel is open. Only touches the style when the state changes.
pub fn cp_text_sizes(ctx: &egui::Context, small: bool) {
    use egui::{FontFamily, FontId, TextStyle};
    use std::sync::atomic::{AtomicU8, Ordering};
    static LAST: AtomicU8 = AtomicU8::new(2);
    let want = small as u8;
    if LAST.swap(want, Ordering::Relaxed) == want {
        return;
    }
    ctx.style_mut(|st| {
        let (h, b, sm, m) = if small { (16.0, 13.5, 10.5, 13.0) } else { (20.0, 14.5, 12.0, 13.5) };
        st.text_styles = std::collections::BTreeMap::from([
            (TextStyle::Heading, FontId::new(h, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(b, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(m, FontFamily::Monospace)),
            (TextStyle::Button, FontId::new(b, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(sm, FontFamily::Proportional)),
        ]);
        st.spacing.item_spacing = vec2(8.0, 6.0);
        st.spacing.interact_size.y = if small { 24.0 } else { 26.0 };
        st.spacing.button_padding = vec2(10.0, 4.0);
    });
}
