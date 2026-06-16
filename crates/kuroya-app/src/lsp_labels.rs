use kuroya_core::DiagnosticSeverity;

const MAX_DIAGNOSTIC_MESSAGE_SUMMARY_CHARS: usize = 160;

pub(crate) fn symbol_kind_label(kind: u8) -> &'static str {
    match kind {
        1 => "file",
        2 => "mod",
        3 => "ns",
        4 => "pkg",
        5 => "class",
        6 => "method",
        7 => "prop",
        8 => "field",
        9 => "ctor",
        10 => "enum",
        11 => "iface",
        12 => "fn",
        13 => "var",
        14 => "const",
        15 => "str",
        16 => "num",
        17 => "bool",
        18 => "array",
        19 => "object",
        20 => "key",
        21 => "null",
        22 => "member",
        23 => "struct",
        24 => "event",
        25 => "op",
        26 => "type",
        _ => "sym",
    }
}

pub(crate) fn completion_kind_label(kind: u8) -> &'static str {
    match kind {
        1 => "text",
        2 => "method",
        3 => "fn",
        4 => "ctor",
        5 => "field",
        6 => "var",
        7 => "class",
        8 => "iface",
        9 => "mod",
        10 => "prop",
        11 => "unit",
        12 => "value",
        13 => "enum",
        14 => "kw",
        15 => "snippet",
        16 => "color",
        17 => "file",
        18 => "ref",
        19 => "folder",
        20 => "member",
        21 => "const",
        22 => "struct",
        23 => "event",
        24 => "op",
        25 => "type",
        _ => "item",
    }
}

pub(crate) fn severity_label(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Info => "info",
        DiagnosticSeverity::Hint => "hint",
    }
}

pub(crate) fn diagnostic_priority(severity: DiagnosticSeverity) -> u8 {
    match severity {
        DiagnosticSeverity::Error => 0,
        DiagnosticSeverity::Warning => 1,
        DiagnosticSeverity::Info => 2,
        DiagnosticSeverity::Hint => 3,
    }
}

pub(crate) fn diagnostic_message_summary(message: &str) -> String {
    let limit = MAX_DIAGNOSTIC_MESSAGE_SUMMARY_CHARS.saturating_sub(3);
    let mut output = String::with_capacity(message.len().min(MAX_DIAGNOSTIC_MESSAGE_SUMMARY_CHARS));
    let mut output_chars = 0usize;
    let mut pending_space = false;
    let mut truncated = false;

    for ch in message.chars() {
        if is_diagnostic_message_bidi_format_control(ch) {
            continue;
        }

        if ch.is_whitespace() || ch.is_control() {
            if output_chars > 0 {
                pending_space = true;
            }
            continue;
        }

        if pending_space {
            if output_chars >= limit {
                truncated = true;
                break;
            }
            output.push(' ');
            output_chars += 1;
            pending_space = false;
        }

        if output_chars >= limit {
            truncated = true;
            break;
        }
        output.push(ch);
        output_chars += 1;
    }

    if truncated {
        output.push_str("...");
    }
    output
}

fn is_diagnostic_message_bidi_format_control(ch: char) -> bool {
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
    use super::{MAX_DIAGNOSTIC_MESSAGE_SUMMARY_CHARS, diagnostic_message_summary};

    #[test]
    fn diagnostic_message_summary_collapses_whitespace_and_controls() {
        assert_eq!(
            diagnostic_message_summary(
                "\n  unresolved\tname\u{7}\u{202e}\ntry importing it\u{2066}  "
            ),
            "unresolved name try importing it"
        );
        assert_eq!(diagnostic_message_summary("  \n\t  "), "");
    }

    #[test]
    fn diagnostic_message_summary_caps_display_length() {
        let summary = diagnostic_message_summary(&"a".repeat(240));

        assert_eq!(
            summary.chars().count(),
            MAX_DIAGNOSTIC_MESSAGE_SUMMARY_CHARS
        );
        assert!(summary.ends_with("..."));
    }
}
