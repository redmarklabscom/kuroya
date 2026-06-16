use eframe::egui::{self, Color32, Response, RichText, Stroke, Ui, vec2};

#[derive(Clone, Copy)]
pub(crate) enum PopupButtonKind {
    Secondary,
    Primary,
    Danger,
}

pub(crate) fn popup_button(
    ui: &mut Ui,
    label: impl Into<String>,
    kind: PopupButtonKind,
) -> Response {
    ui.add(popup_button_widget(label, kind, 78.0))
}

pub(crate) fn popup_compact_button(
    ui: &mut Ui,
    label: impl Into<String>,
    kind: PopupButtonKind,
) -> Response {
    ui.add(popup_button_widget(label, kind, 58.0))
}

pub(crate) fn popup_compact_button_enabled(
    ui: &mut Ui,
    enabled: bool,
    label: impl Into<String>,
    kind: PopupButtonKind,
) -> Response {
    ui.add_enabled(enabled, popup_button_widget(label, kind, 58.0))
}

pub(crate) fn popup_button_enabled(
    ui: &mut Ui,
    enabled: bool,
    label: impl Into<String>,
    kind: PopupButtonKind,
) -> Response {
    ui.add_enabled(enabled, popup_button_widget(label, kind, 78.0))
}

fn popup_button_widget(
    label: impl Into<String>,
    kind: PopupButtonKind,
    min_width: f32,
) -> egui::Button<'static> {
    let (fill, stroke, text, strong) = match kind {
        PopupButtonKind::Secondary => (
            Color32::from_rgb(58, 58, 58),
            Color32::from_rgb(82, 82, 82),
            Color32::from_rgb(214, 214, 214),
            false,
        ),
        PopupButtonKind::Primary => (
            Color32::from_rgb(84, 84, 84),
            Color32::from_rgb(112, 112, 112),
            Color32::from_rgb(245, 245, 245),
            true,
        ),
        PopupButtonKind::Danger => (
            Color32::from_rgb(111, 48, 48),
            Color32::from_rgb(163, 74, 74),
            Color32::from_rgb(255, 238, 238),
            true,
        ),
    };
    let mut text = RichText::new(label.into()).color(text);
    if strong {
        text = text.strong();
    }
    egui::Button::new(text)
        .min_size(vec2(min_width, 28.0))
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(egui::CornerRadius::same(4))
}
