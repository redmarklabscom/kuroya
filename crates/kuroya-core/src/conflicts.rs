#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeConflictResolution {
    Current,
    Incoming,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeConflictLineKind {
    Start,
    Current,
    Separator,
    Incoming,
    End,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeConflict {
    pub start_line: usize,
    pub separator_line: usize,
    pub end_line: usize,
}

impl MergeConflict {
    pub fn contains_line(&self, line: usize) -> bool {
        self.is_valid() && line >= self.start_line && line <= self.end_line
    }

    pub fn line_kind(&self, line: usize) -> Option<MergeConflictLineKind> {
        if !self.is_valid() {
            return None;
        }

        if line == self.start_line {
            Some(MergeConflictLineKind::Start)
        } else if line > self.start_line && line < self.separator_line {
            Some(MergeConflictLineKind::Current)
        } else if line == self.separator_line {
            Some(MergeConflictLineKind::Separator)
        } else if line > self.separator_line && line < self.end_line {
            Some(MergeConflictLineKind::Incoming)
        } else if line == self.end_line {
            Some(MergeConflictLineKind::End)
        } else {
            None
        }
    }

    fn is_valid(&self) -> bool {
        self.start_line < self.separator_line && self.separator_line < self.end_line
    }
}

pub fn merge_conflicts(text: &str) -> Vec<MergeConflict> {
    merge_conflicts_from_lines(text.split_inclusive('\n'))
}

pub fn merge_conflicts_from_lines<I, S>(lines: I) -> Vec<MergeConflict>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut conflicts = Vec::new();
    let mut start_line = None;
    let mut separator_line = None;

    for (index, line) in lines.into_iter().enumerate() {
        let line = line.as_ref();
        if is_conflict_start(line) && separator_line.is_none() {
            start_line = Some(index);
            separator_line = None;
            continue;
        }

        let Some(start) = start_line else {
            continue;
        };

        if separator_line.is_none() && is_conflict_separator(line) {
            separator_line = Some(index);
        } else if let Some(separator) = separator_line
            && is_conflict_end(line)
        {
            conflicts.push(MergeConflict {
                start_line: start,
                separator_line: separator,
                end_line: index,
            });
            start_line = None;
            separator_line = None;
        }
    }

    conflicts
}

pub fn merge_conflict_at_line(conflicts: &[MergeConflict], line: usize) -> Option<&MergeConflict> {
    conflicts
        .iter()
        .find(|conflict| conflict.contains_line(line))
}

pub fn merge_conflict_line_kind(
    conflicts: &[MergeConflict],
    line: usize,
) -> Option<MergeConflictLineKind> {
    merge_conflict_at_line(conflicts, line).and_then(|conflict| conflict.line_kind(line))
}

pub fn resolve_merge_conflict(
    text: &str,
    line: usize,
    resolution: MergeConflictResolution,
) -> Option<String> {
    let span = merge_conflict_span_at_line(text, line)?;
    resolve_merge_conflict_span(text, &span, resolution)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MergeConflictSpan {
    start_byte: usize,
    current_start_byte: usize,
    separator_byte: usize,
    incoming_start_byte: usize,
    end_byte: usize,
    after_end_byte: usize,
}

fn merge_conflict_span_at_line(text: &str, target_line: usize) -> Option<MergeConflictSpan> {
    let mut start_line = None;
    let mut separator_line = None;
    let mut start_byte = 0;
    let mut current_start_byte = 0;
    let mut separator_byte = 0;
    let mut incoming_start_byte = 0;
    let mut byte = 0;

    for (index, line) in text.split_inclusive('\n').enumerate() {
        let line_start = byte;
        let line_end = line_start + line.len();
        byte = line_end;

        if is_conflict_start(line) && separator_line.is_none() {
            if index > target_line {
                return None;
            }
            start_line = Some(index);
            separator_line = None;
            start_byte = line_start;
            current_start_byte = line_end;
            continue;
        }

        let Some(start) = start_line else {
            if index > target_line {
                return None;
            }
            continue;
        };

        if separator_line.is_none() && is_conflict_separator(line) {
            separator_line = Some(index);
            separator_byte = line_start;
            incoming_start_byte = line_end;
        } else if let Some(separator) = separator_line
            && is_conflict_end(line)
        {
            let conflict = MergeConflict {
                start_line: start,
                separator_line: separator,
                end_line: index,
            };
            if conflict.contains_line(target_line) {
                return Some(MergeConflictSpan {
                    start_byte,
                    current_start_byte,
                    separator_byte,
                    incoming_start_byte,
                    end_byte: line_start,
                    after_end_byte: line_end,
                });
            }
            start_line = None;
            separator_line = None;
        }
    }

    None
}

fn resolve_merge_conflict_span(
    text: &str,
    span: &MergeConflictSpan,
    resolution: MergeConflictResolution,
) -> Option<String> {
    let current = span.current_start_byte..span.separator_byte;
    let incoming = span.incoming_start_byte..span.end_byte;
    let current_text = text.get(current.clone())?;
    let incoming_text = text.get(incoming.clone())?;
    let before = text.get(..span.start_byte)?;
    let after = text.get(span.after_end_byte..)?;
    let replacement_len = match resolution {
        MergeConflictResolution::Current => current_text.len(),
        MergeConflictResolution::Incoming => incoming_text.len(),
        MergeConflictResolution::Both => current_text.len().checked_add(incoming_text.len())?,
    };
    let removed_len = span.after_end_byte.checked_sub(span.start_byte)?;
    let capacity = text
        .len()
        .checked_sub(removed_len)?
        .checked_add(replacement_len)?;
    let mut resolved = String::with_capacity(capacity);

    resolved.push_str(before);
    match resolution {
        MergeConflictResolution::Current => resolved.push_str(current_text),
        MergeConflictResolution::Incoming => resolved.push_str(incoming_text),
        MergeConflictResolution::Both => {
            resolved.push_str(current_text);
            resolved.push_str(incoming_text);
        }
    }
    resolved.push_str(after);

    Some(resolved)
}

fn is_conflict_start(line: &str) -> bool {
    trimmed_line(line).starts_with("<<<<<<<")
}

pub fn is_conflict_start_line(line: &str) -> bool {
    is_conflict_start(line)
}

fn is_conflict_separator(line: &str) -> bool {
    trimmed_line(line) == "======="
}

pub fn is_conflict_separator_line(line: &str) -> bool {
    is_conflict_separator(line)
}

fn is_conflict_end(line: &str) -> bool {
    trimmed_line(line).starts_with(">>>>>>>")
}

pub fn is_conflict_end_line(line: &str) -> bool {
    is_conflict_end(line)
}

fn trimmed_line(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n']).trim_start()
}

#[cfg(test)]
mod tests {
    use super::{
        MergeConflict, MergeConflictLineKind, MergeConflictResolution, merge_conflict_at_line,
        merge_conflict_line_kind, merge_conflicts, resolve_merge_conflict,
    };

    #[test]
    fn merge_conflicts_detects_complete_marker_blocks() {
        let text = "one\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\ntwo\n";

        assert_eq!(
            merge_conflicts(text),
            vec![MergeConflict {
                start_line: 1,
                separator_line: 3,
                end_line: 5
            }]
        );
    }

    #[test]
    fn merge_conflicts_ignores_incomplete_blocks() {
        assert!(merge_conflicts("<<<<<<< HEAD\nours\n=======\n").is_empty());
        assert!(merge_conflicts("=======\ntheirs\n>>>>>>> branch\n").is_empty());
    }

    #[test]
    fn merge_conflict_line_kind_classifies_markers_and_content() {
        let conflicts = merge_conflicts("<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\n");

        assert_eq!(
            merge_conflict_line_kind(&conflicts, 0),
            Some(MergeConflictLineKind::Start)
        );
        assert_eq!(
            merge_conflict_line_kind(&conflicts, 1),
            Some(MergeConflictLineKind::Current)
        );
        assert_eq!(
            merge_conflict_line_kind(&conflicts, 2),
            Some(MergeConflictLineKind::Separator)
        );
        assert_eq!(
            merge_conflict_line_kind(&conflicts, 3),
            Some(MergeConflictLineKind::Incoming)
        );
        assert_eq!(
            merge_conflict_line_kind(&conflicts, 4),
            Some(MergeConflictLineKind::End)
        );
        assert_eq!(merge_conflict_line_kind(&conflicts, 5), None);
    }

    #[test]
    fn merge_conflict_line_kind_ignores_invalid_ranges() {
        let conflicts = vec![MergeConflict {
            start_line: 4,
            separator_line: 2,
            end_line: 6,
        }];

        assert_eq!(merge_conflict_at_line(&conflicts, 4), None);
        assert_eq!(merge_conflict_line_kind(&conflicts, 4), None);
        assert_eq!(conflicts[0].line_kind(6), None);
    }

    #[test]
    fn resolve_merge_conflict_accepts_current_incoming_or_both() {
        let text = "one\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\ntwo\n";

        assert_eq!(
            resolve_merge_conflict(text, 2, MergeConflictResolution::Current).as_deref(),
            Some("one\nours\ntwo\n")
        );
        assert_eq!(
            resolve_merge_conflict(text, 2, MergeConflictResolution::Incoming).as_deref(),
            Some("one\ntheirs\ntwo\n")
        );
        assert_eq!(
            resolve_merge_conflict(text, 2, MergeConflictResolution::Both).as_deref(),
            Some("one\nours\ntheirs\ntwo\n")
        );
    }

    #[test]
    fn resolve_merge_conflict_uses_target_line_and_preserves_other_blocks() {
        let text = concat!(
            "<<<<<<< HEAD\n",
            "ours one\n",
            "=======\n",
            "theirs one\n",
            ">>>>>>> feature\n",
            "middle\n",
            "<<<<<<< HEAD\n",
            "ours two\n",
            "=======\n",
            "theirs two\n",
            ">>>>>>> feature",
        );

        assert_eq!(
            resolve_merge_conflict(text, 9, MergeConflictResolution::Incoming).as_deref(),
            Some(concat!(
                "<<<<<<< HEAD\n",
                "ours one\n",
                "=======\n",
                "theirs one\n",
                ">>>>>>> feature\n",
                "middle\n",
                "theirs two\n",
            ))
        );
    }

    #[test]
    fn resolve_merge_conflict_ignores_incomplete_target_blocks() {
        assert_eq!(
            resolve_merge_conflict(
                "before\n<<<<<<< HEAD\nours\n=======\n",
                2,
                MergeConflictResolution::Current
            ),
            None
        );
    }

    #[test]
    fn merge_conflicts_reset_stale_start_markers() {
        let text = concat!(
            "<<<<<<< stale\n",
            "abandoned\n",
            "<<<<<<< HEAD\n",
            "ours\n",
            "=======\n",
            "theirs\n",
            ">>>>>>> feature\n",
        );

        assert_eq!(
            merge_conflicts(text),
            vec![MergeConflict {
                start_line: 2,
                separator_line: 4,
                end_line: 6
            }]
        );
        assert_eq!(
            resolve_merge_conflict(text, 1, MergeConflictResolution::Current),
            None
        );
        assert_eq!(
            resolve_merge_conflict(text, 3, MergeConflictResolution::Incoming).as_deref(),
            Some("<<<<<<< stale\nabandoned\ntheirs\n")
        );
    }

    #[test]
    fn merge_conflicts_keep_outer_block_when_incoming_contains_start_marker() {
        let text = concat!(
            "<<<<<<< HEAD\n",
            "ours\n",
            "=======\n",
            "<<<<<<< stale incoming\n",
            "theirs\n",
            ">>>>>>> feature\n",
            "after\n",
        );

        assert_eq!(
            merge_conflicts(text),
            vec![MergeConflict {
                start_line: 0,
                separator_line: 2,
                end_line: 5
            }]
        );
        assert_eq!(
            resolve_merge_conflict(text, 3, MergeConflictResolution::Incoming).as_deref(),
            Some("<<<<<<< stale incoming\ntheirs\nafter\n")
        );
        assert_eq!(
            resolve_merge_conflict(text, 1, MergeConflictResolution::Current).as_deref(),
            Some("ours\nafter\n")
        );
    }

    #[test]
    fn merge_conflicts_ignore_stale_separator_and_end_markers_before_valid_block() {
        let text = concat!(
            "=======\n",
            "stale incoming\n",
            ">>>>>>> stale\n",
            "<<<<<<< HEAD\n",
            "ours\n",
            "=======\n",
            "theirs\n",
            ">>>>>>> feature\n",
        );

        assert_eq!(
            merge_conflicts(text),
            vec![MergeConflict {
                start_line: 3,
                separator_line: 5,
                end_line: 7
            }]
        );
        assert_eq!(
            resolve_merge_conflict(text, 1, MergeConflictResolution::Current),
            None
        );
        assert_eq!(
            resolve_merge_conflict(text, 4, MergeConflictResolution::Current).as_deref(),
            Some("=======\nstale incoming\n>>>>>>> stale\nours\n")
        );
    }

    #[test]
    fn resolve_merge_conflict_span_rejects_invalid_byte_ranges() {
        let text = "<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\n";
        let reversed_current = super::MergeConflictSpan {
            start_byte: 0,
            current_start_byte: 20,
            separator_byte: 15,
            incoming_start_byte: 23,
            end_byte: 30,
            after_end_byte: text.len(),
        };
        let out_of_bounds = super::MergeConflictSpan {
            start_byte: 0,
            current_start_byte: 13,
            separator_byte: 18,
            incoming_start_byte: 26,
            end_byte: 32,
            after_end_byte: text.len() + 1,
        };

        assert_eq!(
            super::resolve_merge_conflict_span(
                text,
                &reversed_current,
                MergeConflictResolution::Current
            ),
            None
        );
        assert_eq!(
            super::resolve_merge_conflict_span(
                text,
                &out_of_bounds,
                MergeConflictResolution::Incoming
            ),
            None
        );
    }
}
