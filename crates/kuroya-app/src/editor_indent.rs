use crate::KuroyaApp;
use kuroya_core::{BufferId, EditorSettings, LanguageId, Selection, TextBuffer};

const INDENT_DETECTION_MAX_LINES: usize = 200;
const INDENT_DETECTION_MAX_LINE_CHARS: usize = 256;
const SPACE_INDENT_CANDIDATES: [usize; 3] = [2, 4, 8];
const RECORDED_SPACE_INDENT_MAX: usize = 12;
const NEWLINE_INDENT_CONTEXT_MAX_CHARS: usize = 4096;
const NEWLINE_INDENT_STACK_MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndentOptions {
    pub(crate) unit: String,
    pub(crate) tab_size: usize,
    pub(crate) insert_spaces: bool,
}

impl KuroyaApp {
    pub(crate) fn indent_options_for_buffer(&self, id: BufferId) -> IndentOptions {
        indent_options(self.buffer(id), &self.settings)
    }

    pub(crate) fn insert_newline_with_auto_indent_for_buffer(
        &mut self,
        id: BufferId,
        indent_unit: &str,
    ) -> bool {
        let Some(buffer_index) = self.buffers.iter().position(|buffer| buffer.id() == id) else {
            return false;
        };
        let indent_overrides = {
            let buffer = &self.buffers[buffer_index];
            let syntax_tree_overrides = self
                .syntax_tree_cache
                .newline_indent_overrides_for_buffer(buffer, indent_unit);
            newline_indent_overrides_for_buffer(buffer, indent_unit, syntax_tree_overrides)
        };
        self.buffers[buffer_index]
            .insert_newline_with_indent_overrides(indent_unit, &indent_overrides);
        true
    }
}

pub(crate) fn indent_options(
    buffer: Option<&TextBuffer>,
    settings: &EditorSettings,
) -> IndentOptions {
    let configured = configured_indent_options(settings);
    if !settings.detect_indentation {
        return configured;
    }

    buffer
        .and_then(detected_indent_options_for_buffer)
        .unwrap_or(configured)
}

fn configured_indent_options(settings: &EditorSettings) -> IndentOptions {
    let tab_size = settings.tab_width.max(1);
    if settings.insert_spaces {
        IndentOptions {
            unit: " ".repeat(tab_size),
            tab_size,
            insert_spaces: true,
        }
    } else {
        IndentOptions {
            unit: "\t".to_owned(),
            tab_size,
            insert_spaces: false,
        }
    }
}

#[cfg(test)]
fn detected_indent_options(text: &str) -> Option<IndentOptions> {
    detected_indent_options_from_lines(text.lines())
}

fn detected_indent_options_for_buffer(buffer: &TextBuffer) -> Option<IndentOptions> {
    let mut samples = IndentSamples::default();

    for line_idx in 0..buffer.len_lines().min(INDENT_DETECTION_MAX_LINES) {
        record_indent_sample_for_buffer(buffer, line_idx, &mut samples);
    }

    detected_indent_options_from_samples(samples)
}

#[cfg(test)]
fn detected_indent_options_from_lines<'a>(
    lines: impl IntoIterator<Item = impl AsRef<str> + 'a>,
) -> Option<IndentOptions> {
    let mut samples = IndentSamples::default();

    for line in lines.into_iter().take(INDENT_DETECTION_MAX_LINES) {
        record_indent_sample(line.as_ref(), &mut samples);
    }

    detected_indent_options_from_samples(samples)
}

fn detected_indent_options_from_samples(samples: IndentSamples) -> Option<IndentOptions> {
    if samples.tab_indents > samples.space_indents {
        return Some(IndentOptions {
            unit: "\t".to_owned(),
            tab_size: 4,
            insert_spaces: false,
        });
    }

    samples
        .most_likely_space_indent()
        .map(|tab_size| IndentOptions {
            unit: " ".repeat(tab_size),
            tab_size,
            insert_spaces: true,
        })
}

#[cfg(test)]
fn record_indent_sample(line: &str, samples: &mut IndentSamples) {
    let mut saw_space = false;
    let mut saw_tab = false;
    let mut spaces = 0usize;

    for ch in line.trim_end_matches(['\r', '\n']).chars() {
        match ch {
            ' ' => {
                saw_space = true;
                spaces = spaces.saturating_add(1);
            }
            '\t' => {
                saw_tab = true;
            }
            _ => {
                record_finished_indent_sample(samples, saw_space, saw_tab, spaces);
                return;
            }
        }
    }
}

fn record_indent_sample_for_buffer(
    buffer: &TextBuffer,
    line_idx: usize,
    samples: &mut IndentSamples,
) {
    let mut saw_space = false;
    let mut saw_tab = false;
    let mut spaces = 0usize;
    let start = buffer.line_column_to_char(line_idx, 0);
    let end = buffer.line_content_end_char(line_idx);

    for char_idx in start..end.min(start.saturating_add(INDENT_DETECTION_MAX_LINE_CHARS)) {
        match buffer.char_at(char_idx) {
            Some(' ') => {
                saw_space = true;
                spaces = spaces.saturating_add(1);
            }
            Some('\t') => {
                saw_tab = true;
            }
            Some(_) => {
                record_finished_indent_sample(samples, saw_space, saw_tab, spaces);
                return;
            }
            None => return,
        }
    }
}

fn record_finished_indent_sample(
    samples: &mut IndentSamples,
    saw_space: bool,
    saw_tab: bool,
    spaces: usize,
) {
    if saw_space && saw_tab {
        return;
    }

    if saw_tab {
        samples.record_tab_indent();
    } else if spaces > 0 {
        samples.record_space_indent(spaces);
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct IndentSamples {
    tab_indents: usize,
    space_indents: usize,
    space_candidate_hits: [usize; 3],
}

impl IndentSamples {
    fn record_tab_indent(&mut self) {
        self.tab_indents = self.tab_indents.saturating_add(1);
    }

    fn record_space_indent(&mut self, spaces: usize) {
        let width = spaces.min(RECORDED_SPACE_INDENT_MAX);
        self.space_indents = self.space_indents.saturating_add(1);
        for (index, candidate) in SPACE_INDENT_CANDIDATES.iter().copied().enumerate() {
            if width >= candidate && width.is_multiple_of(candidate) {
                self.space_candidate_hits[index] =
                    self.space_candidate_hits[index].saturating_add(1);
            }
        }
    }

    fn most_likely_space_indent(self) -> Option<usize> {
        let mut best = None;
        for (index, score) in self.space_candidate_hits.iter().copied().enumerate() {
            if score == 0 {
                continue;
            }

            match best {
                Some((_, best_score)) if score < best_score => {}
                Some((best_index, best_score)) if score == best_score && index <= best_index => {}
                _ => best = Some((index, score)),
            }
        }
        best.map(|(index, _)| SPACE_INDENT_CANDIDATES[index])
    }
}

fn newline_indent_overrides_for_buffer(
    buffer: &TextBuffer,
    indent_unit: &str,
    syntax_tree_overrides: Option<Vec<Option<String>>>,
) -> Vec<Option<String>> {
    let mut overrides = syntax_tree_overrides.unwrap_or_default();
    if indent_unit.is_empty() || !brace_context_indent_enabled(buffer) {
        return overrides;
    }

    let indent_unit_chars = indent_unit.chars().count();
    if indent_unit_chars == 0 {
        return overrides;
    }

    let selection_count = buffer.selections().len();
    for (index, selection) in buffer.selections().iter().copied().enumerate() {
        if overrides.get(index).is_some_and(|indent| indent.is_some()) {
            continue;
        }

        let Some(indent) = brace_context_indent_override_for_selection(
            buffer,
            selection,
            indent_unit,
            indent_unit_chars,
        ) else {
            continue;
        };

        if overrides.len() < selection_count {
            overrides.resize_with(selection_count, || None);
        }
        overrides[index] = Some(indent);
    }

    if overrides.iter().any(Option::is_some) {
        overrides
    } else {
        Vec::new()
    }
}

fn brace_context_indent_enabled(buffer: &TextBuffer) -> bool {
    !matches!(buffer.language(), LanguageId::Markdown | LanguageId::Diff)
}

fn brace_context_indent_override_for_selection(
    buffer: &TextBuffer,
    selection: Selection,
    indent_unit: &str,
    indent_unit_chars: usize,
) -> Option<String> {
    if !selection.is_caret() {
        return None;
    }

    let cursor = selection.cursor.min(buffer.len_chars());
    let position = buffer.char_position(cursor);
    let line_start = buffer.line_column_to_char(position.line, 0);
    let line_end = buffer.line_content_end_char(position.line);
    let cursor = cursor.min(line_end).max(line_start);
    let opener = unmatched_opening_delimiter_before(buffer, cursor)?;
    let opener_line = buffer.char_position(opener).line;
    let desired_indent_len =
        leading_indent_char_count_for_line(buffer, opener_line).saturating_add(indent_unit_chars);
    let current_indent_len = leading_indent_char_count_for_line(buffer, position.line);
    if desired_indent_len <= current_indent_len {
        return None;
    }

    let mut indent = leading_indent_for_line(buffer, opener_line);
    indent.push_str(indent_unit);
    Some(indent)
}

#[derive(Debug, Clone, Copy)]
enum NewlineIndentScanState {
    Code,
    LineComment,
    BlockComment,
    String { quote: char, escaped: bool },
}

fn unmatched_opening_delimiter_before(buffer: &TextBuffer, cursor: usize) -> Option<usize> {
    let cursor = cursor.min(buffer.len_chars());
    let start = newline_indent_scan_start(buffer, cursor);
    let line_comment_prefix = buffer
        .language()
        .line_comment_prefix()
        .filter(|prefix| !prefix.is_empty());
    let line_comment_prefix_len = line_comment_prefix
        .map(|prefix| prefix.chars().count())
        .unwrap_or_default();
    let mut state = NewlineIndentScanState::Code;
    let mut stack = [('\0', 0usize); NEWLINE_INDENT_STACK_MAX_DEPTH];
    let mut depth = 0usize;
    let mut overflowed = false;
    let mut char_idx = start;

    while char_idx < cursor {
        let Some(ch) = buffer.char_at(char_idx) else {
            break;
        };

        match state {
            NewlineIndentScanState::Code => {
                if let Some(prefix) = line_comment_prefix {
                    if buffer_starts_with_before(buffer, char_idx, cursor, prefix) {
                        state = NewlineIndentScanState::LineComment;
                        char_idx = char_idx.saturating_add(line_comment_prefix_len);
                        continue;
                    }
                }

                if ch == '/' && buffer.char_at(char_idx.saturating_add(1)) == Some('*') {
                    state = NewlineIndentScanState::BlockComment;
                    char_idx = char_idx.saturating_add(2);
                    continue;
                }

                if matches!(ch, '"' | '`') {
                    state = NewlineIndentScanState::String {
                        quote: ch,
                        escaped: false,
                    };
                    char_idx = char_idx.saturating_add(1);
                    continue;
                }

                if ch == '\'' {
                    if let Some(end) = single_quoted_literal_end(buffer, char_idx, cursor) {
                        char_idx = end.saturating_add(1);
                        continue;
                    }
                }

                if is_opening_delimiter(ch) {
                    if depth < NEWLINE_INDENT_STACK_MAX_DEPTH {
                        stack[depth] = (ch, char_idx);
                        depth += 1;
                    } else {
                        overflowed = true;
                    }
                } else if let Some(open) = opening_delimiter_for_close(ch) {
                    if depth > 0 && stack[depth - 1].0 == open {
                        depth -= 1;
                    }
                }
                char_idx = char_idx.saturating_add(1);
            }
            NewlineIndentScanState::LineComment => {
                if matches!(ch, '\n' | '\r') {
                    state = NewlineIndentScanState::Code;
                }
                char_idx = char_idx.saturating_add(1);
            }
            NewlineIndentScanState::BlockComment => {
                if ch == '*' && buffer.char_at(char_idx.saturating_add(1)) == Some('/') {
                    state = NewlineIndentScanState::Code;
                    char_idx = char_idx.saturating_add(2);
                } else {
                    char_idx = char_idx.saturating_add(1);
                }
            }
            NewlineIndentScanState::String { quote, escaped } => {
                if escaped {
                    state = NewlineIndentScanState::String {
                        quote,
                        escaped: false,
                    };
                } else if quote != '`' && ch == '\\' {
                    state = NewlineIndentScanState::String {
                        quote,
                        escaped: true,
                    };
                } else if ch == quote {
                    state = NewlineIndentScanState::Code;
                }
                char_idx = char_idx.saturating_add(1);
            }
        }
    }

    if overflowed || depth == 0 {
        None
    } else {
        Some(stack[depth - 1].1)
    }
}

fn single_quoted_literal_end(buffer: &TextBuffer, start: usize, end: usize) -> Option<usize> {
    let mut escaped = false;
    let mut char_idx = start.saturating_add(1);
    while char_idx < end {
        let ch = buffer.char_at(char_idx)?;

        match ch {
            '\n' | '\r' => return None,
            '\\' if !escaped => escaped = true,
            '\'' if !escaped => return Some(char_idx),
            _ => escaped = false,
        }
        char_idx = char_idx.saturating_add(1);
    }
    None
}

fn newline_indent_scan_start(buffer: &TextBuffer, cursor: usize) -> usize {
    let lower = cursor.saturating_sub(NEWLINE_INDENT_CONTEXT_MAX_CHARS);
    if lower == 0 {
        return 0;
    }

    let mut char_idx = lower;
    while char_idx < cursor {
        if matches!(
            buffer.char_at(char_idx.saturating_sub(1)),
            Some('\n' | '\r')
        ) {
            return char_idx;
        }
        char_idx = char_idx.saturating_add(1);
    }
    cursor
}

fn buffer_starts_with_before(buffer: &TextBuffer, start: usize, end: usize, needle: &str) -> bool {
    let mut char_idx = start;
    for expected in needle.chars() {
        if char_idx >= end || buffer.char_at(char_idx) != Some(expected) {
            return false;
        }
        char_idx = char_idx.saturating_add(1);
    }
    true
}

fn is_opening_delimiter(ch: char) -> bool {
    matches!(ch, '(' | '[' | '{')
}

fn opening_delimiter_for_close(ch: char) -> Option<char> {
    match ch {
        ')' => Some('('),
        ']' => Some('['),
        '}' => Some('{'),
        _ => None,
    }
}

fn leading_indent_char_count_for_line(buffer: &TextBuffer, line: usize) -> usize {
    let start = buffer.line_column_to_char(line, 0);
    let end = buffer.line_content_end_char(line);
    let mut char_idx = start;
    while char_idx < end && matches!(buffer.char_at(char_idx), Some(' ' | '\t')) {
        char_idx = char_idx.saturating_add(1);
    }
    char_idx.saturating_sub(start)
}

fn leading_indent_for_line(buffer: &TextBuffer, line: usize) -> String {
    let count = leading_indent_char_count_for_line(buffer, line);
    let start = buffer.line_column_to_char(line, 0);
    let mut indent = String::with_capacity(count);
    for char_idx in start..start.saturating_add(count) {
        let Some(ch @ (' ' | '\t')) = buffer.char_at(char_idx) else {
            break;
        };
        indent.push(ch);
    }
    indent
}

#[cfg(test)]
mod tests {
    use super::{
        INDENT_DETECTION_MAX_LINE_CHARS, detected_indent_options,
        detected_indent_options_for_buffer, indent_options, newline_indent_overrides_for_buffer,
    };
    use kuroya_core::{EditorSettings, LanguageId, TextBuffer};

    #[test]
    fn indent_options_respect_insert_spaces_when_detection_is_off() {
        let settings = EditorSettings {
            tab_width: 2,
            insert_spaces: false,
            detect_indentation: false,
            ..EditorSettings::default()
        };

        assert_eq!(indent_options(None, &settings).unit, "\t");
        assert!(!indent_options(None, &settings).insert_spaces);
    }

    #[test]
    fn indent_options_detect_space_indentation_from_buffer() {
        let settings = EditorSettings {
            tab_width: 4,
            insert_spaces: false,
            detect_indentation: true,
            ..EditorSettings::default()
        };
        let buffer = TextBuffer::from_text(1, None, "root\n  child\n    leaf\n".to_owned());

        let options = indent_options(Some(&buffer), &settings);
        assert_eq!(options.unit, "  ");
        assert_eq!(options.tab_size, 2);
        assert!(options.insert_spaces);
    }

    #[test]
    fn indent_options_detect_tabs_from_buffer() {
        let settings = EditorSettings {
            tab_width: 2,
            insert_spaces: true,
            detect_indentation: true,
            ..EditorSettings::default()
        };
        let buffer = TextBuffer::from_text(1, None, "root\n\tchild\n\t\tleaf\n".to_owned());

        let options = indent_options(Some(&buffer), &settings);
        assert_eq!(options.unit, "\t");
        assert!(!options.insert_spaces);
    }

    #[test]
    fn indent_options_samples_buffer_lines_without_later_lines_changing_detection() {
        let settings = EditorSettings {
            tab_width: 4,
            insert_spaces: false,
            detect_indentation: true,
            ..EditorSettings::default()
        };
        let mut lines =
            std::iter::repeat_n("  child", super::INDENT_DETECTION_MAX_LINES).collect::<Vec<_>>();
        lines.extend(std::iter::repeat_n("\tchild", 500));
        let buffer = TextBuffer::from_text(1, None, lines.join("\n"));

        let options = indent_options(Some(&buffer), &settings);

        assert_eq!(options.unit, "  ");
        assert_eq!(options.tab_size, 2);
    }

    #[test]
    fn indent_detection_caps_sampled_buffer_lines() {
        let huge_line = "x".repeat(INDENT_DETECTION_MAX_LINE_CHARS + 10_000);
        let buffer = TextBuffer::from_text(1, None, format!("{huge_line}\n\tchild\n\t\tleaf\n"));

        let options = detected_indent_options_for_buffer(&buffer).unwrap();

        assert_eq!(options.unit, "\t");
        assert!(!options.insert_spaces);
    }

    #[test]
    fn indent_detection_scans_buffer_lines_without_prefix_allocations() {
        let buffer = TextBuffer::from_text(
            1,
            None,
            "root\n  child\n    leaf\n  \tmixed\n\tother\n".to_owned(),
        );

        let options = detected_indent_options_for_buffer(&buffer).unwrap();

        assert_eq!(options.unit, "  ");
        assert_eq!(options.tab_size, 2);
        assert!(options.insert_spaces);
    }

    #[test]
    fn text_indent_detection_skips_blank_and_unindented_lines() {
        let options = detected_indent_options("root\n\n  child\n\tother\n").unwrap();

        assert_eq!(options.unit, "  ");
        assert_eq!(options.tab_size, 2);
    }

    #[test]
    fn text_indent_detection_ignores_mixed_leading_whitespace_samples() {
        let options = detected_indent_options("  \tmixed\n\t  mixed\n    child\n").unwrap();

        assert_eq!(options.unit, "    ");
        assert_eq!(options.tab_size, 4);
        assert!(options.insert_spaces);
    }

    #[test]
    fn newline_indent_fallback_uses_unmatched_brace_context() {
        let mut buffer = TextBuffer::from_text(1, None, "fn main() {\nlet x = 1;\n}".to_owned());
        buffer.set_single_cursor(buffer.line_content_end_char(1));

        let overrides = newline_indent_overrides_for_buffer(&buffer, "  ", None);
        buffer.insert_newline_with_indent_overrides("  ", &overrides);

        assert_eq!(buffer.text(), "fn main() {\nlet x = 1;\n  \n}");
        assert_eq!(buffer.cursor_position().line, 2);
        assert_eq!(buffer.cursor_position().column, 2);
    }

    #[test]
    fn newline_indent_fallback_keeps_deeper_manual_indent() {
        let mut buffer =
            TextBuffer::from_text(1, None, "fn main() {\n    let x = 1;\n}".to_owned());
        buffer.set_single_cursor(buffer.line_content_end_char(1));

        let overrides = newline_indent_overrides_for_buffer(&buffer, "  ", None);

        assert!(overrides.is_empty());
    }

    #[test]
    fn newline_indent_fallback_preserves_syntax_tree_override() {
        let mut buffer = TextBuffer::from_text(1, None, "fn main() {\nlet x = 1;\n}".to_owned());
        buffer.set_single_cursor(buffer.line_content_end_char(1));

        let overrides = newline_indent_overrides_for_buffer(
            &buffer,
            "  ",
            Some(vec![Some("      ".to_owned())]),
        );

        assert_eq!(overrides, vec![Some("      ".to_owned())]);
    }

    #[test]
    fn newline_indent_fallback_ignores_delimiters_inside_strings_and_comments() {
        let mut string_buffer =
            TextBuffer::from_text(1, None, "let text = \"{\";\nvalue".to_owned());
        string_buffer.set_single_cursor(string_buffer.line_content_end_char(1));

        assert!(newline_indent_overrides_for_buffer(&string_buffer, "  ", None).is_empty());

        let mut char_buffer = TextBuffer::from_text(1, None, "let ch = '{';\nvalue".to_owned());
        char_buffer.set_single_cursor(char_buffer.line_content_end_char(1));

        assert!(newline_indent_overrides_for_buffer(&char_buffer, "  ", None).is_empty());

        let mut comment_buffer = TextBuffer::from_text_with_language(
            1,
            None,
            "// {\nvalue".to_owned(),
            LanguageId::Rust,
        );
        comment_buffer.set_single_cursor(comment_buffer.line_content_end_char(1));

        assert!(newline_indent_overrides_for_buffer(&comment_buffer, "  ", None).is_empty());
    }
}
