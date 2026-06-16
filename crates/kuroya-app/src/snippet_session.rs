use crate::KuroyaApp;
use kuroya_core::{BufferId, Selection, TextBuffer};
use std::ops::Range;

const MAX_SNIPPET_SESSION_GROUPS: usize = 128;
const MAX_SNIPPET_SESSION_RANGES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SnippetSession {
    pub(crate) buffer_id: BufferId,
    pub(crate) tabstop_groups: Vec<Vec<Range<usize>>>,
    pub(crate) active: usize,
}

impl SnippetSession {
    pub(crate) fn new_grouped(
        buffer_id: BufferId,
        tabstop_groups: Vec<Vec<Range<usize>>>,
    ) -> Option<Self> {
        let tabstop_groups = sanitize_tabstop_groups(tabstop_groups);
        (!tabstop_groups.is_empty()).then_some(Self {
            buffer_id,
            tabstop_groups,
            active: 0,
        })
    }

    pub(crate) fn active_ranges(&self, buffer_id: BufferId) -> Option<&[Range<usize>]> {
        (self.buffer_id == buffer_id)
            .then(|| self.tabstop_groups.get(self.active).map(Vec::as_slice))
            .flatten()
    }

    pub(crate) fn update_after_active_edit(
        &mut self,
        old_range: Range<usize>,
        new_cursor: usize,
        delta: isize,
    ) -> bool {
        if !valid_tabstop_range(&old_range)
            || !self.active_group_matches(std::slice::from_ref(&old_range))
        {
            return false;
        }
        for (index, group) in self.tabstop_groups.iter_mut().enumerate() {
            if index == self.active {
                group.clear();
                group.push(new_cursor..new_cursor);
            } else {
                for range in group {
                    if range.start >= old_range.end {
                        *range = shift_range(range.start..range.end, delta);
                    }
                }
            }
        }
        true
    }

    pub(crate) fn update_after_active_group_edit(
        &mut self,
        old_ranges: &[Range<usize>],
        new_ranges: &[Range<usize>],
    ) -> bool {
        if !self.active_group_matches(old_ranges)
            || !valid_group_replacement(old_ranges, new_ranges)
        {
            return false;
        }
        for (index, group) in self.tabstop_groups.iter_mut().enumerate() {
            if index == self.active {
                *group = new_ranges.to_vec();
            } else {
                for range in group {
                    *range = shift_range_through_replacements(
                        range.start..range.end,
                        old_ranges,
                        new_ranges,
                    );
                }
            }
        }
        true
    }

    fn active_group_matches(&self, ranges: &[Range<usize>]) -> bool {
        self.tabstop_groups
            .get(self.active)
            .is_some_and(|group| group.as_slice() == ranges)
    }

    fn is_valid_for_buffer_len(&self, len_chars: usize) -> bool {
        self.active < self.tabstop_groups.len()
            && self.tabstop_groups.len() <= MAX_SNIPPET_SESSION_GROUPS
            && self.tabstop_groups.iter().map(Vec::len).sum::<usize>() <= MAX_SNIPPET_SESSION_RANGES
            && self
                .tabstop_groups
                .iter()
                .flatten()
                .all(|range| valid_tabstop_range(range) && range.end <= len_chars)
    }
}

pub(crate) fn move_snippet_session(
    session: &mut Option<SnippetSession>,
    buffer: &mut TextBuffer,
    buffer_id: BufferId,
    backwards: bool,
) -> bool {
    let Some(active_session) = session.as_mut() else {
        return false;
    };
    if active_session.buffer_id != buffer_id
        || active_session.tabstop_groups.is_empty()
        || !active_session.is_valid_for_buffer_len(buffer.len_chars())
    {
        *session = None;
        return false;
    }

    let next = if backwards {
        active_session.active.checked_sub(1)
    } else {
        active_session.active.checked_add(1)
    };
    let Some(next) = next else {
        return true;
    };
    if next >= active_session.tabstop_groups.len() {
        *session = None;
        return true;
    }

    active_session.active = next;
    let ranges = &active_session.tabstop_groups[next];
    buffer.set_selections(ranges.iter().map(|range| Selection {
        anchor: range.start,
        cursor: range.end,
    }));
    true
}

impl KuroyaApp {
    pub(crate) fn move_snippet_session_for_buffer(
        &mut self,
        buffer_id: BufferId,
        backwards: bool,
    ) -> bool {
        let mut session = self.snippet_session.take();
        let moved = if let Some(buffer) = self.buffer_mut(buffer_id) {
            move_snippet_session(&mut session, buffer, buffer_id, backwards)
        } else {
            false
        };
        self.snippet_session = session;
        moved
    }

    pub(crate) fn update_snippet_session_after_active_edit(
        &mut self,
        buffer_id: BufferId,
        old_range: Range<usize>,
        new_cursor: usize,
        delta: isize,
    ) {
        let len_chars = self.buffer(buffer_id).map(TextBuffer::len_chars);
        let Some(session) = self.snippet_session.as_mut() else {
            return;
        };
        if session.buffer_id != buffer_id {
            return;
        }
        if len_chars.is_some_and(|len| new_cursor > len)
            || !session.update_after_active_edit(old_range, new_cursor, delta)
            || len_chars.is_some_and(|len| !session.is_valid_for_buffer_len(len))
        {
            self.snippet_session = None;
        }
    }

    pub(crate) fn active_snippet_ranges_slice_for_buffer(
        &self,
        buffer_id: BufferId,
    ) -> Option<&[Range<usize>]> {
        self.snippet_session
            .as_ref()
            .and_then(|session| session.active_ranges(buffer_id))
    }

    pub(crate) fn has_active_snippet_ranges_for_buffer(&self, buffer_id: BufferId) -> bool {
        self.active_snippet_ranges_slice_for_buffer(buffer_id)
            .is_some()
    }

    pub(crate) fn update_snippet_session_after_active_group_edit(
        &mut self,
        buffer_id: BufferId,
        old_ranges: &[Range<usize>],
        new_ranges: &[Range<usize>],
    ) -> bool {
        let len_chars = self.buffer(buffer_id).map(TextBuffer::len_chars);
        let Some(session) = self.snippet_session.as_mut() else {
            return false;
        };
        if session.buffer_id != buffer_id {
            return false;
        }
        if session.update_after_active_group_edit(old_ranges, new_ranges)
            && len_chars.is_none_or(|len| session.is_valid_for_buffer_len(len))
        {
            true
        } else {
            self.snippet_session = None;
            false
        }
    }

    pub(crate) fn clear_snippet_session_for_buffer(&mut self, buffer_id: BufferId) {
        if self
            .snippet_session
            .as_ref()
            .is_some_and(|session| session.buffer_id == buffer_id)
        {
            self.snippet_session = None;
        }
    }
}

fn shift_range(range: Range<usize>, delta: isize) -> Range<usize> {
    shift_index(range.start, delta)..shift_index(range.end, delta)
}

fn shift_range_through_replacements(
    range: Range<usize>,
    old_ranges: &[Range<usize>],
    new_ranges: &[Range<usize>],
) -> Range<usize> {
    shift_index_through_replacements(range.start, old_ranges, new_ranges)
        ..shift_index_through_replacements(range.end, old_ranges, new_ranges)
}

fn shift_index_through_replacements(
    index: usize,
    old_ranges: &[Range<usize>],
    new_ranges: &[Range<usize>],
) -> usize {
    let mut shifted = index;
    for (old, new) in old_ranges.iter().zip(new_ranges) {
        let old_len = old.end.saturating_sub(old.start);
        let new_len = new.end.saturating_sub(new.start);
        let should_shift = if old.start == old.end {
            index >= old.start
        } else {
            index >= old.end
        };
        if should_shift {
            if new_len >= old_len {
                shifted = shifted.saturating_add(new_len - old_len);
            } else {
                shifted = shifted.saturating_sub(old_len - new_len);
            }
        }
    }
    shifted
}

fn shift_index(index: usize, delta: isize) -> usize {
    if delta.is_negative() {
        index.saturating_sub(delta.unsigned_abs())
    } else {
        index.saturating_add(delta as usize)
    }
}

fn sanitize_tabstop_groups(tabstop_groups: Vec<Vec<Range<usize>>>) -> Vec<Vec<Range<usize>>> {
    let mut sanitized = Vec::new();
    let mut total_ranges = 0usize;
    for group in tabstop_groups.into_iter().take(MAX_SNIPPET_SESSION_GROUPS) {
        let mut sanitized_group = Vec::new();
        for range in group {
            if total_ranges >= MAX_SNIPPET_SESSION_RANGES {
                break;
            }
            if valid_tabstop_range(&range) {
                sanitized_group.push(range);
                total_ranges += 1;
            }
        }
        if !sanitized_group.is_empty() {
            sanitized.push(sanitized_group);
        }
        if total_ranges >= MAX_SNIPPET_SESSION_RANGES {
            break;
        }
    }
    sanitized
}

fn valid_group_replacement(old_ranges: &[Range<usize>], new_ranges: &[Range<usize>]) -> bool {
    !old_ranges.is_empty()
        && old_ranges.len() == new_ranges.len()
        && old_ranges.iter().all(valid_tabstop_range)
        && new_ranges.iter().all(valid_tabstop_range)
        && ordered_non_overlapping_ranges(old_ranges)
        && ordered_non_overlapping_ranges(new_ranges)
}

fn valid_tabstop_range(range: &Range<usize>) -> bool {
    range.start <= range.end
}

fn ordered_non_overlapping_ranges(ranges: &[Range<usize>]) -> bool {
    let mut previous_end = None;
    for range in ranges {
        if let Some(end) = previous_end
            && range.start < end
        {
            return false;
        }
        previous_end = Some(range.end);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_SNIPPET_SESSION_GROUPS, MAX_SNIPPET_SESSION_RANGES, SnippetSession,
        move_snippet_session,
    };
    use kuroya_core::{Selection, TextBuffer};

    #[test]
    fn snippet_session_moves_forward_and_exits_after_last_tabstop() {
        let mut buffer = TextBuffer::from_text(1, None, "println!(value, other);".to_owned());
        let mut session =
            SnippetSession::new_grouped(1, vec![vec![9..14], vec![16..21], vec![23..23]]);
        buffer.set_selection(9, 14);

        assert!(move_snippet_session(&mut session, &mut buffer, 1, false));
        assert_eq!(
            buffer.selections(),
            &[Selection {
                anchor: 16,
                cursor: 21
            }]
        );
        assert!(move_snippet_session(&mut session, &mut buffer, 1, false));
        assert_eq!(
            buffer.selections(),
            &[Selection {
                anchor: 23,
                cursor: 23
            }]
        );
        assert!(move_snippet_session(&mut session, &mut buffer, 1, false));
        assert!(session.is_none());
    }

    #[test]
    fn snippet_session_updates_future_ranges_after_active_edit() {
        let mut session = SnippetSession::new_grouped(1, vec![vec![9..14], vec![16..21]]).unwrap();
        assert!(session.update_after_active_edit(9..14, 12, -2));

        assert_eq!(session.tabstop_groups, vec![vec![12..12], vec![14..19]]);
    }

    #[test]
    fn snippet_session_moves_to_linked_tabstop_group() {
        let mut buffer = TextBuffer::from_text(1, None, "value = value; next".to_owned());
        let mut session = SnippetSession::new_grouped(1, vec![vec![0..5, 8..13], vec![15..19]]);
        buffer.set_selections([0..5, 8..13].into_iter().map(|range| Selection {
            anchor: range.start,
            cursor: range.end,
        }));

        assert!(move_snippet_session(&mut session, &mut buffer, 1, false));
        assert_eq!(
            buffer.selections(),
            &[Selection {
                anchor: 15,
                cursor: 19
            }]
        );
    }

    #[test]
    fn snippet_session_updates_grouped_future_ranges_after_linked_edit() {
        let mut session =
            SnippetSession::new_grouped(1, vec![vec![0..5, 8..13], vec![15..19]]).unwrap();

        assert!(session.update_after_active_group_edit(&[0..5, 8..13], &[3..3, 9..9]));

        assert_eq!(session.tabstop_groups, vec![vec![3..3, 9..9], vec![5..9]]);
    }

    #[test]
    fn snippet_session_drops_invalid_tabstop_ranges_on_creation() {
        let invalid_range = std::ops::Range { start: 5, end: 3 };
        let session =
            SnippetSession::new_grouped(1, vec![vec![invalid_range], vec![], vec![2..4, 8..8]])
                .unwrap();

        assert_eq!(session.tabstop_groups, vec![vec![2..4, 8..8]]);
    }

    #[test]
    fn snippet_session_rejects_mismatched_group_edit_without_mutation() {
        let mut session =
            SnippetSession::new_grouped(1, vec![vec![0..5, 8..13], vec![15..19]]).unwrap();
        let before = session.clone();
        let replacement_ranges = vec![std::ops::Range { start: 3, end: 3 }];

        assert!(!session.update_after_active_group_edit(&[0..5, 8..13], &replacement_ranges));

        assert_eq!(session, before);
    }

    #[test]
    fn snippet_session_rejects_overlapping_group_edit_without_mutation() {
        let mut session =
            SnippetSession::new_grouped(1, vec![vec![0..5, 4..9], vec![12..16]]).unwrap();
        let before = session.clone();

        assert!(!session.update_after_active_group_edit(&[0..5, 4..9], &[1..1, 2..2]));

        assert_eq!(session, before);
    }

    #[test]
    fn snippet_session_rejects_stale_active_edit_without_mutation() {
        let mut session = SnippetSession::new_grouped(1, vec![vec![9..14], vec![16..21]]).unwrap();
        let before = session.clone();

        assert!(!session.update_after_active_edit(10..14, 12, -2));

        assert_eq!(session, before);
    }

    #[test]
    fn snippet_session_rejects_stale_group_edit_without_mutation() {
        let mut session =
            SnippetSession::new_grouped(1, vec![vec![0..5, 8..13], vec![15..19]]).unwrap();
        let before = session.clone();
        let old_ranges = std::iter::once(0..5).collect::<Vec<_>>();
        let new_ranges = std::iter::once(3..3).collect::<Vec<_>>();

        assert!(!session.update_after_active_group_edit(&old_ranges, &new_ranges));

        assert_eq!(session, before);
    }

    #[test]
    fn snippet_session_bounds_tabstop_metadata_on_creation() {
        let groups = (0..MAX_SNIPPET_SESSION_GROUPS + 4)
            .map(|group| {
                (0..8)
                    .map(|idx| {
                        let start = group * 16 + idx;
                        start..start + 1
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let session = SnippetSession::new_grouped(1, groups).unwrap();

        assert!(session.tabstop_groups.len() <= MAX_SNIPPET_SESSION_GROUPS);
        assert!(
            session.tabstop_groups.iter().map(Vec::len).sum::<usize>()
                <= MAX_SNIPPET_SESSION_RANGES
        );
    }

    #[test]
    fn snippet_session_drops_stale_out_of_bounds_session_on_move() {
        let mut buffer = TextBuffer::from_text(1, None, "short".to_owned());
        let mut session = Some(SnippetSession {
            buffer_id: 1,
            tabstop_groups: vec![vec![0..8]],
            active: 0,
        });

        assert!(!move_snippet_session(&mut session, &mut buffer, 1, false));
        assert!(session.is_none());
    }
}
