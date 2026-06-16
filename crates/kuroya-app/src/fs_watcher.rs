use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};
use notify::{
    EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{AccessKind, AccessMode, MetadataKind, ModifyKind},
    recommended_watcher,
};
use std::{
    collections::HashSet,
    ffi::OsStr,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const FILE_WATCHER_CHANNEL_BOUND: usize = 4096;
const FILE_WATCHER_DRAIN_BUDGET: usize = 512;

pub(crate) struct FileWatcher {
    _watcher: RecommendedWatcher,
    root: PathBuf,
    rx: Receiver<PathBuf>,
    overflowed: Arc<AtomicBool>,
}

#[derive(Default)]
pub(crate) struct FileWatcherDrain {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) overflowed: bool,
}

impl FileWatcher {
    pub(crate) fn new(root: &Path) -> anyhow::Result<Self> {
        let (tx, rx) = bounded(FILE_WATCHER_CHANNEL_BOUND);
        let overflowed = Arc::new(AtomicBool::new(false));
        let callback_overflowed = Arc::clone(&overflowed);
        let mut watcher = recommended_watcher(move |event: notify::Result<notify::Event>| {
            enqueue_watcher_event(&tx, &callback_overflowed, event);
        })?;
        watcher.watch(root, RecursiveMode::Recursive)?;
        Ok(Self {
            _watcher: watcher,
            root: root.to_path_buf(),
            rx,
            overflowed,
        })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn drain(&self) -> FileWatcherDrain {
        drain_watched_paths(&self.rx, &self.overflowed, FILE_WATCHER_DRAIN_BUDGET)
    }
}

fn enqueue_watcher_event(
    tx: &Sender<PathBuf>,
    overflowed: &AtomicBool,
    event: notify::Result<notify::Event>,
) {
    match event {
        Ok(event) => {
            if event.need_rescan() {
                overflowed.store(true, Ordering::SeqCst);
            }
            if watcher_event_kind_affects_filesystem(event.kind) {
                enqueue_unique_watched_paths(tx, overflowed, event.paths);
            }
        }
        Err(error) => {
            overflowed.store(true, Ordering::SeqCst);
            enqueue_unique_watched_paths(tx, overflowed, error.paths);
        }
    }
}

fn watcher_event_kind_affects_filesystem(kind: EventKind) -> bool {
    match kind {
        EventKind::Access(AccessKind::Close(
            AccessMode::Write | AccessMode::Any | AccessMode::Other,
        )) => true,
        EventKind::Access(_) => false,
        EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::AccessTime
            | MetadataKind::Permissions
            | MetadataKind::Ownership
            | MetadataKind::Extended,
        )) => false,
        EventKind::Modify(ModifyKind::Metadata(_)) => true,
        EventKind::Any
        | EventKind::Other
        | EventKind::Create(_)
        | EventKind::Modify(
            ModifyKind::Any | ModifyKind::Data(_) | ModifyKind::Name(_) | ModifyKind::Other,
        )
        | EventKind::Remove(_) => true,
    }
}

fn enqueue_unique_watched_paths(
    tx: &Sender<PathBuf>,
    overflowed: &AtomicBool,
    paths: impl IntoIterator<Item = PathBuf>,
) {
    let mut seen = HashSet::new();
    for path in paths {
        if !seen.insert(watcher_path_key(&path)) {
            continue;
        }
        if !enqueue_watched_path(tx, overflowed, path) {
            break;
        }
    }
}

fn enqueue_watched_path(tx: &Sender<PathBuf>, overflowed: &AtomicBool, path: PathBuf) -> bool {
    match tx.try_send(path) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) => {
            overflowed.store(true, Ordering::SeqCst);
            false
        }
        Err(TrySendError::Disconnected(_)) => false,
    }
}

fn drain_watched_paths(
    rx: &Receiver<PathBuf>,
    overflowed: &AtomicBool,
    max_paths: usize,
) -> FileWatcherDrain {
    let mut overflowed = overflowed.swap(false, Ordering::SeqCst);
    let mut paths = Vec::with_capacity(max_paths.min(FILE_WATCHER_DRAIN_BUDGET));
    let mut seen = HashSet::with_capacity(paths.capacity());
    let mut read_count = 0;
    let read_budget = FILE_WATCHER_CHANNEL_BOUND.max(max_paths);
    while read_count < read_budget {
        let Ok(path) = rx.try_recv() else {
            break;
        };
        read_count += 1;
        if !seen.insert(watcher_path_key(&path)) {
            continue;
        }
        if paths.len() < max_paths {
            paths.push(path);
        } else {
            overflowed = true;
        }
    }
    if read_count >= read_budget && !rx.is_empty() {
        overflowed = true;
    }
    FileWatcherDrain { paths, overflowed }
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct WatcherPathKey {
    prefix: Option<String>,
    rooted: bool,
    components: Vec<String>,
}

fn watcher_path_key(path: &Path) -> WatcherPathKey {
    let mut key = WatcherPathKey {
        prefix: None,
        rooted: false,
        components: Vec::new(),
    };

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                key.prefix = Some(watcher_path_component_key(prefix.as_os_str()));
            }
            Component::RootDir => key.rooted = true,
            Component::CurDir => {}
            Component::ParentDir => {
                if key
                    .components
                    .last()
                    .is_some_and(|component| component != "..")
                {
                    key.components.pop();
                } else if !key.rooted {
                    key.components.push("..".to_owned());
                }
            }
            Component::Normal(component) => {
                key.components.push(watcher_path_component_key(component));
            }
        }
    }

    key
}

fn watcher_path_component_key(component: &OsStr) -> String {
    let component = component.to_string_lossy();
    #[cfg(windows)]
    {
        if component.is_ascii() {
            let mut component = component.into_owned();
            component.make_ascii_lowercase();
            component
        } else {
            component.to_lowercase()
        }
    }
    #[cfg(not(windows))]
    {
        component.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{drain_watched_paths, enqueue_watched_path, enqueue_watcher_event};
    use crossbeam_channel::bounded;
    use notify::{
        Event, EventKind,
        event::{
            AccessKind, AccessMode, CreateKind, DataChange, Flag, MetadataKind, ModifyKind,
            RemoveKind, RenameMode,
        },
    };
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn watcher_queue_overflow_is_signaled_without_blocking() {
        let (tx, rx) = bounded(1);
        let overflowed = AtomicBool::new(false);

        assert!(enqueue_watched_path(
            &tx,
            &overflowed,
            PathBuf::from("workspace/src/main.rs")
        ));
        assert!(!enqueue_watched_path(
            &tx,
            &overflowed,
            PathBuf::from("workspace/src/lib.rs")
        ));

        assert!(overflowed.load(Ordering::SeqCst));
        assert_eq!(rx.len(), 1);
    }

    #[test]
    fn watcher_drain_reports_and_resets_overflow() {
        let (tx, rx) = bounded(4);
        let overflowed = AtomicBool::new(true);
        tx.send(PathBuf::from("workspace/src/main.rs")).unwrap();
        tx.send(PathBuf::from("workspace/src/lib.rs")).unwrap();

        let drain = drain_watched_paths(&rx, &overflowed, 1);

        assert!(drain.overflowed);
        assert_eq!(drain.paths, [PathBuf::from("workspace/src/main.rs")]);
        assert!(!overflowed.load(Ordering::SeqCst));
        assert!(rx.is_empty());

        let drain = drain_watched_paths(&rx, &overflowed, 8);

        assert!(!drain.overflowed);
        assert!(drain.paths.is_empty());
    }

    #[test]
    fn watcher_callback_errors_force_refresh_and_keep_error_paths() {
        let (tx, rx) = bounded(4);
        let overflowed = AtomicBool::new(false);
        let path = PathBuf::from("workspace/src/main.rs");

        enqueue_watcher_event(
            &tx,
            &overflowed,
            Err(notify::Error::generic("watcher dropped events").add_path(path.clone())),
        );
        let drain = drain_watched_paths(&rx, &overflowed, 8);

        assert!(drain.overflowed);
        assert_eq!(drain.paths, [path]);
        assert!(!overflowed.load(Ordering::SeqCst));
    }

    #[test]
    fn watcher_rescan_events_force_refresh_even_without_paths() {
        let (tx, rx) = bounded(4);
        let overflowed = AtomicBool::new(false);

        enqueue_watcher_event(
            &tx,
            &overflowed,
            Ok(Event::new(EventKind::Any).set_flag(Flag::Rescan)),
        );
        let drain = drain_watched_paths(&rx, &overflowed, 8);

        assert!(drain.overflowed);
        assert!(drain.paths.is_empty());
    }

    #[test]
    fn watcher_ignores_non_mutating_access_events() {
        for kind in [
            EventKind::Access(AccessKind::Read),
            EventKind::Access(AccessKind::Any),
            EventKind::Access(AccessKind::Other),
            EventKind::Access(AccessKind::Open(AccessMode::Read)),
            EventKind::Access(AccessKind::Open(AccessMode::Write)),
            EventKind::Access(AccessKind::Close(AccessMode::Read)),
            EventKind::Access(AccessKind::Close(AccessMode::Execute)),
        ] {
            let (overflowed, paths) = drain_single_event(kind);

            assert!(!overflowed);
            assert!(paths.is_empty());
        }
    }

    #[test]
    fn watcher_keeps_write_capable_close_access_events() {
        for kind in [
            EventKind::Access(AccessKind::Close(AccessMode::Write)),
            EventKind::Access(AccessKind::Close(AccessMode::Any)),
            EventKind::Access(AccessKind::Close(AccessMode::Other)),
        ] {
            let (overflowed, paths) = drain_single_event(kind);

            assert!(!overflowed);
            assert_eq!(paths, [PathBuf::from("workspace/src/main.rs")]);
        }
    }

    #[test]
    fn watcher_ignores_noisy_metadata_events() {
        for kind in [
            EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime)),
            EventKind::Modify(ModifyKind::Metadata(MetadataKind::Permissions)),
            EventKind::Modify(ModifyKind::Metadata(MetadataKind::Ownership)),
            EventKind::Modify(ModifyKind::Metadata(MetadataKind::Extended)),
        ] {
            let (overflowed, paths) = drain_single_event(kind);

            assert!(!overflowed);
            assert!(paths.is_empty());
        }
    }

    #[test]
    fn watcher_keeps_write_time_and_unknown_metadata_events() {
        for kind in [
            EventKind::Modify(ModifyKind::Metadata(MetadataKind::WriteTime)),
            EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)),
            EventKind::Modify(ModifyKind::Metadata(MetadataKind::Other)),
        ] {
            let (overflowed, paths) = drain_single_event(kind);

            assert!(!overflowed);
            assert_eq!(paths, [PathBuf::from("workspace/src/main.rs")]);
        }
    }

    #[test]
    fn watcher_keeps_mutating_events() {
        for kind in [
            EventKind::Any,
            EventKind::Other,
            EventKind::Create(CreateKind::File),
            EventKind::Modify(ModifyKind::Data(DataChange::Content)),
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            EventKind::Remove(RemoveKind::File),
        ] {
            let (overflowed, paths) = drain_single_event(kind);

            assert!(!overflowed);
            assert_eq!(paths, [PathBuf::from("workspace/src/main.rs")]);
        }
    }

    #[test]
    fn watcher_rescan_on_filtered_events_still_forces_refresh() {
        let (tx, rx) = bounded(4);
        let overflowed = AtomicBool::new(false);
        let path = PathBuf::from("workspace/src/main.rs");

        enqueue_watcher_event(
            &tx,
            &overflowed,
            Ok(Event::new(EventKind::Access(AccessKind::Read))
                .set_flag(Flag::Rescan)
                .add_path(path)),
        );
        let drain = drain_watched_paths(&rx, &overflowed, 8);

        assert!(drain.overflowed);
        assert!(drain.paths.is_empty());
    }

    #[test]
    fn watcher_event_paths_are_deduplicated_before_queueing() {
        let (tx, rx) = bounded(4);
        let overflowed = AtomicBool::new(false);
        let path = PathBuf::from("workspace/src/main.rs");

        enqueue_watcher_event(
            &tx,
            &overflowed,
            Ok(Event::new(EventKind::Any)
                .add_path(path.clone())
                .add_path(path.clone())),
        );
        let drain = drain_watched_paths(&rx, &overflowed, 8);

        assert!(!drain.overflowed);
        assert_eq!(drain.paths, [path]);
    }

    #[test]
    fn watcher_event_paths_are_lexically_deduplicated_before_queueing() {
        let (tx, rx) = bounded(4);
        let overflowed = AtomicBool::new(false);
        let raw_path = PathBuf::from("workspace/src/./main.rs");
        let equivalent_path = PathBuf::from("workspace/src/generated/../main.rs");

        enqueue_watcher_event(
            &tx,
            &overflowed,
            Ok(Event::new(EventKind::Any)
                .add_path(raw_path.clone())
                .add_path(equivalent_path)),
        );
        let drain = drain_watched_paths(&rx, &overflowed, 8);

        assert!(!drain.overflowed);
        assert_eq!(drain.paths, [raw_path]);
    }

    #[test]
    fn watcher_drain_collapses_duplicate_bursts_before_later_paths() {
        let (tx, rx) = bounded(8);
        let overflowed = AtomicBool::new(false);
        let first = PathBuf::from("workspace/src/main.rs");
        let second = PathBuf::from("workspace/src/lib.rs");
        tx.send(first.clone()).unwrap();
        tx.send(first.clone()).unwrap();
        tx.send(first.clone()).unwrap();
        tx.send(second.clone()).unwrap();

        let drain = drain_watched_paths(&rx, &overflowed, 2);

        assert!(!drain.overflowed);
        assert_eq!(drain.paths, [first, second]);
        assert!(rx.is_empty());
    }

    #[test]
    fn watcher_drain_collapses_equivalent_bursts_before_later_paths() {
        let (tx, rx) = bounded(8);
        let overflowed = AtomicBool::new(false);
        let first = PathBuf::from("workspace/src/./main.rs");
        let first_equivalent = PathBuf::from("workspace/src/generated/../main.rs");
        let second = PathBuf::from("workspace/src/lib.rs");
        tx.send(first.clone()).unwrap();
        tx.send(first_equivalent).unwrap();
        tx.send(second.clone()).unwrap();

        let drain = drain_watched_paths(&rx, &overflowed, 2);

        assert!(!drain.overflowed);
        assert_eq!(drain.paths, [first, second]);
        assert!(rx.is_empty());
    }

    #[test]
    fn watcher_drain_signals_overflow_when_unique_paths_exceed_budget() {
        let (tx, rx) = bounded(8);
        let overflowed = AtomicBool::new(false);
        let first = PathBuf::from("workspace/src/main.rs");
        let second = PathBuf::from("workspace/src/lib.rs");
        tx.send(first.clone()).unwrap();
        tx.send(second).unwrap();

        let drain = drain_watched_paths(&rx, &overflowed, 1);

        assert!(drain.overflowed);
        assert_eq!(drain.paths, [first]);
        assert!(rx.is_empty());
    }

    fn drain_single_event(kind: EventKind) -> (bool, Vec<PathBuf>) {
        let (tx, rx) = bounded(4);
        let overflowed = AtomicBool::new(false);
        let path = PathBuf::from("workspace/src/main.rs");

        enqueue_watcher_event(&tx, &overflowed, Ok(Event::new(kind).add_path(path)));
        let drain = drain_watched_paths(&rx, &overflowed, 8);

        (drain.overflowed, drain.paths)
    }
}
