use crate::{
    KuroyaApp,
    file_io::{file_size_exceeds_limit, file_too_large_message},
    git_diff_view::{
        diff_label_for_path, source_control_diff_hunk_base_open_success_status,
        source_control_head_revision_failure_status, source_control_head_revision_missing_status,
        source_control_index_revision_failure_status, source_control_index_revision_missing_status,
    },
    path_display::{
        DISPLAY_PATH_LABEL_MAX_CHARS, display_error_label_cow, display_path_label_cow,
        sanitized_display_label_cow,
    },
    source_control_runtime::reserve_source_control_load_request_id_state,
    ui_events::UiEvent,
    virtual_diff_runtime::read_compare_text,
};
use kuroya_core::{file_text_at_head, file_text_at_index};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

const VIRTUAL_REVISION_DETAIL_MAX_CHARS: usize = 180;
const VIRTUAL_REVISION_STATUS_MAX_CHARS: usize = 240;

#[derive(Debug, Clone)]
pub(crate) enum VirtualRevisionOpenRequest {
    Head {
        path: PathBuf,
        jump: Option<VirtualRevisionJump>,
    },
    Index {
        path: PathBuf,
        jump: Option<VirtualRevisionJump>,
    },
    Saved {
        path: PathBuf,
        jump: Option<VirtualRevisionJump>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct VirtualRevisionJump {
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) label: String,
    pub(crate) hunk_index: usize,
}

#[derive(Debug)]
pub(crate) struct VirtualRevisionOpenJob {
    request: VirtualRevisionOpenRequest,
}

#[derive(Debug)]
pub(crate) enum VirtualRevisionOpenOutcome {
    Open(VirtualRevisionOpen),
    Status(String),
}

#[derive(Debug)]
pub(crate) struct VirtualRevisionOpen {
    label: String,
    path: PathBuf,
    text: String,
    target: String,
    kind: &'static str,
    jump: Option<VirtualRevisionJump>,
}

impl KuroyaApp {
    pub(crate) fn spawn_virtual_revision_open(&mut self, job: VirtualRevisionOpenJob) {
        let request_id = self.reserve_virtual_revision_open_request_id();
        let event_root = self.workspace.root.clone();
        let git_root = self.source_control_git_operation_root();
        let generation = self.workspace_event_generation;
        let max_bytes = self.diff_options().max_file_size_bytes;
        let tx = self.tx.clone();
        let request = job.request.clone();
        let detail = virtual_revision_open_detail(&request);
        self.status = virtual_revision_open_pending_status(&detail);
        self.record_async_task_started(
            "Virtual Revision",
            virtual_revision_task_detail_from_detail(request_id, &detail),
        );
        self.runtime.spawn_blocking(move || {
            let result = compute_virtual_revision_open(&git_root, job, max_bytes);
            let _ = crate::ui_event_channel::send_ui_event(
                &tx,
                UiEvent::VirtualRevisionOpenFinished {
                    root: event_root,
                    generation,
                    request_id,
                    request,
                    result,
                },
            );
        });
    }

    pub(crate) fn reserve_virtual_revision_open_request_id(&mut self) -> u64 {
        reserve_source_control_load_request_id_state(
            &mut self.virtual_revision_open_next_request_id,
            &mut self.virtual_revision_open_active_request_id,
        )
    }

    pub(crate) fn apply_virtual_revision_open_finished(
        &mut self,
        root: PathBuf,
        generation: u64,
        request_id: u64,
        _request: VirtualRevisionOpenRequest,
        result: Result<VirtualRevisionOpenOutcome, String>,
    ) {
        if !self.workspace_event_is_current(&root, generation)
            || self.virtual_revision_open_active_request_id == 0
            || request_id != self.virtual_revision_open_active_request_id
        {
            return;
        }
        self.virtual_revision_open_active_request_id = 0;

        match result {
            Ok(VirtualRevisionOpenOutcome::Open(open)) => {
                let VirtualRevisionOpen {
                    label,
                    path,
                    text,
                    target,
                    kind,
                    jump,
                } = open;
                if let Some(jump) = jump {
                    let status_path = path.clone();
                    let id = self.open_virtual_revision_buffer(label, path, text, target, kind);
                    self.apply_file_jump(id, jump.line, jump.column);
                    self.status = source_control_diff_hunk_base_open_success_status(
                        &jump.label,
                        &status_path,
                        jump.hunk_index,
                        jump.line,
                    );
                } else {
                    self.open_virtual_revision_buffer(label, path, text, target, kind);
                }
            }
            Ok(VirtualRevisionOpenOutcome::Status(status)) | Err(status) => {
                self.status = virtual_revision_status_text_owned(status);
            }
        }
    }
}

impl VirtualRevisionOpenJob {
    pub(crate) fn head(path: PathBuf, jump: Option<VirtualRevisionJump>) -> Self {
        Self {
            request: VirtualRevisionOpenRequest::Head { path, jump },
        }
    }

    pub(crate) fn index(path: PathBuf, jump: Option<VirtualRevisionJump>) -> Self {
        Self {
            request: VirtualRevisionOpenRequest::Index { path, jump },
        }
    }

    pub(crate) fn saved(path: PathBuf, jump: Option<VirtualRevisionJump>) -> Self {
        Self {
            request: VirtualRevisionOpenRequest::Saved { path, jump },
        }
    }
}

fn compute_virtual_revision_open(
    root: &Path,
    job: VirtualRevisionOpenJob,
    max_bytes: usize,
) -> Result<VirtualRevisionOpenOutcome, String> {
    match job.request {
        VirtualRevisionOpenRequest::Head { path, jump } => {
            compute_head_revision_open(root, path, jump, max_bytes)
        }
        VirtualRevisionOpenRequest::Index { path, jump } => {
            compute_index_revision_open(root, path, jump, max_bytes)
        }
        VirtualRevisionOpenRequest::Saved { path, jump } => {
            compute_saved_revision_open(path, jump, max_bytes)
        }
    }
}

fn compute_head_revision_open(
    root: &Path,
    path: PathBuf,
    jump: Option<VirtualRevisionJump>,
    max_bytes: usize,
) -> Result<VirtualRevisionOpenOutcome, String> {
    let path = PreparedVirtualRevisionPath::new(path);
    let text = match file_text_at_head(root, &path.raw) {
        Ok(Some(text)) => text,
        Ok(None) => {
            return Ok(VirtualRevisionOpenOutcome::Status(
                source_control_head_revision_missing_status(&path.raw),
            ));
        }
        Err(error) => {
            return Ok(VirtualRevisionOpenOutcome::Status(
                source_control_head_revision_failure_status(&path.raw, &error.to_string()),
            ));
        }
    };
    let text = match checked_virtual_revision_text(text, max_bytes) {
        Ok(text) => text,
        Err(error) => {
            return Ok(VirtualRevisionOpenOutcome::Status(
                source_control_head_revision_failure_status(&path.raw, &error),
            ));
        }
    };

    Ok(VirtualRevisionOpenOutcome::Open(VirtualRevisionOpen {
        label: virtual_revision_label(&path.raw, "HEAD"),
        target: path.label,
        path: path.raw,
        text,
        kind: "HEAD revision",
        jump,
    }))
}

fn compute_index_revision_open(
    root: &Path,
    path: PathBuf,
    jump: Option<VirtualRevisionJump>,
    max_bytes: usize,
) -> Result<VirtualRevisionOpenOutcome, String> {
    let path = PreparedVirtualRevisionPath::new(path);
    let text = match file_text_at_index(root, &path.raw) {
        Ok(Some(text)) => text,
        Ok(None) => {
            return Ok(VirtualRevisionOpenOutcome::Status(
                source_control_index_revision_missing_status(&path.raw),
            ));
        }
        Err(error) => {
            return Ok(VirtualRevisionOpenOutcome::Status(
                source_control_index_revision_failure_status(&path.raw, &error.to_string()),
            ));
        }
    };
    let text = match checked_virtual_revision_text(text, max_bytes) {
        Ok(text) => text,
        Err(error) => {
            return Ok(VirtualRevisionOpenOutcome::Status(
                source_control_index_revision_failure_status(&path.raw, &error),
            ));
        }
    };

    Ok(VirtualRevisionOpenOutcome::Open(VirtualRevisionOpen {
        label: virtual_revision_label(&path.raw, "Index"),
        target: path.label,
        path: path.raw,
        text,
        kind: "index revision",
        jump,
    }))
}

fn compute_saved_revision_open(
    path: PathBuf,
    jump: Option<VirtualRevisionJump>,
    max_bytes: usize,
) -> Result<VirtualRevisionOpenOutcome, String> {
    let path = PreparedVirtualRevisionPath::new(path);
    let text = match read_compare_text(&path.raw, max_bytes) {
        Ok(text) => text,
        Err(error) => {
            let error = display_error_label_cow(&error);
            return Ok(VirtualRevisionOpenOutcome::Status(
                virtual_revision_status_text_owned(format!(
                    "Could not open saved {}: {}",
                    path.label,
                    error.as_ref()
                )),
            ));
        }
    };

    Ok(VirtualRevisionOpenOutcome::Open(VirtualRevisionOpen {
        label: virtual_revision_title_label(&path.label, "Saved"),
        target: path.label,
        path: path.raw,
        text,
        kind: "saved file",
        jump,
    }))
}

pub(crate) fn virtual_revision_open_detail(request: &VirtualRevisionOpenRequest) -> String {
    let (kind, path, jump) = match request {
        VirtualRevisionOpenRequest::Head { path, jump } => ("HEAD", path, jump),
        VirtualRevisionOpenRequest::Index { path, jump } => ("index", path, jump),
        VirtualRevisionOpenRequest::Saved { path, jump } => ("saved", path, jump),
    };
    let path = virtual_revision_path_label(path);
    match jump {
        Some(jump) => virtual_revision_detail_text_owned(format!(
            "{kind} {} hunk {}",
            path.as_ref(),
            jump.hunk_index.saturating_add(1)
        )),
        None => virtual_revision_detail_text_owned(format!("{kind} {}", path.as_ref())),
    }
}

pub(crate) fn virtual_revision_task_detail(
    request_id: u64,
    request: &VirtualRevisionOpenRequest,
) -> String {
    virtual_revision_task_detail_from_detail(request_id, &virtual_revision_open_detail(request))
}

fn virtual_revision_task_detail_from_detail(request_id: u64, detail: &str) -> String {
    virtual_revision_detail_text_owned(format!("#{request_id} {detail}"))
}

fn virtual_revision_open_pending_status(detail: &str) -> String {
    virtual_revision_status_text_owned(format!("Preparing revision for {detail}"))
}

#[derive(Debug)]
struct PreparedVirtualRevisionPath {
    raw: PathBuf,
    label: String,
}

impl PreparedVirtualRevisionPath {
    fn new(raw: PathBuf) -> Self {
        let label = virtual_revision_path_label(&raw).into_owned();
        Self { raw, label }
    }
}

fn checked_virtual_revision_text(text: String, max_bytes: usize) -> Result<String, String> {
    let bytes = u64::try_from(text.len()).unwrap_or(u64::MAX);
    let max_bytes = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    if file_size_exceeds_limit(bytes, max_bytes) {
        return Err(file_too_large_message(bytes, max_bytes));
    }
    if text.as_bytes().contains(&0) {
        return Err("binary file skipped".to_owned());
    }
    Ok(text)
}

fn virtual_revision_label(path: &Path, suffix: &str) -> String {
    virtual_revision_title_label(&diff_label_for_path(path), suffix)
}

fn virtual_revision_title_label(label: &str, suffix: &str) -> String {
    let suffix = format!(" ({suffix})");
    let suffix_chars = suffix.chars().count();
    if suffix_chars >= DISPLAY_PATH_LABEL_MAX_CHARS {
        return virtual_revision_title_text_owned(
            format!("{label}{suffix}"),
            DISPLAY_PATH_LABEL_MAX_CHARS,
        );
    }

    let label_chars = DISPLAY_PATH_LABEL_MAX_CHARS - suffix_chars;
    let label = virtual_revision_title_text_cow(label, label_chars);
    format!("{}{suffix}", label.as_ref())
}

fn virtual_revision_path_label(path: &Path) -> Cow<'_, str> {
    display_path_label_cow(path)
}

fn virtual_revision_title_text_cow<'a>(value: &'a str, max_chars: usize) -> Cow<'a, str> {
    sanitized_display_label_cow(value, max_chars, "revision")
}

fn virtual_revision_title_text_owned(value: String, max_chars: usize) -> String {
    virtual_revision_display_text_owned(value, max_chars, "revision")
}

#[cfg(test)]
fn virtual_revision_detail_text(value: impl AsRef<str>) -> String {
    virtual_revision_detail_text_cow(value.as_ref()).into_owned()
}

fn virtual_revision_detail_text_owned(value: String) -> String {
    virtual_revision_display_text_owned(value, VIRTUAL_REVISION_DETAIL_MAX_CHARS, "revision")
}

#[cfg(test)]
fn virtual_revision_detail_text_cow<'a>(value: &'a str) -> Cow<'a, str> {
    sanitized_display_label_cow(value, VIRTUAL_REVISION_DETAIL_MAX_CHARS, "revision")
}

#[cfg(test)]
fn virtual_revision_status_text(value: impl AsRef<str>) -> String {
    virtual_revision_status_text_cow(value.as_ref()).into_owned()
}

fn virtual_revision_status_text_owned(value: String) -> String {
    virtual_revision_display_text_owned(
        value,
        VIRTUAL_REVISION_STATUS_MAX_CHARS,
        "Virtual revision status unavailable",
    )
}

#[cfg(test)]
fn virtual_revision_status_text_cow<'a>(value: &'a str) -> Cow<'a, str> {
    sanitized_display_label_cow(
        value,
        VIRTUAL_REVISION_STATUS_MAX_CHARS,
        "Virtual revision status unavailable",
    )
}

fn virtual_revision_display_text_owned(value: String, max_chars: usize, fallback: &str) -> String {
    let sanitized = {
        let raw = value.as_str();
        match sanitized_display_label_cow(raw, max_chars, fallback) {
            Cow::Borrowed(label) => {
                let borrowed_original =
                    !raw.is_empty() && label.as_ptr() == raw.as_ptr() && label.len() == raw.len();
                if borrowed_original {
                    None
                } else {
                    Some(label.to_owned())
                }
            }
            Cow::Owned(label) => Some(label),
        }
    };

    match sanitized {
        Some(label) => label,
        None => value,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        VIRTUAL_REVISION_DETAIL_MAX_CHARS, VIRTUAL_REVISION_STATUS_MAX_CHARS, VirtualRevisionJump,
        VirtualRevisionOpen, VirtualRevisionOpenJob, VirtualRevisionOpenOutcome,
        VirtualRevisionOpenRequest, checked_virtual_revision_text, compute_saved_revision_open,
        virtual_revision_detail_text, virtual_revision_detail_text_cow,
        virtual_revision_detail_text_owned, virtual_revision_label, virtual_revision_open_detail,
        virtual_revision_open_pending_status, virtual_revision_status_text,
        virtual_revision_status_text_cow, virtual_revision_status_text_owned,
        virtual_revision_task_detail, virtual_revision_title_label,
        virtual_revision_title_text_cow,
    };
    use crate::{
        git_diff_state::DiffBufferSource,
        path_display::{DISPLAY_PATH_LABEL_MAX_CHARS, sanitized_display_label_cow},
        source_control_runtime::source_control_app_for_test,
    };
    use kuroya_core::{GitChangeStage, TextBuffer};
    use std::{
        borrow::Cow,
        env, fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        env::temp_dir().join(format!(
            "kuroya-virtual-revision-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn saved_revision_request(path: PathBuf) -> VirtualRevisionOpenRequest {
        VirtualRevisionOpenRequest::Saved { path, jump: None }
    }

    fn revision_open_outcome(label: &str) -> Result<VirtualRevisionOpenOutcome, String> {
        Ok(VirtualRevisionOpenOutcome::Open(VirtualRevisionOpen {
            label: label.to_owned(),
            path: PathBuf::from(label),
            text: "revision\n".to_owned(),
            target: label.to_owned(),
            kind: "test revision",
            jump: None,
        }))
    }

    #[test]
    fn saved_revision_open_respects_size_limit_before_reading() {
        let path = temp_path("oversize.txt");
        fs::write(&path, "too large").unwrap();

        let outcome = compute_saved_revision_open(path.clone(), None, 3).unwrap();

        match outcome {
            VirtualRevisionOpenOutcome::Status(status) => {
                assert!(status.contains("Could not open saved"));
                assert!(status.contains("file is too large to open"));
                assert!(status.contains("9 B"));
                assert!(status.contains("3 B"));
            }
            VirtualRevisionOpenOutcome::Open(_) => panic!("expected oversize status"),
        }
        let _ = fs::remove_file(path);
    }

    #[test]
    fn saved_revision_open_rejects_binary_text_file() {
        let path = temp_path("binary.dat");
        fs::write(&path, b"binary\0text\n").unwrap();

        let outcome = compute_saved_revision_open(path.clone(), None, 99).unwrap();

        match outcome {
            VirtualRevisionOpenOutcome::Status(status) => {
                assert!(status.contains("Could not open saved"));
                assert!(status.contains("binary.dat"));
                assert!(status.contains("binary file skipped"));
            }
            VirtualRevisionOpenOutcome::Open(_) => panic!("expected binary-file status"),
        }
        let _ = fs::remove_file(path);
    }

    #[test]
    fn virtual_revision_detail_names_revision_path_and_hunk() {
        let jump = VirtualRevisionJump {
            line: 12,
            column: 1,
            label: "main.rs (Changes)".to_owned(),
            hunk_index: 2,
        };
        assert_eq!(
            virtual_revision_open_detail(&VirtualRevisionOpenRequest::Head {
                path: PathBuf::from("src/main.rs"),
                jump: Some(jump),
            }),
            "HEAD main.rs hunk 3"
        );
        assert_eq!(
            virtual_revision_open_detail(&VirtualRevisionOpenRequest::Saved {
                path: PathBuf::from("src/lib.rs"),
                jump: None,
            }),
            "saved lib.rs"
        );
    }

    #[test]
    fn virtual_revision_task_detail_includes_request_id() {
        let request = saved_revision_request(PathBuf::from("src/main.rs"));

        assert_eq!(
            virtual_revision_task_detail(42, &request),
            "#42 saved main.rs"
        );
    }

    #[test]
    fn virtual_revision_detail_saturates_hunk_index_and_bounds_pending_status() {
        let request = VirtualRevisionOpenRequest::Head {
            path: PathBuf::from(format!(
                "workspace/src/{}tail.rs",
                "long-component-".repeat(32)
            )),
            jump: Some(VirtualRevisionJump {
                line: 1,
                column: 1,
                label: "diff".to_owned(),
                hunk_index: usize::MAX,
            }),
        };

        let detail = virtual_revision_open_detail(&request);
        let task_detail = virtual_revision_task_detail(u64::MAX, &request);
        let pending = virtual_revision_open_pending_status(&detail);

        assert!(detail.contains("hunk"));
        assert!(detail.contains(&usize::MAX.to_string()));
        assert!(detail.chars().count() <= VIRTUAL_REVISION_DETAIL_MAX_CHARS);
        assert!(task_detail.chars().count() <= VIRTUAL_REVISION_DETAIL_MAX_CHARS);
        assert!(pending.chars().count() <= VIRTUAL_REVISION_STATUS_MAX_CHARS);
    }

    #[test]
    fn virtual_revision_display_text_cow_helpers_borrow_clean_ascii_and_unicode() {
        let ascii = "saved src/main.rs";
        assert!(matches!(
            virtual_revision_detail_text_cow(ascii),
            Cow::Borrowed(label) if label == ascii
        ));

        let unicode = "r\u{00e9}vision-\u{03bb}.rs";
        match virtual_revision_status_text_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed status, got {label:?}"),
        }

        match virtual_revision_title_text_cow(unicode, DISPLAY_PATH_LABEL_MAX_CHARS) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed title, got {label:?}"),
        }
    }

    #[test]
    fn virtual_revision_display_text_cow_helpers_own_dirty_truncated_and_fallback_values() {
        match virtual_revision_detail_text_cow("alpha\nbeta\u{202e}") {
            Cow::Owned(label) => assert_eq!(label, "alpha beta"),
            Cow::Borrowed(label) => panic!("expected owned dirty detail, got {label:?}"),
        }

        let long_detail = format!(
            "detail-{}-tail",
            "x".repeat(VIRTUAL_REVISION_DETAIL_MAX_CHARS * 2)
        );
        match virtual_revision_detail_text_cow(&long_detail) {
            Cow::Owned(label) => {
                assert!(label.contains("..."));
                assert!(label.chars().count() <= VIRTUAL_REVISION_DETAIL_MAX_CHARS);
            }
            Cow::Borrowed(label) => panic!("expected owned truncated detail, got {label:?}"),
        }

        match virtual_revision_detail_text_cow("\n\u{202e}") {
            Cow::Owned(label) => assert_eq!(label, "revision"),
            Cow::Borrowed(label) => panic!("expected owned fallback detail, got {label:?}"),
        }

        match virtual_revision_status_text_cow("\n\u{202e}") {
            Cow::Owned(label) => assert_eq!(label, "Virtual revision status unavailable"),
            Cow::Borrowed(label) => panic!("expected owned fallback status, got {label:?}"),
        }
    }

    #[test]
    fn virtual_revision_string_wrappers_match_cow_helpers() {
        for value in [
            "saved main.rs",
            "  saved main.rs  ",
            "alpha\nbeta\u{202e}",
            "\n\u{202e}",
        ] {
            assert_eq!(
                virtual_revision_detail_text(value),
                virtual_revision_detail_text_cow(value).into_owned()
            );
            assert_eq!(
                virtual_revision_status_text(value),
                virtual_revision_status_text_cow(value).into_owned()
            );
        }

        let long = format!(
            "revision-{}-tail",
            "long-component-".repeat(VIRTUAL_REVISION_STATUS_MAX_CHARS)
        );
        assert_eq!(
            virtual_revision_detail_text(&long),
            virtual_revision_detail_text_cow(&long).into_owned()
        );
        assert_eq!(
            virtual_revision_status_text(&long),
            virtual_revision_status_text_cow(&long).into_owned()
        );
    }

    #[test]
    fn virtual_revision_owned_display_helpers_reuse_clean_strings_and_sanitize_formatted_text() {
        let detail = format!("saved {}", "main.rs");
        let detail_ptr = detail.as_ptr();
        let detail_len = detail.len();
        let detail = virtual_revision_detail_text_owned(detail);
        assert_eq!(detail, "saved main.rs");
        assert_eq!(detail.as_ptr(), detail_ptr);
        assert_eq!(detail.len(), detail_len);

        let status = format!("Preparing revision for {}", "r\u{00e9}vision.rs");
        let status_ptr = status.as_ptr();
        let status_len = status.len();
        let status = virtual_revision_status_text_owned(status);
        assert_eq!(status, "Preparing revision for r\u{00e9}vision.rs");
        assert_eq!(status.as_ptr(), status_ptr);
        assert_eq!(status.len(), status_len);

        assert_eq!(
            virtual_revision_detail_text_owned(format!("saved {}\n{}", "bad", "name")),
            "saved bad name"
        );
        assert_eq!(
            virtual_revision_status_text_owned(format!("Preparing revision for {}\u{202e}", "bad")),
            "Preparing revision for bad"
        );
    }

    #[test]
    fn virtual_revision_jobs_preserve_request_kinds() {
        assert!(matches!(
            VirtualRevisionOpenJob::head(PathBuf::from("a.rs"), None).request,
            VirtualRevisionOpenRequest::Head { .. }
        ));
        assert!(matches!(
            VirtualRevisionOpenJob::index(PathBuf::from("b.rs"), None).request,
            VirtualRevisionOpenRequest::Index { .. }
        ));
        assert!(matches!(
            VirtualRevisionOpenJob::saved(PathBuf::from("c.rs"), None).request,
            VirtualRevisionOpenRequest::Saved { .. }
        ));
    }

    #[test]
    fn virtual_revision_display_text_sanitizes_and_bounds_path_labels() {
        let path = temp_path(&format!("bad-{}tail.rs", "very-long-component-".repeat(5)));
        fs::write(&path, "saved\n").unwrap();

        let detail = virtual_revision_open_detail(&VirtualRevisionOpenRequest::Saved {
            path: path.clone(),
            jump: None,
        });

        assert!(detail.starts_with("saved "));
        assert!(detail.contains("..."));
        assert!(detail.chars().count() <= "saved ".len() + DISPLAY_PATH_LABEL_MAX_CHARS);

        let outcome = compute_saved_revision_open(path.clone(), None, 128).unwrap();

        match outcome {
            VirtualRevisionOpenOutcome::Open(open) => {
                assert_eq!(open.path, path);
                assert!(open.label.contains("..."));
                assert!(open.label.ends_with(" (Saved)"));
                assert!(open.label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
                assert!(open.target.contains("..."));
                assert!(open.target.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
            }
            VirtualRevisionOpenOutcome::Status(status) => {
                panic!("expected saved revision, got {status}")
            }
        }

        let _ = fs::remove_file(path);
    }

    #[test]
    fn virtual_revision_title_labels_preserve_suffix_and_stay_bounded() {
        let raw_label = format!(
            "revision-{}tail.rs",
            "very-long-component-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        );
        let label = virtual_revision_title_label(&raw_label, "HEAD");

        assert!(label.ends_with(" (HEAD)"));
        assert!(label.contains("..."));
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);

        let path_label = virtual_revision_label(&PathBuf::from(raw_label), "Index");

        assert!(path_label.ends_with(" (Index)"));
        assert!(path_label.contains("..."));
        assert!(path_label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    }

    #[test]
    fn virtual_revision_title_labels_sanitize_fallback_and_handle_full_suffix_budget() {
        let label = virtual_revision_title_label("alpha\nbeta\u{202e}", "HEAD");
        assert_eq!(label, "alpha beta (HEAD)");
        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{202e}'));

        assert_eq!(
            virtual_revision_title_label("\n\u{202e}", "HEAD"),
            "revision (HEAD)"
        );

        let suffix = "S".repeat(DISPLAY_PATH_LABEL_MAX_CHARS);
        let expected_raw = format!("alpha.rs ({suffix})");
        let expected =
            sanitized_display_label_cow(&expected_raw, DISPLAY_PATH_LABEL_MAX_CHARS, "revision")
                .into_owned();
        let label = virtual_revision_title_label("alpha.rs", &suffix);

        assert_eq!(label, expected);
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    }

    #[test]
    fn virtual_revision_detail_text_remains_safe_and_bounded() {
        let detail = virtual_revision_detail_text(format!(
            "detail\n{}\u{202e}tail",
            "x".repeat(VIRTUAL_REVISION_DETAIL_MAX_CHARS * 4)
        ));
        assert!(!detail.contains('\n'));
        assert!(!detail.contains('\u{202e}'));
        assert!(detail.contains("..."));
        assert!(detail.chars().count() <= VIRTUAL_REVISION_DETAIL_MAX_CHARS);
        assert_eq!(virtual_revision_detail_text("\n\u{202e}"), "revision");
    }

    #[test]
    fn virtual_revision_status_text_remains_safe_and_bounded() {
        let status = virtual_revision_status_text(format!(
            "status\n{}\u{202e}tail",
            "x".repeat(VIRTUAL_REVISION_STATUS_MAX_CHARS * 4)
        ));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(status.chars().count() <= VIRTUAL_REVISION_STATUS_MAX_CHARS);
        assert_eq!(
            virtual_revision_status_text("\n\u{202e}"),
            "Virtual revision status unavailable"
        );
    }

    #[test]
    fn virtual_revision_text_rejects_oversize_and_binary_revisions() {
        let oversize = checked_virtual_revision_text("too large".to_owned(), 3).unwrap_err();

        assert!(oversize.contains("file is too large to open"));
        assert!(oversize.contains("9 B"));
        assert!(oversize.contains("3 B"));

        let binary = checked_virtual_revision_text("binary\0text\n".to_owned(), 99).unwrap_err();

        assert_eq!(binary, "binary file skipped");
        assert_eq!(
            checked_virtual_revision_text("plain\n".to_owned(), 99).unwrap(),
            "plain\n"
        );
    }

    #[test]
    fn virtual_revision_open_finished_without_active_request_is_ignored() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = source_control_app_for_test(root.clone(), true);
        app.status = "idle".to_owned();
        app.virtual_revision_open_active_request_id = 0;

        app.apply_virtual_revision_open_finished(
            root,
            app.workspace_event_generation,
            0,
            saved_revision_request(path),
            revision_open_outcome("src/main.rs"),
        );

        assert!(app.buffers.is_empty());
        assert!(app.virtual_buffer_labels.is_empty());
        assert_eq!(app.status, "idle");
        assert_eq!(app.virtual_revision_open_active_request_id, 0);
    }

    #[test]
    fn stale_virtual_revision_open_finished_after_newer_request_is_ignored() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = source_control_app_for_test(root.clone(), true);
        app.status = "newer request pending".to_owned();
        app.virtual_revision_open_active_request_id = 2;

        app.apply_virtual_revision_open_finished(
            root,
            app.workspace_event_generation,
            1,
            saved_revision_request(path),
            Ok(VirtualRevisionOpenOutcome::Status(
                "stale revision completed".to_owned(),
            )),
        );

        assert_eq!(app.status, "newer request pending");
        assert_eq!(app.virtual_revision_open_active_request_id, 2);
    }

    #[test]
    fn stale_virtual_revision_open_finished_after_generation_change_is_ignored() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = source_control_app_for_test(root.clone(), true);
        let stale_generation = app.workspace_event_generation;
        app.workspace_event_generation = stale_generation + 1;
        app.status = "current workspace".to_owned();
        app.virtual_revision_open_active_request_id = 1;

        app.apply_virtual_revision_open_finished(
            root,
            stale_generation,
            1,
            saved_revision_request(path),
            Ok(VirtualRevisionOpenOutcome::Status(
                "stale generation".to_owned(),
            )),
        );

        assert_eq!(app.status, "current workspace");
        assert_eq!(app.virtual_revision_open_active_request_id, 1);
    }

    #[test]
    fn stale_virtual_revision_open_finished_after_workspace_root_change_is_ignored() {
        let root = PathBuf::from("workspace/current");
        let stale_root = PathBuf::from("workspace/old");
        let path = root.join("src/main.rs");
        let mut app = source_control_app_for_test(root, true);
        app.status = "current workspace".to_owned();
        app.virtual_revision_open_active_request_id = 1;

        app.apply_virtual_revision_open_finished(
            stale_root,
            app.workspace_event_generation,
            1,
            saved_revision_request(path),
            Ok(VirtualRevisionOpenOutcome::Status("stale root".to_owned())),
        );

        assert_eq!(app.status, "current workspace");
        assert_eq!(app.virtual_revision_open_active_request_id, 1);
    }

    #[test]
    fn stale_virtual_revision_open_finished_does_not_open_buffer() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = source_control_app_for_test(root.clone(), true);
        app.status = "current revision".to_owned();
        app.virtual_revision_open_active_request_id = 2;

        app.apply_virtual_revision_open_finished(
            root,
            app.workspace_event_generation,
            1,
            saved_revision_request(path),
            revision_open_outcome("src/main.rs"),
        );

        assert!(app.buffers.is_empty());
        assert!(app.virtual_buffer_labels.is_empty());
        assert_eq!(app.status, "current revision");
        assert_eq!(app.virtual_revision_open_active_request_id, 2);
    }

    #[test]
    fn stale_virtual_revision_open_finished_does_not_update_existing_buffer() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = source_control_app_for_test(root.clone(), true);
        let existing_id = app.open_virtual_revision_buffer(
            "src/main.rs".to_owned(),
            path.clone(),
            "current revision\n".to_owned(),
            "src/main.rs".to_owned(),
            "test revision",
        );
        app.buffers
            .push(TextBuffer::from_text(99, None, "active\n".to_owned()));
        app.set_active_buffer(99);
        app.pending_scroll_lines.insert(existing_id, 12);
        app.diff_buffer_sources.insert(
            existing_id,
            DiffBufferSource {
                path: path.clone(),
                base_path: None,
                hunk_stage: Some(GitChangeStage::Unstaged),
                saved_buffer_id: Some(7),
            },
        );
        app.status = "current revision".to_owned();
        app.virtual_revision_open_active_request_id = 2;

        app.apply_virtual_revision_open_finished(
            root,
            app.workspace_event_generation,
            1,
            saved_revision_request(path),
            revision_open_outcome("src/main.rs"),
        );

        assert_eq!(
            app.buffer(existing_id).unwrap().text(),
            "current revision\n"
        );
        assert_eq!(app.active, Some(99));
        assert_eq!(app.pending_scroll_lines.get(&existing_id), Some(&12));
        assert!(app.diff_buffer_sources.contains_key(&existing_id));
        assert_eq!(app.status, "current revision");
        assert_eq!(app.virtual_revision_open_active_request_id, 2);
    }

    #[test]
    fn duplicate_current_virtual_revision_open_finished_is_ignored_after_first_apply() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = source_control_app_for_test(root.clone(), true);
        app.virtual_revision_open_active_request_id = 1;

        app.apply_virtual_revision_open_finished(
            root.clone(),
            app.workspace_event_generation,
            1,
            saved_revision_request(path.clone()),
            revision_open_outcome("src/main.rs"),
        );

        assert_eq!(app.virtual_revision_open_active_request_id, 0);
        assert_eq!(app.buffers.len(), 1);
        assert_eq!(app.virtual_buffer_labels.len(), 1);

        app.status = "first result applied".to_owned();
        app.apply_virtual_revision_open_finished(
            root,
            app.workspace_event_generation,
            1,
            saved_revision_request(path),
            Ok(VirtualRevisionOpenOutcome::Status(
                "duplicate revision replayed".to_owned(),
            )),
        );

        assert_eq!(app.status, "first result applied");
        assert_eq!(app.buffers.len(), 1);
        assert_eq!(app.virtual_buffer_labels.len(), 1);
    }

    #[test]
    fn current_virtual_revision_open_finished_bounds_applied_status() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = source_control_app_for_test(root.clone(), true);
        app.virtual_revision_open_active_request_id = 1;

        app.apply_virtual_revision_open_finished(
            root,
            app.workspace_event_generation,
            1,
            saved_revision_request(path),
            Ok(VirtualRevisionOpenOutcome::Status(format!(
                "completed {}",
                "status-".repeat(80)
            ))),
        );

        assert!(app.status.contains("..."));
        assert!(app.status.chars().count() <= VIRTUAL_REVISION_STATUS_MAX_CHARS);
        assert_eq!(app.virtual_revision_open_active_request_id, 0);
    }

    #[test]
    fn saved_revision_error_status_sanitizes_path_label() {
        let path = temp_path(&format!(
            "missing\n{}\u{202e}tail.rs",
            "very-long-component-".repeat(16)
        ));

        let outcome = compute_saved_revision_open(path, None, 128).unwrap();

        match outcome {
            VirtualRevisionOpenOutcome::Status(status) => {
                assert!(status.starts_with("Could not open saved "));
                assert!(!status.contains('\n'));
                assert!(!status.contains('\u{202e}'));
                assert!(status.contains("..."));
            }
            VirtualRevisionOpenOutcome::Open(_) => panic!("expected missing-file status"),
        }
    }
}
