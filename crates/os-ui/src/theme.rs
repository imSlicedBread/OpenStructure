//! OpenStructure's light workspace tokens and independently drawn command icons.
use eframe::egui::{self, Color32, Stroke};

pub const BACKGROUND: Color32 = Color32::from_rgb(243, 244, 246);
pub const SURFACE: Color32 = Color32::WHITE;
pub const CANVAS: Color32 = Color32::from_rgb(250, 251, 252);
pub const BORDER: Color32 = Color32::from_rgb(209, 213, 219);
pub const TEXT: Color32 = Color32::from_rgb(32, 37, 43);
pub const MUTED: Color32 = Color32::from_rgb(91, 101, 115);
pub const ACCENT: Color32 = Color32::from_rgb(37, 99, 235);
pub const SELECTED: Color32 = Color32::from_rgb(226, 237, 255);
pub const ERROR: Color32 = Color32::from_rgb(180, 35, 24);
pub const GRID: Color32 = Color32::from_rgb(221, 226, 232);

/// Call once during application setup; logical sizes follow egui's DPI scaling.
pub fn apply(ctx: &egui::Context) {
    let mut style = egui::Style {
        visuals: egui::Visuals::light(),
        ..Default::default()
    };
    style.visuals.panel_fill = SURFACE;
    style.visuals.window_fill = SURFACE;
    style.visuals.extreme_bg_color = SURFACE;
    style.visuals.faint_bg_color = BACKGROUND;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.selection.bg_fill = SELECTED;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    style.visuals.error_fg_color = ERROR;
    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(2);
        widget.bg_stroke = Stroke::new(1.0_f32, BORDER);
        widget.fg_stroke = Stroke::new(1.0_f32, TEXT);
    }
    style.visuals.widgets.inactive.bg_fill = BACKGROUND;
    style.visuals.widgets.inactive.weak_bg_fill = BACKGROUND;
    style.visuals.widgets.hovered.bg_fill = SELECTED;
    style.visuals.widgets.hovered.weak_bg_fill = SELECTED;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    style.visuals.widgets.active.bg_fill = SELECTED;
    style.visuals.widgets.active.weak_bg_fill = SELECTED;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, ACCENT);
    style.visuals.widgets.open.bg_fill = SELECTED;
    style.visuals.widgets.open.weak_bg_fill = SELECTED;
    style.visuals.window_corner_radius = egui::CornerRadius::same(2);
    style.spacing.item_spacing = egui::vec2(8.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.interact_size = egui::vec2(24.0, 24.0);
    style.spacing.indent = 16.0;
    style.spacing.text_edit_width = 160.0;
    for (kind, size) in [
        (egui::TextStyle::Body, 13.0),
        (egui::TextStyle::Button, 13.0),
        (egui::TextStyle::Small, 12.0),
        (egui::TextStyle::Heading, 14.0),
    ] {
        style
            .text_styles
            .insert(kind, egui::FontId::proportional(size));
    }
    ctx.set_style(style);
}

#[derive(Clone, Copy)]
pub(crate) enum Icon {
    Wall,
    Room,
    Level,
    Fit,
    Apply,
    Delete,
    Save,
    Undo,
    Redo,
    Measure,
}

fn icon(painter: &egui::Painter, rect: egui::Rect, kind: Icon, color: Color32) {
    let p = |x, y| {
        egui::pos2(
            rect.left() + rect.width() * x,
            rect.top() + rect.height() * y,
        )
    };
    let stroke = Stroke::new(1.5_f32, color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([p(a.0, a.1), p(b.0, b.1)], stroke);
    };
    match kind {
        Icon::Wall => {
            for y in [0.15, 0.5, 0.85] {
                line((0.1, y), (0.9, y));
            }
            for x in [0.1, 0.9] {
                line((x, 0.15), (x, 0.85));
            }
            line((0.45, 0.15), (0.45, 0.5));
            line((0.65, 0.5), (0.65, 0.85));
        }
        Icon::Room => {
            for (a, b) in [
                ((0.15, 0.15), (0.85, 0.15)),
                ((0.85, 0.15), (0.85, 0.85)),
                ((0.85, 0.85), (0.15, 0.85)),
                ((0.15, 0.85), (0.15, 0.15)),
                ((0.28, 0.5), (0.72, 0.5)),
            ] {
                line(a, b);
            }
        }
        Icon::Level => {
            line((0.1, 0.75), (0.9, 0.75));
            line((0.1, 0.3), (0.6, 0.3));
            line((0.75, 0.1), (0.75, 0.5));
            line((0.55, 0.3), (0.95, 0.3));
        }
        Icon::Fit => {
            for (x, y, dx, dy) in [
                (0.1, 0.1, 0.25, 0.25),
                (0.9, 0.1, -0.25, 0.25),
                (0.1, 0.9, 0.25, -0.25),
                (0.9, 0.9, -0.25, -0.25),
            ] {
                line((x, y), (x + dx, y));
                line((x, y), (x, y + dy));
            }
        }
        Icon::Apply => {
            line((0.15, 0.5), (0.4, 0.75));
            line((0.4, 0.75), (0.85, 0.2));
        }
        Icon::Delete => {
            line((0.15, 0.25), (0.85, 0.25));
            line((0.35, 0.1), (0.65, 0.1));
            line((0.25, 0.25), (0.3, 0.9));
            line((0.3, 0.9), (0.7, 0.9));
            line((0.7, 0.9), (0.75, 0.25));
            line((0.5, 0.4), (0.5, 0.75));
        }
        Icon::Save => {
            for (a, b) in [
                ((0.15, 0.1), (0.75, 0.1)),
                ((0.75, 0.1), (0.9, 0.25)),
                ((0.9, 0.25), (0.9, 0.9)),
                ((0.9, 0.9), (0.15, 0.9)),
                ((0.15, 0.9), (0.15, 0.1)),
                ((0.35, 0.1), (0.35, 0.4)),
                ((0.35, 0.4), (0.7, 0.4)),
                ((0.7, 0.4), (0.7, 0.1)),
                ((0.35, 0.9), (0.35, 0.6)),
                ((0.35, 0.6), (0.7, 0.6)),
                ((0.7, 0.6), (0.7, 0.9)),
            ] {
                line(a, b);
            }
        }
        Icon::Undo | Icon::Redo => {
            let flip = |x| {
                if matches!(kind, Icon::Redo) {
                    1.0 - x
                } else {
                    x
                }
            };
            for (a, b) in [
                ((0.1, 0.35), (0.4, 0.1)),
                ((0.1, 0.35), (0.4, 0.6)),
                ((0.1, 0.35), (0.65, 0.35)),
                ((0.65, 0.35), (0.85, 0.5)),
                ((0.85, 0.5), (0.85, 0.8)),
                ((0.85, 0.8), (0.5, 0.8)),
            ] {
                line((flip(a.0), a.1), (flip(b.0), b.1));
            }
        }
        Icon::Measure => {
            line((0.15, 0.72), (0.85, 0.72));
            line((0.15, 0.56), (0.15, 0.88));
            line((0.85, 0.56), (0.85, 0.88));
            line((0.22, 0.78), (0.34, 0.66));
            line((0.48, 0.78), (0.60, 0.66));
            line((0.72, 0.78), (0.84, 0.66));
        }
    }
}

pub(crate) fn command(ui: &mut egui::Ui, kind: Icon, label: &str, large: bool) -> egui::Response {
    let size = if large {
        egui::vec2(96.0, 52.0)
    } else {
        egui::vec2(72.0, 24.0)
    };
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        let fill =
            if response.hovered() || response.has_focus() || response.is_pointer_button_down_on() {
                visuals.bg_fill
            } else {
                Color32::TRANSPARENT
            };
        let stroke = if response.hovered() || response.has_focus() {
            Stroke::new(1.0_f32, ACCENT)
        } else {
            Stroke::NONE
        };
        ui.painter()
            .rect(rect, 2, fill, stroke, egui::StrokeKind::Inside);
        let color = if ui.is_enabled() {
            TEXT
        } else {
            MUTED.gamma_multiply(0.55)
        };
        let icon_rect = if large {
            egui::Rect::from_center_size(
                rect.center_top() + egui::vec2(0.0, 17.0),
                egui::vec2(26.0, 26.0),
            )
        } else {
            egui::Rect::from_min_size(rect.min + egui::vec2(5.0, 4.0), egui::vec2(16.0, 16.0))
        };
        icon(
            ui.painter(),
            icon_rect,
            kind,
            if large { ACCENT } else { color },
        );
        ui.painter().text(
            if large {
                rect.center_bottom() - egui::vec2(0.0, 9.0)
            } else {
                rect.left_center() + egui::vec2(26.0, 0.0)
            },
            if large {
                egui::Align2::CENTER_CENTER
            } else {
                egui::Align2::LEFT_CENTER
            },
            label,
            egui::FontId::proportional(12.0),
            color,
        );
    }
    response
}

pub(crate) fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(title).strong().size(12.0).color(MUTED));
    ui.separator();
}
