pub(crate) fn count_label(count: usize, singular: &str, plural: &str) -> String {
    let noun = if count == 1 { singular } else { plural };
    let mut label = String::with_capacity(decimal_digit_count(count) + 1 + noun.len());
    push_usize_decimal(&mut label, count);
    label.push(' ');
    label.push_str(noun);
    label
}

fn decimal_digit_count(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn push_usize_decimal(output: &mut String, mut value: usize) {
    const MAX_USIZE_DECIMAL_DIGITS: usize = 39;

    let mut digits = [0_u8; MAX_USIZE_DECIMAL_DIGITS];
    let mut digit_count = 0;
    loop {
        digits[digit_count] = b'0' + (value % 10) as u8;
        digit_count += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }

    for digit in digits[..digit_count].iter().rev() {
        output.push(*digit as char);
    }
}

pub(crate) fn truncate_middle(value: &str, max_chars: usize) -> String {
    if let Some(sanitized) = without_hidden_format_controls(value) {
        return truncate_middle_raw(&sanitized, max_chars);
    }

    truncate_middle_raw(value, max_chars)
}

fn truncate_middle_raw(value: &str, max_chars: usize) -> String {
    if value.len() <= max_chars {
        return value.to_owned();
    }

    if value.is_ascii() {
        if max_chars <= 3 {
            return tiny_ellipsis(max_chars);
        }
        let keep = max_chars.saturating_sub(3);
        let head = keep / 2;
        let tail = keep.saturating_sub(head);
        let tail_start = value.len().saturating_sub(tail);
        let mut output = String::with_capacity(max_chars.min(value.len()));
        output.push_str(&value[..head]);
        output.push_str("...");
        output.push_str(&value[tail_start..]);
        return output;
    }

    if max_chars <= 3 {
        if value.char_indices().nth(max_chars).is_none() {
            return value.to_owned();
        }
        return tiny_ellipsis(max_chars);
    }
    let keep = max_chars.saturating_sub(3);
    let head = keep / 2;
    let tail = keep.saturating_sub(head);
    let mut head_end = 0;
    let mut exceeds_max = false;
    for (char_index, (byte_index, _)) in value.char_indices().enumerate() {
        if char_index == head {
            head_end = byte_index;
        }
        if char_index == max_chars {
            exceeds_max = true;
            break;
        }
    }
    if !exceeds_max {
        return value.to_owned();
    }
    let tail_start = value
        .char_indices()
        .rev()
        .nth(tail.saturating_sub(1))
        .map_or(value.len(), |(index, _)| index);
    let mut output = String::with_capacity(
        head_end
            .saturating_add(3)
            .saturating_add(value.len().saturating_sub(tail_start)),
    );
    output.push_str(&value[..head_end]);
    output.push_str("...");
    output.push_str(&value[tail_start..]);
    output
}

fn without_hidden_format_controls(value: &str) -> Option<String> {
    let (first_hidden_index, first_hidden) = value
        .char_indices()
        .find(|(_, ch)| is_hidden_format_control(*ch))?;
    let mut sanitized = String::with_capacity(value.len());
    sanitized.push_str(&value[..first_hidden_index]);

    let remaining_start = first_hidden_index + first_hidden.len_utf8();
    for ch in value[remaining_start..].chars() {
        if !is_hidden_format_control(ch) {
            sanitized.push(ch);
        }
    }
    Some(sanitized)
}

fn is_hidden_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
            | '\u{feff}'
    )
}

fn tiny_ellipsis(max_chars: usize) -> String {
    match max_chars {
        0 => String::new(),
        1 => ".".to_owned(),
        2 => "..".to_owned(),
        _ => "...".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{count_label, truncate_middle, without_hidden_format_controls};

    #[test]
    fn count_label_uses_singular_only_for_one() {
        assert_eq!(count_label(0, "file", "files"), "0 files");
        assert_eq!(count_label(1, "file", "files"), "1 file");
        assert_eq!(count_label(2, "file", "files"), "2 files");
        assert_eq!(count_label(1_000, "file", "files"), "1000 files");
    }

    #[test]
    fn truncate_middle_preserves_short_values_and_middle_ellipsizes_long_values() {
        assert_eq!(truncate_middle("short.rs", 16), "short.rs");
        assert_eq!(
            truncate_middle("abcdefghijklmnopqrstuvwxyz", 12),
            "abcd...vwxyz"
        );
        assert_eq!(truncate_middle("é.rs", 4), "é.rs");
        assert_eq!(truncate_middle("αβγδεζηθικ", 8), "αβ...θικ");
    }

    #[test]
    fn truncate_middle_respects_tiny_display_budgets() {
        assert_eq!(truncate_middle("abcdef", 0), "");
        assert_eq!(truncate_middle("abcdef", 1), ".");
        assert_eq!(truncate_middle("abcdef", 2), "..");
        assert_eq!(truncate_middle("abcdef", 3), "...");
    }

    #[test]
    fn truncate_middle_strips_hidden_format_controls_before_bounding() {
        let value = "alpha\u{202e}beta\u{200b}gamma";

        assert_eq!(truncate_middle(value, 16), "alphabetagamma");
        assert_eq!(truncate_middle(value, 9), "alp...mma");
        assert_eq!(truncate_middle("\u{202e}\u{200b}", 8), "");
    }

    #[test]
    fn hidden_format_control_stripping_allocates_only_when_needed() {
        assert_eq!(without_hidden_format_controls("plain label"), None);
        assert_eq!(
            without_hidden_format_controls("alpha\u{202e}beta\u{200b}gamma").as_deref(),
            Some("alphabetagamma")
        );
        assert_eq!(
            without_hidden_format_controls("\u{202e}\u{200b}").as_deref(),
            Some("")
        );
    }
}
