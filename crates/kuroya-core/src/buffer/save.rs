use super::text::rope_slice_text;
use super::{TextBuffer, TextEdit};

impl TextBuffer {
    pub fn apply_save_cleanup(
        &mut self,
        trim_trailing_whitespace: bool,
        insert_final_newline: bool,
        trim_final_newlines: bool,
    ) -> bool {
        if self.read_only
            || (!trim_trailing_whitespace && !insert_final_newline && !trim_final_newlines)
        {
            return false;
        }

        let cleaned = self.cleaned_text_for_save(
            trim_trailing_whitespace,
            insert_final_newline,
            trim_final_newlines,
        );
        if self.text_equals(&cleaned) {
            return false;
        }

        self.apply_transaction(vec![TextEdit {
            range: 0..self.len_chars(),
            inserted: cleaned,
        }])
    }

    fn cleaned_text_for_save(
        &self,
        trim_trailing_whitespace: bool,
        insert_final_newline: bool,
        trim_final_newlines: bool,
    ) -> String {
        let line_ending = self.preferred_line_ending();
        let mut cleaned = String::with_capacity(self.len_bytes());

        if trim_trailing_whitespace {
            for line_idx in 0..self.len_lines() {
                let line = self.rope.line(line_idx);
                let line = rope_slice_text(&line);
                let line = line.as_ref();
                let (content, ending) = if let Some(content) = line.strip_suffix("\r\n") {
                    (content, "\r\n")
                } else if let Some(content) = line.strip_suffix('\n') {
                    (content, "\n")
                } else {
                    (line, "")
                };
                cleaned.push_str(content.trim_end_matches([' ', '\t']));
                cleaned.push_str(ending);
            }
        } else {
            for chunk in self.rope.chunks() {
                cleaned.push_str(chunk);
            }
        }

        if trim_final_newlines {
            cleaned = trim_extra_final_newlines(&cleaned, line_ending);
        }

        if insert_final_newline && !cleaned.is_empty() && !cleaned.ends_with('\n') {
            cleaned.push_str(line_ending);
        }

        cleaned
    }

    fn preferred_line_ending(&self) -> &'static str {
        let mut previous_was_cr = false;
        for chunk in self.rope.chunks() {
            for ch in chunk.chars() {
                if previous_was_cr && ch == '\n' {
                    return "\r\n";
                }
                previous_was_cr = ch == '\r';
            }
        }
        "\n"
    }
}

pub fn clean_text_for_save(
    text: &str,
    trim_trailing_whitespace: bool,
    insert_final_newline: bool,
    trim_final_newlines: bool,
) -> String {
    let line_ending = preferred_line_ending(text);
    let mut cleaned = if trim_trailing_whitespace {
        trim_line_trailing_whitespace(text)
    } else {
        text.to_owned()
    };

    if trim_final_newlines {
        cleaned = trim_extra_final_newlines(&cleaned, line_ending);
    }

    if insert_final_newline && !cleaned.is_empty() && !cleaned.ends_with('\n') {
        cleaned.push_str(line_ending);
    }

    cleaned
}

fn preferred_line_ending(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

fn trim_line_trailing_whitespace(text: &str) -> String {
    let mut cleaned = String::with_capacity(text.len());
    for segment in text.split_inclusive('\n') {
        let (content, ending) = if let Some(content) = segment.strip_suffix("\r\n") {
            (content, "\r\n")
        } else if let Some(content) = segment.strip_suffix('\n') {
            (content, "\n")
        } else {
            (segment, "")
        };
        cleaned.push_str(content.trim_end_matches([' ', '\t']));
        cleaned.push_str(ending);
    }
    cleaned
}

pub(super) fn trim_extra_final_newlines(text: &str, line_ending: &str) -> String {
    let mut trimmed = text.to_owned();
    let mut ending_count = 0usize;
    while trimmed.ends_with(line_ending) {
        let new_len = trimmed.len().saturating_sub(line_ending.len());
        trimmed.truncate(new_len);
        ending_count += 1;
    }

    if ending_count > 0 {
        trimmed.push_str(line_ending);
    }
    trimmed
}
