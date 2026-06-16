use super::{TerminalCellPosition, TerminalPane, TerminalSession};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TerminalPathLink {
    pub(super) path: PathBuf,
    pub(super) line: usize,
    pub(super) column: usize,
}

impl TerminalPane {
    pub(super) fn terminal_path_link_at_cell(
        &self,
        index: usize,
        position: TerminalCellPosition,
    ) -> Option<TerminalPathLink> {
        let session = self.sessions.get(index)?;
        let (line, column) =
            terminal_screen_line_and_text_column(session, position.row, position.col)?;
        let base_dir = session.initial_cwd.as_deref().unwrap_or(&self.cwd);
        terminal_path_link_at_text_position(&line, column, base_dir)
    }
}

pub(super) fn terminal_path_link_at_text_position(
    text: &str,
    column: usize,
    workspace_root: &Path,
) -> Option<TerminalPathLink> {
    let mut home_dir = TerminalHomeDir::Lazy(None);
    terminal_path_link_at_text_position_with_home_dir(text, column, workspace_root, &mut home_dir)
}

#[cfg(test)]
pub(super) fn terminal_path_link_at_text_position_with_home(
    text: &str,
    column: usize,
    workspace_root: &Path,
    home_dir: Option<&Path>,
) -> Option<TerminalPathLink> {
    let mut home_dir = TerminalHomeDir::Known(home_dir);
    terminal_path_link_at_text_position_with_home_dir(text, column, workspace_root, &mut home_dir)
}

fn terminal_path_link_at_text_position_with_home_dir(
    text: &str,
    column: usize,
    workspace_root: &Path,
    home_dir: &mut TerminalHomeDir<'_>,
) -> Option<TerminalPathLink> {
    let token = terminal_link_token_at_column(text, column)?;
    let suffix = terminal_link_suffix_after_token(text, token.suffix_start_byte);
    terminal_path_link_from_token(
        token.text,
        suffix,
        token.unescape_escaped_whitespace,
        workspace_root,
        home_dir,
    )
}

enum TerminalHomeDir<'a> {
    #[allow(dead_code)]
    Known(Option<&'a Path>),
    Lazy(Option<PathBuf>),
}

impl TerminalHomeDir<'_> {
    fn get(&mut self) -> Option<&Path> {
        match self {
            Self::Known(home_dir) => *home_dir,
            Self::Lazy(home_dir) => {
                if home_dir.is_none() {
                    *home_dir = terminal_home_dir();
                }
                home_dir.as_deref()
            }
        }
    }
}

fn terminal_screen_line_and_text_column(
    session: &TerminalSession,
    row: u16,
    target_col: u16,
) -> Option<(String, usize)> {
    let screen = session.parser.screen();
    let (rows, cols) = screen.size();
    if row >= rows || target_col >= cols {
        return None;
    }

    let mut line = String::with_capacity(usize::from(cols));
    let mut text_column = None;
    let mut next_text_column = 0usize;
    let mut last_text_column = None;
    for col in 0..cols {
        let Some(cell) = screen.cell(row, col) else {
            if col == target_col {
                text_column = Some(next_text_column);
            }
            line.push(' ');
            last_text_column = Some(next_text_column);
            next_text_column += 1;
            continue;
        };
        if cell.is_wide_continuation() {
            if col == target_col {
                text_column = last_text_column.or(Some(next_text_column));
            }
            continue;
        }
        if col == target_col {
            text_column = Some(next_text_column);
        }
        last_text_column = Some(next_text_column);
        let text = cell.contents();
        if text.is_empty() {
            line.push(' ');
            next_text_column += 1;
        } else {
            line.push_str(text);
            next_text_column += text.chars().count();
        }
    }
    let trimmed_len = line.trim_end().len();
    line.truncate(trimmed_len);
    Some((line, text_column?))
}

struct TerminalLinkToken<'a> {
    text: &'a str,
    suffix_start_byte: usize,
    unescape_escaped_whitespace: bool,
}

const TERMINAL_LINK_MAX_TOKEN_CHARS: usize = 4096;
const TERMINAL_LINK_MAX_SUFFIX_CHARS: usize = 256;

fn terminal_link_suffix_after_token(text: &str, suffix_start_byte: usize) -> &str {
    if suffix_start_byte >= text.len() {
        return "";
    }
    if !text.is_char_boundary(suffix_start_byte) {
        return "";
    }

    let suffix_end =
        terminal_link_context_end_byte(text, suffix_start_byte, TERMINAL_LINK_MAX_SUFFIX_CHARS);
    &text[suffix_start_byte..suffix_end]
}

fn terminal_link_token_at_column(text: &str, column: usize) -> Option<TerminalLinkToken<'_>> {
    let column_context = terminal_link_column_context_at_column(text, column)?;

    let mut points_before_wrapped_path_suffix = false;
    let unquoted_token = if let Some(bounds) = column_context.unquoted_token {
        let start = bounds.start_byte;
        let end = bounds.end_byte;
        let raw_token = &text[start..end];
        let column_in_token = column_context.column_byte - start;
        points_before_wrapped_path_suffix =
            unquoted_token_points_before_wrapped_path_suffix(raw_token, column_in_token);
        if points_before_wrapped_path_suffix && !terminal_link_open_delimiter_before(text, start) {
            return None;
        }
        terminal_link_target_from_raw_token(raw_token).and_then(|target| {
            (target.start_byte <= column_in_token && column_in_token < target.end_byte).then_some((
                target.text,
                start + target.end_byte,
                bounds.has_escaped_whitespace,
            ))
        })
    } else {
        None
    };

    if let Some(token) = quoted_terminal_link_token_at_column(text, column_context.column_byte) {
        return Some(token);
    }
    if points_before_wrapped_path_suffix {
        return None;
    }

    let (raw_token, end, has_escaped_whitespace) = unquoted_token?;
    Some(TerminalLinkToken {
        text: raw_token,
        suffix_start_byte: end,
        unescape_escaped_whitespace: has_escaped_whitespace,
    })
}

struct TerminalLinkColumnContext {
    column_byte: usize,
    unquoted_token: Option<TerminalUnquotedTokenBounds>,
}

#[derive(Clone, Copy)]
struct TerminalUnquotedTokenBounds {
    start_byte: usize,
    end_byte: usize,
    has_escaped_whitespace: bool,
}

fn terminal_link_column_context_at_column(
    text: &str,
    column: usize,
) -> Option<TerminalLinkColumnContext> {
    let (column_byte, ch) = terminal_link_column_byte_and_char(text, column)?;
    Some(TerminalLinkColumnContext {
        column_byte,
        unquoted_token: terminal_link_char_allows_unquoted_token(text, column_byte, ch)
            .then(|| terminal_unquoted_token_bounds_at_byte(text, column_byte, ch))
            .flatten(),
    })
}

fn terminal_link_column_byte_and_char(text: &str, column: usize) -> Option<(usize, char)> {
    text.char_indices()
        .enumerate()
        .find_map(|(text_column, (byte, ch))| (text_column == column).then_some((byte, ch)))
}

fn terminal_unquoted_token_bounds_at_byte(
    text: &str,
    column_byte: usize,
    column_char: char,
) -> Option<TerminalUnquotedTokenBounds> {
    let mut start_byte = 0;
    let mut token_chars = 1usize;
    let mut has_escaped_whitespace = terminal_link_escaped_whitespace_at_byte(text, column_byte);

    for (byte, ch) in text[..column_byte].char_indices().rev() {
        if terminal_link_unquoted_token_breaks_at(text, byte, ch) {
            start_byte = byte + ch.len_utf8();
            break;
        }
        has_escaped_whitespace |= terminal_link_escaped_whitespace_at_byte(text, byte);
        token_chars += 1;
        if token_chars > TERMINAL_LINK_MAX_TOKEN_CHARS {
            return None;
        }
    }

    let after_column_byte = column_byte + column_char.len_utf8();
    let mut end_byte = text.len();
    for (offset, ch) in text[after_column_byte..].char_indices() {
        let byte = after_column_byte + offset;
        if terminal_link_unquoted_token_breaks_at(text, byte, ch) {
            end_byte = after_column_byte + offset;
            break;
        }
        has_escaped_whitespace |= terminal_link_escaped_whitespace_at_byte(text, byte);
        token_chars += 1;
        if token_chars > TERMINAL_LINK_MAX_TOKEN_CHARS {
            return None;
        }
    }

    Some(TerminalUnquotedTokenBounds {
        start_byte,
        end_byte,
        has_escaped_whitespace,
    })
}

fn terminal_link_char_allows_unquoted_token(text: &str, byte: usize, ch: char) -> bool {
    !ch.is_whitespace() || terminal_link_escaped_whitespace_at_byte(text, byte)
}

fn terminal_link_unquoted_token_breaks_at(text: &str, byte: usize, ch: char) -> bool {
    ch.is_whitespace() && !terminal_link_escaped_whitespace_at_byte(text, byte)
}

fn terminal_link_escaped_whitespace_at_byte(text: &str, byte: usize) -> bool {
    text[byte..].chars().next().is_some_and(char::is_whitespace)
        && terminal_link_backslash_run_before_byte(text, byte) % 2 == 1
}

fn terminal_link_backslash_run_before_byte(text: &str, byte: usize) -> usize {
    text[..byte]
        .chars()
        .rev()
        .take_while(|ch| *ch == '\\')
        .count()
}

fn terminal_link_open_delimiter_before(text: &str, byte: usize) -> bool {
    text[..byte]
        .chars()
        .rev()
        .take(TERMINAL_LINK_MAX_TOKEN_CHARS)
        .any(|ch| terminal_link_closing_delimiter(ch).is_some())
}

fn unquoted_token_points_before_wrapped_path_suffix(token: &str, column_byte: usize) -> bool {
    let token = token.trim_end_matches([',', ';', '.']);
    let Some(before_close) = token.strip_suffix(')') else {
        return false;
    };
    let Some(open_byte) = before_close.rfind('(') else {
        return false;
    };
    if column_byte >= open_byte {
        return false;
    }

    let prefix = before_close[..open_byte].trim();
    if prefix.is_empty() || prefix.contains(['/', '\\']) || strip_file_uri_prefix(prefix).is_some()
    {
        return false;
    }

    wrapped_path_suffix_inner_looks_clickable(&before_close[open_byte + 1..])
}

fn wrapped_path_suffix_inner_looks_clickable(inner: &str) -> bool {
    let inner = trim_terminal_link_token(inner.trim());
    if inner.is_empty() {
        return false;
    }

    let (path_text, _, _, _) = split_terminal_path_location(inner);
    file_uri_path_looks_clickable(path_text) || is_terminal_path_like(path_text)
}

fn quoted_terminal_link_token_at_column(
    text: &str,
    column_byte: usize,
) -> Option<TerminalLinkToken<'_>> {
    let context_start =
        terminal_link_context_start_byte(text, column_byte, TERMINAL_LINK_MAX_TOKEN_CHARS + 1);
    let context_end =
        terminal_link_context_end_byte(text, column_byte, TERMINAL_LINK_MAX_TOKEN_CHARS + 1);
    let context = &text[context_start..context_end];
    let mut next_close_bytes = [None; TERMINAL_LINK_CLOSE_DELIMITER_COUNT];
    let mut token = None;

    for (relative_start_byte, ch) in context.char_indices().rev() {
        let start_byte = context_start + relative_start_byte;
        let Some(close) = terminal_link_closing_delimiter(ch) else {
            if let Some(close_index) = terminal_link_close_delimiter_index(ch) {
                next_close_bytes[close_index] = Some(start_byte);
            }
            continue;
        };
        let close_index = terminal_link_close_delimiter_index(close)?;
        let end_byte = next_close_bytes[close_index];
        let inner_start = start_byte + ch.len_utf8();

        if inner_start <= column_byte
            && let Some(end_byte) = end_byte
            && column_byte < end_byte
        {
            let Some(candidate) = terminal_link_target_from_raw_token(&text[inner_start..end_byte])
            else {
                continue;
            };
            let target_start_byte = inner_start + candidate.start_byte;
            let target_end_byte = inner_start + candidate.end_byte;
            if column_byte < target_start_byte || target_end_byte <= column_byte {
                continue;
            }
            if ch == '('
                && start_byte > 0
                && text[..start_byte]
                    .chars()
                    .next_back()
                    .is_some_and(|ch| !ch.is_whitespace())
                && is_parenthesized_location_inner(candidate.text)
            {
                continue;
            }
            token = Some(TerminalLinkToken {
                text: candidate.text,
                suffix_start_byte: end_byte + close.len_utf8(),
                unescape_escaped_whitespace: false,
            });
        }

        if let Some(close_index) = terminal_link_close_delimiter_index(ch) {
            next_close_bytes[close_index] = Some(start_byte);
        }
    }

    token
}

fn terminal_link_context_start_byte(
    text: &str,
    column_byte: usize,
    max_chars_before: usize,
) -> usize {
    let mut start_byte = 0;
    for (scanned_chars, (byte, ch)) in text[..column_byte].char_indices().rev().enumerate() {
        if scanned_chars == max_chars_before {
            return byte + ch.len_utf8();
        }
        start_byte = byte;
    }
    start_byte
}

fn terminal_link_context_end_byte(text: &str, column_byte: usize, max_chars_after: usize) -> usize {
    let mut end_byte = text.len();
    for (scanned_chars, (offset, ch)) in text[column_byte..].char_indices().enumerate() {
        if scanned_chars == max_chars_after {
            return column_byte + offset;
        }
        end_byte = column_byte + offset + ch.len_utf8();
    }
    end_byte
}

const TERMINAL_LINK_CLOSE_DELIMITER_COUNT: usize = 7;

fn is_parenthesized_location_inner(token: &str) -> bool {
    let mut parts = token.split(',');
    let Some(line) = parts.next().map(str::trim) else {
        return false;
    };
    let column = parts.next().map(str::trim);
    if parts.next().is_some() || line.is_empty() || !line.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }
    column.is_none_or(|column| !column.is_empty() && column.chars().all(|ch| ch.is_ascii_digit()))
}

fn terminal_link_closing_delimiter(ch: char) -> Option<char> {
    match ch {
        '"' => Some('"'),
        '\'' => Some('\''),
        '`' => Some('`'),
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '<' => Some('>'),
        _ => None,
    }
}

fn terminal_link_close_delimiter_index(ch: char) -> Option<usize> {
    match ch {
        '"' => Some(0),
        '\'' => Some(1),
        '`' => Some(2),
        ')' => Some(3),
        ']' => Some(4),
        '}' => Some(5),
        '>' => Some(6),
        _ => None,
    }
}

fn terminal_path_link_from_token(
    token: &str,
    suffix: &str,
    unescape_escaped_whitespace: bool,
    workspace_root: &Path,
    home_dir: &mut TerminalHomeDir<'_>,
) -> Option<TerminalPathLink> {
    if terminal_path_text_has_unsafe_display_chars(token) {
        return None;
    }

    let token = terminal_path_location_token(token);
    if let Some((path, (line, column))) = file_uri_path_and_embedded_location(token) {
        let path = terminal_path_from_path_buf(path, workspace_root);
        return Some(TerminalPathLink { path, line, column });
    }

    let (path_text, mut line, mut column, mut has_inline_location) =
        split_terminal_path_location_suffixes(token);
    let path = if let Some((path, uri_location)) = file_uri_path_and_location(path_text) {
        if !has_inline_location && let Some((uri_line, uri_column)) = uri_location {
            line = uri_line;
            column = uri_column;
            has_inline_location = true;
        }
        terminal_path_from_path_buf(path, workspace_root)
    } else {
        if !is_terminal_path_like(path_text) {
            return None;
        }
        terminal_path_from_text(
            path_text,
            unescape_escaped_whitespace,
            workspace_root,
            home_dir,
        )?
    };

    if !has_inline_location
        && let Some((suffix_line, suffix_column)) = split_following_line_location(suffix)
    {
        line = suffix_line;
        column = suffix_column;
    }

    Some(TerminalPathLink { path, line, column })
}

fn terminal_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

fn terminal_path_from_text(
    path_text: &str,
    unescape_escaped_whitespace: bool,
    workspace_root: &Path,
    home_dir: &mut TerminalHomeDir<'_>,
) -> Option<PathBuf> {
    if terminal_path_text_has_unsafe_display_chars(path_text) {
        return None;
    }

    let path_text = if unescape_escaped_whitespace {
        unescape_terminal_path_escaped_whitespace(path_text)
    } else {
        Cow::Borrowed(path_text)
    };
    let path_text = path_text.as_ref();

    if let Some(rest) = path_text
        .strip_prefix("~/")
        .or_else(|| path_text.strip_prefix("~\\"))
    {
        return home_dir.get().map(|home| home.join(rest));
    }

    let path = PathBuf::from(path_text);
    Some(terminal_path_from_path_buf(path, workspace_root))
}

fn unescape_terminal_path_escaped_whitespace(path: &str) -> Cow<'_, str> {
    let mut output = String::with_capacity(path.len());
    let mut chars = path.chars().peekable();
    let mut backslash_run = 0usize;
    let mut changed = false;

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            backslash_run += 1;
            if chars
                .peek()
                .is_some_and(|next| next.is_whitespace() && backslash_run % 2 == 1)
            {
                if let Some(escaped) = chars.next() {
                    output.push(escaped);
                    backslash_run = 0;
                    changed = true;
                    continue;
                }
            }
            output.push(ch);
        } else {
            output.push(ch);
            backslash_run = 0;
        }
    }

    if changed {
        Cow::Owned(output)
    } else {
        Cow::Borrowed(path)
    }
}

fn terminal_path_from_path_buf(path: PathBuf, workspace_root: &Path) -> PathBuf {
    if terminal_path_buf_is_absolute(&path) {
        path
    } else {
        workspace_root.join(path)
    }
}

fn terminal_path_buf_is_absolute(path: &Path) -> bool {
    path.is_absolute()
        || path
            .as_os_str()
            .to_str()
            .is_some_and(terminal_path_text_is_absolute)
}

fn terminal_path_text_is_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes
        .first()
        .is_some_and(|byte| is_terminal_path_separator_byte(*byte))
        || is_windows_drive_absolute_path_text(path)
        || is_windows_unc_path_text(path)
}

fn is_windows_drive_absolute_path_text(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && is_terminal_path_separator_byte(bytes[2])
}

fn is_windows_unc_path_text(path: &str) -> bool {
    let bytes = path.as_bytes();
    if bytes.len() < 5
        || !is_terminal_path_separator_byte(bytes[0])
        || !is_terminal_path_separator_byte(bytes[1])
    {
        return false;
    }

    let mut parts = path[2..].split(['/', '\\']);
    let Some(server) = parts.next() else {
        return false;
    };
    let Some(share) = parts.next() else {
        return false;
    };
    !server.is_empty() && !share.is_empty()
}

fn is_terminal_path_separator_byte(byte: u8) -> bool {
    matches!(byte, b'/' | b'\\')
}

fn file_uri_path_and_location(text: &str) -> Option<(PathBuf, Option<(usize, usize)>)> {
    let uri = strip_file_uri_prefix(text)?;
    let (path, location) = split_file_uri_path_and_location(uri);
    Some((file_uri_path_buf(path)?, location))
}

fn file_uri_path_and_embedded_location(text: &str) -> Option<(PathBuf, (usize, usize))> {
    let uri = strip_file_uri_prefix(text)?;
    let (path, location) = split_file_uri_path_and_location(uri);
    Some((file_uri_path_buf(path)?, location?))
}

fn file_uri_path_buf(path: &str) -> Option<PathBuf> {
    let path = percent_decode_file_uri_path(path)?;
    let path = normalize_file_uri_path(path);
    if terminal_path_text_has_unsafe_display_chars(path.as_ref()) {
        return None;
    }
    Some(PathBuf::from(path.as_ref()))
}

fn file_uri_path_looks_clickable(text: &str) -> bool {
    let Some(uri) = strip_file_uri_prefix(text) else {
        return false;
    };
    let (path, _) = split_file_uri_path_and_location(uri);
    file_uri_path_buf(path).is_some()
}

fn strip_file_uri_prefix(text: &str) -> Option<&str> {
    let prefix = text.get(..7)?;
    if !prefix.eq_ignore_ascii_case("file://") {
        return None;
    }
    let rest = &text[7..];
    let localhost_prefix = rest.get(..10);
    if localhost_prefix.is_some_and(|prefix| prefix.eq_ignore_ascii_case("localhost/")) {
        return Some(&rest[9..]);
    }
    Some(rest)
}

fn split_file_uri_path_and_location(uri: &str) -> (&str, Option<(usize, usize)>) {
    let query_start = uri.find('?');
    let fragment_start = uri.find('#');
    let path_end = match (query_start, fragment_start) {
        (Some(query), Some(fragment)) => query.min(fragment),
        (Some(query), None) => query,
        (None, Some(fragment)) => fragment,
        (None, None) => return (uri, None),
    };
    let location = fragment_start
        .and_then(|index| parse_file_uri_fragment_location(&uri[index + 1..]))
        .or_else(|| {
            query_start.and_then(|index| {
                let query_end = fragment_start
                    .filter(|fragment| *fragment > index)
                    .unwrap_or(uri.len());
                parse_file_uri_query_location(&uri[index + 1..query_end])
            })
        });
    (&uri[..path_end], location)
}

fn parse_file_uri_fragment_location(fragment: &str) -> Option<(usize, usize)> {
    let rest = fragment.trim();
    let rest = rest.strip_prefix('L').or_else(|| rest.strip_prefix('l'))?;
    let (line, rest) = split_leading_location_number(rest)?;
    let column = parse_file_uri_fragment_column(rest).unwrap_or(1);
    Some((line, column))
}

fn parse_file_uri_fragment_column(rest: &str) -> Option<usize> {
    let rest = rest.trim_start();
    let rest = rest
        .strip_prefix('C')
        .or_else(|| rest.strip_prefix('c'))
        .or_else(|| rest.strip_prefix(':'))
        .or_else(|| rest.strip_prefix(','))?;
    split_leading_location_number(rest).map(|(column, _)| column)
}

fn parse_file_uri_query_location(query: &str) -> Option<(usize, usize)> {
    let mut line = None;
    let mut column = None;
    for part in query.split(['&', ';']) {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.eq_ignore_ascii_case("line") || key.eq_ignore_ascii_case("l") {
            line = parse_location_number(value);
        } else if key.eq_ignore_ascii_case("column")
            || key.eq_ignore_ascii_case("col")
            || key.eq_ignore_ascii_case("c")
        {
            column = parse_location_number(value);
        }
    }
    line.map(|line| (line, column.unwrap_or(1)))
}

fn parse_location_number(value: &str) -> Option<usize> {
    if value.is_empty() || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(value.parse::<usize>().ok()?.max(1))
}

fn percent_decode_file_uri_path(text: &str) -> Option<Cow<'_, str>> {
    let bytes = text.as_bytes();
    if !bytes.contains(&b'%') {
        return Some(Cow::Borrowed(text));
    }

    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_value(*bytes.get(index + 1)?)?;
            let low = hex_value(*bytes.get(index + 2)?)?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok().map(Cow::Owned)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn normalize_file_uri_path(path: Cow<'_, str>) -> Cow<'_, str> {
    let path = normalize_file_uri_windows_drive_pipe(path);
    let strip_windows_drive_slash = {
        let bytes = path.as_bytes();
        bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':'
    };
    if strip_windows_drive_slash {
        match path {
            Cow::Borrowed(path) => Cow::Borrowed(&path[1..]),
            Cow::Owned(mut path) => {
                path.remove(0);
                Cow::Owned(path)
            }
        }
    } else {
        path
    }
}

fn normalize_file_uri_windows_drive_pipe(path: Cow<'_, str>) -> Cow<'_, str> {
    let bytes = path.as_bytes();
    let drive_separator_index = if bytes.len() >= 3
        && bytes[0] == b'/'
        && bytes[1].is_ascii_alphabetic()
        && bytes[2] == b'|'
    {
        Some(2)
    } else if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b'|' {
        Some(1)
    } else {
        None
    };

    let Some(drive_separator_index) = drive_separator_index else {
        return path;
    };

    let mut path = path.into_owned();
    path.replace_range(drive_separator_index..drive_separator_index + 1, ":");
    Cow::Owned(path)
}

fn split_terminal_path_location(token: &str) -> (&str, usize, usize, bool) {
    let path = terminal_path_location_token(token);

    if let Some((uri_line, uri_column)) = file_uri_embedded_location(path) {
        return (path, uri_line, uri_column, true);
    }

    split_terminal_path_location_suffixes(path)
}

fn terminal_path_location_token(token: &str) -> &str {
    strip_terminal_test_node_suffix(strip_trailing_diagnostic_text_after_location(
        token.trim_end_matches(':'),
    ))
}

fn strip_terminal_test_node_suffix(text: &str) -> &str {
    let Some((path, node)) = text.split_once("::") else {
        return text;
    };
    if node.is_empty()
        || !node
            .chars()
            .next()
            .is_some_and(is_terminal_test_node_start_char)
        || Path::new(path).extension().is_none()
    {
        return text;
    }
    path
}

fn is_terminal_test_node_start_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn split_terminal_path_location_suffixes(mut path: &str) -> (&str, usize, usize, bool) {
    let mut line = 1usize;
    let mut column = 1usize;

    if let Some((head, parenthesized_line, parenthesized_column)) =
        split_trailing_parenthesized_location(path)
    {
        return (head, parenthesized_line, parenthesized_column, true);
    }

    if let Some((head, start_line, start_column)) = split_trailing_colon_range_location(path) {
        return (head, start_line, start_column, true);
    }

    let mut has_location = false;
    if let Some((head, value)) = split_trailing_colon_number(path) {
        line = value;
        path = head;
        has_location = true;
        if let Some((head, value)) = split_trailing_colon_number(path) {
            column = line;
            line = value;
            path = head;
        }
    }

    (path, line, column, has_location)
}

fn file_uri_embedded_location(text: &str) -> Option<(usize, usize)> {
    let uri = strip_file_uri_prefix(text)?;
    let (_, location) = split_file_uri_path_and_location(uri);
    location
}

fn strip_trailing_diagnostic_text_after_location(text: &str) -> &str {
    let mut candidate = text;
    while let Some((head, tail)) = candidate.rsplit_once(':') {
        let tail = tail.trim();
        if tail.is_empty()
            || tail.chars().all(|ch| ch.is_ascii_digit())
            || parse_location_start_number(tail).is_some()
        {
            return text;
        }
        if tail.contains(['/', '\\']) {
            return text;
        }
        if split_trailing_colon_number(head).is_some()
            || split_trailing_parenthesized_location(head).is_some()
        {
            return head;
        }
        candidate = head;
    }
    text
}

fn split_trailing_parenthesized_location(text: &str) -> Option<(&str, usize, usize)> {
    let (head, tail) = text.rsplit_once('(')?;
    let inner = tail.strip_suffix(')')?;
    if head.is_empty() || inner.is_empty() {
        return None;
    }
    let (line, column) = parse_parenthesized_location_inner(inner)?;
    Some((head, line, column))
}

fn parse_parenthesized_location_inner(inner: &str) -> Option<(usize, usize)> {
    let mut parts = inner.split(',');
    let line_text = parts.next()?.trim();
    let column_text = parts.next().map(str::trim);
    if parts.next().is_some() || line_text.is_empty() {
        return None;
    }

    let line = parse_location_start_number(line_text)?;
    let column = if let Some(column_text) = column_text {
        parse_location_start_number(column_text)?
    } else {
        1
    };
    Some((line, column))
}

fn split_trailing_colon_range_location(text: &str) -> Option<(&str, usize, usize)> {
    let (range_head, range_tail) = text.rsplit_once('-')?;
    if !is_location_range_end(range_tail) {
        return None;
    }

    let (head, value) = split_trailing_colon_number(range_head)?;
    if let Some((head, line)) = split_trailing_colon_number(head) {
        Some((head, line, value))
    } else {
        Some((head, value, 1))
    }
}

fn parse_location_start_number(text: &str) -> Option<usize> {
    let (head, tail) = text.split_once('-').unwrap_or((text, ""));
    if !tail.is_empty() && !is_location_range_end(tail) {
        return None;
    }
    let head = head.trim();
    if head.is_empty() || !head.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(head.parse::<usize>().ok()?.max(1))
}

fn is_location_range_end(text: &str) -> bool {
    let mut parts = text.split(':');
    let Some(first) = parts.next() else {
        return false;
    };
    if !is_location_number_text(first) {
        return false;
    }
    if let Some(second) = parts.next() {
        is_location_number_text(second) && parts.next().is_none()
    } else {
        true
    }
}

fn is_location_number_text(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|ch| ch.is_ascii_digit())
}

fn split_trailing_colon_number(text: &str) -> Option<(&str, usize)> {
    let (head, tail) = text.rsplit_once(':')?;
    if head.is_empty() || tail.is_empty() || !tail.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let value = tail.parse::<usize>().ok()?.max(1);
    Some((head, value))
}

fn split_following_line_location(text: &str) -> Option<(usize, usize)> {
    let rest = strip_location_prefix_punctuation(text);
    let rest = strip_location_keyword(rest, "line")?;
    let rest = strip_location_value_separator(rest);
    let (line, rest) = split_leading_location_number(rest)?;
    let column = split_following_column_location(rest).unwrap_or(1);
    Some((line, column))
}

fn split_following_column_location(text: &str) -> Option<usize> {
    let rest = text.trim_start();
    if let Some(rest) = rest.strip_prefix(':') {
        return split_leading_location_number(rest).map(|(column, _)| column);
    }

    let rest = strip_location_prefix_punctuation(rest);
    let rest =
        strip_location_keyword(rest, "column").or_else(|| strip_location_keyword(rest, "col"))?;
    let rest = strip_location_value_separator(rest);
    split_leading_location_number(rest).map(|(column, _)| column)
}

fn strip_location_prefix_punctuation(text: &str) -> &str {
    text.trim_start()
        .trim_start_matches([',', ':', ';'])
        .trim_start()
}

fn strip_location_keyword<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let text = text.trim_start();
    let head = text.get(..keyword.len())?;
    if !head.eq_ignore_ascii_case(keyword) {
        return None;
    }
    let tail = &text[keyword.len()..];
    if tail
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    Some(tail)
}

fn strip_location_value_separator(text: &str) -> &str {
    text.trim_start()
        .trim_start_matches([':', '='])
        .trim_start()
}

fn split_leading_location_number(text: &str) -> Option<(usize, &str)> {
    let text = text.trim_start();
    let end = text
        .char_indices()
        .find_map(|(index, ch)| (!ch.is_ascii_digit()).then_some(index))
        .unwrap_or(text.len());
    if end == 0 {
        return None;
    }
    let value = text[..end].parse::<usize>().ok()?.max(1);
    Some((value, &text[end..]))
}

fn is_terminal_path_like(path: &str) -> bool {
    let path = path.trim();
    if path.is_empty() || path.contains("://") {
        return false;
    }
    path.contains('/')
        || path.contains('\\')
        || path.starts_with('.')
        || Path::new(path).extension().is_some()
}

fn terminal_path_text_has_unsafe_display_chars(path: &str) -> bool {
    path.chars()
        .any(|ch| ch.is_control() || is_terminal_path_bidi_control(ch))
}

fn is_terminal_path_bidi_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

struct TerminalLinkTarget<'a> {
    text: &'a str,
    start_byte: usize,
    end_byte: usize,
}

fn terminal_link_target_from_raw_token(token: &str) -> Option<TerminalLinkTarget<'_>> {
    let (start_byte, end_byte) = terminal_link_target_bounds(token)?;
    Some(TerminalLinkTarget {
        text: &token[start_byte..end_byte],
        start_byte,
        end_byte,
    })
}

fn terminal_link_target_bounds(token: &str) -> Option<(usize, usize)> {
    let mut start_byte = 0;
    let mut end_byte = token.len();

    while start_byte < end_byte {
        let ch = token[start_byte..].chars().next()?;
        if !is_terminal_link_token_delimiter(ch) {
            break;
        }
        start_byte += ch.len_utf8();
    }

    while start_byte < end_byte {
        let ch = token[..end_byte].chars().next_back()?;
        if !is_terminal_link_token_delimiter(ch) {
            break;
        }
        end_byte -= ch.len_utf8();
    }

    while start_byte < end_byte {
        let ch = token[..end_byte].chars().next_back()?;
        if !is_terminal_link_token_trailing_punctuation(ch) {
            break;
        }
        end_byte -= ch.len_utf8();
    }

    (start_byte < end_byte).then_some((start_byte, end_byte))
}

fn is_terminal_link_token_delimiter(ch: char) -> bool {
    matches!(
        ch,
        '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
    )
}

fn is_terminal_link_token_trailing_punctuation(ch: char) -> bool {
    matches!(ch, ',' | ';' | '.' | ':')
}

fn trim_terminal_link_token(token: &str) -> &str {
    terminal_link_target_from_raw_token(token)
        .map(|target| target.text)
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::{
        terminal_path_link_at_text_position, terminal_path_link_at_text_position_with_home,
    };
    use std::path::PathBuf;

    #[test]
    fn terminal_path_links_scan_quoted_paths_after_many_unmatched_openers() {
        let workspace = PathBuf::from("workspace");
        let prefix = "(".repeat(256);
        let text = format!("{prefix} \"src/my file.rs:12:5\"");

        let link = terminal_path_link_at_text_position(
            &text,
            prefix.chars().count() + "\"src/my ".chars().count(),
            &workspace,
        )
        .unwrap();

        assert_eq!(link.path, workspace.join("src/my file.rs"));
        assert_eq!(link.line, 12);
        assert_eq!(link.column, 5);
    }

    #[test]
    fn terminal_path_links_keep_java_stack_frame_prefix_unlinked() {
        let workspace = PathBuf::from("workspace");
        let text = "at com.example.Widget.render(Widget.java:42)";

        assert!(
            terminal_path_link_at_text_position(text, "at com.example".chars().count(), &workspace)
                .is_none()
        );

        let link = terminal_path_link_at_text_position(
            text,
            text.find("Widget.java").unwrap(),
            &workspace,
        )
        .unwrap();

        assert_eq!(link.path, workspace.join("Widget.java"));
        assert_eq!(link.line, 42);
        assert_eq!(link.column, 1);
    }

    #[test]
    fn terminal_path_links_expand_tilde_with_supplied_home() {
        let workspace = PathBuf::from("workspace");
        let home = PathBuf::from("/home/me");
        let text = "~/project/file.rs:7";

        let link = terminal_path_link_at_text_position_with_home(text, 2, &workspace, Some(&home))
            .unwrap();

        assert_eq!(link.path, home.join("project/file.rs"));
        assert_eq!(link.line, 7);
        assert_eq!(link.column, 1);
    }

    #[test]
    fn terminal_path_links_respect_unicode_columns_before_unquoted_path() {
        let workspace = PathBuf::from("workspace");
        let text = "\u{03bb}\u{03bb} src/main.rs:12:8";

        let link = terminal_path_link_at_text_position(
            text,
            "\u{03bb}\u{03bb} src/main".chars().count(),
            &workspace,
        )
        .unwrap();

        assert_eq!(link.path, workspace.join("src/main.rs"));
        assert_eq!(link.line, 12);
        assert_eq!(link.column, 8);
    }

    #[test]
    fn terminal_path_links_respect_unicode_columns_inside_quoted_path() {
        let workspace = PathBuf::from("workspace");
        let text = "\"src/my \u{0444}\u{0430}\u{0439}\u{043b}.rs:4:2\"";

        let link =
            terminal_path_link_at_text_position(text, "\"src/my ".chars().count(), &workspace)
                .unwrap();

        assert_eq!(
            link.path,
            workspace.join("src/my \u{0444}\u{0430}\u{0439}\u{043b}.rs")
        );
        assert_eq!(link.line, 4);
        assert_eq!(link.column, 2);
    }

    #[test]
    fn terminal_path_links_parse_unquoted_paths_with_escaped_spaces() {
        let workspace = PathBuf::from("workspace");
        let text = "src/my\\ file.rs:12:5";
        let hover_columns = [
            text.find("my").unwrap(),
            text.find(' ').unwrap(),
            text.find("file").unwrap(),
        ];

        for column in hover_columns {
            let link = terminal_path_link_at_text_position(text, column, &workspace).unwrap();

            assert_eq!(link.path, workspace.join("src/my file.rs"));
            assert_eq!(link.line, 12);
            assert_eq!(link.column, 5);
        }
    }

    #[test]
    fn terminal_path_links_prefer_file_uri_embedded_location_over_following_suffix() {
        let workspace = PathBuf::from("workspace");
        let text = "file://src/main.rs?line=8&column=3 line 99";

        let link =
            terminal_path_link_at_text_position(text, text.find("main").unwrap(), &workspace)
                .unwrap();

        assert_eq!(link.path, workspace.join("src/main.rs"));
        assert_eq!(link.line, 8);
        assert_eq!(link.column, 3);
    }

    #[test]
    fn terminal_path_links_keep_file_uri_trailing_locations() {
        let workspace = PathBuf::from("workspace");
        let text = "file://src/main.rs:9:4";

        let link =
            terminal_path_link_at_text_position(text, text.find("main").unwrap(), &workspace)
                .unwrap();

        assert_eq!(link.path, workspace.join("src/main.rs"));
        assert_eq!(link.line, 9);
        assert_eq!(link.column, 4);
    }

    #[test]
    fn terminal_path_links_ignore_trimmed_punctuation_hover_targets() {
        let workspace = PathBuf::from("workspace");
        let text = "src/main.rs, line 12, column 5";

        assert!(
            terminal_path_link_at_text_position(text, text.find(',').unwrap(), &workspace)
                .is_none()
        );

        let link =
            terminal_path_link_at_text_position(text, text.find("main").unwrap(), &workspace)
                .unwrap();

        assert_eq!(link.path, workspace.join("src/main.rs"));
        assert_eq!(link.line, 12);
        assert_eq!(link.column, 5);
    }

    #[test]
    fn terminal_path_links_bound_following_location_suffix_scan() {
        let workspace = PathBuf::from("workspace");
        let text = format!(
            "src/main.rs{} line 77, column 9",
            " ".repeat(super::TERMINAL_LINK_MAX_SUFFIX_CHARS + 8)
        );

        let link = terminal_path_link_at_text_position(&text, 4, &workspace).unwrap();

        assert_eq!(link.path, workspace.join("src/main.rs"));
        assert_eq!(link.line, 1);
        assert_eq!(link.column, 1);
    }

    #[test]
    fn terminal_path_links_ignore_quote_delimiter_hover_targets() {
        let workspace = PathBuf::from("workspace");
        let text = "\"src/my file.rs:12:5\"";

        assert!(terminal_path_link_at_text_position(text, 0, &workspace).is_none());
        assert!(
            terminal_path_link_at_text_position(text, text.rfind('"').unwrap(), &workspace)
                .is_none()
        );

        let link =
            terminal_path_link_at_text_position(text, text.find("my file").unwrap(), &workspace)
                .unwrap();

        assert_eq!(link.path, workspace.join("src/my file.rs"));
        assert_eq!(link.line, 12);
        assert_eq!(link.column, 5);
    }

    #[test]
    fn terminal_path_links_bound_unquoted_tokens_without_partial_matches() {
        let workspace = PathBuf::from("workspace");
        let huge_token = format!(
            "{}src/main.rs:7",
            "x".repeat(super::TERMINAL_LINK_MAX_TOKEN_CHARS)
        );

        assert!(
            terminal_path_link_at_text_position(
                &huge_token,
                super::TERMINAL_LINK_MAX_TOKEN_CHARS,
                &workspace,
            )
            .is_none()
        );

        let long_prefix = format!(
            "{} src/main.rs:7",
            "x".repeat(super::TERMINAL_LINK_MAX_TOKEN_CHARS + 8)
        );
        let link = terminal_path_link_at_text_position(
            &long_prefix,
            long_prefix.find("main").unwrap(),
            &workspace,
        )
        .unwrap();

        assert_eq!(link.path, workspace.join("src/main.rs"));
        assert_eq!(link.line, 7);
        assert_eq!(link.column, 1);
    }

    #[test]
    fn terminal_path_links_ignore_paths_with_unsafe_display_controls() {
        let workspace = PathBuf::from("workspace");

        for text in ["src/bad\u{202e}name.rs:3:2", "src/bad\u{1b}name.rs:3:2"] {
            assert!(
                terminal_path_link_at_text_position(text, text.find("bad").unwrap(), &workspace)
                    .is_none(),
                "text={text:?}"
            );
        }
    }

    #[test]
    fn terminal_path_links_ignore_file_uris_with_decoded_unsafe_controls() {
        let workspace = PathBuf::from("workspace");
        let text = "file://src/bad%E2%80%AEname.rs:3:2";

        assert!(
            terminal_path_link_at_text_position(text, text.find("bad").unwrap(), &workspace)
                .is_none()
        );
    }

    #[test]
    fn terminal_path_links_ignore_file_uris_with_unsafe_display_controls_in_metadata() {
        let workspace = PathBuf::from("workspace");

        for text in [
            "file://src/main.rs?line=3&column=2\u{202e}",
            "file://src/main.rs#L3\u{1b}",
        ] {
            assert!(
                terminal_path_link_at_text_position(text, text.find("main").unwrap(), &workspace)
                    .is_none(),
                "text={text:?}"
            );
        }
    }

    #[test]
    fn terminal_path_links_keep_windows_absolute_paths_out_of_workspace() {
        let workspace = PathBuf::from("workspace");

        let drive = "C:/repo/src/main.rs:42:9";
        let drive_link =
            terminal_path_link_at_text_position(drive, drive.find("main").unwrap(), &workspace)
                .unwrap();
        assert_eq!(drive_link.path, PathBuf::from("C:/repo/src/main.rs"));
        assert_eq!(drive_link.line, 42);
        assert_eq!(drive_link.column, 9);

        let unc = r"\\server\share\src\main.rs:8";
        let unc_link =
            terminal_path_link_at_text_position(unc, unc.find("main").unwrap(), &workspace)
                .unwrap();
        assert_eq!(unc_link.path, PathBuf::from(r"\\server\share\src\main.rs"));
        assert_eq!(unc_link.line, 8);
        assert_eq!(unc_link.column, 1);
    }

    #[test]
    fn terminal_path_links_normalize_legacy_windows_file_uri_drives() {
        let workspace = PathBuf::from("workspace");
        let text = "file:///C|/repo/src/main.rs#L42C9";

        let link =
            terminal_path_link_at_text_position(text, text.find("main").unwrap(), &workspace)
                .unwrap();

        assert_eq!(link.path, PathBuf::from("C:/repo/src/main.rs"));
        assert_eq!(link.line, 42);
        assert_eq!(link.column, 9);
    }
}
