mod begin;
mod submit;

use std::fmt::Write as _;

const LSP_RENAME_MAX_TARGET_CHARS: usize = 256;
const LSP_RENAME_DISPLAY_LABEL_CHARS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LspRenameTargetError {
    Empty,
    ControlOnly,
    ContainsControl,
    TooLong,
}

pub(crate) fn lsp_rename_request_target(input: &str) -> Result<String, LspRenameTargetError> {
    let target = input.trim();
    if target.is_empty() {
        return Err(LspRenameTargetError::Empty);
    }

    let mut has_visible_text = false;
    let mut has_control = false;
    for (index, ch) in target.chars().enumerate() {
        if index >= LSP_RENAME_MAX_TARGET_CHARS {
            return Err(LspRenameTargetError::TooLong);
        }
        if ch.is_control() || is_lsp_rename_format_control(ch) {
            has_control = true;
        } else {
            has_visible_text = true;
        }
    }

    if !has_visible_text {
        Err(LspRenameTargetError::ControlOnly)
    } else if has_control {
        Err(LspRenameTargetError::ContainsControl)
    } else {
        Ok(target.to_owned())
    }
}

pub(crate) fn lsp_rename_prefill_target(input: &str) -> Option<String> {
    lsp_rename_request_target(input).ok()
}

pub(crate) fn lsp_rename_bound_input(input: &mut String) {
    let max_kept_chars = LSP_RENAME_MAX_TARGET_CHARS + 1;
    if let Some((byte_index, _)) = input.char_indices().nth(max_kept_chars) {
        input.truncate(byte_index);
    }
}

pub(crate) fn lsp_rename_target_error_status(error: LspRenameTargetError) -> String {
    match error {
        LspRenameTargetError::Empty => "Rename target is empty".to_owned(),
        LspRenameTargetError::ControlOnly => {
            "Rename target contains only control characters".to_owned()
        }
        LspRenameTargetError::ContainsControl => {
            "Rename target contains control characters".to_owned()
        }
        LspRenameTargetError::TooLong => {
            format!("Rename target is too long (max {LSP_RENAME_MAX_TARGET_CHARS} characters)")
        }
    }
}

pub(crate) fn lsp_rename_display_label(text: &str) -> String {
    lsp_rename_bounded_display_label(text, LSP_RENAME_DISPLAY_LABEL_CHARS)
}

pub(crate) fn lsp_rename_bounded_display_label(text: &str, max_chars: usize) -> String {
    let mut label = String::with_capacity(lsp_rename_display_label_capacity(text, max_chars));
    let mut chars = text.chars();
    for _ in 0..max_chars {
        let Some(ch) = chars.next() else {
            return label;
        };
        append_lsp_rename_display_char(&mut label, ch);
    }
    if chars.next().is_some() {
        label.push_str("...");
    }
    label
}

fn append_lsp_rename_display_char(label: &mut String, ch: char) {
    if is_lsp_rename_format_control(ch) {
        return;
    }
    match ch {
        '\n' => label.push_str("\\n"),
        '\r' => label.push_str("\\r"),
        '\t' => label.push_str("\\t"),
        ch if ch.is_control() => {
            label.push_str("\\u{");
            let _ = write!(label, "{:X}", ch as u32);
            label.push('}');
        }
        ch => label.push(ch),
    }
}

fn lsp_rename_display_label_capacity(text: &str, max_chars: usize) -> usize {
    text.len().min(max_chars.saturating_add(3))
}

fn is_lsp_rename_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

#[cfg(test)]
mod tests {
    use super::{
        LSP_RENAME_MAX_TARGET_CHARS, LspRenameTargetError, lsp_rename_bound_input,
        lsp_rename_bounded_display_label, lsp_rename_display_label, lsp_rename_request_target,
    };

    #[test]
    fn rename_request_target_trims_and_preserves_valid_text() {
        assert_eq!(
            lsp_rename_request_target("  renamed_symbol  "),
            Ok("renamed_symbol".to_owned())
        );

        let max_len_name = "a".repeat(LSP_RENAME_MAX_TARGET_CHARS);
        assert_eq!(lsp_rename_request_target(&max_len_name), Ok(max_len_name));
    }

    #[test]
    fn rename_request_target_rejects_empty_control_and_too_long_text() {
        assert_eq!(
            lsp_rename_request_target(" \t\r\n "),
            Err(LspRenameTargetError::Empty)
        );
        assert_eq!(
            lsp_rename_request_target("\u{0}\u{1f}"),
            Err(LspRenameTargetError::ControlOnly)
        );
        assert_eq!(
            lsp_rename_request_target("\u{202e}\u{2066}"),
            Err(LspRenameTargetError::ControlOnly)
        );
        assert_eq!(
            lsp_rename_request_target("new\nname"),
            Err(LspRenameTargetError::ContainsControl)
        );
        assert_eq!(
            lsp_rename_request_target("new\u{202e}name"),
            Err(LspRenameTargetError::ContainsControl)
        );
        assert_eq!(
            lsp_rename_request_target(&"a".repeat(LSP_RENAME_MAX_TARGET_CHARS + 1)),
            Err(LspRenameTargetError::TooLong)
        );
    }

    #[test]
    fn rename_bound_input_keeps_one_over_limit_for_submit_rejection() {
        let mut input = "a".repeat(LSP_RENAME_MAX_TARGET_CHARS + 20);

        lsp_rename_bound_input(&mut input);

        assert_eq!(input.chars().count(), LSP_RENAME_MAX_TARGET_CHARS + 1);
        assert_eq!(
            lsp_rename_request_target(&input),
            Err(LspRenameTargetError::TooLong)
        );
    }

    #[test]
    fn rename_display_label_escapes_controls_and_bounds_text() {
        assert_eq!(lsp_rename_display_label("renamed_symbol"), "renamed_symbol");
        assert_eq!(
            lsp_rename_display_label("line\nname\t\u{7}"),
            "line\\nname\\t\\u{7}"
        );
        assert_eq!(
            lsp_rename_display_label("line\u{202e}name\u{2066}"),
            "linename"
        );
        assert_eq!(
            lsp_rename_bounded_display_label("abcdefghijklmnopqrstuvwxyz", 8),
            "abcdefgh..."
        );
    }
}
