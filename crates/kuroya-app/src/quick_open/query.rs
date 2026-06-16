pub(crate) const MAX_QUICK_OPEN_QUERY_PATTERN_CHARS: usize = 256;
pub(crate) const MAX_QUICK_OPEN_QUERY_MEMORY_CHARS: usize = 128;
const QUICK_OPEN_QUERY_MEMORY_MIN_CHARS: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuickOpenQuery {
    pub(crate) pattern: String,
    pub(crate) line: Option<usize>,
    pub(crate) column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuickOpenMatchQuery {
    pub(super) raw: String,
    pub(super) lowercase: String,
    pub(super) tokens: Vec<String>,
    pub(super) token_lowercases: Vec<String>,
    pub(super) normalized_memory_query: Option<String>,
}

impl QuickOpenMatchQuery {
    #[cfg(test)]
    pub(crate) fn new(query: &str) -> Self {
        Self::from_sanitized_query(sanitize_quick_open_query_input(query))
    }

    pub(crate) fn from_sanitized_query(raw: String) -> Self {
        let mut split_tokens = raw.split_whitespace().filter(|token| !token.is_empty());
        let first_token = split_tokens.next();
        let second_token = split_tokens.next();
        let (tokens, token_lowercases, lowercase) =
            if let (Some(first_token), Some(second_token)) = (first_token, second_token) {
                let mut tokens = Vec::with_capacity(split_tokens.size_hint().0.saturating_add(2));
                tokens.push(first_token.to_owned());
                tokens.push(second_token.to_owned());
                tokens.extend(split_tokens.map(ToOwned::to_owned));
                let token_lowercases = tokens
                    .iter()
                    .map(|token| quick_open_lowercase(token))
                    .collect();
                (tokens, token_lowercases, String::new())
            } else {
                (Vec::new(), Vec::new(), quick_open_lowercase(&raw))
            };
        let normalized_memory_query = normalize_quick_open_memory_query_sanitized(&raw);

        Self {
            raw,
            lowercase,
            tokens,
            token_lowercases,
            normalized_memory_query,
        }
    }
}

pub(super) fn quick_open_lowercase(text: &str) -> String {
    if text.is_ascii() {
        text.to_ascii_lowercase()
    } else {
        text.chars().flat_map(char::to_lowercase).collect()
    }
}

pub(crate) fn parse_line_column(input: &str) -> Option<(usize, usize)> {
    let input = sanitize_quick_open_query_input(input);
    let mut parts = input
        .trim()
        .split(|ch: char| ch == ':' || ch == ',' || ch.is_whitespace())
        .filter(|part| !part.is_empty());
    let line = parts.next()?.parse::<usize>().ok()?;
    let column = parts
        .next()
        .map(|part| part.parse::<usize>().ok())
        .unwrap_or(Some(1))?;
    if parts.next().is_some() {
        return None;
    }
    (line > 0 && column > 0).then_some((line, column))
}

pub(crate) fn parse_quick_open_query(input: &str) -> QuickOpenQuery {
    let input = sanitize_quick_open_query_input(input);
    let trimmed = input.trim();
    let (pattern, line, column) = if let Some((pattern, column)) = split_numeric_suffix(trimmed) {
        if let Some((pattern, line)) = split_numeric_suffix(pattern) {
            if line > 0 && column > 0 {
                (pattern, Some(line), column)
            } else {
                (trimmed, None, 1)
            }
        } else if column > 0 {
            (pattern, Some(column), 1)
        } else {
            (trimmed, None, 1)
        }
    } else {
        (trimmed, None, 1)
    };

    QuickOpenQuery {
        pattern: sanitize_quick_open_query(pattern, MAX_QUICK_OPEN_QUERY_PATTERN_CHARS),
        line,
        column,
    }
}

fn split_numeric_suffix(input: &str) -> Option<(&str, usize)> {
    let input = input.trim_end();
    let separator = input.rfind([':', ','])?;
    let suffix = input[separator + 1..].trim();
    if suffix.is_empty() || !suffix.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let value = suffix.parse::<usize>().ok()?;
    Some((input[..separator].trim_end(), value))
}

pub(crate) fn normalize_quick_open_memory_query(query: &str) -> Option<String> {
    let query = sanitize_quick_open_query_input(query);
    normalize_quick_open_memory_query_sanitized(&query)
}

fn normalize_quick_open_memory_query_sanitized(query: &str) -> Option<String> {
    let query = query.trim();
    if query.chars().count() < QUICK_OPEN_QUERY_MEMORY_MIN_CHARS {
        return None;
    }

    let query = sanitize_quick_open_query(query, MAX_QUICK_OPEN_QUERY_MEMORY_CHARS);
    if query.chars().count() < QUICK_OPEN_QUERY_MEMORY_MIN_CHARS {
        None
    } else {
        Some(quick_open_lowercase(&query))
    }
}

pub(crate) fn sanitize_quick_open_query_input(input: &str) -> String {
    sanitize_quick_open_query(input, MAX_QUICK_OPEN_QUERY_PATTERN_CHARS)
}

fn sanitize_quick_open_query(input: &str, max_chars: usize) -> String {
    let mut output = String::with_capacity(input.len().min(max_chars.saturating_mul(4)));
    let mut chars = 0usize;
    let mut pending_space = false;

    for ch in input.chars() {
        if is_quick_open_format_control(ch) {
            continue;
        }

        if ch.is_whitespace() {
            pending_space = chars > 0;
            continue;
        }
        if ch.is_control() {
            continue;
        }

        if pending_space && !output.ends_with(' ') {
            if chars >= max_chars {
                break;
            }
            output.push(' ');
            chars += 1;
        }
        pending_space = false;

        if chars >= max_chars {
            break;
        }
        output.push(ch);
        chars += 1;
    }

    output.trim().to_owned()
}

fn is_quick_open_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
            | '\u{feff}'
    )
}
