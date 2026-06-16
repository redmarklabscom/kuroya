use super::MAX_LSP_TEXT_EDIT_NEW_TEXT_BYTES;

pub(super) fn bounded_lsp_text(text: &str, max_chars: usize) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    let mut output = String::with_capacity(bounded_lsp_text_capacity(text, max_chars));
    let mut chars = 0;
    push_bounded_lsp_text(&mut output, &mut chars, text, max_chars);
    (!output.is_empty()).then_some(output)
}

pub(super) fn bounded_lsp_markdown_text(text: &str, max_chars: usize) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    let mut output = String::with_capacity(bounded_lsp_text_capacity(text, max_chars));
    let mut chars = 0;
    push_bounded_lsp_markdown_text(&mut output, &mut chars, text, max_chars);

    let trimmed_len = output.trim_end().len();
    output.truncate(trimmed_len);
    (!output.trim().is_empty()).then_some(output)
}

pub(super) fn bounded_lsp_insert_text(text: &str) -> Option<String> {
    (text.len() <= MAX_LSP_TEXT_EDIT_NEW_TEXT_BYTES).then(|| text.to_owned())
}

pub(super) fn bounded_lsp_text_capacity(text: &str, max_chars: usize) -> usize {
    text.len().min(max_chars.saturating_mul(4))
}

pub(super) fn trim_lsp_text_in_place(mut text: String) -> Option<String> {
    let start = text
        .char_indices()
        .find_map(|(idx, ch)| (!ch.is_whitespace()).then_some(idx))?;
    let end = text
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| (!ch.is_whitespace()).then_some(idx + ch.len_utf8()))?;
    if end < text.len() {
        text.truncate(end);
    }
    if start > 0 {
        text.drain(..start);
    }
    Some(text)
}

pub(super) fn push_bounded_lsp_text(
    output: &mut String,
    chars: &mut usize,
    text: &str,
    max_chars: usize,
) {
    for ch in text.chars() {
        if *chars >= max_chars {
            break;
        }
        *chars += 1;
        if is_lsp_text_format_control(ch) {
            continue;
        }
        output.push(if ch.is_control() { ' ' } else { ch });
    }
}

pub(super) fn push_bounded_lsp_markdown_text(
    output: &mut String,
    chars: &mut usize,
    text: &str,
    max_chars: usize,
) {
    for ch in text.chars() {
        if *chars >= max_chars {
            break;
        }
        *chars += 1;
        if is_lsp_text_format_control(ch) {
            continue;
        }
        output.push(match ch {
            '\n' | '\r' | '\t' => ch,
            ch if ch.is_control() => ' ',
            ch => ch,
        });
    }
}

fn is_lsp_text_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{00ad}'
            | '\u{034f}'
            | '\u{061c}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
    )
}
