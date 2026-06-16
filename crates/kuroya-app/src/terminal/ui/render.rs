use super::{
    colors,
    colors::{
        terminal_background_color, terminal_bold_foreground_color, terminal_contrast_color,
        terminal_dim_foreground_color, terminal_foreground_color,
    },
    layout::TerminalRenderGrid,
};
use egui::{Color32, Pos2, Rect, pos2};
use std::borrow::Cow;

pub(in crate::terminal) fn terminal_rendered_text_color(
    foreground_color: vt100::Color,
    foreground: Color32,
    background: Color32,
    bold: bool,
    dim: bool,
    draw_bold_text_in_bright_colors: bool,
    minimum_contrast_ratio: f32,
    ansi_palette: &colors::TerminalAnsiPalette,
) -> Color32 {
    let mut text_color = if bold {
        terminal_bold_foreground_color(
            foreground_color,
            foreground,
            draw_bold_text_in_bright_colors,
            ansi_palette,
        )
    } else {
        foreground
    };
    if dim {
        text_color = terminal_dim_foreground_color(text_color, background);
    }
    terminal_contrast_color(text_color, background, minimum_contrast_ratio)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TerminalRenderBaseColorKey {
    foreground_color: vt100::Color,
    background_color: vt100::Color,
    inverse: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::terminal) struct TerminalRenderBaseColors {
    pub(in crate::terminal) foreground: Color32,
    pub(in crate::terminal) background: Color32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TerminalRenderTextColorKey {
    foreground_color: vt100::Color,
    foreground: Color32,
    background: Color32,
    bold: bool,
    dim: bool,
}

pub(super) struct TerminalRenderColorCache<'a> {
    default_text: Color32,
    terminal_background: Color32,
    draw_bold_text_in_bright_colors: bool,
    minimum_contrast_ratio: f32,
    ansi_palette: &'a colors::TerminalAnsiPalette,
    last_base: Option<(TerminalRenderBaseColorKey, TerminalRenderBaseColors)>,
    last_text: Option<(TerminalRenderTextColorKey, Color32)>,
}

impl<'a> TerminalRenderColorCache<'a> {
    pub(super) fn new(
        default_text: Color32,
        terminal_background: Color32,
        draw_bold_text_in_bright_colors: bool,
        minimum_contrast_ratio: f32,
        ansi_palette: &'a colors::TerminalAnsiPalette,
    ) -> Self {
        Self {
            default_text,
            terminal_background,
            draw_bold_text_in_bright_colors,
            minimum_contrast_ratio,
            ansi_palette,
            last_base: None,
            last_text: None,
        }
    }

    pub(super) fn base_colors(
        &mut self,
        foreground_color: vt100::Color,
        background_color: vt100::Color,
        inverse: bool,
    ) -> TerminalRenderBaseColors {
        let key = TerminalRenderBaseColorKey {
            foreground_color,
            background_color,
            inverse,
        };
        if let Some((cached_key, colors)) = self.last_base
            && cached_key == key
        {
            return colors;
        }

        let mut foreground =
            terminal_foreground_color(foreground_color, self.default_text, self.ansi_palette);
        let mut background = terminal_background_color(
            background_color,
            self.terminal_background,
            self.ansi_palette,
        );
        if inverse {
            std::mem::swap(&mut foreground, &mut background);
        }

        let colors = TerminalRenderBaseColors {
            foreground,
            background,
        };
        self.last_base = Some((key, colors));
        colors
    }

    pub(super) fn text_color(
        &mut self,
        foreground_color: vt100::Color,
        foreground: Color32,
        background: Color32,
        bold: bool,
        dim: bool,
    ) -> Color32 {
        let key = TerminalRenderTextColorKey {
            foreground_color,
            foreground,
            background,
            bold,
            dim,
        };
        if let Some((cached_key, color)) = self.last_text
            && cached_key == key
        {
            return color;
        }

        let color = terminal_rendered_text_color(
            foreground_color,
            foreground,
            background,
            bold,
            dim,
            self.draw_bold_text_in_bright_colors,
            self.minimum_contrast_ratio,
            self.ansi_palette,
        );
        self.last_text = Some((key, color));
        color
    }
}

const TERMINAL_TEXT_RUN_MERGE_WIDTH_TOLERANCE: f32 = 0.5;

pub(in crate::terminal) fn terminal_text_runs_can_merge(
    cell_width: f32,
    measured_monospace_width: f32,
    letter_spacing: f32,
) -> bool {
    cell_width.is_finite()
        && measured_monospace_width.is_finite()
        && cell_width > 0.0
        && measured_monospace_width > 0.0
        && letter_spacing.abs() <= f32::EPSILON
        && (cell_width - measured_monospace_width).abs() <= TERMINAL_TEXT_RUN_MERGE_WIDTH_TOLERANCE
}

pub(super) fn terminal_cell_text_is_single_char(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some() && chars.next().is_none()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::terminal) struct TerminalRenderTextRun<'a> {
    pub(in crate::terminal) row: u16,
    pub(in crate::terminal) start_col: u16,
    pub(in crate::terminal) width_cols: u16,
    pub(in crate::terminal) text: Cow<'a, str>,
    pub(in crate::terminal) color: Color32,
    pub(in crate::terminal) underline: bool,
    mergeable: bool,
}

pub(in crate::terminal) fn push_terminal_text_run<'a>(
    runs: &mut Vec<TerminalRenderTextRun<'a>>,
    row: u16,
    start_col: u16,
    width_cols: u16,
    text: &'a str,
    color: Color32,
    underline: bool,
    mergeable: bool,
) {
    if text.is_empty() || width_cols == 0 {
        return;
    }

    if mergeable
        && let Some(last) = runs.last_mut()
        && last.mergeable
        && last.row == row
        && last.start_col.saturating_add(last.width_cols) == start_col
        && last.color == color
        && last.underline == underline
    {
        last.width_cols = last.width_cols.saturating_add(width_cols);
        last.text.to_mut().push_str(text);
        return;
    }

    runs.push(TerminalRenderTextRun {
        row,
        start_col,
        width_cols,
        text: Cow::Borrowed(text),
        color,
        underline,
        mergeable,
    });
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TerminalPreparedTextRun<'a> {
    pub(super) position: Pos2,
    pub(super) text: &'a str,
    pub(super) color: Color32,
    pub(super) underline: Option<(Pos2, Pos2)>,
}

pub(super) fn prepare_terminal_text_runs<'a, 'text>(
    runs: &'a [TerminalRenderTextRun<'text>],
    render_grid: Option<TerminalRenderGrid>,
) -> impl Iterator<Item = TerminalPreparedTextRun<'a>> + 'a {
    runs.iter()
        .filter_map(move |run| prepare_terminal_text_run(run, render_grid?))
}

fn prepare_terminal_text_run<'a, 'text>(
    run: &'a TerminalRenderTextRun<'text>,
    render_grid: TerminalRenderGrid,
) -> Option<TerminalPreparedTextRun<'a>> {
    let rect = render_grid.cell_rect(run.row, run.start_col, run.width_cols)?;
    Some(TerminalPreparedTextRun {
        position: rect.left_top(),
        text: run.text.as_ref(),
        color: run.color,
        underline: run.underline.then(|| {
            let y = terminal_text_underline_y(rect);
            (pos2(rect.left(), y), pos2(rect.right(), y))
        }),
    })
}

fn terminal_text_underline_y(rect: Rect) -> f32 {
    (rect.bottom() - 2.0).clamp(rect.top(), rect.bottom())
}

fn terminal_cell_char(screen: &vt100::Screen, row: u16, col: u16) -> Option<char> {
    let cell = screen.cell(row, col)?;
    (!cell.is_wide_continuation())
        .then(|| cell.contents().chars().next())
        .flatten()
}

pub(in crate::terminal) fn terminal_word_selection_at_cell(
    session: &super::super::TerminalSession,
    row: u16,
    col: u16,
    word_separators: &str,
) -> Option<super::super::TerminalTextSelection> {
    let screen = session.parser.screen();
    let (rows, cols) = screen.size();
    if row >= rows || col >= cols {
        return None;
    }

    let ch = terminal_cell_char(screen, row, col)?;
    if !is_terminal_word_char(ch, word_separators) {
        return None;
    }

    let mut start_col = col;
    while start_col > 0
        && terminal_cell_char(screen, row, start_col - 1)
            .is_some_and(|ch| is_terminal_word_char(ch, word_separators))
    {
        start_col -= 1;
    }

    let mut end_col = col + 1;
    while end_col < cols
        && terminal_cell_char(screen, row, end_col)
            .is_some_and(|ch| is_terminal_word_char(ch, word_separators))
    {
        end_col += 1;
    }

    let text = (start_col..end_col)
        .filter_map(|col| terminal_cell_char(screen, row, col))
        .collect::<String>();
    (!text.is_empty()).then_some(super::super::TerminalTextSelection {
        session_id: session.id,
        text,
        range: super::super::TerminalSelectionRange {
            start: super::super::TerminalCellPosition {
                row,
                col: start_col,
            },
            end: super::super::TerminalCellPosition { row, col: end_col },
        },
    })
}

pub(super) fn terminal_selection_contains_cell(
    selection: &super::super::TerminalTextSelection,
    row: u16,
    col: u16,
) -> bool {
    let range = selection.range;
    if row < range.start.row || row > range.end.row {
        return false;
    }

    let start_col = if row == range.start.row {
        range.start.col
    } else {
        0
    };
    let end_col = if row == range.end.row {
        range.end.col
    } else {
        u16::MAX
    };
    col >= start_col && col < end_col
}

fn is_terminal_word_char(ch: char, word_separators: &str) -> bool {
    !ch.is_whitespace() && !word_separators.contains(ch)
}
