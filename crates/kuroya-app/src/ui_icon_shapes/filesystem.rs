use crate::{
    ui_icon_primitives::{draw_file, draw_folder, draw_plus},
    ui_icon_shapes::IconFrame,
    ui_icons::IconKind,
};
use egui::{Color32, Ui};

pub(super) fn draw_filesystem_icon(ui: &Ui, frame: &IconFrame, icon: IconKind, color: Color32) {
    match icon {
        IconKind::NewFile => {
            draw_file(ui, frame.rect(), color);
            draw_plus(ui, frame.p(16.0, 16.0), frame.rect().width() * 0.18, color);
        }
        IconKind::File => draw_file(ui, frame.rect(), color),
        IconKind::Folder => draw_folder(ui, frame.rect(), color, false),
        IconKind::FolderOpen => draw_folder(ui, frame.rect(), color, true),
        _ => {}
    }
}
