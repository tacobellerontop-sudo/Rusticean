//! Colours, fonts and widgets shared by every window, following Roblox's dark and light
//! themes.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rbx_core::settings::Theme;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Sense, Stroke, StrokeKind, Vec2,
};

/// A colour scheme. Roblox's own dark and light themes.
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub blue: Color32,
    pub blue_light: Color32,
    pub red: Color32,
    /// Title bars and text boxes, a step darker than `bg`.
    pub deep: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub danger_bg: Color32,
    pub dark: bool,
}

pub const DARK: Palette = Palette {
    bg: Color32::from_rgb(0x19, 0x1B, 0x1D),
    surface: Color32::from_rgb(0x23, 0x25, 0x27),
    border: Color32::from_rgb(0x39, 0x3B, 0x3D),
    text: Color32::from_rgb(0xF7, 0xF7, 0xF8),
    text_dim: Color32::from_rgb(0xBD, 0xBE, 0xBE),
    blue: Color32::from_rgb(0x33, 0x5F, 0xFF),
    blue_light: Color32::from_rgb(0x6E, 0x8C, 0xFF),
    red: Color32::from_rgb(0xE5, 0x48, 0x4D),
    deep: Color32::from_rgb(0x12, 0x13, 0x15),
    hover: Color32::from_rgb(0x2B, 0x2D, 0x30),
    selected: Color32::from_rgb(0x2E, 0x33, 0x45),
    danger_bg: Color32::from_rgb(0x3A, 0x1F, 0x22),
    dark: true,
};

pub const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(0xF2, 0xF4, 0xF5),
    surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    border: Color32::from_rgb(0xD6, 0xD9, 0xDC),
    text: Color32::from_rgb(0x39, 0x3B, 0x3D),
    text_dim: Color32::from_rgb(0x60, 0x62, 0x64),
    blue: Color32::from_rgb(0x33, 0x5F, 0xFF),
    blue_light: Color32::from_rgb(0x24, 0x4C, 0xE6),
    red: Color32::from_rgb(0xD2, 0x32, 0x38),
    deep: Color32::from_rgb(0xE3, 0xE6, 0xE8),
    hover: Color32::from_rgb(0xE6, 0xE9, 0xEC),
    selected: Color32::from_rgb(0xDD, 0xE5, 0xFF),
    danger_bg: Color32::from_rgb(0xFD, 0xE8, 0xE9),
    dark: false,
};

static LIGHT_MODE: AtomicBool = AtomicBool::new(false);

/// Switch every window to the light or dark palette.
pub fn set_light(light: bool) {
    LIGHT_MODE.store(light, Ordering::Relaxed);
}

/// Whether the chosen theme is light, asking Windows for "System default".
pub fn wants_light(theme: Theme) -> bool {
    match theme {
        Theme::Dark => false,
        Theme::Light => true,
        Theme::System => rbx_win::registry::system_uses_light_theme(),
    }
}

pub fn palette() -> &'static Palette {
    if LIGHT_MODE.load(Ordering::Relaxed) {
        &LIGHT
    } else {
        &DARK
    }
}

pub fn bg() -> Color32 {
    palette().bg
}
pub fn surface() -> Color32 {
    palette().surface
}
pub fn border() -> Color32 {
    palette().border
}
pub fn text() -> Color32 {
    palette().text
}
pub fn text_dim() -> Color32 {
    palette().text_dim
}
pub fn blue() -> Color32 {
    palette().blue
}
pub fn blue_light() -> Color32 {
    palette().blue_light
}
pub fn red() -> Color32 {
    palette().red
}

/// Bold text uses this family when a bold system font is available.
pub const BOLD: &str = "bold";
pub fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    styled_button(ui, text, blue(), Color32::WHITE, Stroke::NONE)
}

pub fn secondary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    styled_button(
        ui,
        text,
        Color32::TRANSPARENT,
        palette().text,
        Stroke::new(1.0, border()),
    )
}

pub fn styled_button(
    ui: &mut egui::Ui,
    text: &str,
    fill: Color32,
    color: Color32,
    stroke: Stroke,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(120.0, 34.0), Sense::click());
    let fill = if resp.hovered() {
        if fill == Color32::TRANSPARENT {
            surface()
        } else {
            blue_light()
        }
    } else {
        fill
    };
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    ui.painter()
        .rect_stroke(rect, CornerRadius::same(8), stroke, StrokeKind::Inside);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        bold(14.0),
        color,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD.into()))
}

/// Use Segoe UI from Windows when present (closest to Roblox's own UI); egui's bundled
/// font otherwise.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let windows_fonts = std::env::var_os("WINDIR")
        .map(|w| std::path::PathBuf::from(w).join("Fonts"))
        .unwrap_or_default();

    let mut load = |key: &str, file: &str| -> bool {
        match std::fs::read(windows_fonts.join(file)) {
            Ok(bytes) => {
                fonts
                    .font_data
                    .insert(key.to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
                true
            }
            Err(_) => false,
        }
    };
    let regular = load("segoe", "segoeui.ttf");
    let semibold = load("segoe-semibold", "seguisb.ttf");

    if regular {
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "segoe".into());
    }

    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut bold_family = Vec::new();
    if semibold {
        bold_family.push("segoe-semibold".to_owned());
    }
    bold_family.extend(fallback);
    fonts
        .families
        .insert(FontFamily::Name(BOLD.into()), bold_family);

    ctx.set_fonts(fonts);
}

/// The logo drawn into a 64×64 icon for the taskbar.
pub fn app_icon() -> egui::IconData {
    const N: usize = 64;
    let mut rgba = vec![0u8; N * N * 4];
    let c = (N as f32 - 1.0) / 2.0;
    let (sin, cos) = 0.26f32.sin_cos();
    for y in 0..N {
        for x in 0..N {
            let (dx, dy) = (x as f32 - c, y as f32 - c);
            // rotate into the square's frame
            let (u, v) = (dx * cos + dy * sin, -dx * sin + dy * cos);
            let m = u.abs().max(v.abs());
            if m <= 26.0 && m > 11.0 {
                let i = (y * N + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[0x33, 0x5F, 0xFF, 0xFF]);
            }
        }
    }
    egui::IconData {
        rgba,
        width: N as u32,
        height: N as u32,
    }
}

/// Apply the theme to egui's built-in widgets (text boxes, checkboxes, combo boxes...).
pub fn apply_visuals(ctx: &egui::Context) {
    let p = palette();
    let mut v = if p.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.deep;
    v.faint_bg_color = p.surface;
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = p.blue;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.hyperlink_color = p.blue_light;
    for (w, fill) in [
        (&mut v.widgets.noninteractive, p.bg),
        (&mut v.widgets.inactive, p.surface),
        (&mut v.widgets.hovered, p.hover),
        (&mut v.widgets.active, p.border),
        (&mut v.widgets.open, p.surface),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.corner_radius = CornerRadius::same(6);
        w.fg_stroke.color = p.text;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.blue_light);
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.blue);
    // our palette decides, whatever egui thinks the OS theme is
    let theme = if p.dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.set_theme(theme);
    ctx.set_visuals_of(theme, v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 8.0);
        s.spacing.button_padding = Vec2::new(10.0, 5.0);
        s.spacing.interact_size.y = 28.0;
    });
}

/// Our mark, drawn still: a tilted blue square with a square hole of colour `hole`.
pub fn draw_mark(painter: &egui::Painter, center: egui::Pos2, half: f32, hole: Color32) {
    painter.add(egui::Shape::convex_polygon(
        square(center, half, 0.26),
        blue(),
        Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        square(center, half * 0.46, 0.26),
        hole,
        Stroke::NONE,
    ));
}

/// Corners of a square with half-width `half`, rotated by `angle` radians.
pub fn square(center: egui::Pos2, half: f32, angle: f32) -> Vec<egui::Pos2> {
    use std::f32::consts::{FRAC_PI_2, SQRT_2, TAU};
    (0..4)
        .map(|i| {
            let a = angle + FRAC_PI_2 * i as f32 + TAU / 8.0;
            center + Vec2::angled(a) * half * SQRT_2
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleAction {
    None,
    Minimize,
    Close,
}

/// Our own title bar for undecorated windows: drag anywhere on it to move the window,
/// with a minimise (optional) and close button on the right.
pub fn title_bar(ui: &mut egui::Ui, bar: egui::Rect, title: &str, minimize: bool) -> TitleAction {
    let drag = ui.interact(bar, ui.id().with("title_bar"), Sense::click_and_drag());
    if drag.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }

    let mark_center = bar.left_center() + Vec2::new(18.0, 0.0);
    draw_mark(
        ui.painter(),
        mark_center,
        6.5,
        Color32::from_rgb(0x14, 0x15, 0x17),
    );
    ui.painter().text(
        mark_center + Vec2::new(14.0, 0.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(12.5),
        text_dim(),
    );

    let mut action = TitleAction::None;
    let size = Vec2::new(40.0, bar.height());
    let close = egui::Rect::from_min_size(egui::pos2(bar.right() - size.x, bar.top()), size);

    let resp = ui.interact(close, ui.id().with("close"), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(
            close,
            CornerRadius::ZERO,
            Color32::from_rgb(0xC4, 0x2B, 0x1C),
        );
    }
    let c = close.center();
    let s = 4.5;
    let stroke = Stroke::new(
        1.3,
        if resp.hovered() {
            Color32::WHITE
        } else {
            text_dim()
        },
    );
    ui.painter()
        .line_segment([c + Vec2::new(-s, -s), c + Vec2::new(s, s)], stroke);
    ui.painter()
        .line_segment([c + Vec2::new(-s, s), c + Vec2::new(s, -s)], stroke);
    if resp.clicked() {
        action = TitleAction::Close;
    }

    if minimize {
        let min = close.translate(Vec2::new(-size.x, 0.0));
        let resp = ui.interact(min, ui.id().with("minimize"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(min, CornerRadius::ZERO, surface());
        }
        let c = min.center();
        let stroke = Stroke::new(1.3, if resp.hovered() { text() } else { text_dim() });
        ui.painter()
            .line_segment([c + Vec2::new(-5.0, 0.0), c + Vec2::new(5.0, 0.0)], stroke);
        if resp.clicked() {
            action = TitleAction::Minimize;
        }
    }

    action
}
