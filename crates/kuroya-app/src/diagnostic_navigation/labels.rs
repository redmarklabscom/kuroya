#[cfg(test)]
use crate::diagnostic_location::diagnostic_jump_location;
use crate::{diagnostics_panel::diagnostic_display_path, lsp_labels::diagnostic_message_summary};
use kuroya_core::Diagnostic;
use std::fmt::Write as _;
#[cfg(test)]
use std::{
    borrow::Cow,
    collections::{HashMap, hash_map::Entry},
    hash::Hash,
    path::Path,
};

#[cfg(test)]
pub(super) struct DiagnosticNavigationLabelCache<'a> {
    path_labels: HashMap<&'a Path, String>,
    message_summaries: HashMap<&'a str, String>,
    max_entries: usize,
}

#[cfg(test)]
impl<'a> DiagnosticNavigationLabelCache<'a> {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.min(super::MAX_DIAGNOSTIC_NAVIGATION_CACHE_ENTRIES);
        Self {
            path_labels: HashMap::with_capacity(capacity),
            message_summaries: HashMap::with_capacity(capacity),
            max_entries: capacity,
        }
    }

    pub(super) fn label(
        &mut self,
        diagnostic: &'a Diagnostic,
        line: usize,
        column: usize,
    ) -> String {
        let Self {
            path_labels,
            message_summaries,
            max_entries,
        } = self;
        let max_entries = *max_entries;

        let path = diagnostic.path.as_path();
        let path_label = diagnostic_navigation_cached_text(path_labels, max_entries, path, || {
            diagnostic_display_path(path)
        });
        let message = diagnostic.message.as_str();
        let message_summary =
            diagnostic_navigation_cached_text(message_summaries, max_entries, message, || {
                diagnostic_message_summary(message)
            });
        diagnostic_label_with_parts(line, column, path_label.as_ref(), message_summary.as_ref())
    }

    pub(super) fn path_label_count(&self) -> usize {
        self.path_labels.len()
    }

    pub(super) fn message_summary_count(&self) -> usize {
        self.message_summaries.len()
    }
}

#[cfg(test)]
fn diagnostic_navigation_cached_text<'cache, K>(
    cache: &'cache mut HashMap<K, String>,
    max_entries: usize,
    key: K,
    build: impl FnOnce() -> String,
) -> Cow<'cache, str>
where
    K: Eq + Hash,
{
    let can_cache = cache.len() < max_entries;
    match cache.entry(key) {
        Entry::Occupied(entry) => Cow::Borrowed(entry.into_mut().as_str()),
        Entry::Vacant(entry) if can_cache => Cow::Borrowed(entry.insert(build()).as_str()),
        Entry::Vacant(_) => Cow::Owned(build()),
    }
}

#[cfg(test)]
pub(super) fn diagnostic_label(diagnostic: &Diagnostic) -> String {
    let (line, column) = diagnostic_jump_location(diagnostic);
    diagnostic_label_at_location(diagnostic, line, column)
}

pub(super) fn diagnostic_label_at_location(
    diagnostic: &Diagnostic,
    line: usize,
    column: usize,
) -> String {
    let path = diagnostic_display_path(&diagnostic.path);
    let message = diagnostic_message_summary(&diagnostic.message);
    diagnostic_label_with_parts(line, column, &path, &message)
}

fn diagnostic_label_with_parts(line: usize, column: usize, path: &str, message: &str) -> String {
    let mut label = String::with_capacity(path.len() + message.len() + 24);
    let _ = write!(label, "{path}:{line}:{column} {message}");
    label
}
