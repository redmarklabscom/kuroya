use crate::{KuroyaApp, lsp_labels::severity_label};
use kuroya_core::{Diagnostic, DiagnosticSet, DiagnosticSeverity};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

#[path = "diagnostic_navigation/labels.rs"]
mod labels;
#[path = "diagnostic_navigation/targets.rs"]
mod targets;

#[cfg(test)]
use crate::diagnostic_location::diagnostic_jump_location;
use labels::diagnostic_label_at_location;
#[cfg(test)]
use labels::{DiagnosticNavigationLabelCache, diagnostic_label};
use targets::{
    DiagnosticBufferLookup, DiagnosticOpenTarget, DiagnosticTargetOpenability,
    DiagnosticTargetOpenabilityCache, diagnostic_open_target_for_resolved_path,
    diagnostic_path_is_openable_file, diagnostic_resolved_jump_location,
    diagnostic_target_openability_cached, diagnostic_target_openability_for,
    diagnostic_untitled_path,
};
#[cfg(test)]
use targets::{diagnostic_buffer_id_for_buffers, diagnostic_path_key};

const MAX_DIAGNOSTIC_NAVIGATION_CACHE_ENTRIES: usize = 64;

impl KuroyaApp {
    pub(crate) fn goto_diagnostic(&mut self, direction: isize) {
        let diagnostic_count = self.diagnostics.len();
        if diagnostic_count == 0 {
            self.status = "No diagnostics".to_owned();
            return;
        }

        let target = {
            let anchor: Option<(Cow<'_, Path>, usize, usize)> =
                self.active_buffer().map(|buffer| {
                    let position = buffer.cursor_position();
                    let path = match buffer.path() {
                        Some(path) => Cow::Borrowed(path.as_path()),
                        None => Cow::Owned(diagnostic_untitled_path(buffer.id())),
                    };
                    (path, position.line + 1, position.column + 1)
                });

            let buffer_lookup = DiagnosticBufferLookup::new(&self.buffers);
            let indexed_files = self.index.files();
            let ordered_diagnostics = self.diagnostics.all_sorted();
            let start_index = diagnostic_navigation_start_index(
                &self.diagnostics,
                &ordered_diagnostics,
                anchor
                    .as_ref()
                    .map(|(path, line, column)| (path.as_ref(), *line, *column)),
                direction,
            );
            let mut target_openability_cache =
                DiagnosticTargetOpenabilityCache::with_capacity(diagnostic_count);
            let mut target = None;
            if let Some(start_index) = start_index {
                for offset in 0..diagnostic_count {
                    let diagnostic_index = diagnostic_navigation_index(
                        start_index,
                        offset,
                        diagnostic_count,
                        direction,
                    );
                    let Some(diagnostic) = ordered_diagnostics.get(diagnostic_index).copied()
                    else {
                        continue;
                    };
                    let path = diagnostic.path.as_path();
                    if let Some(openability) = diagnostic_target_openability_cached(
                        &mut target_openability_cache,
                        &buffer_lookup,
                        indexed_files,
                        path,
                        diagnostic_path_is_openable_file,
                    ) {
                        if let Some((line, column)) = diagnostic_resolved_jump_location(
                            &self.buffers,
                            diagnostic,
                            openability,
                        ) {
                            target = Some((
                                path.to_path_buf(),
                                line,
                                column,
                                diagnostic.severity,
                                diagnostic_label_at_location(diagnostic, line, column),
                                openability,
                            ));
                            break;
                        }
                    }
                }
            }
            target
        };

        let Some((path, line, column, severity, label, openability)) = target else {
            self.status = "No available diagnostics".to_owned();
            return;
        };
        self.open_diagnostic(path, line, column, severity, label, openability);
    }

    fn open_diagnostic(
        &mut self,
        path: PathBuf,
        line: usize,
        column: usize,
        severity: DiagnosticSeverity,
        label: String,
        openability: DiagnosticTargetOpenability,
    ) {
        if !self.open_resolved_diagnostic_target(DiagnosticOpenTarget {
            path,
            line,
            column,
            openability,
        }) {
            return;
        }
        self.status = format!("{} diagnostic: {label}", severity_label(severity));
    }

    pub(crate) fn diagnostic_open_target(
        &self,
        diagnostic: &Diagnostic,
    ) -> Option<DiagnosticOpenTarget> {
        let openability = self.diagnostic_target_openability(&diagnostic.path)?;
        let (line, column) =
            diagnostic_resolved_jump_location(&self.buffers, diagnostic, openability)?;
        Some(diagnostic_open_target_for_resolved_path(
            &diagnostic.path,
            line,
            column,
            openability,
        ))
    }

    pub(crate) fn open_resolved_diagnostic_target(&mut self, target: DiagnosticOpenTarget) -> bool {
        let Some(openability) = self.diagnostic_target_openability(&target.path) else {
            self.status = "Diagnostic target is no longer available".to_owned();
            return false;
        };
        let openability = if target.openability == openability {
            target.openability
        } else {
            openability
        };

        match openability {
            DiagnosticTargetOpenability::OpenBuffer(id) => {
                self.set_active_buffer(id);
                self.apply_file_jump_with_history(id, target.line, target.column);
            }
            DiagnosticTargetOpenability::OpenableFile => {
                self.open_file_at_known_openable(target.path, target.line, target.column);
            }
        }
        true
    }

    fn diagnostic_target_openability(&self, path: &Path) -> Option<DiagnosticTargetOpenability> {
        let buffer_lookup = DiagnosticBufferLookup::new(&self.buffers);
        diagnostic_target_openability_for(
            &buffer_lookup,
            self.index.files(),
            path,
            diagnostic_path_is_openable_file,
        )
    }

    #[cfg(test)]
    fn diagnostic_open_target_for_path(
        &self,
        path: &Path,
        line: usize,
        column: usize,
    ) -> Option<DiagnosticOpenTarget> {
        let openability = self.diagnostic_target_openability(path)?;
        Some(diagnostic_open_target_for_resolved_path(
            path,
            line,
            column,
            openability,
        ))
    }
}

fn diagnostic_navigation_start_index(
    diagnostics: &DiagnosticSet,
    ordered_diagnostics: &[&Diagnostic],
    anchor: Option<(&Path, usize, usize)>,
    direction: isize,
) -> Option<usize> {
    if ordered_diagnostics.is_empty() {
        return None;
    }

    let Some((path, line, column)) = anchor else {
        return Some(diagnostic_navigation_edge_index(
            ordered_diagnostics.len(),
            direction,
        ));
    };
    let candidate = if direction < 0 {
        diagnostics.previous_before(path, line, column)
    } else {
        diagnostics.next_after(path, line, column)
    };

    candidate
        .and_then(|candidate| diagnostic_sorted_index(ordered_diagnostics, candidate))
        .or_else(|| {
            Some(diagnostic_navigation_edge_index(
                ordered_diagnostics.len(),
                direction,
            ))
        })
}

fn diagnostic_navigation_edge_index(len: usize, direction: isize) -> usize {
    if direction < 0 {
        len.saturating_sub(1)
    } else {
        0
    }
}

fn diagnostic_sorted_index(
    ordered_diagnostics: &[&Diagnostic],
    target: &Diagnostic,
) -> Option<usize> {
    ordered_diagnostics
        .iter()
        .position(|diagnostic| std::ptr::eq(*diagnostic, target))
}

fn diagnostic_navigation_index(
    start_index: usize,
    offset: usize,
    len: usize,
    direction: isize,
) -> usize {
    if len == 0 {
        return 0;
    }
    let start_index = start_index % len;
    let offset = offset % len;
    if direction < 0 {
        if offset <= start_index {
            start_index - offset
        } else {
            len - (offset - start_index)
        }
    } else {
        let remaining = len - start_index;
        if offset < remaining {
            start_index + offset
        } else {
            offset - remaining
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app_startup_context::AppStartupContext, terminal::TerminalPane};
    use kuroya_core::{EditorSettings, TextBuffer, Workspace};
    use std::{
        cell::Cell,
        fs,
        path::PathBuf,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    use tokio::runtime::Runtime;

    #[test]
    fn diagnostic_target_available_accepts_already_open_missing_path() {
        let root = missing_path("workspace-root");
        let target = missing_path("open-target.rs");
        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(target.clone()),
            "open only".to_owned(),
        ));

        assert!(!target.exists());
        assert!(app.diagnostic_target_openability(&target).is_some());
        assert!(
            app.diagnostic_target_openability(&root.join("missing.rs"))
                .is_none()
        );
    }

    #[test]
    fn diagnostic_target_available_accepts_lexically_equivalent_open_path() {
        let root = missing_path("workspace-root");
        let target = root.join("src").join("main.rs");
        let diagnostic_path = root.join("src").join("..").join("src").join("main.rs");
        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(target),
            "open only".to_owned(),
        ));

        assert!(!diagnostic_path.exists());
        assert!(
            app.diagnostic_target_openability(&diagnostic_path)
                .is_some()
        );
        assert!(
            app.diagnostic_target_openability(&root.join("missing.rs"))
                .is_none()
        );
    }

    #[test]
    fn diagnostic_openability_cache_reuses_missing_target_result() {
        let path = PathBuf::from("workspace").join("src").join("missing.rs");
        let buffers = Vec::new();
        let indexed_files = Vec::new();
        let probes = Cell::new(0usize);
        let mut cache = DiagnosticTargetOpenabilityCache::new();

        assert_eq!(
            diagnostic_target_openability_cached(
                &mut cache,
                &DiagnosticBufferLookup::new(&buffers),
                &indexed_files,
                &path,
                |_| {
                    probes.set(probes.get() + 1);
                    false
                },
            ),
            None
        );
        assert_eq!(
            diagnostic_target_openability_cached(
                &mut cache,
                &DiagnosticBufferLookup::new(&buffers),
                &indexed_files,
                &path,
                |_| {
                    probes.set(probes.get() + 1);
                    true
                },
            ),
            None
        );
        assert_eq!(probes.get(), 1);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn diagnostic_openability_cache_reuses_openable_target_result() {
        let path = PathBuf::from("workspace").join("src").join("main.rs");
        let same_path = PathBuf::from("workspace").join("src").join("main.rs");
        let buffers = Vec::new();
        let indexed_files = Vec::new();
        let probes = Cell::new(0usize);
        let mut cache = DiagnosticTargetOpenabilityCache::new();

        assert_eq!(
            diagnostic_target_openability_cached(
                &mut cache,
                &DiagnosticBufferLookup::new(&buffers),
                &indexed_files,
                &path,
                |_| {
                    probes.set(probes.get() + 1);
                    true
                },
            ),
            Some(DiagnosticTargetOpenability::OpenableFile)
        );
        assert_eq!(
            diagnostic_target_openability_cached(
                &mut cache,
                &DiagnosticBufferLookup::new(&buffers),
                &indexed_files,
                &same_path,
                |_| {
                    probes.set(probes.get() + 1);
                    false
                },
            ),
            Some(DiagnosticTargetOpenability::OpenableFile)
        );
        assert_eq!(probes.get(), 1);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn diagnostic_openability_cache_reuses_equivalent_missing_target_result() {
        let path = PathBuf::from("workspace").join("src").join("missing.rs");
        let equivalent_path = PathBuf::from("workspace")
            .join("src")
            .join("..")
            .join("src")
            .join("missing.rs");
        let buffers = Vec::new();
        let indexed_files = Vec::new();
        let probes = Cell::new(0usize);
        let mut cache = DiagnosticTargetOpenabilityCache::new();

        assert_ne!(path, equivalent_path);
        assert_eq!(
            diagnostic_path_key(&path),
            diagnostic_path_key(&equivalent_path)
        );
        assert_eq!(
            diagnostic_target_openability_cached(
                &mut cache,
                &DiagnosticBufferLookup::new(&buffers),
                &indexed_files,
                &path,
                |_| {
                    probes.set(probes.get() + 1);
                    false
                },
            ),
            None
        );
        assert_eq!(
            diagnostic_target_openability_cached(
                &mut cache,
                &DiagnosticBufferLookup::new(&buffers),
                &indexed_files,
                &equivalent_path,
                |_| {
                    probes.set(probes.get() + 1);
                    true
                },
            ),
            None
        );
        assert_eq!(probes.get(), 1);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn diagnostic_openability_cache_reuses_equivalent_openable_target_result() {
        let path = PathBuf::from("workspace").join("src").join("main.rs");
        let equivalent_path = PathBuf::from("workspace")
            .join("src")
            .join("..")
            .join("src")
            .join("main.rs");
        let buffers = Vec::new();
        let indexed_files = Vec::new();
        let probes = Cell::new(0usize);
        let mut cache = DiagnosticTargetOpenabilityCache::new();

        assert_ne!(path, equivalent_path);
        assert_eq!(
            diagnostic_path_key(&path),
            diagnostic_path_key(&equivalent_path)
        );
        assert_eq!(
            diagnostic_target_openability_cached(
                &mut cache,
                &DiagnosticBufferLookup::new(&buffers),
                &indexed_files,
                &path,
                |_| {
                    probes.set(probes.get() + 1);
                    true
                },
            ),
            Some(DiagnosticTargetOpenability::OpenableFile)
        );
        assert_eq!(
            diagnostic_target_openability_cached(
                &mut cache,
                &DiagnosticBufferLookup::new(&buffers),
                &indexed_files,
                &equivalent_path,
                |_| {
                    probes.set(probes.get() + 1);
                    false
                },
            ),
            Some(DiagnosticTargetOpenability::OpenableFile)
        );
        assert_eq!(probes.get(), 1);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn diagnostic_openability_cache_prefers_exact_buffer_over_equivalent_cache_entry() {
        let root = PathBuf::from("workspace");
        let exact_path = root.join("src").join("main.rs");
        let earlier_equivalent_buffer_path =
            root.join("src").join("..").join("src").join("main.rs");
        let diagnostic_equivalent_path = root.join("src").join("nested").join("..").join("main.rs");
        let buffers = vec![
            TextBuffer::from_text(
                1,
                Some(earlier_equivalent_buffer_path),
                "lexical\n".to_owned(),
            ),
            TextBuffer::from_text(2, Some(exact_path.clone()), "exact\n".to_owned()),
        ];
        let indexed_files = Vec::new();
        let probes = Cell::new(0usize);
        let mut cache = DiagnosticTargetOpenabilityCache::new();

        assert_ne!(exact_path, diagnostic_equivalent_path);
        assert_eq!(
            diagnostic_path_key(&exact_path),
            diagnostic_path_key(&diagnostic_equivalent_path)
        );
        assert_eq!(
            diagnostic_target_openability_cached(
                &mut cache,
                &DiagnosticBufferLookup::new(&buffers),
                &indexed_files,
                &diagnostic_equivalent_path,
                |_| {
                    probes.set(probes.get() + 1);
                    false
                },
            ),
            Some(DiagnosticTargetOpenability::OpenBuffer(1))
        );
        assert_eq!(
            diagnostic_target_openability_cached(
                &mut cache,
                &DiagnosticBufferLookup::new(&buffers),
                &indexed_files,
                &exact_path,
                |_| {
                    probes.set(probes.get() + 1);
                    false
                },
            ),
            Some(DiagnosticTargetOpenability::OpenBuffer(2))
        );
        assert_eq!(probes.get(), 0);
    }

    #[test]
    fn diagnostic_openability_cache_bounds_unique_target_results() {
        let paths = (0..4)
            .map(|idx| {
                PathBuf::from("workspace")
                    .join("src")
                    .join(format!("{idx}.rs"))
            })
            .collect::<Vec<_>>();
        let buffers = Vec::new();
        let indexed_files = Vec::new();
        let probes = Cell::new(0usize);
        let mut cache = DiagnosticTargetOpenabilityCache::with_capacity(2);

        for path in &paths {
            assert_eq!(
                diagnostic_target_openability_cached(
                    &mut cache,
                    &DiagnosticBufferLookup::new(&buffers),
                    &indexed_files,
                    path,
                    |_| {
                        probes.set(probes.get() + 1);
                        false
                    },
                ),
                None
            );
        }

        assert_eq!(probes.get(), paths.len());
        assert_eq!(cache.len(), 2);
        assert_eq!(
            diagnostic_target_openability_cached(
                &mut cache,
                &DiagnosticBufferLookup::new(&buffers),
                &indexed_files,
                &paths[0],
                |_| {
                    probes.set(probes.get() + 1);
                    true
                },
            ),
            None
        );
        assert_eq!(
            diagnostic_target_openability_cached(
                &mut cache,
                &DiagnosticBufferLookup::new(&buffers),
                &indexed_files,
                &paths[3],
                |_| {
                    probes.set(probes.get() + 1);
                    true
                },
            ),
            Some(DiagnosticTargetOpenability::OpenableFile)
        );
        assert_eq!(probes.get(), paths.len() + 1);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn diagnostic_openability_uses_open_buffer_without_file_probe() {
        let path = PathBuf::from("workspace").join("src").join("main.rs");
        let buffers = vec![TextBuffer::from_text(
            1,
            Some(path.clone()),
            "open\n".to_owned(),
        )];
        let indexed_files = Vec::new();
        let probes = Cell::new(0usize);

        assert_eq!(
            diagnostic_target_openability_for(
                &DiagnosticBufferLookup::new(&buffers),
                &indexed_files,
                &path,
                |_| {
                    probes.set(probes.get() + 1);
                    false
                },
            ),
            Some(DiagnosticTargetOpenability::OpenBuffer(1))
        );
        assert_eq!(probes.get(), 0);
    }

    #[test]
    fn diagnostic_buffer_lookup_prefers_exact_path_before_lexical_equivalent() {
        let root = PathBuf::from("workspace");
        let path = root.join("src").join("main.rs");
        let equivalent_path = root.join("src").join("..").join("src").join("main.rs");
        let buffers = vec![
            TextBuffer::from_text(1, Some(equivalent_path), "equivalent\n".to_owned()),
            TextBuffer::from_text(2, Some(path.clone()), "exact\n".to_owned()),
        ];

        assert_eq!(diagnostic_buffer_id_for_buffers(&buffers, &path), Some(2));
    }

    #[test]
    fn diagnostic_buffer_lookup_reuses_prebuilt_exact_path_index() {
        let path = PathBuf::from("workspace").join("src").join("main.rs");
        let other_path = PathBuf::from("workspace").join("src").join("lib.rs");
        let buffers = vec![
            TextBuffer::from_text(1, Some(path.clone()), "main\n".to_owned()),
            TextBuffer::from_text(2, Some(other_path), "lib\n".to_owned()),
        ];
        let lookup = DiagnosticBufferLookup::new(&buffers);

        assert_eq!(lookup.id_for_path(&path), Some(1));
        assert_eq!(lookup.id_for_path(Path::new("<untitled-1>")), None);
    }

    #[test]
    fn diagnostic_buffer_lookup_uses_prebuilt_lexical_path_index() {
        let root = PathBuf::from("workspace");
        let path = root.join("src").join("main.rs");
        let equivalent_path = root.join("src").join("..").join("src").join("main.rs");
        let buffers = vec![TextBuffer::from_text(
            1,
            Some(path.clone()),
            "main\n".to_owned(),
        )];
        let lookup = DiagnosticBufferLookup::new(&buffers);

        assert_eq!(lookup.id_for_path(&equivalent_path), Some(1));
        assert_eq!(lookup.lexical_paths.len(), 1);
        assert_eq!(
            diagnostic_path_key(&path),
            diagnostic_path_key(&equivalent_path)
        );
    }

    #[test]
    fn diagnostic_untitled_lookup_requires_exact_generated_label() {
        let buffers = vec![TextBuffer::from_text(3, None, "untitled\n".to_owned())];
        let lookup = DiagnosticBufferLookup::new(&buffers);

        assert_eq!(lookup.id_for_path(Path::new("<untitled-3>")), Some(3));
        assert_eq!(lookup.id_for_path(Path::new("<untitled-03>")), None);
        assert_eq!(
            lookup.id_for_path(Path::new("workspace/<untitled-3>")),
            None
        );
        assert_eq!(lookup.id_for_path(Path::new("<untitled-4>")), None);
    }

    #[test]
    fn diagnostic_navigation_index_order_is_directional_and_wraps() {
        let forward = (0..4)
            .map(|offset| diagnostic_navigation_index(1, offset, 4, 1))
            .collect::<Vec<_>>();
        let previous = (0..4)
            .map(|offset| diagnostic_navigation_index(1, offset, 4, -1))
            .collect::<Vec<_>>();

        assert_eq!(forward, vec![1, 2, 3, 0]);
        assert_eq!(previous, vec![1, 0, 3, 2]);
    }

    #[test]
    fn diagnostic_navigation_index_clamps_stale_extreme_inputs() {
        assert_eq!(diagnostic_navigation_index(usize::MAX, usize::MAX, 4, 1), 2);
        assert_eq!(
            diagnostic_navigation_index(usize::MAX, usize::MAX, 4, -1),
            0
        );
        assert_eq!(diagnostic_navigation_index(usize::MAX, 0, 4, 1), 3);
        assert_eq!(diagnostic_navigation_index(0, usize::MAX, 4, -1), 1);
        assert_eq!(diagnostic_navigation_index(usize::MAX, usize::MAX, 0, 1), 0);
    }

    #[test]
    fn goto_diagnostic_opens_inactive_missing_buffer() {
        let root = missing_path("workspace-root");
        let active_path = root.join("src/active.rs");
        let target_path = missing_path("diagnostic-target.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            1,
            Some(active_path),
            "active\n".to_owned(),
        ));
        app.buffers.push(TextBuffer::from_text(
            2,
            Some(target_path.clone()),
            "one\ntwo\nabcdef\n".to_owned(),
        ));
        app.set_active_buffer(1);
        app.diagnostics.replace(
            target_path.clone(),
            vec![Diagnostic {
                path: target_path.clone(),
                line: 3,
                column: 5,
                char_range: 0..1,
                severity: DiagnosticSeverity::Error,
                source: "rust-analyzer".to_owned(),
                message: "inactive target".to_owned(),
                unused: false,
                deprecated: false,
            }],
        );

        assert!(!target_path.exists());
        app.goto_diagnostic(1);

        assert_eq!(app.active, Some(2));
        let cursor = app.buffer(2).unwrap().cursor_position();
        assert_eq!((cursor.line, cursor.column), (2, 4));
        assert!(app.status.starts_with("error diagnostic: "));
    }

    #[test]
    fn goto_diagnostic_keeps_active_untitled_fallback() {
        let root = missing_path("workspace-root");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            3,
            None,
            "one\ntwo\nabcdef\n".to_owned(),
        ));
        app.set_active_buffer(3);
        let diagnostic_path = PathBuf::from("<untitled-3>");
        app.diagnostics.replace(
            diagnostic_path.clone(),
            vec![Diagnostic {
                path: diagnostic_path,
                line: 3,
                column: 5,
                char_range: 0..1,
                severity: DiagnosticSeverity::Warning,
                source: "kuroya-static".to_owned(),
                message: "untitled target".to_owned(),
                unused: false,
                deprecated: false,
            }],
        );

        app.goto_diagnostic(1);

        assert_eq!(app.active, Some(3));
        let cursor = app.buffer(3).unwrap().cursor_position();
        assert_eq!((cursor.line, cursor.column), (2, 4));
        assert!(app.status.starts_with("warning diagnostic: "));
    }

    #[test]
    fn resolved_diagnostic_target_uses_untitled_buffer_without_file_load() {
        let root = missing_path("workspace-root");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            3,
            None,
            "one\ntwo\nabcdef\n".to_owned(),
        ));

        let target = app
            .diagnostic_open_target_for_path(Path::new("<untitled-3>"), 3, 5)
            .expect("untitled diagnostic target should resolve");
        app.open_resolved_diagnostic_target(target);

        assert_eq!(app.active, Some(3));
        let cursor = app.buffer(3).unwrap().cursor_position();
        assert_eq!((cursor.line, cursor.column), (2, 4));
        assert!(app.pending_open_paths.is_empty());
    }

    #[test]
    fn resolved_diagnostic_target_rejects_stale_open_buffer_path() {
        let root = missing_path("workspace-root");
        let stale_path = missing_path("stale-diagnostic-target.rs");
        let current_path = missing_path("current-buffer.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            3,
            Some(stale_path.clone()),
            "one\ntwo\n".to_owned(),
        ));
        let target = app
            .diagnostic_open_target_for_path(&stale_path, 2, 1)
            .expect("open buffer diagnostic target should resolve");
        app.buffers[0].set_path(current_path);

        assert!(!app.open_resolved_diagnostic_target(target));

        assert_eq!(app.active, None);
        assert_eq!(app.status, "Diagnostic target is no longer available");
    }

    #[test]
    fn diagnostic_open_target_returns_none_for_unavailable_path() {
        let root = missing_path("workspace-root");
        let missing_target = root.join("src").join("missing.rs");
        let app = app_for_test(root);

        assert!(
            app.diagnostic_open_target_for_path(&missing_target, 3, 5)
                .is_none()
        );
    }

    #[test]
    fn diagnostic_open_target_returns_none_for_existing_directory_path() {
        let root = missing_path("workspace-root");
        let directory_target = missing_path("diagnostic-target-dir");
        fs::create_dir_all(&directory_target).unwrap();
        let app = app_for_test(root);

        assert!(directory_target.is_dir());
        assert!(
            app.diagnostic_open_target_for_path(&directory_target, 3, 5)
                .is_none()
        );

        fs::remove_dir_all(directory_target).unwrap();
    }

    #[test]
    fn goto_diagnostic_uses_open_buffer_for_lexically_equivalent_missing_path() {
        let root = missing_path("workspace-root");
        let active_path = root.join("src").join("active.rs");
        let target_path = root.join("src").join("main.rs");
        let diagnostic_path = root.join("src").join("..").join("src").join("main.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            1,
            Some(active_path),
            "active\n".to_owned(),
        ));
        app.buffers.push(TextBuffer::from_text(
            2,
            Some(target_path),
            "one\ntwo\nabcdef\n".to_owned(),
        ));
        app.set_active_buffer(1);
        app.diagnostics.replace(
            diagnostic_path.clone(),
            vec![Diagnostic {
                path: diagnostic_path.clone(),
                line: 3,
                column: 5,
                char_range: 0..1,
                severity: DiagnosticSeverity::Error,
                source: "rust-analyzer".to_owned(),
                message: "equivalent target".to_owned(),
                unused: false,
                deprecated: false,
            }],
        );

        assert!(!diagnostic_path.exists());
        app.goto_diagnostic(1);

        assert_eq!(app.active, Some(2));
        let cursor = app.buffer(2).unwrap().cursor_position();
        assert_eq!((cursor.line, cursor.column), (2, 4));
        assert!(app.status.starts_with("error diagnostic: "));
        assert!(app.status.contains("equivalent target"));
    }

    #[test]
    fn goto_diagnostic_skips_unavailable_targets() {
        let root = missing_path("workspace-root");
        let active_path = root.join("src/0.rs");
        let missing_target = root.join("src/a.rs");
        let open_target = root.join("src/b.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            1,
            Some(active_path),
            "active\n".to_owned(),
        ));
        app.buffers.push(TextBuffer::from_text(
            2,
            Some(open_target.clone()),
            "one\ntwo\nabcdef\n".to_owned(),
        ));
        app.set_active_buffer(1);
        app.diagnostics.replace(
            missing_target.clone(),
            vec![Diagnostic {
                path: missing_target.clone(),
                line: 1,
                column: 1,
                char_range: 0..1,
                severity: DiagnosticSeverity::Error,
                source: "rust-analyzer".to_owned(),
                message: "missing target".to_owned(),
                unused: false,
                deprecated: false,
            }],
        );
        app.diagnostics.replace(
            open_target.clone(),
            vec![Diagnostic {
                path: open_target.clone(),
                line: 3,
                column: 5,
                char_range: 0..1,
                severity: DiagnosticSeverity::Warning,
                source: "rust-analyzer".to_owned(),
                message: "open target".to_owned(),
                unused: false,
                deprecated: false,
            }],
        );

        assert!(!missing_target.exists());
        assert!(!open_target.exists());
        app.goto_diagnostic(1);

        assert_eq!(app.active, Some(2));
        let cursor = app.buffer(2).unwrap().cursor_position();
        assert_eq!((cursor.line, cursor.column), (2, 4));
        assert!(app.status.starts_with("warning diagnostic: "));
        assert!(app.status.contains("open target"));
    }

    #[test]
    fn goto_diagnostic_clamps_invalid_zero_location() {
        let root = missing_path("workspace-root");
        let target_path = missing_path("diagnostic-target.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            2,
            Some(target_path.clone()),
            "one\ntwo\n".to_owned(),
        ));
        app.set_active_buffer(2);
        app.diagnostics.replace(
            target_path.clone(),
            vec![Diagnostic {
                path: target_path,
                line: 0,
                column: 0,
                char_range: 0..1,
                severity: DiagnosticSeverity::Error,
                source: "rust-analyzer".to_owned(),
                message: "zero based bad payload".to_owned(),
                unused: false,
                deprecated: false,
            }],
        );

        app.goto_diagnostic(1);

        let cursor = app.buffer(2).unwrap().cursor_position();
        assert_eq!((cursor.line, cursor.column), (0, 0));
        assert!(app.status.starts_with("error diagnostic: "));
        assert!(app.status.contains(":1:1 zero based bad payload"));
    }

    #[test]
    fn goto_diagnostic_clamps_stale_open_buffer_location_and_label() {
        let root = missing_path("workspace-root");
        let target_path = missing_path("diagnostic-target.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            2,
            Some(target_path.clone()),
            "one\ntwo".to_owned(),
        ));
        app.set_active_buffer(2);
        app.diagnostics.replace(
            target_path.clone(),
            vec![Diagnostic {
                path: target_path,
                line: 99,
                column: 99,
                char_range: 0..1,
                severity: DiagnosticSeverity::Error,
                source: "rust-analyzer".to_owned(),
                message: "stale location".to_owned(),
                unused: false,
                deprecated: false,
            }],
        );

        app.goto_diagnostic(1);

        let cursor = app.buffer(2).unwrap().cursor_position();
        assert_eq!((cursor.line, cursor.column), (1, 3));
        assert!(app.status.starts_with("error diagnostic: "));
        assert!(app.status.contains(":2:4 stale location"));
    }

    #[test]
    fn diagnostic_navigation_label_sanitizes_and_bounds_user_visible_text() {
        let diagnostic = Diagnostic {
            path: PathBuf::from(format!("workspace/src/{}tail.rs", "very-long\n".repeat(24))),
            line: 3,
            column: 5,
            char_range: 0..1,
            severity: DiagnosticSeverity::Error,
            source: "rust-analyzer\nwith controls\u{7}".to_owned(),
            message: format!("{}\nsecond line", "mismatch ".repeat(40)),
            unused: false,
            deprecated: false,
        };

        let label = diagnostic_label(&diagnostic);

        assert!(!label.contains('\n'));
        assert!(label.contains(":3:5 "));
        assert!(label.ends_with("..."));
        assert!(label.chars().count() <= 250);
    }

    #[test]
    fn diagnostic_navigation_label_cache_reuses_sanitized_parts_without_mutating_diagnostics() {
        let diagnostic = Diagnostic {
            path: PathBuf::from(format!("workspace/src/{}tail.rs", "very-long\n".repeat(24))),
            line: 3,
            column: 5,
            char_range: 0..1,
            severity: DiagnosticSeverity::Error,
            source: "rust-analyzer\nwith controls\u{7}".to_owned(),
            message: format!("{}\nsecond line", "mismatch ".repeat(40)),
            unused: false,
            deprecated: false,
        };
        let mut same_parts = diagnostic.clone();
        same_parts.line = 9;
        let original = diagnostic.clone();
        let mut cache = DiagnosticNavigationLabelCache::with_capacity(4);
        let (line, column) = diagnostic_jump_location(&diagnostic);
        let (same_parts_line, same_parts_column) = diagnostic_jump_location(&same_parts);

        let first = cache.label(&diagnostic, line, column);
        let second = cache.label(&diagnostic, line, column);
        let third = cache.label(&same_parts, same_parts_line, same_parts_column);

        assert_eq!(first, second);
        assert_eq!(first, diagnostic_label(&diagnostic));
        assert!(third.contains(":9:5 "));
        assert_eq!(cache.path_label_count(), 1);
        assert_eq!(cache.message_summary_count(), 1);
        assert_eq!(diagnostic, original);
        assert!(!first.contains('\n'));
        assert!(first.ends_with("..."));
    }

    #[test]
    fn diagnostic_navigation_label_cache_bounds_unique_path_and_message_entries() {
        let diagnostics = (0..4)
            .map(|idx| Diagnostic {
                path: PathBuf::from("workspace")
                    .join("src")
                    .join(format!("diagnostic-{idx}.rs")),
                line: idx + 1,
                column: idx + 2,
                char_range: 0..1,
                severity: DiagnosticSeverity::Error,
                source: "rust-analyzer".to_owned(),
                message: format!("message {idx}\nsecond line"),
                unused: false,
                deprecated: false,
            })
            .collect::<Vec<_>>();
        let mut cache = DiagnosticNavigationLabelCache::with_capacity(2);

        for diagnostic in &diagnostics {
            let (line, column) = diagnostic_jump_location(diagnostic);
            assert_eq!(
                cache.label(diagnostic, line, column),
                diagnostic_label(diagnostic)
            );
        }

        assert_eq!(cache.path_label_count(), 2);
        assert_eq!(cache.message_summary_count(), 2);
        let overflow = diagnostics.last().expect("diagnostics include overflow");
        let (line, column) = diagnostic_jump_location(overflow);
        assert_eq!(
            cache.label(overflow, line, column),
            diagnostic_label(overflow)
        );
        assert_eq!(cache.path_label_count(), 2);
        assert_eq!(cache.message_summary_count(), 2);
    }

    #[test]
    fn diagnostic_navigation_label_uses_resolved_location() {
        let diagnostic = Diagnostic {
            path: PathBuf::from("workspace/src/main.rs"),
            line: 0,
            column: 0,
            char_range: 0..1,
            severity: DiagnosticSeverity::Error,
            source: "rust-analyzer".to_owned(),
            message: "bad payload".to_owned(),
            unused: false,
            deprecated: false,
        };
        let mut cache = DiagnosticNavigationLabelCache::with_capacity(1);

        assert_eq!(cache.label(&diagnostic, 8, 2), "main.rs:8:2 bad payload");
        assert_eq!(diagnostic_label(&diagnostic), "main.rs:1:1 bad payload");
    }

    #[test]
    fn goto_diagnostic_reports_when_all_targets_are_unavailable() {
        let root = missing_path("workspace-root");
        let active_path = root.join("src/0.rs");
        let missing_target = root.join("src/a.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            1,
            Some(active_path),
            "active\n".to_owned(),
        ));
        app.set_active_buffer(1);
        app.diagnostics.replace(
            missing_target.clone(),
            vec![Diagnostic {
                path: missing_target.clone(),
                line: 1,
                column: 1,
                char_range: 0..1,
                severity: DiagnosticSeverity::Error,
                source: "rust-analyzer".to_owned(),
                message: "missing target".to_owned(),
                unused: false,
                deprecated: false,
            }],
        );

        app.goto_diagnostic(1);

        assert_eq!(app.active, Some(1));
        assert_eq!(app.status, "No available diagnostics");
    }

    #[test]
    fn goto_diagnostic_skips_existing_directory_targets() {
        let root = missing_path("workspace-root");
        let active_path = root.join("src/0.rs");
        let directory_target = missing_path("diagnostic-target-dir");
        fs::create_dir_all(&directory_target).unwrap();
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            1,
            Some(active_path),
            "active\n".to_owned(),
        ));
        app.set_active_buffer(1);
        app.diagnostics.replace(
            directory_target.clone(),
            vec![Diagnostic {
                path: directory_target.clone(),
                line: 1,
                column: 1,
                char_range: 0..1,
                severity: DiagnosticSeverity::Error,
                source: "rust-analyzer".to_owned(),
                message: "directory target".to_owned(),
                unused: false,
                deprecated: false,
            }],
        );

        app.goto_diagnostic(1);

        assert_eq!(app.active, Some(1));
        assert!(app.pending_open_paths.is_empty());
        assert_eq!(app.status, "No available diagnostics");

        fs::remove_dir_all(directory_target).unwrap();
    }

    fn app_for_test(root: PathBuf) -> KuroyaApp {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let settings = EditorSettings::default();
        KuroyaApp::from_startup_context(AppStartupContext {
            runtime: Runtime::new().expect("test runtime"),
            tx,
            rx,
            workspace: Workspace::new(root.clone()),
            settings: settings.clone(),
            settings_panel_draft: settings,
            settings_editor_font_path: String::new(),
            settings_ui_font_path: String::new(),
            theme_picker_selected: 0,
            saved_session: None,
            terminal: TerminalPane::new(root.clone(), 100, 12.0, 1.2),
            watcher: None,
            recent_projects: Vec::new(),
            trusted_workspaces: vec![root],
            now: Instant::now(),
            startup_timings: Vec::new(),
        })
    }

    fn missing_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "kuroya-diagnostic-navigation-{}-{unique}-{name}",
            std::process::id()
        ))
    }
}
