use regex::Regex;

use super::{MAX_GIT_BRANCH_VALIDATION_ERROR_CHARS, trim_string_in_place};

pub fn git_branch_validation_error(branch_name: &str, validation_regex: &str) -> Option<String> {
    let pattern = validation_regex.trim();
    if pattern.is_empty() {
        return None;
    }

    match Regex::new(pattern) {
        Ok(regex) if regex.is_match(branch_name) => None,
        Ok(_) => Some("Branch name does not match git.branchValidationRegex".to_owned()),
        Err(error) => Some(format!(
            "Invalid git.branchValidationRegex: {}",
            git_branch_validation_error_detail(&error.to_string())
        )),
    }
}

fn git_branch_validation_error_detail(error: &str) -> String {
    bounded_git_branch_validation_text(
        error,
        MAX_GIT_BRANCH_VALIDATION_ERROR_CHARS,
        "invalid regex",
    )
}

fn bounded_git_branch_validation_text(text: &str, max_chars: usize, fallback: &str) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let mut output = String::new();
    let mut chars = 0usize;
    let mut truncated = false;
    let mut pending_space = false;

    for ch in text.chars() {
        if is_git_branch_validation_format_control(ch) {
            continue;
        }

        if ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}') {
            pending_space = chars > 0;
            continue;
        }

        if pending_space && !output.ends_with(' ') {
            if chars >= max_chars {
                truncated = true;
                break;
            }
            output.push(' ');
            chars += 1;
        }
        pending_space = false;

        if chars >= max_chars {
            truncated = true;
            break;
        }
        output.push(ch);
        chars += 1;
    }

    if truncated && max_chars > 3 {
        truncate_git_branch_validation_text(&mut output, max_chars - 3);
        output.push_str("...");
    }

    trim_string_in_place(&mut output);
    if output.is_empty() {
        fallback.to_owned()
    } else {
        output
    }
}

fn truncate_git_branch_validation_text(text: &mut String, max_chars: usize) {
    if let Some((byte_index, _)) = text.char_indices().nth(max_chars) {
        text.truncate(byte_index);
    }
}

fn is_git_branch_validation_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}
