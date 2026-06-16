#[derive(Debug)]
pub(crate) struct DecodedText {
    pub(crate) text: String,
    pub(crate) lossy: bool,
    pub(crate) binary: bool,
}

pub(crate) const PROTECTED_PREVIEW_MAX_BYTES: usize =
    crate::large_file_mode::LARGE_FILE_MODE_MAX_BYTES;

pub(crate) fn decode_text_bytes(bytes: Vec<u8>) -> DecodedText {
    decode_text_bytes_with_protected_preview_limit(bytes, PROTECTED_PREVIEW_MAX_BYTES)
}

fn decode_text_bytes_with_protected_preview_limit(
    bytes: Vec<u8>,
    protected_preview_max_bytes: usize,
) -> DecodedText {
    let binary = bytes.contains(&0);
    match String::from_utf8(bytes) {
        Ok(text) => DecodedText {
            text: if binary {
                truncate_protected_preview_text(text, protected_preview_max_bytes, false, binary)
            } else {
                text
            },
            lossy: false,
            binary,
        },
        Err(error) => {
            let first_error = error.utf8_error();
            DecodedText {
                text: decode_lossy_protected_preview_text(
                    &error.into_bytes(),
                    protected_preview_max_bytes,
                    binary,
                    first_error,
                ),
                lossy: true,
                binary,
            }
        }
    }
}

fn truncate_protected_preview_text(
    text: String,
    max_bytes: usize,
    lossy: bool,
    binary: bool,
) -> String {
    if text.len() <= max_bytes {
        return text;
    }

    truncated_valid_utf8_preview_text(&text, max_bytes, lossy, binary)
}

fn truncated_valid_utf8_preview_text(
    text: &str,
    max_bytes: usize,
    lossy: bool,
    binary: bool,
) -> String {
    let notice = protected_preview_truncation_notice(lossy, binary);
    let content_limit = max_bytes.saturating_sub(notice.len());
    let content_end = floor_char_boundary(text, content_limit);
    let mut preview = String::with_capacity(max_bytes.min(text.len()));
    preview.push_str(&text[..content_end]);
    push_str_bounded(&mut preview, notice, max_bytes);
    preview
}

fn decode_lossy_protected_preview_text(
    bytes: &[u8],
    max_bytes: usize,
    binary: bool,
    first_error: std::str::Utf8Error,
) -> String {
    let notice = protected_preview_truncation_notice(true, binary);
    let content_limit = max_bytes.saturating_sub(notice.len());
    let (mut text, consumed_all) =
        lossy_utf8_prefix_by_output_bytes_from_error(bytes, content_limit, first_error);
    if !consumed_all {
        push_str_bounded(&mut text, notice, max_bytes);
    }
    text
}

#[cfg(test)]
fn lossy_utf8_prefix_by_output_bytes(bytes: &[u8], max_output_bytes: usize) -> (String, bool) {
    match std::str::from_utf8(bytes) {
        Ok(valid) => {
            let mut text = String::with_capacity(max_output_bytes.min(valid.len()));
            let consumed_all = push_str_bounded(&mut text, valid, max_output_bytes);
            (text, consumed_all)
        }
        Err(error) => lossy_utf8_prefix_by_output_bytes_from_error(bytes, max_output_bytes, error),
    }
}

fn lossy_utf8_prefix_by_output_bytes_from_error(
    bytes: &[u8],
    max_output_bytes: usize,
    first_error: std::str::Utf8Error,
) -> (String, bool) {
    let mut text = String::with_capacity(max_output_bytes.min(bytes.len()));
    let mut remaining = bytes;
    let mut next_error = Some(first_error);
    while !remaining.is_empty() {
        let decoded = if let Some(error) = next_error.take() {
            Err(error)
        } else {
            std::str::from_utf8(remaining)
        };
        match decoded {
            Ok(valid) => {
                let consumed_all = push_str_bounded(&mut text, valid, max_output_bytes);
                return (text, consumed_all);
            }
            Err(error) => {
                let valid_up_to = error.valid_up_to();
                if valid_up_to > 0 {
                    let valid_prefix = remaining.get(..valid_up_to).unwrap_or(remaining);
                    let valid = String::from_utf8_lossy(valid_prefix);
                    if !push_str_bounded(&mut text, valid.as_ref(), max_output_bytes) {
                        return (text, false);
                    }
                }

                if text.len().saturating_add('\u{fffd}'.len_utf8()) > max_output_bytes {
                    return (text, false);
                }
                text.push('\u{fffd}');

                let invalid_len = error
                    .error_len()
                    .unwrap_or_else(|| remaining.len().saturating_sub(valid_up_to));
                let next_index = valid_up_to.saturating_add(invalid_len).min(remaining.len());
                remaining = remaining.get(next_index..).unwrap_or_default();
            }
        }
    }

    (text, true)
}

fn protected_preview_truncation_notice(lossy: bool, binary: bool) -> &'static str {
    match (binary, lossy) {
        (true, true) => {
            "\n\n[Preview truncated: binary file contains invalid UTF-8 and is read-only]\n"
        }
        (true, false) => "\n\n[Preview truncated: binary file is read-only]\n",
        (false, true) => "\n\n[Preview truncated: invalid UTF-8 file is read-only]\n",
        (false, false) => "\n\n[Preview truncated]\n",
    }
}

fn push_str_bounded(output: &mut String, text: &str, max_output_bytes: usize) -> bool {
    let remaining = max_output_bytes.saturating_sub(output.len());
    if text.len() <= remaining {
        output.push_str(text);
        true
    } else {
        output.push_str(&text[..floor_char_boundary(text, remaining)]);
        false
    }
}

fn floor_char_boundary(text: &str, byte_limit: usize) -> usize {
    let mut end = byte_limit.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::{
        decode_text_bytes_with_protected_preview_limit, lossy_utf8_prefix_by_output_bytes,
        protected_preview_truncation_notice, truncated_valid_utf8_preview_text,
    };

    #[test]
    fn decode_keeps_unprotected_valid_text_unbounded() {
        let decoded = decode_text_bytes_with_protected_preview_limit(b"abcdef".to_vec(), 4);

        assert_eq!(decoded.text, "abcdef");
        assert!(!decoded.lossy);
        assert!(!decoded.binary);
    }

    #[test]
    fn decode_caps_binary_preview_text() {
        let notice = protected_preview_truncation_notice(false, true);
        let bytes = format!("abc\0d{}", "x".repeat(notice.len() + 10)).into_bytes();
        let decoded = decode_text_bytes_with_protected_preview_limit(bytes, notice.len() + 5);

        assert_eq!(decoded.text.len(), notice.len() + 5);
        assert!(decoded.text.starts_with("abc\0d"));
        assert!(decoded.text.ends_with(notice));
        assert!(!decoded.lossy);
        assert!(decoded.binary);
    }

    #[test]
    fn decode_caps_lossy_preview_by_output_bytes() {
        let notice = protected_preview_truncation_notice(true, false);
        let decoded =
            decode_text_bytes_with_protected_preview_limit(vec![0xff; 16], notice.len() + 9);

        assert_eq!(decoded.text.len(), notice.len() + 9);
        assert_eq!(decoded.text.matches('\u{fffd}').count(), 3);
        assert!(decoded.text.ends_with(notice));
        assert!(decoded.lossy);
        assert!(!decoded.binary);
    }

    #[test]
    fn decode_caps_lossy_preview_with_late_invalid_byte() {
        let notice = protected_preview_truncation_notice(true, false);
        let mut bytes = "x".repeat(notice.len() + 32).into_bytes();
        bytes.push(0xff);
        bytes.extend_from_slice(b"TAIL");

        let decoded = decode_text_bytes_with_protected_preview_limit(bytes, notice.len() + 5);

        assert_eq!(decoded.text.len(), notice.len() + 5);
        assert!(decoded.text.starts_with("xxxxx"));
        assert!(decoded.text.ends_with(notice));
        assert!(!decoded.text.contains("TAIL"));
        assert!(decoded.lossy);
        assert!(!decoded.binary);
    }

    #[test]
    fn lossy_utf8_prefix_decodes_mixed_valid_and_invalid_at_byte_limits() {
        let bytes = b"ab\xc3\xa9\xff\xc3\xa7d";

        let (full_text, consumed_all) = lossy_utf8_prefix_by_output_bytes(bytes, 10);
        assert_eq!(full_text, "ab\u{e9}\u{fffd}\u{e7}d");
        assert!(consumed_all);

        let (fits_replacement, consumed_all) = lossy_utf8_prefix_by_output_bytes(bytes, 7);
        assert_eq!(fits_replacement, "ab\u{e9}\u{fffd}");
        assert!(!consumed_all);

        let (before_replacement, consumed_all) = lossy_utf8_prefix_by_output_bytes(bytes, 6);
        assert_eq!(before_replacement, "ab\u{e9}");
        assert!(!consumed_all);
    }

    #[test]
    fn lossy_utf8_prefix_truncates_valid_text_at_char_boundaries() {
        let (text, consumed_all) = lossy_utf8_prefix_by_output_bytes("\u{e9}\u{e9}x".as_bytes(), 3);

        assert_eq!(text, "\u{e9}");
        assert!(!consumed_all);
        assert!(text.is_char_boundary(text.len()));
    }

    #[test]
    fn decode_caps_valid_binary_preview_at_utf8_boundary() {
        let notice = protected_preview_truncation_notice(false, true);
        let bytes = format!("\0ab\u{e9}{}", "z".repeat(notice.len() + 10)).into_bytes();
        let decoded = decode_text_bytes_with_protected_preview_limit(bytes, notice.len() + 4);

        assert!(decoded.text.starts_with("\0ab"));
        assert!(!decoded.text.contains('\u{fffd}'));
        assert!(decoded.text.ends_with(notice));
        assert!(!decoded.lossy);
        assert!(decoded.binary);
    }

    #[test]
    fn truncated_valid_utf8_preview_drops_suffix_beyond_preview_limit() {
        let notice = protected_preview_truncation_notice(false, true);
        let text = format!("\0ab{}TAIL", "x".repeat(notice.len() + 16));
        let preview = truncated_valid_utf8_preview_text(&text, notice.len() + 5, false, true);

        assert_eq!(preview.len(), notice.len() + 5);
        assert!(preview.starts_with("\0abxx"));
        assert!(preview.ends_with(notice));
        assert!(!preview.contains("TAIL"));
    }
}
