use crate::editor_text_geometry::visual_width_for_char;
use egui::{Color32, FontFamily, FontId, TextFormat, text::LayoutJob};
use syntect::{
    highlighting::{HighlightIterator, HighlightState, Highlighter, Style},
    parsing::ScopeStackOp,
};

const DEFAULT_FONT_SIZE: f32 = 13.0;
const MAX_FONT_SIZE: f32 = 128.0;
const DEFAULT_TAB_WIDTH: usize = 4;
const MAX_TAB_WIDTH: usize = 32;

pub(crate) fn normalize_layout_inputs(font_size: f32, tab_width: usize) -> (f32, usize) {
    let font_size = if font_size.is_finite() && font_size > 0.0 {
        font_size.min(MAX_FONT_SIZE)
    } else {
        DEFAULT_FONT_SIZE
    };
    let tab_width = if tab_width == 0 {
        DEFAULT_TAB_WIDTH
    } else {
        tab_width.min(MAX_TAB_WIDTH)
    };
    (font_size, tab_width)
}

pub(crate) fn highlighted_job(
    text: &str,
    ops: &[(usize, ScopeStackOp)],
    highlight_state: &mut HighlightState,
    highlighter: &Highlighter<'_>,
    font_size: f32,
    tab_width: usize,
) -> LayoutJob {
    let (font_size, tab_width) = normalize_layout_inputs(font_size, tab_width);
    let mut job = layout_job_with_text_capacity(expanded_text_capacity(text, tab_width));
    let mut visual_column = 0usize;
    for (style, slice) in HighlightIterator::new(highlight_state, ops, text, highlighter) {
        append_text_with_expanded_tabs(
            &mut job,
            slice,
            format_from_style(style, font_size),
            &mut visual_column,
            tab_width,
        );
    }
    job
}

pub(crate) fn advance_highlight_state(
    text: &str,
    ops: &[(usize, ScopeStackOp)],
    highlight_state: &mut HighlightState,
    highlighter: &Highlighter<'_>,
) {
    for _ in HighlightIterator::new(highlight_state, ops, text, highlighter) {}
}

fn format_from_style(style: Style, font_size: f32) -> TextFormat {
    TextFormat {
        font_id: FontId::new(font_size, FontFamily::Monospace),
        color: Color32::from_rgb(style.foreground.r, style.foreground.g, style.foreground.b),
        ..Default::default()
    }
}

pub(crate) fn plain_job(text: &str, font_size: f32, tab_width: usize) -> LayoutJob {
    let (font_size, tab_width) = normalize_layout_inputs(font_size, tab_width);
    let mut job = layout_job_with_text_capacity(expanded_text_capacity(text, tab_width));
    let mut visual_column = 0usize;
    append_text_with_expanded_tabs(
        &mut job,
        text,
        plain_format(font_size),
        &mut visual_column,
        tab_width,
    );
    job
}

fn layout_job_with_text_capacity(capacity: usize) -> LayoutJob {
    LayoutJob {
        text: String::with_capacity(capacity),
        ..Default::default()
    }
}

fn plain_format(font_size: f32) -> TextFormat {
    TextFormat {
        font_id: FontId::new(font_size, FontFamily::Monospace),
        color: Color32::from_rgb(222, 226, 233),
        ..Default::default()
    }
}

fn append_text_with_expanded_tabs(
    job: &mut LayoutJob,
    text: &str,
    format: TextFormat,
    visual_column: &mut usize,
    tab_width: usize,
) {
    if !text.as_bytes().contains(&b'\t') {
        *visual_column += text_columns_without_tabs(text);
        job.append(text, 0.0, format);
        return;
    }

    let tab_width = tab_width.max(1);
    let mut run_start = 0usize;
    for (byte_idx, byte) in text.as_bytes().iter().enumerate() {
        if *byte != b'\t' {
            continue;
        }

        if run_start < byte_idx {
            let run = &text[run_start..byte_idx];
            *visual_column += text_columns_without_tabs(run);
            job.append(run, 0.0, format.clone());
        }

        let spaces = visual_width_for_char('\t', *visual_column, tab_width);
        append_spaces(job, spaces, format.clone());
        *visual_column += spaces;
        run_start = byte_idx + 1;
    }

    if run_start < text.len() {
        let run = &text[run_start..];
        *visual_column += text_columns_without_tabs(run);
        job.append(run, 0.0, format);
    }
}

fn text_columns_without_tabs(text: &str) -> usize {
    if text.is_ascii() {
        text.len()
    } else {
        text.chars().count()
    }
}

fn expanded_text_capacity(text: &str, tab_width: usize) -> usize {
    if !text.as_bytes().contains(&b'\t') {
        return text.len();
    }

    let tab_width = tab_width.max(1);
    let mut capacity = 0usize;
    let mut visual_column = 0usize;
    let mut run_start = 0usize;

    for (byte_idx, byte) in text.as_bytes().iter().enumerate() {
        if *byte != b'\t' {
            continue;
        }

        if run_start < byte_idx {
            let run = &text[run_start..byte_idx];
            capacity += run.len();
            visual_column += text_columns_without_tabs(run);
        }

        let spaces = visual_width_for_char('\t', visual_column, tab_width);
        capacity += spaces;
        visual_column += spaces;
        run_start = byte_idx + 1;
    }

    if run_start < text.len() {
        capacity += text[run_start..].len();
    }

    capacity
}

fn append_spaces(job: &mut LayoutJob, spaces: usize, format: TextFormat) {
    const SPACES: &str = "                                ";

    if spaces <= SPACES.len() {
        job.append(&SPACES[..spaces], 0.0, format);
    } else {
        let repeated = " ".repeat(spaces);
        job.append(&repeated, 0.0, format);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_TAB_WIDTH, advance_highlight_state, append_spaces, expanded_text_capacity,
        highlighted_job, plain_job, text_columns_without_tabs,
    };
    use egui::{TextFormat, text::LayoutJob};
    use syntect::{
        highlighting::{HighlightState, Highlighter, ThemeSet},
        parsing::{ParseState, ScopeStack, SyntaxSet},
    };

    #[test]
    fn plain_layout_expands_tabs_to_configured_tab_stops() {
        assert_eq!(plain_job("\ta", 13.0, 4).text, "    a");
        assert_eq!(plain_job("ab\tc", 13.0, 4).text, "ab  c");
    }

    #[test]
    fn tab_expansion_appends_contiguous_non_tab_runs() {
        let job = plain_job("a\tbc\t🙂", 13.0, 4);

        assert_eq!(job.text, "a   bc  🙂");
        assert_eq!(job.sections.len(), 5);
        assert_eq!(job.sections[0].byte_range, 0..1);
        assert_eq!(job.sections[1].byte_range, 1..4);
        assert_eq!(job.sections[2].byte_range, 4..6);
        assert_eq!(job.sections[3].byte_range, 6..8);
        assert_eq!(job.sections[4].byte_range, 8..12);
    }

    #[test]
    fn text_columns_without_tabs_uses_byte_len_only_for_ascii() {
        assert_eq!(text_columns_without_tabs("abc"), 3);
        assert_eq!(text_columns_without_tabs("🙂bc"), 3);
    }

    #[test]
    fn append_spaces_handles_common_and_large_tab_widths() {
        let mut common = LayoutJob::default();
        append_spaces(&mut common, 4, TextFormat::default());
        assert_eq!(common.text, "    ");

        let mut large = LayoutJob::default();
        append_spaces(&mut large, 40, TextFormat::default());
        assert_eq!(large.text.len(), 40);
        assert!(large.text.chars().all(|ch| ch == ' '));
    }

    #[test]
    fn expanded_text_capacity_accounts_for_tab_expansion() {
        assert_eq!(expanded_text_capacity("abc", 4), 3);
        assert_eq!(expanded_text_capacity("\ta", 4), 5);
        assert_eq!(expanded_text_capacity("ab\tc", 4), 5);
        assert_eq!(expanded_text_capacity("a\tbc\t\u{1f642}", 4), 12);
    }

    #[test]
    fn tabbed_plain_jobs_preallocate_expanded_text_capacity() {
        let job = plain_job("a\tbc\t\u{1f642}", 13.0, 4);

        assert_eq!(job.text, "a   bc  \u{1f642}");
        assert_eq!(job.text.capacity(), job.text.len());
    }

    #[test]
    fn layout_jobs_use_safe_defaults_for_invalid_inputs() {
        assert_eq!(plain_job("\ta", f32::NAN, 0).text, "    a");
        assert_eq!(
            plain_job("\ta", 13.0, usize::MAX).text.len(),
            MAX_TAB_WIDTH + 1
        );
    }

    #[test]
    fn advance_highlight_state_matches_highlighted_job_for_next_line() {
        let syntaxes = SyntaxSet::load_defaults_newlines();
        let syntax = syntaxes.find_syntax_by_extension("rs").unwrap();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set
            .themes
            .get("base16-ocean.dark")
            .or_else(|| theme_set.themes.values().next())
            .unwrap();
        let highlighter = Highlighter::new(theme);
        let first_line = "/* comment";
        let next_line = "still comment */";

        let mut job_parse_state = ParseState::new(syntax);
        let mut job_highlight_state = HighlightState::new(&highlighter, ScopeStack::new());
        let first_ops = job_parse_state.parse_line(first_line, &syntaxes).unwrap();
        let _ = highlighted_job(
            first_line,
            &first_ops,
            &mut job_highlight_state,
            &highlighter,
            13.0,
            4,
        );
        let next_ops = job_parse_state.parse_line(next_line, &syntaxes).unwrap();
        let after_job_replay = highlighted_job(
            next_line,
            &next_ops,
            &mut job_highlight_state,
            &highlighter,
            13.0,
            4,
        );

        let mut advance_parse_state = ParseState::new(syntax);
        let mut advance_highlight_state_value =
            HighlightState::new(&highlighter, ScopeStack::new());
        let first_ops = advance_parse_state
            .parse_line(first_line, &syntaxes)
            .unwrap();
        advance_highlight_state(
            first_line,
            &first_ops,
            &mut advance_highlight_state_value,
            &highlighter,
        );
        let next_ops = advance_parse_state
            .parse_line(next_line, &syntaxes)
            .unwrap();
        let after_state_advance = highlighted_job(
            next_line,
            &next_ops,
            &mut advance_highlight_state_value,
            &highlighter,
            13.0,
            4,
        );

        let job_replay_sections = after_job_replay
            .sections
            .iter()
            .map(|section| (section.byte_range.clone(), section.format.color))
            .collect::<Vec<_>>();
        let state_advance_sections = after_state_advance
            .sections
            .iter()
            .map(|section| (section.byte_range.clone(), section.format.color))
            .collect::<Vec<_>>();

        assert_eq!(after_job_replay.text, after_state_advance.text);
        assert_eq!(job_replay_sections, state_advance_sections);
    }
}
