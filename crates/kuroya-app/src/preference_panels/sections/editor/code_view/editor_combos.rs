use eframe::egui;
use kuroya_core::{
    DiffAlgorithm, DiffWordWrap, EditorBracketPairGuideMode, EditorFindAutoFindInSelection,
    EditorFindHistory, EditorFindSeedSearchStringFromSelection, EditorFoldingStrategy,
    EditorHighlightActiveIndentation, EditorMatchBrackets, EditorMinimapAutohide,
    EditorMinimapShowSlider, EditorMinimapSide, EditorMinimapSize, EditorShowFoldingControls,
    EditorStickyScrollDefaultModel,
};

pub(super) fn editor_show_folding_controls_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut EditorShowFoldingControls,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(editor_show_folding_controls_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, EditorShowFoldingControls::Always, "Always");
            ui.selectable_value(value, EditorShowFoldingControls::Never, "Never");
            ui.selectable_value(value, EditorShowFoldingControls::Mouseover, "Mouseover");
        });
}

fn editor_show_folding_controls_label(mode: EditorShowFoldingControls) -> &'static str {
    match mode {
        EditorShowFoldingControls::Always => "Always",
        EditorShowFoldingControls::Never => "Never",
        EditorShowFoldingControls::Mouseover => "Mouseover",
    }
}

pub(super) fn editor_folding_strategy_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut EditorFoldingStrategy,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(editor_folding_strategy_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, EditorFoldingStrategy::Auto, "Auto");
            ui.selectable_value(value, EditorFoldingStrategy::Indentation, "Indentation");
        });
}

fn editor_folding_strategy_label(mode: EditorFoldingStrategy) -> &'static str {
    match mode {
        EditorFoldingStrategy::Auto => "Auto",
        EditorFoldingStrategy::Indentation => "Indentation",
    }
}

pub(super) fn editor_highlight_active_indentation_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut EditorHighlightActiveIndentation,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(editor_highlight_active_indentation_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, EditorHighlightActiveIndentation::Off, "Off");
            ui.selectable_value(value, EditorHighlightActiveIndentation::Focused, "Focused");
            ui.selectable_value(value, EditorHighlightActiveIndentation::Always, "Always");
        });
}

fn editor_highlight_active_indentation_label(
    mode: EditorHighlightActiveIndentation,
) -> &'static str {
    match mode {
        EditorHighlightActiveIndentation::Off => "Off",
        EditorHighlightActiveIndentation::Focused => "Focused",
        EditorHighlightActiveIndentation::Always => "Always",
    }
}

pub(super) fn editor_bracket_pair_guide_mode_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut EditorBracketPairGuideMode,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(editor_bracket_pair_guide_mode_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, EditorBracketPairGuideMode::Off, "Off");
            ui.selectable_value(value, EditorBracketPairGuideMode::Active, "Active");
            ui.selectable_value(value, EditorBracketPairGuideMode::On, "On");
        });
}

fn editor_bracket_pair_guide_mode_label(mode: EditorBracketPairGuideMode) -> &'static str {
    match mode {
        EditorBracketPairGuideMode::Off => "Off",
        EditorBracketPairGuideMode::Active => "Active",
        EditorBracketPairGuideMode::On => "On",
    }
}

pub(super) fn editor_match_brackets_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut EditorMatchBrackets,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(editor_match_brackets_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, EditorMatchBrackets::Always, "Always");
            ui.selectable_value(value, EditorMatchBrackets::Near, "Near");
            ui.selectable_value(value, EditorMatchBrackets::Never, "Never");
        });
}

fn editor_match_brackets_label(mode: EditorMatchBrackets) -> &'static str {
    match mode {
        EditorMatchBrackets::Always => "Always",
        EditorMatchBrackets::Near => "Near",
        EditorMatchBrackets::Never => "Never",
    }
}

pub(super) fn editor_find_seed_search_string_from_selection_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut EditorFindSeedSearchStringFromSelection,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(editor_find_seed_search_string_from_selection_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(
                value,
                EditorFindSeedSearchStringFromSelection::Always,
                "Always",
            );
            ui.selectable_value(
                value,
                EditorFindSeedSearchStringFromSelection::Selection,
                "Selection only",
            );
            ui.selectable_value(
                value,
                EditorFindSeedSearchStringFromSelection::Never,
                "Never",
            );
        });
}

fn editor_find_seed_search_string_from_selection_label(
    mode: EditorFindSeedSearchStringFromSelection,
) -> &'static str {
    match mode {
        EditorFindSeedSearchStringFromSelection::Always => "Always",
        EditorFindSeedSearchStringFromSelection::Selection => "Selection only",
        EditorFindSeedSearchStringFromSelection::Never => "Never",
    }
}

pub(super) fn editor_find_auto_find_in_selection_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut EditorFindAutoFindInSelection,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(editor_find_auto_find_in_selection_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, EditorFindAutoFindInSelection::Never, "Never");
            ui.selectable_value(value, EditorFindAutoFindInSelection::Always, "Always");
            ui.selectable_value(value, EditorFindAutoFindInSelection::Multiline, "Multiline");
        });
}

fn editor_find_auto_find_in_selection_label(mode: EditorFindAutoFindInSelection) -> &'static str {
    match mode {
        EditorFindAutoFindInSelection::Never => "Never",
        EditorFindAutoFindInSelection::Always => "Always",
        EditorFindAutoFindInSelection::Multiline => "Multiline",
    }
}

pub(super) fn editor_find_history_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut EditorFindHistory,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(editor_find_history_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, EditorFindHistory::Workspace, "Workspace");
            ui.selectable_value(value, EditorFindHistory::Never, "Never");
        });
}

fn editor_find_history_label(mode: EditorFindHistory) -> &'static str {
    match mode {
        EditorFindHistory::Never => "Never",
        EditorFindHistory::Workspace => "Workspace",
    }
}

pub(super) fn editor_minimap_side_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut EditorMinimapSide,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(editor_minimap_side_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, EditorMinimapSide::Left, "Left");
            ui.selectable_value(value, EditorMinimapSide::Right, "Right");
        });
}

fn editor_minimap_side_label(side: EditorMinimapSide) -> &'static str {
    match side {
        EditorMinimapSide::Left => "Left",
        EditorMinimapSide::Right => "Right",
    }
}

pub(super) fn editor_minimap_autohide_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut EditorMinimapAutohide,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(editor_minimap_autohide_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, EditorMinimapAutohide::None, "None");
            ui.selectable_value(value, EditorMinimapAutohide::Mouseover, "Mouseover");
            ui.selectable_value(value, EditorMinimapAutohide::Scroll, "Scroll");
        });
}

fn editor_minimap_autohide_label(mode: EditorMinimapAutohide) -> &'static str {
    match mode {
        EditorMinimapAutohide::None => "None",
        EditorMinimapAutohide::Mouseover => "Mouseover",
        EditorMinimapAutohide::Scroll => "Scroll",
    }
}

pub(super) fn editor_minimap_size_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut EditorMinimapSize,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(editor_minimap_size_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, EditorMinimapSize::Proportional, "Proportional");
            ui.selectable_value(value, EditorMinimapSize::Fill, "Fill");
            ui.selectable_value(value, EditorMinimapSize::Fit, "Fit");
        });
}

fn editor_minimap_size_label(mode: EditorMinimapSize) -> &'static str {
    match mode {
        EditorMinimapSize::Proportional => "Proportional",
        EditorMinimapSize::Fill => "Fill",
        EditorMinimapSize::Fit => "Fit",
    }
}

pub(super) fn editor_minimap_show_slider_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut EditorMinimapShowSlider,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(editor_minimap_show_slider_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, EditorMinimapShowSlider::Always, "Always");
            ui.selectable_value(value, EditorMinimapShowSlider::Mouseover, "Mouseover");
        });
}

fn editor_minimap_show_slider_label(mode: EditorMinimapShowSlider) -> &'static str {
    match mode {
        EditorMinimapShowSlider::Always => "Always",
        EditorMinimapShowSlider::Mouseover => "Mouseover",
    }
}

pub(super) fn editor_sticky_scroll_default_model_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut EditorStickyScrollDefaultModel,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(editor_sticky_scroll_default_model_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(
                value,
                EditorStickyScrollDefaultModel::OutlineModel,
                "Outline",
            );
            ui.selectable_value(
                value,
                EditorStickyScrollDefaultModel::FoldingProviderModel,
                "Folding provider",
            );
            ui.selectable_value(
                value,
                EditorStickyScrollDefaultModel::IndentationModel,
                "Indentation",
            );
        });
}

fn editor_sticky_scroll_default_model_label(mode: EditorStickyScrollDefaultModel) -> &'static str {
    match mode {
        EditorStickyScrollDefaultModel::OutlineModel => "Outline",
        EditorStickyScrollDefaultModel::FoldingProviderModel => "Folding provider",
        EditorStickyScrollDefaultModel::IndentationModel => "Indentation",
    }
}

pub(super) fn diff_algorithm_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut DiffAlgorithm,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(diff_algorithm_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, DiffAlgorithm::Advanced, "Advanced");
            ui.selectable_value(value, DiffAlgorithm::AdvancedExternal, "Advanced external");
            ui.selectable_value(value, DiffAlgorithm::AdvancedWasm, "Advanced WASM");
            ui.selectable_value(value, DiffAlgorithm::Legacy, "Legacy");
        });
}

fn diff_algorithm_label(mode: DiffAlgorithm) -> &'static str {
    match mode {
        DiffAlgorithm::Advanced => "Advanced",
        DiffAlgorithm::AdvancedExternal => "Advanced external",
        DiffAlgorithm::AdvancedWasm => "Advanced WASM",
        DiffAlgorithm::Legacy => "Legacy",
    }
}

pub(super) fn diff_word_wrap_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut DiffWordWrap,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(diff_word_wrap_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, DiffWordWrap::Inherit, "Inherit");
            ui.selectable_value(value, DiffWordWrap::On, "On");
            ui.selectable_value(value, DiffWordWrap::Off, "Off");
        });
}

fn diff_word_wrap_label(mode: DiffWordWrap) -> &'static str {
    match mode {
        DiffWordWrap::Inherit => "Inherit",
        DiffWordWrap::On => "On",
        DiffWordWrap::Off => "Off",
    }
}
