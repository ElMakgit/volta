//! One set of measurements for the whole window.
//!
//! The interface is meant to read as a single flat slab: square corners, one
//! pixel rules, the same control height everywhere. Keeping the numbers here
//! stops the panels from drifting apart as the app grows. The drawing on the
//! bench is a different story: a breadboard and a motor keep their physical
//! radii, because that picture has to look like Tinkercad.

use eframe::egui::{self, vec2, Color32, CornerRadius, Margin, Stroke};

/// Nothing in the chrome is ever rounded.
pub const RADIUS: CornerRadius = CornerRadius::ZERO;

/// Height of a palette cell: an icon slot plus a one-line caption.
pub const CONTROL_H: f32 = 36.0;
/// The icon slot inside a palette cell.
pub const ICON: f32 = 26.0;
/// Gap between cells and sections.
pub const GAP: f32 = 6.0;
/// Margin inside a panel.
pub const PAD: f32 = 16.0;

pub const TEXT_MICRO: f32 = 10.5;
pub const TEXT_SMALL: f32 = 11.5;
pub const TEXT_BODY: f32 = 13.5;
pub const TEXT_TITLE: f32 = 16.0;
pub const TEXT_BRAND: f32 = 22.0;

/// Flat visual style: no rounding, no shadows, one accent.
pub fn apply(style: &mut egui::Style, accent: Color32, bg: Color32, fg: Color32, muted: Color32) {
    let panel = bg.lerp_to_gamma(Color32::WHITE, 0.035);
    let rule = muted.gamma_multiply(0.35);

    style.spacing.item_spacing = vec2(8.0, 8.0);
    style.spacing.button_padding = vec2(10.0, 5.0);
    style.spacing.menu_margin = Margin::same(6);
    style.spacing.scroll.floating = false;
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.bar_inner_margin = 3.0;
    style.spacing.scroll.bar_outer_margin = 2.0;
    // A solid bar paints its handle in `widgets.*.bg_fill`; asking for the
    // text colour turns it into a bright stripe that shouts at the eye.
    style.spacing.scroll.foreground_color = false;
    style.visuals.striped = false;

    let v = &mut style.visuals;
    v.panel_fill = panel;
    v.window_fill = panel;
    v.extreme_bg_color = bg.lerp_to_gamma(Color32::WHITE, 0.02);
    v.faint_bg_color = bg.lerp_to_gamma(Color32::WHITE, 0.05);
    v.override_text_color = Some(fg);
    v.hyperlink_color = accent;
    v.selection.bg_fill = accent;
    v.selection.stroke = Stroke::new(1.0, bg);
    v.window_corner_radius = RADIUS;
    v.menu_corner_radius = RADIUS;
    v.window_shadow = egui::epaint::Shadow {
        offset: [0, 0],
        blur: 0,
        spread: 0,
        color: Color32::TRANSPARENT,
    };
    v.popup_shadow = v.window_shadow;

    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = RADIUS;
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, rule);
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_fill = muted.gamma_multiply(0.55);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, rule);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, fg);
    v.widgets.hovered.weak_bg_fill = panel.lerp_to_gamma(Color32::WHITE, 0.05);
    v.widgets.hovered.bg_fill = muted;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, muted);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, fg);
    v.widgets.active.weak_bg_fill = accent;
    v.widgets.active.bg_fill = accent;
    v.widgets.active.fg_stroke = Stroke::new(1.0, bg);
    v.widgets.active.bg_stroke = Stroke::new(1.0, accent);
}
