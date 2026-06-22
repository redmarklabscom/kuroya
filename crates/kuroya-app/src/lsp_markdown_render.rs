use crate::lsp_hover_markdown::{HoverMarkdownBlock, parse_hover_markdown};
use eframe::egui::{self, Color32, FontFamily, FontId, RichText, TextFormat, Ui, text::LayoutJob};
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LspMarkdownTextSize {
    Normal,
    Small,
}

const MAX_LSP_MARKDOWN_INLINE_CHARS: usize = 2_000;
const MAX_LSP_MARKDOWN_CODE_CHARS: usize = 4_000;
const MAX_LSP_MARKDOWN_INLINE_LAYOUT_SECTIONS: usize = 256;
const MAX_LSP_MARKDOWN_INLINE_LINK_LABEL_BYTES: usize = 1_024;
const MAX_LSP_MARKDOWN_INLINE_LINK_DESTINATION_BYTES: usize = 1_024;
const MAX_LSP_MARKDOWN_INLINE_LINK_TITLE_BYTES: usize = 512;
const MAX_LSP_MARKDOWN_INLINE_LINK_NESTING: usize = 16;
const LSP_MARKDOWN_TEXT_TRUNCATED_NOTICE: &str = " [truncated]";
const LSP_MARKDOWN_CODE_TRUNCATED_NOTICE: &str = "\n[Code block truncated]";
const LSP_MARKDOWN_TRUNCATION_NOTICES: &[&str] = &[
    "[Hover truncated]",
    "[Documentation truncated]",
    "[Signature documentation truncated]",
];

pub(crate) fn render_lsp_markdown(ui: &mut Ui, contents: &str, size: LspMarkdownTextSize) {
    let blocks = parse_hover_markdown(contents);
    render_lsp_markdown_blocks(ui, &blocks, size);
}

pub(crate) fn render_lsp_inline_markdown(
    ui: &mut Ui,
    contents: &str,
    size: LspMarkdownTextSize,
    color: Color32,
) {
    render_inline_markdown_label(ui, contents, size, color);
}

pub(crate) fn render_lsp_markdown_blocks(
    ui: &mut Ui,
    blocks: &[HoverMarkdownBlock],
    size: LspMarkdownTextSize,
) {
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            ui.add_space(8.0);
        }

        match block {
            HoverMarkdownBlock::Heading { level, text } => {
                let text = bounded_inline_markdown_text(text);
                ui.label(
                    heading_text(text.as_ref(), *level, size).color(ui.visuals().text_color()),
                );
            }
            HoverMarkdownBlock::Paragraph(text) => {
                render_inline_markdown_label(ui, text, size, ui.visuals().text_color());
            }
            HoverMarkdownBlock::List(items) => {
                for item in items {
                    render_inline_markdown_label_with_prefix(
                        ui,
                        "- ",
                        item,
                        size,
                        ui.visuals().text_color(),
                    );
                }
            }
            HoverMarkdownBlock::Quote(lines) => {
                let weak_text_color = ui.visuals().weak_text_color();
                egui::Frame::new()
                    .fill(faint_markdown_fill(weak_text_color, 24))
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        for line in lines {
                            render_inline_markdown_label(ui, line, size, weak_text_color);
                        }
                    });
            }
            HoverMarkdownBlock::Code { language, text } => {
                let (text, trailing_notice) = split_trailing_markdown_truncation_notice(text);
                let text = bounded_code_block_text(text);
                if !text.is_empty() {
                    egui::Frame::new()
                        .fill(ui.visuals().code_bg_color)
                        .inner_margin(egui::Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            if let Some(language) = language {
                                ui.label(
                                    RichText::new(language)
                                        .small()
                                        .color(ui.visuals().weak_text_color()),
                                );
                                ui.add_space(4.0);
                            }
                            ui.label(sized_text(RichText::new(text.as_ref()).monospace(), size));
                        });
                }
                if let Some(notice) = trailing_notice {
                    if !text.is_empty() {
                        ui.add_space(8.0);
                    }
                    render_inline_markdown_label(ui, notice, size, ui.visuals().weak_text_color());
                }
            }
        }
    }
}

fn faint_markdown_fill(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

fn sized_text(text: RichText, size: LspMarkdownTextSize) -> RichText {
    match size {
        LspMarkdownTextSize::Normal => text,
        LspMarkdownTextSize::Small => text.small(),
    }
}

fn heading_text(text: &str, level: usize, size: LspMarkdownTextSize) -> RichText {
    let base_size: f32 = match size {
        LspMarkdownTextSize::Normal => 16.0,
        LspMarkdownTextSize::Small => 13.0,
    };
    let level_adjustment: f32 = match level {
        0 | 1 => 2.0,
        2 => 1.0,
        3 => 0.0,
        _ => -1.0,
    };
    RichText::new(text)
        .strong()
        .size((base_size + level_adjustment).max(11.0_f32))
}

fn render_inline_markdown_label(
    ui: &mut Ui,
    text: &str,
    size: LspMarkdownTextSize,
    color: Color32,
) {
    let prepared = prepare_inline_markdown_text(text);
    if !prepared.needs_layout_job {
        ui.label(sized_text(
            RichText::new(prepared.text.as_ref()).color(color),
            size,
        ));
        return;
    }

    ui.label(markdown_inline_job(prepared.text.as_ref(), size, color));
}

fn render_inline_markdown_label_with_prefix(
    ui: &mut Ui,
    prefix: &str,
    text: &str,
    size: LspMarkdownTextSize,
    color: Color32,
) {
    let prepared = prepare_inline_markdown_text(text);
    if !prepared.needs_layout_job {
        ui.label(plain_markdown_job_with_prefix(
            prefix,
            prepared.text.as_ref(),
            size,
            color,
        ));
        return;
    }

    ui.label(markdown_inline_job_with_prefix(
        prefix,
        prepared.text.as_ref(),
        size,
        color,
    ));
}

fn markdown_inline_job(text: &str, size: LspMarkdownTextSize, color: Color32) -> LayoutJob {
    markdown_inline_job_with_prefix("", text, size, color)
}

fn markdown_inline_job_with_prefix(
    prefix: &str,
    text: &str,
    size: LspMarkdownTextSize,
    color: Color32,
) -> LayoutJob {
    let mut job =
        MarkdownInlineJobBuilder::new(prefix.len().saturating_add(text.len()), size, color);

    if !prefix.is_empty() {
        job.append_text(prefix);
    }
    visit_markdown_inline_segments(text, &mut |segment| match segment {
        MarkdownInlineSegment::Text(text) => job.append_text(text),
        MarkdownInlineSegment::Code(code) => job.append_code(code),
    });
    job.finish()
}

struct MarkdownInlineJobBuilder {
    job: LayoutJob,
    text_format: TextFormat,
    code_format: TextFormat,
}

impl MarkdownInlineJobBuilder {
    fn new(text_capacity: usize, size: LspMarkdownTextSize, color: Color32) -> Self {
        let font_size = markdown_font_size(size);
        let mut job = LayoutJob::default();
        job.text.reserve(text_capacity);

        Self {
            job,
            text_format: markdown_text_format_with_size(font_size, color, FontFamily::Proportional),
            code_format: TextFormat {
                background: faint_markdown_fill(color, 36),
                ..markdown_text_format_with_size(font_size, color, FontFamily::Monospace)
            },
        }
    }

    fn append_text(&mut self, text: &str) {
        let Some((mut escape_start, mut escaped_start, mut escaped_end)) =
            next_markdown_text_escape(text, 0)
        else {
            self.append_formatted(text, self.text_format.clone());
            return;
        };

        let mut cursor = 0;
        loop {
            if cursor < escape_start {
                self.append_formatted(&text[cursor..escape_start], self.text_format.clone());
            }
            self.append_formatted(&text[escaped_start..escaped_end], self.text_format.clone());
            cursor = escaped_end;

            let Some((next_escape_start, next_escaped_start, next_escaped_end)) =
                next_markdown_text_escape(text, cursor)
            else {
                break;
            };
            escape_start = next_escape_start;
            escaped_start = next_escaped_start;
            escaped_end = next_escaped_end;
        }

        if cursor < text.len() {
            self.append_formatted(&text[cursor..], self.text_format.clone());
        }
    }

    fn append_code(&mut self, code: &str) {
        self.append_formatted(code, self.code_format.clone());
    }

    fn append_formatted(&mut self, text: &str, format: TextFormat) {
        if text.is_empty() {
            return;
        }

        if self.job.sections.len() < MAX_LSP_MARKDOWN_INLINE_LAYOUT_SECTIONS {
            self.job.append(text, 0.0, format);
            return;
        }

        self.job.text.push_str(text);
        if let Some(section) = self.job.sections.last_mut() {
            section.byte_range.end = self.job.text.len();
        }
    }

    fn finish(self) -> LayoutJob {
        self.job
    }
}

fn next_markdown_text_escape(text: &str, start: usize) -> Option<(usize, usize, usize)> {
    let mut cursor = start;
    while cursor < text.len() {
        if text.as_bytes()[cursor] == b'\\' {
            let escaped_start = cursor + 1;
            if let Some(escaped) = markdown_escaped_punctuation_at(text, escaped_start) {
                return Some((cursor, escaped_start, escaped_start + escaped.len_utf8()));
            }
        }
        cursor += char_len_at(text, cursor);
    }

    None
}

fn plain_markdown_job_with_prefix(
    prefix: &str,
    text: &str,
    size: LspMarkdownTextSize,
    color: Color32,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.text.reserve(prefix.len().saturating_add(text.len()));

    let format = markdown_text_format(size, color, FontFamily::Proportional);
    job.append(prefix, 0.0, format.clone());
    job.append(text, 0.0, format);
    job
}

fn markdown_text_format(
    size: LspMarkdownTextSize,
    color: Color32,
    family: FontFamily,
) -> TextFormat {
    markdown_text_format_with_size(markdown_font_size(size), color, family)
}

fn markdown_text_format_with_size(
    font_size: f32,
    color: Color32,
    family: FontFamily,
) -> TextFormat {
    TextFormat {
        font_id: FontId::new(font_size, family),
        color,
        ..Default::default()
    }
}

fn markdown_font_size(size: LspMarkdownTextSize) -> f32 {
    match size {
        LspMarkdownTextSize::Normal => 14.0,
        LspMarkdownTextSize::Small => 12.0,
    }
}

#[cfg(test)]
fn has_inline_markdown_controls(text: &str) -> bool {
    let mut cursor = 0;
    while cursor < text.len() {
        match text.as_bytes()[cursor] {
            b'`' | b'[' => return true,
            b'\\' if markdown_escaped_punctuation_at(text, cursor + 1).is_some() => return true,
            _ => cursor += char_len_at(text, cursor),
        }
    }

    false
}

fn bounded_inline_markdown_text(text: &str) -> Cow<'_, str> {
    prepare_inline_markdown_text(text).text
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedInlineMarkdown<'a> {
    text: Cow<'a, str>,
    needs_layout_job: bool,
}

fn prepare_inline_markdown_text(text: &str) -> PreparedInlineMarkdown<'_> {
    let prepared = prepare_markdown_display_text(
        text,
        MAX_LSP_MARKDOWN_INLINE_CHARS,
        LSP_MARKDOWN_TEXT_TRUNCATED_NOTICE,
        MarkdownDisplayTextKind::Inline,
        true,
    );

    PreparedInlineMarkdown {
        text: prepared.text,
        needs_layout_job: prepared.needs_inline_markdown_layout,
    }
}

fn bounded_code_block_text(text: &str) -> Cow<'_, str> {
    bounded_markdown_display_text(
        text,
        MAX_LSP_MARKDOWN_CODE_CHARS,
        LSP_MARKDOWN_CODE_TRUNCATED_NOTICE,
        MarkdownDisplayTextKind::Code,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedMarkdownDisplayText<'a> {
    text: Cow<'a, str>,
    needs_inline_markdown_layout: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkdownDisplayTextKind {
    Inline,
    Code,
}

fn bounded_markdown_display_text<'a>(
    text: &'a str,
    max_chars: usize,
    notice: &str,
    kind: MarkdownDisplayTextKind,
) -> Cow<'a, str> {
    prepare_markdown_display_text(text, max_chars, notice, kind, false).text
}

fn prepare_markdown_display_text<'a>(
    text: &'a str,
    max_chars: usize,
    notice: &str,
    kind: MarkdownDisplayTextKind,
    detect_inline_markdown: bool,
) -> PreparedMarkdownDisplayText<'a> {
    if let Some(prepared) =
        prepare_ascii_markdown_display_text(text, max_chars, kind, detect_inline_markdown)
    {
        return prepared;
    }

    let mut bounded = None;
    let mut needs_inline_markdown_layout = false;
    let mut kept_chars = 0usize;
    for (index, ch) in text.char_indices() {
        if kept_chars == max_chars {
            let mut bounded = match bounded {
                Some(bounded) => bounded,
                None => {
                    let mut bounded = String::with_capacity(bounded_markdown_display_capacity(
                        index, max_chars, notice,
                    ));
                    bounded.push_str(&text[..index]);
                    bounded
                }
            };
            let trimmed_len = bounded.trim_end().len();
            bounded.truncate(trimmed_len);
            bounded.push_str(notice);
            return PreparedMarkdownDisplayText {
                text: Cow::Owned(bounded),
                needs_inline_markdown_layout,
            };
        }

        if markdown_display_char_allowed(ch, kind) {
            if detect_inline_markdown
                && inline_markdown_layout_needed_at(text, index, ch, kept_chars, max_chars)
            {
                needs_inline_markdown_layout = true;
            }
            if let Some(bounded) = &mut bounded {
                bounded.push(ch);
            }
            kept_chars += 1;
        } else {
            bounded.get_or_insert_with(|| {
                let mut bounded = String::with_capacity(bounded_markdown_display_capacity(
                    text.len(),
                    max_chars,
                    notice,
                ));
                bounded.push_str(&text[..index]);
                bounded
            });
        }
    }

    match bounded {
        Some(bounded) => PreparedMarkdownDisplayText {
            text: Cow::Owned(bounded),
            needs_inline_markdown_layout,
        },
        None => PreparedMarkdownDisplayText {
            text: Cow::Borrowed(text),
            needs_inline_markdown_layout,
        },
    }
}

fn prepare_ascii_markdown_display_text<'a>(
    text: &'a str,
    max_chars: usize,
    kind: MarkdownDisplayTextKind,
    detect_inline_markdown: bool,
) -> Option<PreparedMarkdownDisplayText<'a>> {
    if text.len() > max_chars {
        return None;
    }

    let mut needs_inline_markdown_layout = false;
    let bytes = text.as_bytes();
    for (index, byte) in bytes.iter().copied().enumerate() {
        if !byte.is_ascii() || !markdown_display_ascii_byte_allowed(byte, kind) {
            return None;
        }
        if detect_inline_markdown && inline_markdown_layout_needed_at_ascii(bytes, index, byte) {
            needs_inline_markdown_layout = true;
        }
    }

    Some(PreparedMarkdownDisplayText {
        text: Cow::Borrowed(text),
        needs_inline_markdown_layout,
    })
}

fn markdown_display_ascii_byte_allowed(byte: u8, kind: MarkdownDisplayTextKind) -> bool {
    !byte.is_ascii_control()
        || matches!(
            (kind, byte),
            (MarkdownDisplayTextKind::Code, b'\n' | b'\r' | b'\t')
        )
}

fn inline_markdown_layout_needed_at_ascii(bytes: &[u8], index: usize, byte: u8) -> bool {
    if matches!(byte, b'`' | b'[') {
        return true;
    }

    byte == b'\\'
        && bytes
            .get(index + 1)
            .copied()
            .map(|next| next.is_ascii_punctuation())
            .unwrap_or(false)
}

fn inline_markdown_layout_needed_at(
    text: &str,
    index: usize,
    ch: char,
    kept_chars: usize,
    max_chars: usize,
) -> bool {
    if matches!(ch, '`' | '[') {
        return true;
    }

    ch == '\\'
        && kept_chars + 1 < max_chars
        && markdown_escaped_punctuation_at(text, index + ch.len_utf8()).is_some()
}

fn markdown_escaped_punctuation_at(text: &str, index: usize) -> Option<char> {
    let ch = text.get(index..)?.chars().next()?;
    ch.is_ascii()
        .then_some(ch)
        .filter(|ch| ch.is_ascii_punctuation())
}

fn bounded_markdown_display_capacity(content_len: usize, max_chars: usize, notice: &str) -> usize {
    max_chars
        .saturating_mul(4)
        .saturating_add(notice.len())
        .min(content_len.saturating_add(notice.len()))
}

fn markdown_display_char_allowed(ch: char, kind: MarkdownDisplayTextKind) -> bool {
    (!ch.is_control()
        || matches!(
            (kind, ch),
            (MarkdownDisplayTextKind::Code, '\n' | '\r' | '\t')
        ))
        && !matches!(ch, '\u{2028}' | '\u{2029}')
        && !is_bidi_format_control(ch)
}

fn is_bidi_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
    )
}

fn split_trailing_markdown_truncation_notice(text: &str) -> (&str, Option<&'static str>) {
    let trimmed = text.trim_end();
    for notice in LSP_MARKDOWN_TRUNCATION_NOTICES {
        let Some(prefix) = trimmed.strip_suffix(notice) else {
            continue;
        };
        if prefix.ends_with('\n') || prefix.ends_with('\r') {
            return (prefix.trim_end(), Some(*notice));
        }
    }
    (text, None)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkdownInlineSegment<'a> {
    Text(&'a str),
    Code(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineLinkParse<'a> {
    Simple { label: &'a str, end: usize },
    Complex { end: usize },
    None,
}

#[cfg(test)]
fn markdown_inline_segments(text: &str) -> Vec<MarkdownInlineSegment<'_>> {
    let mut segments = Vec::new();
    visit_markdown_inline_segments(text, &mut |segment| segments.push(segment));
    segments
}

fn visit_markdown_inline_segments<'a>(
    text: &'a str,
    visitor: &mut impl FnMut(MarkdownInlineSegment<'a>),
) {
    let mut cursor = 0;
    let mut literal_start = 0;

    while let Some((open_start, control)) = next_markdown_control(text, cursor) {
        match control {
            b'`' => {
                if is_escaped(text, open_start) {
                    cursor = open_start + 1;
                    continue;
                }

                let tick_count = backtick_run_len(&text[open_start..]);
                let code_start = open_start + tick_count;
                let Some(relative_close) = matching_backtick_run(&text[code_start..], tick_count)
                else {
                    break;
                };
                let close_start = code_start + relative_close;

                visit_inline_text_segment(visitor, &text[literal_start..open_start]);
                visitor(MarkdownInlineSegment::Code(&text[code_start..close_start]));
                cursor = close_start + tick_count;
                literal_start = cursor;
            }
            b'[' => {
                if is_escaped(text, open_start) {
                    cursor = open_start + 1;
                    continue;
                }

                match parse_inline_link_at(text, open_start) {
                    InlineLinkParse::Simple { label, end } if !is_image_link(text, open_start) => {
                        visit_inline_text_segment(visitor, &text[literal_start..open_start]);
                        visit_markdown_inline_segments(label, visitor);
                        cursor = end;
                        literal_start = cursor;
                    }
                    InlineLinkParse::Simple { end, .. } | InlineLinkParse::Complex { end } => {
                        cursor = end;
                    }
                    InlineLinkParse::None => {
                        cursor = open_start + 1;
                    }
                }
            }
            _ => unreachable!("next_markdown_control only returns supported controls"),
        }
    }

    visit_inline_text_segment(visitor, &text[literal_start..]);
}

fn visit_inline_text_segment<'a>(
    visitor: &mut impl FnMut(MarkdownInlineSegment<'a>),
    text: &'a str,
) {
    if !text.is_empty() {
        visitor(MarkdownInlineSegment::Text(text));
    }
}

fn backtick_run_len(text: &str) -> usize {
    text.as_bytes()
        .iter()
        .take_while(|byte| **byte == b'`')
        .count()
}

fn matching_backtick_run(text: &str, tick_count: usize) -> Option<usize> {
    let mut cursor = 0;
    while let Some(relative_start) = text[cursor..].find('`') {
        let start = cursor + relative_start;
        let candidate_count = backtick_run_len(&text[start..]);
        if candidate_count == tick_count {
            return Some(start);
        }
        cursor = start + candidate_count;
    }
    None
}

fn next_markdown_control(text: &str, cursor: usize) -> Option<(usize, u8)> {
    for (relative, byte) in text.as_bytes().get(cursor..)?.iter().enumerate() {
        if matches!(byte, b'`' | b'[') {
            return Some((cursor + relative, *byte));
        }
    }

    None
}

fn parse_inline_link_at(text: &str, open_start: usize) -> InlineLinkParse<'_> {
    debug_assert_eq!(text.as_bytes().get(open_start), Some(&b'['));

    let Some((label_close, simple_label)) = matching_link_label_close(text, open_start) else {
        return InlineLinkParse::None;
    };
    if !text[label_close..].starts_with("](") {
        return InlineLinkParse::None;
    }

    let Some((end, simple_destination)) = inline_link_destination_end(text, label_close + 1) else {
        return InlineLinkParse::None;
    };

    let label = &text[open_start + 1..label_close];
    if label.is_empty() || !simple_label || !simple_destination {
        return InlineLinkParse::Complex { end };
    }

    InlineLinkParse::Simple { label, end }
}

fn matching_link_label_close(text: &str, open_start: usize) -> Option<(usize, bool)> {
    let label_start = open_start + 1;
    let mut cursor = label_start;
    let mut depth = 1usize;
    let mut simple = true;

    while cursor < text.len() {
        if cursor.saturating_sub(label_start) > MAX_LSP_MARKDOWN_INLINE_LINK_LABEL_BYTES {
            return None;
        }

        let remaining = &text[cursor..];
        if remaining.starts_with('`') && !is_escaped(text, cursor) {
            let tick_count = backtick_run_len(remaining);
            let code_start = cursor + tick_count;
            let max_code_span_bytes = label_start
                .saturating_add(MAX_LSP_MARKDOWN_INLINE_LINK_LABEL_BYTES)
                .saturating_sub(code_start);
            let relative_close = matching_backtick_run_bounded(
                &text[code_start..],
                tick_count,
                max_code_span_bytes,
            )?;
            cursor = code_start + relative_close + tick_count;
            continue;
        }

        match text.as_bytes()[cursor] {
            b'[' if !is_escaped(text, cursor) => {
                if depth >= MAX_LSP_MARKDOWN_INLINE_LINK_NESTING {
                    return None;
                }
                simple = false;
                depth += 1;
                cursor += 1;
            }
            b']' if !is_escaped(text, cursor) => {
                depth -= 1;
                if depth == 0 {
                    return Some((cursor, simple));
                }
                cursor += 1;
            }
            b'\n' | b'\r' => return None,
            _ => cursor += char_len_at(text, cursor),
        }
    }

    None
}

fn inline_link_destination_end(text: &str, open_paren: usize) -> Option<(usize, bool)> {
    debug_assert_eq!(text.as_bytes().get(open_paren), Some(&b'('));

    let destination_start = open_paren + 1;
    let mut cursor = destination_start;
    let mut depth = 1usize;
    let mut has_destination = false;
    let mut simple = true;

    while cursor < text.len() {
        if cursor.saturating_sub(destination_start) > MAX_LSP_MARKDOWN_INLINE_LINK_DESTINATION_BYTES
        {
            return None;
        }

        match text.as_bytes()[cursor] {
            b'(' if !is_escaped(text, cursor) => {
                if depth >= MAX_LSP_MARKDOWN_INLINE_LINK_NESTING {
                    return None;
                }
                simple = false;
                depth += 1;
                has_destination = true;
                cursor += 1;
            }
            b')' if !is_escaped(text, cursor) => {
                depth -= 1;
                if depth == 0 {
                    return Some((cursor + 1, simple && has_destination));
                }
                has_destination = true;
                cursor += 1;
            }
            b' ' | b'\t' if depth == 1 && has_destination => {
                if let Some((end, title_is_simple)) = inline_link_title_or_closing_end(text, cursor)
                {
                    return Some((end, simple && title_is_simple));
                }
                simple = false;
                cursor += 1;
            }
            b' ' | b'\t' | b'\n' | b'\r' | b'"' | b'\'' | b'<' | b'>' => {
                simple = false;
                has_destination = true;
                cursor += 1;
            }
            _ => {
                has_destination = true;
                cursor += char_len_at(text, cursor);
            }
        }
    }

    None
}

fn matching_backtick_run_bounded(text: &str, tick_count: usize, max_bytes: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let scan_end = bytes.len().min(max_bytes);
    let mut cursor = 0usize;

    while cursor < scan_end {
        if bytes[cursor] != b'`' {
            cursor += 1;
            continue;
        }

        let candidate_count = bytes[cursor..scan_end]
            .iter()
            .take_while(|byte| **byte == b'`')
            .count();
        if candidate_count == tick_count {
            return Some(cursor);
        }
        cursor += candidate_count;
    }

    None
}

fn inline_link_title_or_closing_end(text: &str, whitespace_start: usize) -> Option<(usize, bool)> {
    let mut cursor = whitespace_start;
    while matches!(text.as_bytes().get(cursor), Some(b' ' | b'\t')) {
        if cursor.saturating_sub(whitespace_start) > MAX_LSP_MARKDOWN_INLINE_LINK_TITLE_BYTES {
            return None;
        }
        cursor += 1;
    }

    if text.as_bytes().get(cursor) == Some(&b')') {
        return Some((cursor + 1, true));
    }

    let close = match text.as_bytes().get(cursor).copied()? {
        b'"' => b'"',
        b'\'' => b'\'',
        b'(' => b')',
        _ => return None,
    };
    cursor += 1;
    let title_start = cursor;

    while cursor < text.len() {
        if cursor.saturating_sub(title_start) > MAX_LSP_MARKDOWN_INLINE_LINK_TITLE_BYTES {
            return None;
        }

        match text.as_bytes()[cursor] {
            byte if byte == close && !is_escaped(text, cursor) => {
                cursor += 1;
                while matches!(text.as_bytes().get(cursor), Some(b' ' | b'\t')) {
                    cursor += 1;
                }
                return (text.as_bytes().get(cursor) == Some(&b')')).then_some((cursor + 1, true));
            }
            b'\n' | b'\r' => return None,
            _ => cursor += char_len_at(text, cursor),
        }
    }

    None
}

fn is_image_link(text: &str, open_start: usize) -> bool {
    open_start > 0 && text.as_bytes()[open_start - 1] == b'!'
}

fn is_escaped(text: &str, index: usize) -> bool {
    let bytes = text.as_bytes();
    let mut cursor = index;
    let mut slash_count = 0usize;
    while cursor > 0 && bytes[cursor - 1] == b'\\' {
        slash_count += 1;
        cursor -= 1;
    }
    slash_count % 2 == 1
}

fn char_len_at(text: &str, index: usize) -> usize {
    text[index..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::{
        LSP_MARKDOWN_CODE_TRUNCATED_NOTICE, LSP_MARKDOWN_TEXT_TRUNCATED_NOTICE,
        MAX_LSP_MARKDOWN_CODE_CHARS, MAX_LSP_MARKDOWN_INLINE_CHARS,
        MAX_LSP_MARKDOWN_INLINE_LAYOUT_SECTIONS, MAX_LSP_MARKDOWN_INLINE_LINK_DESTINATION_BYTES,
        MAX_LSP_MARKDOWN_INLINE_LINK_LABEL_BYTES, MAX_LSP_MARKDOWN_INLINE_LINK_NESTING,
        MAX_LSP_MARKDOWN_INLINE_LINK_TITLE_BYTES, MarkdownDisplayTextKind, MarkdownInlineSegment,
        bounded_code_block_text, bounded_inline_markdown_text, bounded_markdown_display_text,
        has_inline_markdown_controls, markdown_inline_job, markdown_inline_segments,
        next_markdown_control, prepare_inline_markdown_text,
        split_trailing_markdown_truncation_notice,
    };
    use eframe::egui::{Color32, FontFamily, text::LayoutJob};
    use std::borrow::Cow;

    #[test]
    fn markdown_inline_segments_split_code_spans() {
        assert_eq!(
            markdown_inline_segments("Use `Vec::new` here"),
            vec![
                MarkdownInlineSegment::Text("Use "),
                MarkdownInlineSegment::Code("Vec::new"),
                MarkdownInlineSegment::Text(" here"),
            ]
        );
    }

    #[test]
    fn markdown_inline_segments_match_same_length_backtick_runs() {
        assert_eq!(
            markdown_inline_segments("Use ``a `tick` value`` now"),
            vec![
                MarkdownInlineSegment::Text("Use "),
                MarkdownInlineSegment::Code("a `tick` value"),
                MarkdownInlineSegment::Text(" now"),
            ]
        );
    }

    #[test]
    fn markdown_inline_segments_preserve_unclosed_backticks_as_text() {
        assert_eq!(
            markdown_inline_segments("Use `Vec::new here"),
            vec![MarkdownInlineSegment::Text("Use `Vec::new here")]
        );
    }

    #[test]
    fn markdown_inline_segments_preserve_escaped_backticks_as_text() {
        assert_eq!(
            markdown_inline_segments("Use \\`Vec::new\\` here"),
            vec![MarkdownInlineSegment::Text("Use \\`Vec::new\\` here")]
        );
    }

    #[test]
    fn markdown_inline_segments_allow_code_after_even_backslash_run() {
        assert_eq!(
            markdown_inline_segments("Use \\\\`Vec::new` here"),
            vec![
                MarkdownInlineSegment::Text("Use \\\\"),
                MarkdownInlineSegment::Code("Vec::new"),
                MarkdownInlineSegment::Text(" here"),
            ]
        );
    }

    #[test]
    fn inline_markdown_normalizes_simple_links_to_labels() {
        assert_eq!(
            markdown_inline_segments("Use [Vec](https://example.test) here"),
            vec![
                MarkdownInlineSegment::Text("Use "),
                MarkdownInlineSegment::Text("Vec"),
                MarkdownInlineSegment::Text(" here"),
            ]
        );
    }

    #[test]
    fn inline_markdown_preserves_code_spans_inside_link_labels() {
        assert_eq!(
            markdown_inline_segments("Use [`Vec`](https://example.test) here"),
            vec![
                MarkdownInlineSegment::Text("Use "),
                MarkdownInlineSegment::Code("Vec"),
                MarkdownInlineSegment::Text(" here"),
            ]
        );
    }

    #[test]
    fn inline_markdown_normalizes_links_with_escaped_backticks_in_label() {
        assert_eq!(
            markdown_inline_segments("Use [a \\` literal](https://example.test) here"),
            vec![
                MarkdownInlineSegment::Text("Use "),
                MarkdownInlineSegment::Text("a \\` literal"),
                MarkdownInlineSegment::Text(" here"),
            ]
        );
    }

    #[test]
    fn inline_markdown_normalizes_links_with_quoted_titles_to_labels() {
        assert_eq!(
            markdown_inline_segments(
                "Use [`Vec`](https://example.test \"standard Vec docs\") and [Map](url 'docs')",
            ),
            vec![
                MarkdownInlineSegment::Text("Use "),
                MarkdownInlineSegment::Code("Vec"),
                MarkdownInlineSegment::Text(" and "),
                MarkdownInlineSegment::Text("Map"),
            ]
        );
    }

    #[test]
    fn inline_markdown_preserves_links_with_malformed_titles() {
        let text = "Use [Vec](https://example.test \"unterminated docs) here";

        assert_eq!(
            markdown_inline_segments(text),
            vec![MarkdownInlineSegment::Text(text)]
        );
    }

    #[test]
    fn inline_markdown_preserves_oversized_link_labels() {
        let label = "a".repeat(MAX_LSP_MARKDOWN_INLINE_LINK_LABEL_BYTES + 1);
        let text = format!("Use [{label}](https://example.test) here");

        assert_eq!(
            markdown_inline_segments(&text),
            vec![MarkdownInlineSegment::Text(text.as_str())]
        );
    }

    #[test]
    fn inline_markdown_preserves_oversized_link_destinations() {
        let destination = "u".repeat(MAX_LSP_MARKDOWN_INLINE_LINK_DESTINATION_BYTES + 1);
        let text = format!("Use [Vec]({destination}) here");

        assert_eq!(
            markdown_inline_segments(&text),
            vec![MarkdownInlineSegment::Text(text.as_str())]
        );
    }

    #[test]
    fn inline_markdown_preserves_oversized_link_titles() {
        let title = "t".repeat(MAX_LSP_MARKDOWN_INLINE_LINK_TITLE_BYTES + 1);
        let text = format!("Use [Vec](https://example.test \"{title}\") here");

        assert_eq!(
            markdown_inline_segments(&text),
            vec![MarkdownInlineSegment::Text(text.as_str())]
        );
    }

    #[test]
    fn inline_markdown_preserves_excessively_nested_link_labels() {
        let nested = "[".repeat(MAX_LSP_MARKDOWN_INLINE_LINK_NESTING + 1);
        let closing = "]".repeat(MAX_LSP_MARKDOWN_INLINE_LINK_NESTING + 1);
        let text = format!("Use [{nested}Vec{closing}](https://example.test) here");

        assert_eq!(
            markdown_inline_segments(&text),
            vec![MarkdownInlineSegment::Text(text.as_str())]
        );
    }

    #[test]
    fn inline_markdown_escaped_backticks_do_not_hide_complex_link_labels() {
        let text = "Use [outer \\`[inner]\\`](https://example.test) here";

        assert_eq!(
            markdown_inline_segments(text),
            vec![MarkdownInlineSegment::Text(text)]
        );
    }

    #[test]
    fn markdown_inline_job_appends_segments_without_collection_step() {
        let job = markdown_inline_job(
            "Use [`Vec`](https://example.test) here",
            super::LspMarkdownTextSize::Normal,
            Color32::WHITE,
        );

        assert_eq!(job.text, "Use Vec here");
        assert_eq!(job.sections.len(), 3);
        assert_eq!(&job.text[job.sections[1].byte_range.clone()], "Vec");
    }

    #[test]
    fn markdown_inline_job_preserves_text_and_code_section_boundaries() {
        let job = markdown_inline_job(
            "A `code` [label](url) and `tail`",
            super::LspMarkdownTextSize::Normal,
            Color32::WHITE,
        );

        assert_eq!(job.text, "A code label and tail");
        assert_eq!(
            layout_job_section_texts(&job),
            vec!["A ", "code", " ", "label", " and ", "tail"]
        );
        assert_eq!(
            job.sections[0].format.font_id.family,
            FontFamily::Proportional
        );
        assert_eq!(job.sections[1].format.font_id.family, FontFamily::Monospace);
        assert_eq!(
            job.sections[3].format.font_id.family,
            FontFamily::Proportional
        );
        assert_eq!(job.sections[5].format.font_id.family, FontFamily::Monospace);
    }

    #[test]
    fn markdown_inline_job_caps_pathological_layout_sections() {
        let span_count = MAX_LSP_MARKDOWN_INLINE_LAYOUT_SECTIONS + 80;
        let text = "`x` ".repeat(span_count);
        let job = markdown_inline_job(&text, super::LspMarkdownTextSize::Normal, Color32::WHITE);

        assert_eq!(job.text, "x ".repeat(span_count));
        assert!(job.sections.len() <= MAX_LSP_MARKDOWN_INLINE_LAYOUT_SECTIONS);
    }

    #[test]
    fn markdown_inline_job_unescapes_markdown_punctuation_in_text_segments() {
        let job = markdown_inline_job(
            "Use \\`literal\\` and \\[not a link](url), keep `C:\\\\tmp`",
            super::LspMarkdownTextSize::Normal,
            Color32::WHITE,
        );

        assert_eq!(
            job.text,
            "Use `literal` and [not a link](url), keep C:\\\\tmp"
        );
        assert_eq!(
            layout_job_section_texts(&job),
            vec![
                "Use ",
                "`",
                "literal",
                "`",
                " and ",
                "[",
                "not a link](url), keep ",
                "C:\\\\tmp",
            ]
        );
        assert_eq!(
            job.sections.last().unwrap().format.font_id.family,
            FontFamily::Monospace
        );
    }

    #[test]
    fn markdown_inline_segments_leave_links_inside_code_spans_literal() {
        assert_eq!(
            markdown_inline_segments("Use `[Vec](https://example.test)` here"),
            vec![
                MarkdownInlineSegment::Text("Use "),
                MarkdownInlineSegment::Code("[Vec](https://example.test)"),
                MarkdownInlineSegment::Text(" here"),
            ]
        );
    }

    #[test]
    fn markdown_inline_segments_leave_non_simple_links_literal() {
        let text = "![Vec](url) [Vec][ref] [outer [Vec](url)](url) [Vec](url(title)) [Vec](url";
        assert_eq!(
            markdown_inline_segments(text),
            vec![MarkdownInlineSegment::Text(text)]
        );
    }

    #[test]
    fn inline_markdown_control_scan_finds_first_byte_control() {
        let prefix = "\u{03b1}\u{03b2} ";
        let text = "\u{03b1}\u{03b2} [label] and `code`";

        assert!(has_inline_markdown_controls(text));
        assert!(has_inline_markdown_controls("\\*escaped emphasis\\*"));
        assert!(!has_inline_markdown_controls("plain \u{03b1}\u{03b2} text"));
        assert_eq!(next_markdown_control(text, 0), Some((prefix.len(), b'[')));
        assert_eq!(
            next_markdown_control(text, "\u{03b1}\u{03b2} [label] and ".len()),
            Some(("\u{03b1}\u{03b2} [label] and ".len(), b'`'))
        );
    }

    #[test]
    fn bounded_inline_markdown_text_strips_controls_and_caps_rendered_text() {
        let bounded = bounded_markdown_display_text(
            "ab\nc\td\u{0007}ef",
            4,
            LSP_MARKDOWN_TEXT_TRUNCATED_NOTICE,
            MarkdownDisplayTextKind::Inline,
        );

        assert_eq!(bounded.as_ref(), "abcd [truncated]");
    }

    #[test]
    fn bounded_inline_markdown_text_sanitizes_before_applying_rendered_limit() {
        let text = format!(
            "{}\n\u{202e}yz",
            "x".repeat(MAX_LSP_MARKDOWN_INLINE_CHARS - 1)
        );
        let expected = format!(
            "{}y{}",
            "x".repeat(MAX_LSP_MARKDOWN_INLINE_CHARS - 1),
            LSP_MARKDOWN_TEXT_TRUNCATED_NOTICE
        );

        let bounded = bounded_inline_markdown_text(&text);

        assert_eq!(bounded.as_ref(), expected.as_str());
    }

    #[test]
    fn bounded_inline_markdown_text_strips_bidi_format_controls() {
        let bounded = bounded_inline_markdown_text("safe\u{202e}\u{200b}\u{2060}\u{feff}txt");

        assert_eq!(bounded.as_ref(), "safetxt");
    }

    #[test]
    fn bounded_inline_markdown_text_strips_unicode_line_separators() {
        let bounded = bounded_inline_markdown_text("safe\u{2028}line\u{2029}txt");

        assert_eq!(bounded.as_ref(), "safelinetxt");
    }

    #[test]
    fn bounded_inline_markdown_text_leaves_small_clean_text_borrowed() {
        let bounded = bounded_inline_markdown_text("Use `Vec::new` here");

        assert_eq!(bounded, Cow::Borrowed("Use `Vec::new` here"));
    }

    #[test]
    fn prepared_inline_markdown_text_keeps_clean_ascii_borrowed_without_layout() {
        let prepared = prepare_inline_markdown_text("Plain ASCII hover text.");

        assert_eq!(prepared.text, Cow::Borrowed("Plain ASCII hover text."));
        assert!(!prepared.needs_layout_job);

        let code = bounded_code_block_text("let x = 1;\n\treturn x;\r\n");

        assert_eq!(code, Cow::Borrowed("let x = 1;\n\treturn x;\r\n"));
    }

    #[test]
    fn prepared_inline_markdown_text_flags_ascii_markdown_controls_without_copying() {
        let prepared = prepare_inline_markdown_text("Use `Vec` and \\*literal\\*");

        assert_eq!(prepared.text, Cow::Borrowed("Use `Vec` and \\*literal\\*"));
        assert!(prepared.needs_layout_job);
    }

    #[test]
    fn prepared_inline_markdown_text_reuses_bounded_scan_for_layout_decision() {
        let plain = "a".repeat(MAX_LSP_MARKDOWN_INLINE_CHARS + 1);
        let prepared = prepare_inline_markdown_text(&plain);
        let expected = format!(
            "{}{}",
            "a".repeat(MAX_LSP_MARKDOWN_INLINE_CHARS),
            LSP_MARKDOWN_TEXT_TRUNCATED_NOTICE
        );

        assert_eq!(prepared.text.as_ref(), expected.as_str());
        assert!(!prepared.needs_layout_job);

        let escaped = prepare_inline_markdown_text("\\[literal]");
        assert_eq!(escaped.text, Cow::Borrowed("\\[literal]"));
        assert!(escaped.needs_layout_job);
    }

    #[test]
    fn bounded_code_block_text_preserves_code_whitespace_but_strips_other_controls() {
        let bounded = bounded_markdown_display_text(
            "a\tb\nc\u{0007}d\u{2028}e\u{2029}f",
            8,
            LSP_MARKDOWN_CODE_TRUNCATED_NOTICE,
            MarkdownDisplayTextKind::Code,
        );

        assert_eq!(bounded.as_ref(), "a\tb\ncdef");
    }

    #[test]
    fn bounded_code_block_text_caps_pathological_code_on_utf8_boundary() {
        let code = format!("{}tail", "\u{03b1}".repeat(MAX_LSP_MARKDOWN_CODE_CHARS + 4));
        let bounded = bounded_code_block_text(&code);

        assert_eq!(
            bounded.matches('\u{03b1}').count(),
            MAX_LSP_MARKDOWN_CODE_CHARS
        );
        assert!(bounded.ends_with("[Code block truncated]"));
        assert!(!bounded.contains("tail"));
    }

    #[test]
    fn trailing_truncation_notice_is_split_from_unclosed_code_blocks() {
        assert_eq!(
            split_trailing_markdown_truncation_notice("let value = 1;\n\n[Hover truncated]"),
            ("let value = 1;", Some("[Hover truncated]"))
        );
        assert_eq!(
            split_trailing_markdown_truncation_notice(
                "fn main() {}\n\n[Signature documentation truncated]\n"
            ),
            ("fn main() {}", Some("[Signature documentation truncated]"))
        );
    }

    #[test]
    fn trailing_truncation_notice_does_not_split_literal_code_text() {
        assert_eq!(
            split_trailing_markdown_truncation_notice("panic!(\"[Hover truncated]\")"),
            ("panic!(\"[Hover truncated]\")", None)
        );
    }

    fn layout_job_section_texts(job: &LayoutJob) -> Vec<&str> {
        job.sections
            .iter()
            .map(|section| &job.text[section.byte_range.clone()])
            .collect()
    }
}
