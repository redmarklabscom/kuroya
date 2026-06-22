#[cfg(test)]
use crate::lsp_hover_markdown::HoverMarkdownBlock;
use crate::{
    KuroyaApp,
    completion_preview::completion_item_preview_text,
    lsp_completion_imports::completion_has_auto_import_edit,
    lsp_hover_markdown::parse_hover_markdown,
    lsp_labels::completion_kind_label,
    lsp_markdown_render::{LspMarkdownTextSize, render_lsp_markdown_blocks},
    ui_state::{clamp_selection, selected_row_scroll_offset},
};
use eframe::egui::{
    RichText, ScrollArea, Ui,
    cache::{ComputerMut, FrameCache},
};
use kuroya_core::{EditorSuggestPreviewMode, LspCompletionItem};
use std::hash::{Hash, Hasher};
use std::{borrow::Cow, sync::Arc};

const MAX_COMPLETION_DOCUMENTATION_CHARS: usize = 8_000;
const COMPLETION_DOCUMENTATION_TRUNCATED_NOTICE: &str = "\n\n[Documentation truncated]";
const COMPLETION_LABEL_DISPLAY_MAX_CHARS: usize = 160;
const COMPLETION_DETAIL_DISPLAY_MAX_CHARS: usize = 180;
const COMPLETION_DISPLAY_TRUNCATED_NOTICE: &str = "...";
const COMPLETION_DISPLAY_SANITIZE_SAMPLE_BYTES: usize = 4 * 1024;
const COMPLETION_AUTO_IMPORT_LABEL: &str = "auto-import";

impl KuroyaApp {
    pub(super) fn render_completion_items(
        &mut self,
        ui: &mut Ui,
        scroll_to_selection: bool,
    ) -> Option<usize> {
        if self.completion_items.is_empty() {
            ui.add_space(24.0);
            ui.centered_and_justified(|ui| {
                ui.label("No completions");
            });
            return None;
        }

        clamp_selection(&mut self.completion_selected, self.completion_items.len());
        let mut apply = None;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() * 0.58).max(260.0));
                let row_height = completion_item_row_height(
                    self.settings.suggest_line_height,
                    ui.spacing().interact_size.y,
                );
                let viewport_height = ui.available_height();
                let mut scroll_area = ScrollArea::vertical().id_salt("completion_items");
                if scroll_to_selection {
                    scroll_area = scroll_area.vertical_scroll_offset(selected_row_scroll_offset(
                        self.completion_selected,
                        self.completion_items.len(),
                        row_height,
                        viewport_height,
                    ));
                }
                scroll_area.show_rows(ui, row_height, self.completion_items.len(), |ui, rows| {
                    if self.settings.suggest_line_height > 0 {
                        ui.spacing_mut().interact_size.y = self.settings.suggest_line_height as f32;
                    }
                    for idx in rows {
                        let clicked = {
                            let item = &self.completion_items[idx];
                            let label = cached_completion_item_rich_label(
                                ui,
                                item,
                                self.settings.suggest_show_icons,
                                self.settings.suggest_show_inline_details,
                                self.settings.suggest_font_size,
                            );
                            ui.selectable_label(idx == self.completion_selected, label)
                                .clicked()
                        };
                        if clicked {
                            self.completion_selected = idx;
                            apply = Some(idx);
                        }
                    }
                });
            });
            ui.separator();
            let selected_item = self.completion_items.get(self.completion_selected);
            render_completion_documentation(
                ui,
                selected_item,
                &self.completion_prefix,
                self.settings.suggest_preview,
                self.settings.suggest_preview_mode,
            );
        });
        apply
    }
}

#[cfg(test)]
pub(crate) fn completion_item_label(
    item: &LspCompletionItem,
    show_icons: bool,
    show_inline_details: bool,
) -> String {
    completion_item_row_display(item, show_icons, show_inline_details).label_text()
}

#[cfg(test)]
fn completion_item_row_display(
    item: &LspCompletionItem,
    show_icons: bool,
    show_inline_details: bool,
) -> CompletionItemRowDisplay<'_> {
    CompletionItemRowInputs::from_item(item, show_icons, show_inline_details).display()
}

fn completion_item_row_kind(item: &LspCompletionItem, show_icons: bool) -> Option<&'static str> {
    show_icons.then(|| item.kind.map(completion_kind_label).unwrap_or("item"))
}

fn completion_item_has_auto_import_label(item: &LspCompletionItem) -> bool {
    !item.additional_text_edits.is_empty() && completion_has_auto_import_edit(item)
}

fn completion_item_detail_display(item: &LspCompletionItem) -> Option<Cow<'_, str>> {
    item.detail
        .as_deref()
        .and_then(completion_detail_display_text)
}

fn completion_detail_display_text(detail: &str) -> Option<Cow<'_, str>> {
    completion_display_text(detail, COMPLETION_DETAIL_DISPLAY_MAX_CHARS)
}

#[derive(Clone, Copy)]
struct CompletionItemRowInputs<'a> {
    kind: Option<&'static str>,
    label: &'a str,
    has_auto_import: bool,
    detail: Option<&'a str>,
    deprecated: bool,
}

impl<'a> CompletionItemRowInputs<'a> {
    fn from_item(item: &'a LspCompletionItem, show_icons: bool, show_inline_details: bool) -> Self {
        Self {
            kind: completion_item_row_kind(item, show_icons),
            label: &item.label,
            has_auto_import: completion_item_has_auto_import_label(item),
            detail: completion_item_row_detail_source(item, show_inline_details),
            deprecated: item.deprecated,
        }
    }

    fn display(self) -> CompletionItemRowDisplay<'a> {
        CompletionItemRowDisplay {
            kind: self.kind,
            label: completion_display_label(self.label),
            has_auto_import: self.has_auto_import,
            detail: self.detail.and_then(completion_detail_display_text),
            deprecated: self.deprecated,
        }
    }
}

fn completion_item_row_detail_source(
    item: &LspCompletionItem,
    show_inline_details: bool,
) -> Option<&str> {
    if show_inline_details {
        item.detail.as_deref()
    } else {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CompletionItemRowDisplay<'a> {
    kind: Option<&'static str>,
    label: Cow<'a, str>,
    has_auto_import: bool,
    detail: Option<Cow<'a, str>>,
    deprecated: bool,
}

impl<'a> CompletionItemRowDisplay<'a> {
    fn as_ref(&self) -> CompletionItemRowDisplayRef<'_> {
        CompletionItemRowDisplayRef {
            kind: self.kind,
            label: self.label.as_ref(),
            has_auto_import: self.has_auto_import,
            detail: self.detail.as_deref(),
            deprecated: self.deprecated,
        }
    }

    #[cfg(test)]
    fn label_text(&self) -> String {
        completion_item_row_label_text(self.as_ref())
    }
}

#[derive(Clone, Copy, Hash)]
struct CompletionItemRowDisplayRef<'a> {
    kind: Option<&'static str>,
    label: &'a str,
    has_auto_import: bool,
    detail: Option<&'a str>,
    deprecated: bool,
}

fn completion_item_row_label_text(display: CompletionItemRowDisplayRef<'_>) -> String {
    let mut capacity = display.label.len();
    if let Some(kind) = display.kind {
        capacity += kind.len() + 2;
    }
    if display.has_auto_import {
        capacity += COMPLETION_AUTO_IMPORT_LABEL.len() + 2;
    }
    if let Some(detail) = display.detail {
        capacity += detail.len() + 2;
    }

    let mut text = String::with_capacity(capacity);
    if let Some(kind) = display.kind {
        text.push_str(kind);
        text.push_str("  ");
    }
    text.push_str(display.label);
    if display.has_auto_import {
        text.push_str("  ");
        text.push_str(COMPLETION_AUTO_IMPORT_LABEL);
    }
    if let Some(detail) = display.detail {
        text.push_str("  ");
        text.push_str(detail);
    }
    text
}

fn completion_item_rich_label(
    display: CompletionItemRowDisplayRef<'_>,
    font_size: usize,
) -> RichText {
    let mut label = RichText::new(completion_item_row_label_text(display));
    if font_size > 0 {
        label = label.size(font_size as f32);
    }
    if display.deprecated {
        label = label.strikethrough();
    }
    label
}

fn cached_completion_item_rich_label(
    ui: &mut Ui,
    item: &LspCompletionItem,
    show_icons: bool,
    show_inline_details: bool,
    font_size: usize,
) -> Arc<RichText> {
    let key = CompletionItemRichLabelCacheKey {
        row: CompletionItemRowInputs::from_item(item, show_icons, show_inline_details),
        font_size,
    };
    ui.memory_mut(|memory| {
        memory
            .caches
            .cache::<CompletionItemRichLabelFrameCache>()
            .get(key)
    })
}

type CompletionItemRichLabelFrameCache = FrameCache<Arc<RichText>, CompletionItemRichLabelComputer>;

#[derive(Default)]
struct CompletionItemRichLabelComputer;

impl<'a> ComputerMut<CompletionItemRichLabelCacheKey<'a>, Arc<RichText>>
    for CompletionItemRichLabelComputer
{
    fn compute(&mut self, key: CompletionItemRichLabelCacheKey<'a>) -> Arc<RichText> {
        let display = key.row.display();
        Arc::new(completion_item_rich_label(display.as_ref(), key.font_size))
    }
}

#[derive(Clone, Copy)]
struct CompletionItemRichLabelCacheKey<'a> {
    row: CompletionItemRowInputs<'a>,
    font_size: usize,
}

impl Hash for CompletionItemRichLabelCacheKey<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        0u8.hash(state);
        self.font_size.hash(state);

        1u8.hash(state);
        self.row.kind.hash(state);
        2u8.hash(state);
        hash_completion_display_label(self.row.label, state);
        3u8.hash(state);
        self.row.has_auto_import.hash(state);
        4u8.hash(state);
        hash_completion_item_row_detail(self.row.detail, state);
        5u8.hash(state);
        self.row.deprecated.hash(state);
    }
}

#[cfg(test)]
fn completion_item_rich_label_cache_hash(
    item: &LspCompletionItem,
    show_icons: bool,
    show_inline_details: bool,
    font_size: usize,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    CompletionItemRichLabelCacheKey {
        row: CompletionItemRowInputs::from_item(item, show_icons, show_inline_details),
        font_size,
    }
    .hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn completion_item_row_height(setting: usize, fallback: f32) -> f32 {
    if setting > 0 {
        setting as f32
    } else {
        fallback
    }
}

fn render_completion_documentation(
    ui: &mut Ui,
    item: Option<&LspCompletionItem>,
    completion_prefix: &str,
    suggest_preview: bool,
    suggest_preview_mode: EditorSuggestPreviewMode,
) {
    let weak_text_color = ui.visuals().weak_text_color();
    ui.vertical(|ui| {
        ui.set_width(ui.available_width().max(220.0));
        ui.label(
            RichText::new("Documentation")
                .small()
                .color(weak_text_color),
        );
        ui.add_space(4.0);

        let Some(item) = item else {
            ui.label(RichText::new("No completion selected").small());
            return;
        };

        ui.label(RichText::new(completion_display_label(&item.label)).strong());
        if let Some(detail) = completion_item_detail_display(item) {
            ui.label(RichText::new(detail).small().color(weak_text_color));
        }

        if suggest_preview
            && let Some(preview) =
                completion_item_preview_text(item, completion_prefix, suggest_preview_mode)
        {
            ui.add_space(8.0);
            ui.label(RichText::new("Preview").small().color(weak_text_color));
            ui.label(RichText::new(preview).monospace().small());
        }

        ui.add_space(8.0);
        match bounded_completion_item_documentation(item) {
            Some(documentation) => {
                let blocks = parse_hover_markdown(documentation.as_ref());
                ScrollArea::vertical()
                    .id_salt("completion_documentation")
                    .max_height(190.0)
                    .show(ui, |ui| {
                        if blocks.is_empty() {
                            ui.label(RichText::new(documentation.as_ref()).small());
                        } else {
                            render_lsp_markdown_blocks(ui, &blocks, LspMarkdownTextSize::Small);
                        }
                    });
            }
            None => {
                ui.label(
                    RichText::new("No documentation provided")
                        .small()
                        .color(weak_text_color),
                );
            }
        }
    });
}

fn completion_display_label(label: &str) -> Cow<'_, str> {
    completion_display_text(label, COMPLETION_LABEL_DISPLAY_MAX_CHARS)
        .unwrap_or(Cow::Borrowed("completion"))
}

fn completion_display_text(text: &str, max_chars: usize) -> Option<Cow<'_, str>> {
    if max_chars == 0 {
        return None;
    }

    let text = completion_display_text_sanitize_input(text, max_chars);
    if text.is_empty() {
        return None;
    }
    if completion_display_text_can_borrow(text, max_chars) {
        return Some(Cow::Borrowed(text));
    }

    let mut output = String::with_capacity(
        text.len()
            .min(max_chars + COMPLETION_DISPLAY_TRUNCATED_NOTICE.len()),
    );
    let mut chars = 0usize;
    let mut pending_space = false;
    let mut truncated = false;

    for ch in text.chars() {
        if ch.is_control() || ch.is_whitespace() {
            pending_space = !output.is_empty();
            continue;
        }
        if is_completion_display_format_control(ch) {
            continue;
        }
        if pending_space {
            if chars >= max_chars {
                truncated = true;
                break;
            }
            output.push(' ');
            chars += 1;
            pending_space = false;
        }
        if chars >= max_chars {
            truncated = true;
            break;
        }
        output.push(ch);
        chars += 1;
    }

    if output.is_empty() {
        return None;
    }

    if truncated && max_chars >= COMPLETION_DISPLAY_TRUNCATED_NOTICE.len() {
        while output.ends_with(' ') && chars > 0 {
            output.pop();
            chars -= 1;
        }
        while chars + COMPLETION_DISPLAY_TRUNCATED_NOTICE.len() > max_chars && chars > 0 {
            output.pop();
            chars -= 1;
        }
        while output.ends_with(' ') && chars > 0 {
            output.pop();
            chars -= 1;
        }
        output.push_str(COMPLETION_DISPLAY_TRUNCATED_NOTICE);
    }

    Some(Cow::Owned(output))
}

fn completion_display_text_sanitize_input(text: &str, max_chars: usize) -> &str {
    let sample_bytes = completion_display_text_sample_bytes(max_chars);
    if text.len() <= sample_bytes {
        return text.trim();
    }

    let mut cut = sample_bytes;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    text[..cut].trim()
}

fn completion_display_text_sample_bytes(max_chars: usize) -> usize {
    COMPLETION_DISPLAY_SANITIZE_SAMPLE_BYTES.max(
        max_chars
            .saturating_mul(4)
            .saturating_add(COMPLETION_DISPLAY_TRUNCATED_NOTICE.len()),
    )
}

fn completion_display_text_can_borrow(text: &str, max_chars: usize) -> bool {
    let mut previous_was_space = false;

    for (chars, ch) in text.chars().enumerate() {
        if chars >= max_chars
            || ch.is_control()
            || is_completion_display_format_control(ch)
            || (ch.is_whitespace() && (ch != ' ' || previous_was_space))
        {
            return false;
        }

        previous_was_space = ch == ' ';
    }

    true
}

fn hash_completion_item_row_detail<H: Hasher>(detail: Option<&str>, state: &mut H) {
    if let Some(plan) = detail.and_then(|detail| {
        completion_display_text_hash_plan(detail, COMPLETION_DETAIL_DISPLAY_MAX_CHARS)
    }) {
        true.hash(state);
        plan.hash(state);
        return;
    }

    false.hash(state);
}

fn hash_completion_display_label<H: Hasher>(label: &str, state: &mut H) {
    match completion_display_text_hash_plan(label, COMPLETION_LABEL_DISPLAY_MAX_CHARS) {
        Some(plan) => plan.hash(state),
        None => hash_completion_display_str("completion", state),
    }
}

#[derive(Clone, Copy)]
enum CompletionDisplayTextHashPlan<'a> {
    Borrowed(&'a str),
    Sanitized {
        text: &'a str,
        prefix_chars: usize,
        append_truncated_notice: bool,
    },
}

impl CompletionDisplayTextHashPlan<'_> {
    fn hash<H: Hasher>(self, state: &mut H) {
        match self {
            Self::Borrowed(text) => hash_completion_display_str(text, state),
            Self::Sanitized {
                text,
                prefix_chars,
                append_truncated_notice,
            } => {
                let mut chars = 0usize;
                visit_sanitized_completion_display_prefix(
                    text,
                    prefix_chars,
                    append_truncated_notice,
                    |ch| {
                        ch.hash(state);
                        chars += 1;
                    },
                );
                if append_truncated_notice {
                    for ch in COMPLETION_DISPLAY_TRUNCATED_NOTICE.chars() {
                        ch.hash(state);
                        chars += 1;
                    }
                }
                chars.hash(state);
            }
        }
    }
}

fn completion_display_text_hash_plan(
    text: &str,
    max_chars: usize,
) -> Option<CompletionDisplayTextHashPlan<'_>> {
    if max_chars == 0 {
        return None;
    }

    let text = completion_display_text_sanitize_input(text, max_chars);
    if text.is_empty() {
        return None;
    }
    if completion_display_text_can_borrow(text, max_chars) {
        return Some(CompletionDisplayTextHashPlan::Borrowed(text));
    }

    let (chars, truncated) = sanitized_completion_display_summary(text, max_chars)?;
    let append_truncated_notice =
        truncated && max_chars >= COMPLETION_DISPLAY_TRUNCATED_NOTICE.len();
    let prefix_chars = if append_truncated_notice {
        chars.min(max_chars - COMPLETION_DISPLAY_TRUNCATED_NOTICE.len())
    } else {
        chars
    };

    Some(CompletionDisplayTextHashPlan::Sanitized {
        text,
        prefix_chars,
        append_truncated_notice,
    })
}

fn sanitized_completion_display_summary(text: &str, max_chars: usize) -> Option<(usize, bool)> {
    let mut chars = 0usize;
    let mut pending_space = false;
    let mut truncated = false;

    for ch in text.chars() {
        if ch.is_control() || ch.is_whitespace() {
            pending_space = chars > 0;
            continue;
        }
        if is_completion_display_format_control(ch) {
            continue;
        }
        if pending_space {
            if chars >= max_chars {
                truncated = true;
                break;
            }
            chars += 1;
            pending_space = false;
        }
        if chars >= max_chars {
            truncated = true;
            break;
        }
        chars += 1;
    }

    (chars > 0).then_some((chars, truncated))
}

fn visit_sanitized_completion_display_prefix(
    text: &str,
    max_output_chars: usize,
    trim_trailing_space: bool,
    mut visitor: impl FnMut(char),
) {
    let mut chars = 0usize;
    let mut pending_space = false;
    let mut pending_emit_space = false;

    for ch in text.chars() {
        if chars >= max_output_chars {
            break;
        }
        if ch.is_control() || ch.is_whitespace() {
            pending_space = chars > 0;
            continue;
        }
        if is_completion_display_format_control(ch) {
            continue;
        }
        if pending_space {
            if chars >= max_output_chars {
                break;
            }
            pending_emit_space = true;
            chars += 1;
            pending_space = false;
            if chars >= max_output_chars {
                break;
            }
        }
        if pending_emit_space {
            visitor(' ');
            pending_emit_space = false;
        }
        visitor(ch);
        chars += 1;
    }

    if pending_emit_space && !trim_trailing_space {
        visitor(' ');
    }
}

fn hash_completion_display_str<H: Hasher>(text: &str, state: &mut H) {
    let mut chars = 0usize;
    for ch in text.chars() {
        ch.hash(state);
        chars += 1;
    }
    chars.hash(state);
}

fn is_completion_display_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
    )
}

pub(crate) fn completion_item_documentation(item: &LspCompletionItem) -> Option<&str> {
    item.documentation
        .as_deref()
        .map(str::trim)
        .filter(|documentation| !documentation.is_empty())
}

#[cfg(test)]
pub(crate) fn completion_item_documentation_blocks(
    item: &LspCompletionItem,
) -> Vec<HoverMarkdownBlock> {
    bounded_completion_item_documentation(item)
        .map(|documentation| parse_hover_markdown(documentation.as_ref()))
        .unwrap_or_default()
}

fn bounded_completion_item_documentation(item: &LspCompletionItem) -> Option<Cow<'_, str>> {
    let documentation = completion_item_documentation(item)?;
    if documentation.len() <= MAX_COMPLETION_DOCUMENTATION_CHARS {
        return Some(Cow::Borrowed(documentation));
    }

    let Some((cut, _)) = documentation
        .char_indices()
        .nth(MAX_COMPLETION_DOCUMENTATION_CHARS)
    else {
        return Some(Cow::Borrowed(documentation));
    };

    let mut bounded = String::with_capacity(cut + COMPLETION_DOCUMENTATION_TRUNCATED_NOTICE.len());
    bounded.push_str(documentation[..cut].trim_end());
    close_open_completion_documentation_fence(&mut bounded);
    bounded.push_str(COMPLETION_DOCUMENTATION_TRUNCATED_NOTICE);
    Some(Cow::Owned(bounded))
}

fn close_open_completion_documentation_fence(documentation: &mut String) {
    let Some(fence) = open_completion_documentation_fence(documentation) else {
        return;
    };
    if !documentation.ends_with('\n') {
        documentation.push('\n');
    }
    for _ in 0..fence.marker_len {
        documentation.push(fence.marker());
    }
}

fn open_completion_documentation_fence(
    documentation: &str,
) -> Option<CompletionDocumentationFence> {
    let mut open: Option<CompletionDocumentationFence> = None;
    for line in documentation.lines() {
        let Some(fence) = completion_documentation_fence(line) else {
            continue;
        };
        match open {
            Some(current)
                if current.kind == fence.kind && fence.marker_len >= current.marker_len =>
            {
                open = None;
            }
            None => open = Some(fence),
            _ => {}
        }
    }
    open
}

fn completion_documentation_fence(line: &str) -> Option<CompletionDocumentationFence> {
    let trimmed = line.trim_start();
    let first = *trimmed.as_bytes().first()?;
    let kind = match first {
        b'`' => CompletionDocumentationFenceKind::Backtick,
        b'~' => CompletionDocumentationFenceKind::Tilde,
        _ => return None,
    };
    let marker_len = trimmed
        .as_bytes()
        .iter()
        .take_while(|byte| **byte == first)
        .count();
    (marker_len >= 3).then_some(CompletionDocumentationFence { kind, marker_len })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CompletionDocumentationFence {
    kind: CompletionDocumentationFenceKind,
    marker_len: usize,
}

impl CompletionDocumentationFence {
    fn marker(self) -> char {
        match self.kind {
            CompletionDocumentationFenceKind::Backtick => '`',
            CompletionDocumentationFenceKind::Tilde => '~',
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompletionDocumentationFenceKind {
    Backtick,
    Tilde,
}

#[cfg(test)]
mod tests {
    use super::{
        COMPLETION_DETAIL_DISPLAY_MAX_CHARS, COMPLETION_DISPLAY_SANITIZE_SAMPLE_BYTES,
        COMPLETION_LABEL_DISPLAY_MAX_CHARS, MAX_COMPLETION_DOCUMENTATION_CHARS,
        bounded_completion_item_documentation, completion_display_label, completion_display_text,
        completion_item_documentation, completion_item_documentation_blocks, completion_item_label,
        completion_item_rich_label_cache_hash, completion_item_row_height,
    };
    use crate::lsp_hover_markdown::HoverMarkdownBlock;
    use kuroya_core::{LspCompletionItem, LspTextEdit};
    use std::{borrow::Cow, path::PathBuf};

    fn item(documentation: Option<&str>) -> LspCompletionItem {
        LspCompletionItem {
            label: "println!".to_owned(),
            detail: Some("macro".to_owned()),
            documentation: documentation.map(str::to_owned),
            kind: Some(3),
            deprecated: false,
            is_snippet: false,
            sort_text: None,
            filter_text: None,
            preselect: false,
            commit_characters: Vec::new(),
            insert_text: "println!".to_owned(),
            snippet_selection: None,
            snippet_tabstops: Vec::new(),
            snippet_tabstop_groups: Vec::new(),
            text_edit: None,
            insert_text_edit: None,
            additional_text_edits: Vec::new(),
            resolve_payload: None,
        }
    }

    fn item_with_auto_import() -> LspCompletionItem {
        LspCompletionItem {
            additional_text_edits: vec![LspTextEdit {
                path: PathBuf::from("src/main.rs"),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                new_text: "use std::fmt::Debug;\n".to_owned(),
            }],
            ..item(None)
        }
    }

    #[test]
    fn completion_item_label_follows_icon_and_detail_settings() {
        let item = item(None);

        assert_eq!(
            completion_item_label(&item, true, true),
            "fn  println!  macro"
        );
        assert_eq!(completion_item_label(&item, false, true), "println!  macro");
        assert_eq!(completion_item_label(&item, true, false), "fn  println!");
    }

    #[test]
    fn completion_labels_include_kind_label_and_detail() {
        assert_eq!(
            completion_item_label(&item(Some("Prints a line.")), true, true),
            "fn  println!  macro"
        );
    }

    #[test]
    fn completion_labels_surface_auto_import_side_edits() {
        let item = item_with_auto_import();

        assert_eq!(
            completion_item_label(&item, true, true),
            "fn  println!  auto-import  macro"
        );
        assert_eq!(
            completion_item_label(&item, false, false),
            "println!  auto-import"
        );
    }

    #[test]
    fn completion_item_rich_label_cache_key_uses_prepared_auto_import_label() {
        let first = item_with_auto_import();
        let mut second = item_with_auto_import();
        second.additional_text_edits[0].new_text = "import { Debug } from 'std';\n".to_owned();

        assert_eq!(
            completion_item_label(&first, true, true),
            completion_item_label(&second, true, true)
        );
        assert_eq!(
            completion_item_rich_label_cache_hash(&first, true, true, 13),
            completion_item_rich_label_cache_hash(&second, true, true, 13)
        );
        assert_ne!(
            completion_item_rich_label_cache_hash(&item(None), true, true, 13),
            completion_item_rich_label_cache_hash(&first, true, true, 13)
        );
    }

    #[test]
    fn completion_item_rich_label_cache_key_tracks_visible_detail() {
        let mut item = item(None);

        let hidden_detail_hash = completion_item_rich_label_cache_hash(&item, true, false, 13);
        item.detail = Some("updated detail".to_owned());

        assert_eq!(
            hidden_detail_hash,
            completion_item_rich_label_cache_hash(&item, true, false, 13)
        );

        let visible_detail_hash = completion_item_rich_label_cache_hash(&item, true, true, 13);
        item.detail = Some("another detail".to_owned());

        assert_ne!(
            visible_detail_hash,
            completion_item_rich_label_cache_hash(&item, true, true, 13)
        );
    }

    #[test]
    fn completion_item_rich_label_cache_key_tracks_deprecated_style() {
        let mut deprecated = item(None);
        deprecated.deprecated = true;

        assert_ne!(
            completion_item_rich_label_cache_hash(&item(None), true, true, 13),
            completion_item_rich_label_cache_hash(&deprecated, true, true, 13)
        );
    }

    #[test]
    fn completion_item_rich_label_cache_key_uses_prepared_kind_label() {
        let mut no_kind = item(None);
        no_kind.kind = None;
        let mut unknown_kind = item(None);
        unknown_kind.kind = Some(99);

        assert_eq!(
            completion_item_label(&no_kind, true, false),
            completion_item_label(&unknown_kind, true, false)
        );
        assert_eq!(
            completion_item_rich_label_cache_hash(&no_kind, true, false, 13),
            completion_item_rich_label_cache_hash(&unknown_kind, true, false, 13)
        );
    }

    #[test]
    fn completion_item_rich_label_cache_key_hashes_sanitized_display_text() {
        let mut dirty = item(None);
        dirty.label = "print\tln\u{202e}".to_owned();
        dirty.detail = Some("macro\nfrom\u{200f} std".to_owned());
        let mut clean = item(None);
        clean.label = "print ln".to_owned();
        clean.detail = Some("macro from std".to_owned());

        assert_eq!(
            completion_item_label(&dirty, false, true),
            completion_item_label(&clean, false, true)
        );
        assert_eq!(
            completion_item_rich_label_cache_hash(&dirty, false, true, 13),
            completion_item_rich_label_cache_hash(&clean, false, true, 13)
        );
    }

    #[test]
    fn completion_item_rich_label_cache_key_uses_prepared_display_text() {
        let mut first = item(None);
        first.label = "\u{202e}\u{feff}".to_owned();
        first.detail = Some("\u{200b}".to_owned());
        let mut second = item(None);
        second.label = "\u{200b}\u{2066}".to_owned();
        second.detail = Some("\u{202e}".to_owned());

        assert_eq!(
            completion_item_label(&first, true, true),
            completion_item_label(&second, true, true)
        );
        assert_eq!(
            completion_item_rich_label_cache_hash(&first, true, true, 13),
            completion_item_rich_label_cache_hash(&second, true, true, 13)
        );
    }

    #[test]
    fn completion_item_display_preparation_preserves_raw_item() {
        let mut completion = item(None);
        completion.label = "  print\u{200d}ln\u{0007}\n\u{202e}macro  ".to_owned();
        completion.detail = Some("  fn\titem\u{200f}\nfrom\u{feff} std  ".to_owned());
        completion.insert_text = "raw\ninsert\u{202e} text".to_owned();
        let raw_completion = completion.clone();

        assert_eq!(
            completion_item_label(&completion, false, true),
            "println macro  fn item from std"
        );
        assert_eq!(completion, raw_completion);
    }

    #[test]
    fn completion_item_label_sanitizes_display_label_and_detail() {
        let mut item = item(None);
        item.label = "  print\u{200d}ln\u{0007}\n\u{202e}macro  ".to_owned();
        item.detail = Some("  fn\titem\u{200f}\nfrom\u{feff} std  ".to_owned());

        assert_eq!(
            completion_item_label(&item, false, true),
            "println macro  fn item from std"
        );
        assert_eq!(
            completion_item_label(&item, true, false),
            "fn  println macro"
        );
    }

    #[test]
    fn completion_display_text_is_bounded_and_falls_back_for_blank_labels() {
        let long_label = format!(
            "{}tail",
            "x".repeat(COMPLETION_LABEL_DISPLAY_MAX_CHARS + 16)
        );
        let label = completion_display_label(&long_label);

        assert_eq!(label.chars().count(), COMPLETION_LABEL_DISPLAY_MAX_CHARS);
        assert!(label.ends_with("..."));
        assert!(!label.contains("tail"));
        assert_eq!(completion_display_label("\n\u{202e}\t"), "completion");
    }

    #[test]
    fn completion_display_label_borrows_clean_label() {
        let label = completion_display_label("println!");

        assert_eq!(label, "println!");
        assert!(matches!(label, Cow::Borrowed("println!")));
    }

    #[test]
    fn completion_display_text_bounds_huge_unsafe_label_and_detail_samples() {
        let hidden_controls = "\u{202e}".repeat(COMPLETION_DISPLAY_SANITIZE_SAMPLE_BYTES + 32);
        let mut completion = item(None);
        completion.label = format!("{hidden_controls}visible-label");
        completion.detail = Some(format!("{hidden_controls}visible-detail"));

        assert_eq!(
            completion_item_label(&completion, false, true),
            "completion"
        );
    }

    #[test]
    fn completion_detail_display_text_is_single_line_and_bounded() {
        let long_detail = format!(
            " detail\n{}\u{2066}END",
            "d".repeat(COMPLETION_DETAIL_DISPLAY_MAX_CHARS + 8)
        );
        let detail =
            completion_display_text(&long_detail, COMPLETION_DETAIL_DISPLAY_MAX_CHARS).unwrap();

        assert_eq!(detail.chars().count(), COMPLETION_DETAIL_DISPLAY_MAX_CHARS);
        assert!(detail.starts_with("detail d"));
        assert!(detail.ends_with("..."));
        assert!(!detail.contains('\n'));
        assert!(!detail.contains('\u{2066}'));
        assert!(!detail.contains("END"));
    }

    #[test]
    fn completion_display_text_drops_hidden_suffix_without_truncation_notice() {
        let text = format!(
            "{}\u{200b}\u{202e}\u{feff}",
            "x".repeat(COMPLETION_LABEL_DISPLAY_MAX_CHARS)
        );
        let display = completion_display_text(&text, COMPLETION_LABEL_DISPLAY_MAX_CHARS)
            .expect("display text");

        assert_eq!(display.chars().count(), COMPLETION_LABEL_DISPLAY_MAX_CHARS);
        assert!(!display.ends_with("..."));
        assert!(!display.contains('\u{200b}'));
        assert!(!display.contains('\u{202e}'));
        assert!(!display.contains('\u{feff}'));
    }

    #[test]
    fn completion_display_text_drops_huge_hidden_suffix_without_truncation_notice() {
        let text = format!(
            "{}{}",
            "x".repeat(COMPLETION_LABEL_DISPLAY_MAX_CHARS),
            "\u{200b}".repeat(COMPLETION_DISPLAY_SANITIZE_SAMPLE_BYTES + 32)
        );
        let display = completion_display_text(&text, COMPLETION_LABEL_DISPLAY_MAX_CHARS)
            .expect("display text");

        assert_eq!(display, "x".repeat(COMPLETION_LABEL_DISPLAY_MAX_CHARS));
        assert!(!display.ends_with("..."));
        assert!(!display.contains('\u{200b}'));
    }

    #[test]
    fn completion_display_text_truncates_only_when_visible_text_remains() {
        let text = format!(
            "{}\u{200b}\u{202e}tail",
            "x".repeat(COMPLETION_LABEL_DISPLAY_MAX_CHARS)
        );
        let display = completion_display_text(&text, COMPLETION_LABEL_DISPLAY_MAX_CHARS)
            .expect("display text");

        assert_eq!(display.chars().count(), COMPLETION_LABEL_DISPLAY_MAX_CHARS);
        assert!(display.ends_with("..."));
        assert!(!display.contains("tail"));
        assert!(!display.contains('\u{200b}'));
        assert!(!display.contains('\u{202e}'));
    }

    #[test]
    fn completion_documentation_trims_empty_docs() {
        assert_eq!(
            completion_item_documentation(&item(Some("  Prints a line.  "))),
            Some("Prints a line.")
        );
        assert_eq!(completion_item_documentation(&item(Some("   "))), None);
        assert_eq!(completion_item_documentation(&item(None)), None);
    }

    #[test]
    fn completion_documentation_blocks_parse_markdown_code_fences() {
        assert_eq!(
            completion_item_documentation_blocks(&item(Some(
                "Prints a line.\n\n```rust\nprintln!(\"hi\");\n```",
            ))),
            vec![
                HoverMarkdownBlock::Paragraph("Prints a line.".to_owned()),
                HoverMarkdownBlock::Code {
                    language: Some("rust".to_owned()),
                    text: "println!(\"hi\");".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn completion_documentation_blocks_parse_tilde_code_fences() {
        assert_eq!(
            completion_item_documentation_blocks(&item(Some(
                "Prints a line.\n\n~~~rust\nprintln!(\"hi\");\n~~~",
            ))),
            vec![
                HoverMarkdownBlock::Paragraph("Prints a line.".to_owned()),
                HoverMarkdownBlock::Code {
                    language: Some("rust".to_owned()),
                    text: "println!(\"hi\");".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn completion_documentation_blocks_preserve_markdown_lists() {
        assert_eq!(
            completion_item_documentation_blocks(&item(Some("Options:\n\n- stdout\n- stderr",))),
            vec![
                HoverMarkdownBlock::Paragraph("Options:".to_owned()),
                HoverMarkdownBlock::List(vec!["stdout".to_owned(), "stderr".to_owned()]),
            ]
        );
    }

    #[test]
    fn completion_documentation_blocks_preserve_block_quotes() {
        assert_eq!(
            completion_item_documentation_blocks(&item(Some(
                "Prints a line.\n\n> Panics if stdout is closed.",
            ))),
            vec![
                HoverMarkdownBlock::Paragraph("Prints a line.".to_owned()),
                HoverMarkdownBlock::Quote(vec!["Panics if stdout is closed.".to_owned()]),
            ]
        );
    }

    #[test]
    fn completion_documentation_blocks_preserve_markdown_headings() {
        assert_eq!(
            completion_item_documentation_blocks(&item(Some(
                "### Safety ###\n\nCall only after init.",
            ))),
            vec![
                HoverMarkdownBlock::Heading {
                    level: 3,
                    text: "Safety".to_owned(),
                },
                HoverMarkdownBlock::Paragraph("Call only after init.".to_owned()),
            ]
        );
    }

    #[test]
    fn completion_documentation_rendering_is_bounded() {
        let long_documentation = format!(
            "{}tail",
            "a".repeat(MAX_COMPLETION_DOCUMENTATION_CHARS + 32)
        );
        let completion = item(Some(&long_documentation));
        let bounded = bounded_completion_item_documentation(&completion).expect("bounded docs");

        assert!(bounded.ends_with("[Documentation truncated]"));
        assert!(!bounded.contains("tail"));

        let blocks = completion_item_documentation_blocks(&completion);
        assert!(matches!(
            blocks.last(),
            Some(HoverMarkdownBlock::Paragraph(text))
                if text.ends_with("[Documentation truncated]") && !text.contains("tail")
        ));
    }

    #[test]
    fn completion_documentation_truncation_closes_open_code_fence_before_notice() {
        let long_documentation = format!(
            "```rust\n{}\ntail",
            "let value = 1;\n".repeat(MAX_COMPLETION_DOCUMENTATION_CHARS / 4)
        );
        let completion = item(Some(&long_documentation));
        let bounded = bounded_completion_item_documentation(&completion).expect("bounded docs");

        assert!(bounded.contains("\n```\n\n[Documentation truncated]"));
        assert!(!bounded.contains("tail"));

        let blocks = completion_item_documentation_blocks(&completion);
        assert!(matches!(
            blocks.last(),
            Some(HoverMarkdownBlock::Paragraph(text)) if text == "[Documentation truncated]"
        ));
        assert!(matches!(
            blocks.first(),
            Some(HoverMarkdownBlock::Code { language, text })
                if language.as_deref() == Some("rust")
                    && text.contains("let value = 1;")
                    && !text.contains("[Documentation truncated]")
        ));
    }

    #[test]
    fn completion_documentation_truncation_closes_matching_tilde_fence_length() {
        let long_documentation = format!(
            "~~~~text\n{}\ntail",
            "plain text\n".repeat(MAX_COMPLETION_DOCUMENTATION_CHARS / 3)
        );
        let completion = item(Some(&long_documentation));
        let bounded = bounded_completion_item_documentation(&completion).expect("bounded docs");

        assert!(bounded.contains("\n~~~~\n\n[Documentation truncated]"));
        assert!(!bounded.contains("tail"));
    }

    #[test]
    fn completion_documentation_bound_preserves_utf8_boundaries() {
        let long_documentation = format!(
            "{}tail",
            "\u{03b1}".repeat(MAX_COMPLETION_DOCUMENTATION_CHARS + 4)
        );
        let completion = item(Some(&long_documentation));
        let bounded = bounded_completion_item_documentation(&completion).expect("bounded docs");

        assert!(bounded.starts_with('\u{03b1}'));
        assert!(bounded.ends_with("[Documentation truncated]"));
        assert!(!bounded.contains("tail"));
    }

    #[test]
    fn completion_item_row_height_uses_setting_or_fallback() {
        assert_eq!(completion_item_row_height(28, 18.0), 28.0);
        assert_eq!(completion_item_row_height(0, 18.0), 18.0);
    }
}
