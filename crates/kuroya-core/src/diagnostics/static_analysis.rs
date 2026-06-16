use std::{
    ops::Range,
    path::{Path, PathBuf},
};

use super::{
    Diagnostic, DiagnosticSeverity, STATIC_DIAGNOSTIC_MAX_BRACKET_DEPTH,
    STATIC_DIAGNOSTIC_MAX_RESULTS, STATIC_DIAGNOSTIC_SCAN_MAX_BYTES, STATIC_DIAGNOSTIC_SOURCE,
};

#[derive(Clone, Copy)]
pub(super) enum CommonMarker {
    Todo,
    Fixme,
}

#[derive(Clone, Copy)]
pub(super) struct CommonMarkerMatch {
    pub(super) marker: CommonMarker,
    #[cfg(test)]
    pub(super) byte_index: usize,
    pub(super) column: usize,
}

impl CommonMarker {
    pub(super) fn text(self) -> &'static str {
        match self {
            CommonMarker::Todo => "TODO",
            CommonMarker::Fixme => "FIXME",
        }
    }

    fn severity(self) -> DiagnosticSeverity {
        match self {
            CommonMarker::Todo => DiagnosticSeverity::Info,
            CommonMarker::Fixme => DiagnosticSeverity::Warning,
        }
    }

    fn message(self) -> &'static str {
        match self {
            CommonMarker::Todo => "TODO marker",
            CommonMarker::Fixme => "FIXME marker",
        }
    }
}

pub(super) fn scan_common_markers(line: &str) -> [Option<CommonMarkerMatch>; 2] {
    let bytes = line.as_bytes();
    let mut matches = [None, None];
    let mut matched_count = 0;
    let mut matched_todo = false;
    let mut matched_fixme = false;
    let mut column = 1;

    for (byte_index, byte) in bytes.iter().copied().enumerate() {
        if utf8_continuation_byte(byte) {
            continue;
        }

        if !matched_todo
            && byte == b'T'
            && common_marker_matches_at(bytes, byte_index, CommonMarker::Todo)
        {
            matches[matched_count] = Some(CommonMarkerMatch {
                marker: CommonMarker::Todo,
                #[cfg(test)]
                byte_index,
                column,
            });
            matched_count += 1;
            matched_todo = true;
        }
        if !matched_fixme
            && byte == b'F'
            && common_marker_matches_at(bytes, byte_index, CommonMarker::Fixme)
        {
            matches[matched_count] = Some(CommonMarkerMatch {
                marker: CommonMarker::Fixme,
                #[cfg(test)]
                byte_index,
                column,
            });
            matched_count += 1;
            matched_fixme = true;
        }
        if matched_count == matches.len() {
            break;
        }
        column += 1;
    }

    matches
}

fn common_marker_matches_at(bytes: &[u8], byte_index: usize, marker: CommonMarker) -> bool {
    bytes[byte_index..].starts_with(marker.text().as_bytes())
}

fn utf8_continuation_byte(byte: u8) -> bool {
    byte & 0b1100_0000 == 0b1000_0000
}

pub fn static_diagnostics_scan_allowed(byte_len: usize) -> bool {
    byte_len <= STATIC_DIAGNOSTIC_SCAN_MAX_BYTES
}

pub fn analyze_text(path: PathBuf, text: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let diagnostic_capacity_hint = static_diagnostics_capacity_hint(text);
    let diagnostic_context = StaticDiagnosticContext {
        path: &path,
        capacity_hint: diagnostic_capacity_hint,
    };
    let mut char_offset = 0;
    let mut bracket_stack = Vec::new();
    let bracket_stack_capacity_hint = static_bracket_stack_capacity_hint(text);
    let mut scanned_bytes = 0usize;

    for (line_idx, raw_line) in text.lines().enumerate() {
        if !static_diagnostics_scan_allowed(scanned_bytes.saturating_add(raw_line.len()))
            || !static_diagnostics_has_capacity(&diagnostics)
        {
            break;
        }

        let line_number = line_idx + 1;
        let line = raw_line.trim_end_matches('\r');
        let removed_cr_chars = raw_line.len().saturating_sub(line.len());
        let mut line_chars = None;

        for marker_match in scan_common_markers(line).into_iter().flatten() {
            let marker = marker_match.marker;
            let column = marker_match.column;
            let marker_start = char_offset + column - 1;
            if !push_static_diagnostic(
                &mut diagnostics,
                diagnostic_context,
                line_number,
                column,
                marker_start..marker_start + marker.text().len(),
                marker.severity(),
                marker.message(),
            ) {
                break;
            }
        }
        if !static_diagnostics_has_capacity(&diagnostics) {
            break;
        }

        let trimmed = line.trim_end_matches([' ', '\t']);
        if trimmed.len() != line.len() {
            let column = trimmed.chars().count() + 1;
            let current_line_chars = *line_chars.get_or_insert_with(|| line.chars().count());
            if !push_static_diagnostic(
                &mut diagnostics,
                diagnostic_context,
                line_number,
                column,
                char_offset + column - 1..char_offset + current_line_chars,
                DiagnosticSeverity::Hint,
                "Trailing whitespace",
            ) {
                break;
            }
        }

        for (column_idx, ch) in line.chars().enumerate() {
            if !static_diagnostics_has_capacity(&diagnostics) {
                break;
            }
            match ch {
                '(' | '[' | '{' => {
                    if bracket_stack.len() < STATIC_DIAGNOSTIC_MAX_BRACKET_DEPTH {
                        reserve_static_bracket_stack_capacity(
                            &mut bracket_stack,
                            bracket_stack_capacity_hint,
                        );
                        bracket_stack.push((ch, line_number, column_idx + 1));
                    }
                }
                ')' | ']' | '}' => {
                    if let Some((open, _, _)) = bracket_stack.last().copied() {
                        if brackets_match(open, ch) {
                            bracket_stack.pop();
                        } else {
                            push_static_diagnostic(
                                &mut diagnostics,
                                diagnostic_context,
                                line_number,
                                column_idx + 1,
                                char_offset + column_idx..char_offset + column_idx + 1,
                                DiagnosticSeverity::Error,
                                "Mismatched closing bracket",
                            );
                        }
                    } else {
                        push_static_diagnostic(
                            &mut diagnostics,
                            diagnostic_context,
                            line_number,
                            column_idx + 1,
                            char_offset + column_idx..char_offset + column_idx + 1,
                            DiagnosticSeverity::Error,
                            "Unmatched closing bracket",
                        );
                    }
                }
                _ => {}
            }
        }
        if !static_diagnostics_has_capacity(&diagnostics) {
            break;
        }

        let current_line_chars = line_chars.unwrap_or_else(|| line.chars().count());
        char_offset += current_line_chars + removed_cr_chars + 1;
        scanned_bytes = scanned_bytes
            .saturating_add(raw_line.len())
            .saturating_add(1);
    }

    for (_, line, column) in bracket_stack {
        if !push_static_diagnostic(
            &mut diagnostics,
            diagnostic_context,
            line,
            column,
            0..0,
            DiagnosticSeverity::Error,
            "Unclosed bracket",
        ) {
            break;
        }
    }

    diagnostics
}

fn static_diagnostics_capacity_hint(text: &str) -> usize {
    text.len()
        .min(STATIC_DIAGNOSTIC_SCAN_MAX_BYTES)
        .min(STATIC_DIAGNOSTIC_MAX_RESULTS)
}

fn static_bracket_stack_capacity_hint(text: &str) -> usize {
    text.len()
        .min(STATIC_DIAGNOSTIC_SCAN_MAX_BYTES)
        .min(STATIC_DIAGNOSTIC_MAX_BRACKET_DEPTH)
}

#[derive(Clone, Copy)]
struct StaticDiagnosticContext<'a> {
    path: &'a Path,
    capacity_hint: usize,
}

fn reserve_static_bracket_stack_capacity(
    bracket_stack: &mut Vec<(char, usize, usize)>,
    capacity_hint: usize,
) {
    if bracket_stack.capacity() == 0 {
        bracket_stack.reserve_exact(capacity_hint);
    }
}

fn static_diagnostics_has_capacity(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.len() < STATIC_DIAGNOSTIC_MAX_RESULTS
}

fn push_static_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    context: StaticDiagnosticContext<'_>,
    line: usize,
    column: usize,
    char_range: Range<usize>,
    severity: DiagnosticSeverity,
    message: &str,
) -> bool {
    if !static_diagnostics_has_capacity(diagnostics) {
        return false;
    }

    if diagnostics.capacity() == 0 {
        diagnostics.reserve_exact(context.capacity_hint);
    }

    diagnostics.push(Diagnostic {
        path: context.path.to_path_buf(),
        line,
        column,
        char_range,
        severity,
        source: STATIC_DIAGNOSTIC_SOURCE.to_owned(),
        message: message.to_owned(),
        unused: false,
        deprecated: false,
    });
    static_diagnostics_has_capacity(diagnostics)
}

fn brackets_match(open: char, close: char) -> bool {
    matches!((open, close), ('(', ')') | ('[', ']') | ('{', '}'))
}
