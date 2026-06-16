use crate::ui_icons::IconKind;
use egui::{Color32, Pos2, Rect, Ui, pos2};

mod chrome;
mod filesystem;
mod tools;

pub(super) struct IconFrame {
    rect: Rect,
}

const MIN_ICON_SIDE: f32 = 0.5;
const MAX_ICON_SIDE: f32 = 4096.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IconFamily {
    Filesystem,
    Chrome,
    Tools,
}

fn icon_family(icon: IconKind) -> IconFamily {
    match icon {
        IconKind::NewFile | IconKind::File | IconKind::Folder | IconKind::FolderOpen => {
            IconFamily::Filesystem
        }
        IconKind::ChevronRight
        | IconKind::ChevronDown
        | IconKind::Plus
        | IconKind::Minus
        | IconKind::Refresh
        | IconKind::Maximize
        | IconKind::Restore
        | IconKind::Close
        | IconKind::Panes => IconFamily::Chrome,
        IconKind::Command
        | IconKind::Search
        | IconKind::Terminal
        | IconKind::Trash
        | IconKind::Copy
        | IconKind::GitBranch
        | IconKind::Diagnostics
        | IconKind::Lsp
        | IconKind::Cursor
        | IconKind::Theme
        | IconKind::Code
        | IconKind::Settings => IconFamily::Tools,
    }
}

impl IconFrame {
    pub(super) fn new(rect: Rect) -> Option<Self> {
        safe_icon_rect(rect).map(|rect| Self { rect })
    }

    pub(super) fn rect(&self) -> Rect {
        self.rect
    }

    pub(super) fn p(&self, x: f32, y: f32) -> Pos2 {
        pos2(
            self.rect.left() + self.rect.width() * (x / 24.0),
            self.rect.top() + self.rect.height() * (y / 24.0),
        )
    }

    pub(super) fn rr(&self, min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Rect {
        Rect::from_min_max(self.p(min_x, min_y), self.p(max_x, max_y))
    }
}

pub(crate) fn draw_icon(ui: &Ui, rect: Rect, icon: IconKind, color: Color32) {
    let Some(frame) = IconFrame::new(rect) else {
        return;
    };
    let color = safe_icon_color(color, ui);
    match icon_family(icon) {
        IconFamily::Filesystem => filesystem::draw_filesystem_icon(ui, &frame, icon, color),
        IconFamily::Chrome => chrome::draw_chrome_icon(ui, &frame, icon, color),
        IconFamily::Tools => tools::draw_tool_icon(ui, &frame, icon, color),
    }
}

fn safe_icon_rect(rect: Rect) -> Option<Rect> {
    if !is_finite_rect(rect) {
        return None;
    }

    let width = rect.width();
    let height = rect.height();
    if !width.is_finite()
        || !height.is_finite()
        || width < MIN_ICON_SIDE
        || height < MIN_ICON_SIDE
        || width > MAX_ICON_SIDE
        || height > MAX_ICON_SIDE
    {
        return None;
    }

    Some(rect)
}

fn is_finite_rect(rect: Rect) -> bool {
    rect.min.x.is_finite()
        && rect.min.y.is_finite()
        && rect.max.x.is_finite()
        && rect.max.y.is_finite()
}

fn safe_icon_color(color: Color32, ui: &Ui) -> Color32 {
    let visuals = ui.visuals();
    let fallback = opaque_color_or(
        visuals.widgets.inactive.fg_stroke.color,
        opaque_color_or(
            visuals.text_color(),
            default_icon_color(visuals.extreme_bg_color),
        ),
    );
    opaque_color_or(color, fallback)
}

fn opaque_color_or(color: Color32, fallback: Color32) -> Color32 {
    let color = if color.a() == 0 { fallback } else { color };
    color.to_opaque()
}

fn default_icon_color(background: Color32) -> Color32 {
    let brightness = background.r() as u16 + background.g() as u16 + background.b() as u16;
    if brightness < 384 {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

#[cfg(test)]
mod tests {
    use super::{IconFamily, IconFrame, icon_family, opaque_color_or, safe_icon_rect};
    use crate::ui_icons::IconKind;
    use egui::{Color32, Rect, pos2, vec2};

    #[test]
    fn icon_family_routes_filesystem_icons() {
        for icon in [
            IconKind::NewFile,
            IconKind::File,
            IconKind::Folder,
            IconKind::FolderOpen,
        ] {
            assert_eq!(icon_family(icon), IconFamily::Filesystem);
        }
    }

    #[test]
    fn icon_family_routes_chrome_icons() {
        for icon in [
            IconKind::ChevronRight,
            IconKind::ChevronDown,
            IconKind::Plus,
            IconKind::Minus,
            IconKind::Refresh,
            IconKind::Maximize,
            IconKind::Restore,
            IconKind::Close,
            IconKind::Panes,
        ] {
            assert_eq!(icon_family(icon), IconFamily::Chrome);
        }
    }

    #[test]
    fn icon_family_routes_tool_icons() {
        for icon in [
            IconKind::Command,
            IconKind::Search,
            IconKind::Terminal,
            IconKind::Trash,
            IconKind::Copy,
            IconKind::GitBranch,
            IconKind::Diagnostics,
            IconKind::Lsp,
            IconKind::Cursor,
            IconKind::Theme,
            IconKind::Code,
            IconKind::Settings,
        ] {
            assert_eq!(icon_family(icon), IconFamily::Tools);
        }
    }

    #[test]
    fn icon_frame_rejects_non_finite_and_degenerate_rects() {
        assert!(IconFrame::new(Rect::from_min_size(pos2(0.0, 0.0), vec2(22.0, 22.0))).is_some());
        assert!(
            IconFrame::new(Rect::from_min_size(pos2(f32::NAN, 0.0), vec2(22.0, 22.0))).is_none()
        );
        assert!(IconFrame::new(Rect::from_min_size(pos2(0.0, 0.0), vec2(0.0, 22.0))).is_none());
        assert!(IconFrame::new(Rect::from_min_size(pos2(0.0, 0.0), vec2(0.1, 0.1))).is_none());
        assert!(IconFrame::new(Rect::from_min_size(pos2(0.0, 0.0), vec2(5000.0, 22.0))).is_none());
    }

    #[test]
    fn safe_icon_rect_rejects_infinite_coordinates() {
        assert!(
            safe_icon_rect(Rect::from_min_max(
                pos2(0.0, 0.0),
                pos2(f32::INFINITY, 22.0)
            ))
            .is_none()
        );
    }

    #[test]
    fn opaque_icon_color_falls_back_from_transparent_tints() {
        let fallback = Color32::from_rgb(10, 20, 30);

        assert_eq!(
            opaque_color_or(Color32::TRANSPARENT, fallback),
            Color32::from_rgb(10, 20, 30)
        );
        assert_eq!(
            opaque_color_or(Color32::from_rgb(40, 50, 60), fallback),
            Color32::from_rgb(40, 50, 60)
        );
    }

    #[test]
    fn draw_icon_ignores_unsafe_rects_without_panicking() {
        let ctx = egui::Context::default();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for rect in [
                    Rect::from_min_size(pos2(0.0, 0.0), vec2(0.0, 0.0)),
                    Rect::from_min_size(pos2(0.0, 0.0), vec2(0.1, 0.1)),
                    Rect::from_min_size(pos2(f32::NAN, 0.0), vec2(22.0, 22.0)),
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(f32::INFINITY, 22.0)),
                ] {
                    super::draw_icon(ui, rect, IconKind::Refresh, Color32::TRANSPARENT);
                }
            });
        });
    }
}
