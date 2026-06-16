use crate::{
    persistence::{RecoveredBuffer, SkippedRecoveredBuffer},
    persistence_models::{
        PERSISTED_SESSION_DISPLAY_TEXT_MAX_CHARS, PERSISTED_SESSION_RECOVERY_BUFFERS_MAX,
        PERSISTED_SESSION_RECOVERY_SKIPPED_MAX, PERSISTED_SESSION_RECOVERY_TEXT_MAX_CHARS,
    },
    save_lifecycle::buffer_display_name,
};
use kuroya_core::{TextBuffer, TextSnapshot};
use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
};

pub(crate) const RECOVERY_BUFFER_MAX_BYTES: usize = 1024 * 1024;
pub(crate) const RECOVERY_SESSION_MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Default, PartialEq)]
pub(crate) struct RecoverySnapshot {
    pub(crate) recovered: Vec<RecoveredBuffer>,
    pub(crate) skipped: Vec<SkippedRecoveredBuffer>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RecoverySnapshotDraft {
    pub(crate) recovered: Vec<RecoveredBufferSnapshot>,
    pub(crate) skipped: Vec<SkippedRecoveredBuffer>,
}

#[derive(Debug, Clone)]
pub(crate) struct RecoveredBufferSnapshot {
    pub(crate) path: Option<std::path::PathBuf>,
    pub(crate) display_name: String,
    pub(crate) text: TextSnapshot,
}

impl RecoverySnapshotDraft {
    pub(crate) fn into_recovery_snapshot(self) -> RecoverySnapshot {
        RecoverySnapshot {
            recovered: self
                .recovered
                .into_iter()
                .take(PERSISTED_SESSION_RECOVERY_BUFFERS_MAX)
                .map(RecoveredBufferSnapshot::into_recovered_buffer)
                .collect(),
            skipped: self
                .skipped
                .into_iter()
                .take(PERSISTED_SESSION_RECOVERY_SKIPPED_MAX)
                .map(sanitized_skipped_recovered_buffer)
                .collect(),
        }
    }
}

impl RecoveredBufferSnapshot {
    fn into_recovered_buffer(self) -> RecoveredBuffer {
        RecoveredBuffer {
            path: self.path,
            display_name: bounded_string_chars(
                self.display_name,
                PERSISTED_SESSION_DISPLAY_TEXT_MAX_CHARS,
            ),
            text: bounded_string_chars(self.text.text(), PERSISTED_SESSION_RECOVERY_TEXT_MAX_CHARS),
        }
    }
}

fn sanitized_skipped_recovered_buffer(
    mut skipped: SkippedRecoveredBuffer,
) -> SkippedRecoveredBuffer {
    truncate_string_chars(
        &mut skipped.display_name,
        PERSISTED_SESSION_DISPLAY_TEXT_MAX_CHARS,
    );
    truncate_string_chars(
        &mut skipped.reason,
        PERSISTED_SESSION_DISPLAY_TEXT_MAX_CHARS,
    );
    skipped
}

fn bounded_string_chars(mut value: String, max_chars: usize) -> String {
    truncate_string_chars(&mut value, max_chars);
    value
}

fn truncate_string_chars(value: &mut String, max_chars: usize) {
    if max_chars == 0 {
        value.clear();
        return;
    }
    if let Some((byte_index, _)) = value.char_indices().nth(max_chars) {
        value.truncate(byte_index);
    }
}

pub(crate) fn recovery_snapshot_for_buffers(
    buffers: &[TextBuffer],
    per_buffer_limit: usize,
    session_limit: usize,
) -> RecoverySnapshot {
    recovery_snapshot_draft_for_buffers(buffers, per_buffer_limit, session_limit)
        .into_recovery_snapshot()
}

pub(crate) fn recovery_snapshot_draft_for_buffers(
    buffers: &[TextBuffer],
    per_buffer_limit: usize,
    session_limit: usize,
) -> RecoverySnapshotDraft {
    let mut snapshot = RecoverySnapshotDraft::default();
    let mut used_bytes = 0usize;
    let path_winners =
        recovery_path_winners_for_duplicate_check(buffers, per_buffer_limit, session_limit);

    for (buffer_index, buffer) in buffers
        .iter()
        .enumerate()
        .filter(|(_, buffer)| buffer.is_dirty())
    {
        let bytes = buffer.len_bytes();
        let display_name = buffer_display_name(buffer);
        if snapshot.recovered.len() >= PERSISTED_SESSION_RECOVERY_BUFFERS_MAX {
            push_skipped_recovered_buffer(
                &mut snapshot,
                SkippedRecoveredBuffer {
                    path: buffer.path().cloned(),
                    display_name,
                    bytes,
                    reason: format!(
                        "exceeds recovery buffer count limit ({PERSISTED_SESSION_RECOVERY_BUFFERS_MAX} buffers)"
                    ),
                },
            );
            continue;
        }
        if bytes > per_buffer_limit {
            push_skipped_recovered_buffer(
                &mut snapshot,
                SkippedRecoveredBuffer {
                    path: buffer.path().cloned(),
                    display_name,
                    bytes,
                    reason: format!("exceeds per-buffer recovery limit ({per_buffer_limit} bytes)"),
                },
            );
            continue;
        }
        if used_bytes.saturating_add(bytes) > session_limit {
            push_skipped_recovered_buffer(
                &mut snapshot,
                SkippedRecoveredBuffer {
                    path: buffer.path().cloned(),
                    display_name,
                    bytes,
                    reason: format!("exceeds total recovery limit ({session_limit} bytes)"),
                },
            );
            continue;
        }
        if !path_winners.is_empty()
            && let Some(path) = buffer.path()
            && path_winners
                .get(&recovery_path_key(path))
                .is_some_and(|winner_index| *winner_index != buffer_index)
        {
            push_skipped_recovered_buffer(
                &mut snapshot,
                SkippedRecoveredBuffer {
                    path: Some(path.clone()),
                    display_name,
                    bytes,
                    reason: "duplicate recovery path already captured by newer dirty buffer"
                        .to_owned(),
                },
            );
            continue;
        }

        used_bytes = used_bytes.saturating_add(bytes);
        snapshot.recovered.push(RecoveredBufferSnapshot {
            path: buffer.path().cloned(),
            display_name,
            text: buffer.text_snapshot(),
        });
    }

    snapshot
}

fn push_skipped_recovered_buffer(
    snapshot: &mut RecoverySnapshotDraft,
    skipped: SkippedRecoveredBuffer,
) {
    if snapshot.skipped.len() < PERSISTED_SESSION_RECOVERY_SKIPPED_MAX {
        snapshot.skipped.push(skipped);
    }
}

pub(crate) fn recovery_path_winners(
    buffers: &[TextBuffer],
    per_buffer_limit: usize,
    session_limit: usize,
) -> HashMap<std::path::PathBuf, usize> {
    collect_recovery_path_winners(buffers, per_buffer_limit, session_limit).0
}

fn recovery_path_winners_for_duplicate_check(
    buffers: &[TextBuffer],
    per_buffer_limit: usize,
    session_limit: usize,
) -> HashMap<std::path::PathBuf, usize> {
    let (winners, has_duplicate_paths) =
        collect_recovery_path_winners(buffers, per_buffer_limit, session_limit);
    if has_duplicate_paths {
        winners
    } else {
        HashMap::new()
    }
}

fn collect_recovery_path_winners(
    buffers: &[TextBuffer],
    per_buffer_limit: usize,
    session_limit: usize,
) -> (HashMap<std::path::PathBuf, usize>, bool) {
    let mut winners = HashMap::new();
    let mut has_duplicate_paths = false;
    for (index, buffer) in buffers.iter().enumerate() {
        if !buffer.is_dirty() {
            continue;
        }
        let bytes = buffer.len_bytes();
        if bytes > per_buffer_limit || bytes > session_limit {
            continue;
        }
        let Some(path) = buffer.path() else {
            continue;
        };
        let key = recovery_path_key(path);
        let version = buffer.version();
        winners
            .entry(key)
            .and_modify(|(winner_index, winner_version)| {
                has_duplicate_paths = true;
                if version > *winner_version
                    || (version == *winner_version && index > *winner_index)
                {
                    *winner_index = index;
                    *winner_version = version;
                }
            })
            .or_insert((index, version));
    }

    (
        winners
            .into_iter()
            .map(|(path, (index, _))| (path, index))
            .collect(),
        has_duplicate_paths,
    )
}

pub(crate) fn recovery_path_key(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut has_root = false;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => {
                has_root = true;
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop_normal = normalized
                    .components()
                    .next_back()
                    .is_some_and(|component| matches!(component, Component::Normal(_)));
                if can_pop_normal {
                    normalized.pop();
                } else if !has_root {
                    normalized.push("..");
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    let normalized = if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    };
    recovery_case_key(normalized)
}

#[cfg(windows)]
fn recovery_case_key(path: PathBuf) -> PathBuf {
    PathBuf::from(path.as_os_str().to_string_lossy().to_lowercase())
}

#[cfg(not(windows))]
fn recovery_case_key(path: PathBuf) -> PathBuf {
    path
}

#[cfg(test)]
mod tests {
    use super::{
        RecoveredBufferSnapshot, RecoverySnapshotDraft, bounded_string_chars,
        recovery_path_winners_for_duplicate_check, recovery_snapshot_for_buffers,
    };
    use crate::{
        persistence::SkippedRecoveredBuffer,
        persistence_models::{
            PERSISTED_SESSION_DISPLAY_TEXT_MAX_CHARS, PERSISTED_SESSION_RECOVERY_BUFFERS_MAX,
            PERSISTED_SESSION_RECOVERY_SKIPPED_MAX, PERSISTED_SESSION_RECOVERY_TEXT_MAX_CHARS,
        },
    };
    use kuroya_core::TextBuffer;
    use std::path::PathBuf;

    #[test]
    fn duplicate_check_winners_are_empty_for_unique_dirty_paths() {
        let mut first = TextBuffer::from_text(
            1,
            Some(PathBuf::from("workspace/src/main.rs")),
            "first".to_owned(),
        );
        first.mark_dirty();
        let mut second = TextBuffer::from_text(
            2,
            Some(PathBuf::from("workspace/src/lib.rs")),
            "second".to_owned(),
        );
        second.mark_dirty();

        let winners = recovery_path_winners_for_duplicate_check(&[first, second], 32, 128);

        assert!(winners.is_empty());
    }

    #[test]
    fn duplicate_check_winners_keep_equivalent_dirty_paths() {
        let direct_path = PathBuf::from("workspace/src/main.rs");
        let equivalent_path = PathBuf::from("workspace/src/../src/main.rs");
        let mut older = TextBuffer::from_text(1, Some(equivalent_path), "older".to_owned());
        older.mark_dirty();
        let mut newer = TextBuffer::from_text(2, Some(direct_path), "new".to_owned());
        assert!(newer.replace_range(3..3, "er"));

        let winners = recovery_path_winners_for_duplicate_check(&[older, newer], 32, 128);

        assert_eq!(winners.len(), 1);
        assert_eq!(
            winners.get(&PathBuf::from("workspace/src/main.rs")),
            Some(&1)
        );
    }

    #[test]
    fn recovery_snapshot_draft_bounds_persisted_text_metadata() {
        let path = PathBuf::from("workspace/src/raw\nname.rs");
        let display_name = "label".repeat(PERSISTED_SESSION_DISPLAY_TEXT_MAX_CHARS);
        let reason = "reason".repeat(PERSISTED_SESSION_DISPLAY_TEXT_MAX_CHARS);
        let text = format!(
            "{}tail",
            "x".repeat(PERSISTED_SESSION_RECOVERY_TEXT_MAX_CHARS)
        );
        let buffer = TextBuffer::from_text(7, Some(path.clone()), text);

        let snapshot = RecoverySnapshotDraft {
            recovered: vec![RecoveredBufferSnapshot {
                path: Some(path.clone()),
                display_name: display_name.clone(),
                text: buffer.text_snapshot(),
            }],
            skipped: vec![SkippedRecoveredBuffer {
                path: Some(path.clone()),
                display_name,
                bytes: 10,
                reason,
            }],
        }
        .into_recovery_snapshot();

        assert_eq!(snapshot.recovered[0].path, Some(path.clone()));
        assert_eq!(
            snapshot.recovered[0].display_name.chars().count(),
            PERSISTED_SESSION_DISPLAY_TEXT_MAX_CHARS
        );
        assert_eq!(
            snapshot.recovered[0].text.chars().count(),
            PERSISTED_SESSION_RECOVERY_TEXT_MAX_CHARS
        );
        assert_eq!(snapshot.skipped[0].path, Some(path));
        assert_eq!(
            snapshot.skipped[0].display_name.chars().count(),
            PERSISTED_SESSION_DISPLAY_TEXT_MAX_CHARS
        );
        assert_eq!(
            snapshot.skipped[0].reason.chars().count(),
            PERSISTED_SESSION_DISPLAY_TEXT_MAX_CHARS
        );
    }

    #[test]
    fn recovery_metadata_truncation_preserves_utf8_boundaries() {
        assert_eq!(
            bounded_string_chars("a\u{00e9}\u{1f642}z".to_owned(), 3),
            "a\u{00e9}\u{1f642}"
        );
    }

    #[test]
    fn recovery_snapshot_caps_zero_byte_dirty_buffers_by_entry_count() {
        let mut buffers = Vec::new();
        for index in
            0..(PERSISTED_SESSION_RECOVERY_BUFFERS_MAX + PERSISTED_SESSION_RECOVERY_SKIPPED_MAX + 3)
        {
            let mut buffer = TextBuffer::from_text(
                index as u64,
                Some(PathBuf::from(format!("workspace/file-{index}.rs"))),
                String::new(),
            );
            buffer.mark_dirty();
            buffers.push(buffer);
        }

        let snapshot = recovery_snapshot_for_buffers(&buffers, 0, 0);

        assert_eq!(
            snapshot.recovered.len(),
            PERSISTED_SESSION_RECOVERY_BUFFERS_MAX
        );
        assert_eq!(
            snapshot.skipped.len(),
            PERSISTED_SESSION_RECOVERY_SKIPPED_MAX
        );
        assert!(snapshot.skipped.iter().all(|skipped| {
            skipped
                .reason
                .starts_with("exceeds recovery buffer count limit")
        }));
    }

    #[test]
    fn recovery_snapshot_draft_conversion_enforces_entry_count_caps() {
        let template = TextBuffer::from_text(1, None, String::new()).text_snapshot();
        let draft = RecoverySnapshotDraft {
            recovered: (0..(PERSISTED_SESSION_RECOVERY_BUFFERS_MAX + 1))
                .map(|index| RecoveredBufferSnapshot {
                    path: None,
                    display_name: format!("scratch-{index}"),
                    text: template.clone(),
                })
                .collect(),
            skipped: (0..(PERSISTED_SESSION_RECOVERY_SKIPPED_MAX + 1))
                .map(|index| SkippedRecoveredBuffer {
                    path: None,
                    display_name: format!("scratch-{index}"),
                    bytes: 0,
                    reason: "count cap".to_owned(),
                })
                .collect(),
        };

        let snapshot = draft.into_recovery_snapshot();

        assert_eq!(
            snapshot.recovered.len(),
            PERSISTED_SESSION_RECOVERY_BUFFERS_MAX
        );
        assert_eq!(
            snapshot.skipped.len(),
            PERSISTED_SESSION_RECOVERY_SKIPPED_MAX
        );
    }
}
