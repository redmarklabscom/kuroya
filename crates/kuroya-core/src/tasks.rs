use anyhow::{Context, bail};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use crate::workspace_paths::normalize_child_path;

mod display;

#[cfg(test)]
use self::display::sanitize_task_display_text;
use self::display::{
    add_display_truncation_marker, is_task_display_format_control, sanitize_task_display_text_cow,
    strip_task_display_format_controls, task_display_label, truncate_task_display_text,
};

const TASK_DISCOVERY_PRUNED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "coverage",
    ".next",
    "out",
];
const WORKSPACE_TASKS_FILE_MAX_BYTES: u64 = 256 * 1024;
const CARGO_MANIFEST_MAX_BYTES: u64 = 512 * 1024;
const PACKAGE_MANIFEST_MAX_BYTES: u64 = 512 * 1024;
const MAKEFILE_MAX_BYTES: u64 = 512 * 1024;
const JUSTFILE_MAX_BYTES: u64 = 512 * 1024;
const WORKSPACE_TASKS_MAX_TASKS: usize = 256;
const PACKAGE_MANIFEST_MAX_SCRIPTS: usize = WORKSPACE_TASKS_MAX_TASKS;
const PACKAGE_WORKSPACE_PATTERNS_MAX_ITEMS: usize = 128;
const PACKAGE_WORKSPACE_DIRS_MAX_ITEMS: usize = 128;
const WORKSPACE_TASK_ARGS_MAX_ITEMS: usize = 256;
const WORKSPACE_TASK_ENV_MAX_ITEMS: usize = 256;
const WORKSPACE_TASK_NAME_MAX_CHARS: usize = 512;
const WORKSPACE_TASK_COMMAND_MAX_CHARS: usize = 1024;
const WORKSPACE_TASK_ARG_MAX_CHARS: usize = 4096;
const WORKSPACE_TASK_CWD_MAX_CHARS: usize = 4096;
const WORKSPACE_TASK_ENV_KEY_MAX_CHARS: usize = 256;
const WORKSPACE_TASK_ENV_VALUE_MAX_CHARS: usize = 4096;
const TASK_DISPLAY_LABEL_MAX_CHARS: usize = 120;
const TASK_COMMAND_PREVIEW_PART_MAX_CHARS: usize = 96;
const TASK_COMMAND_PREVIEW_MAX_CHARS: usize = 240;
const TASK_DISPLAY_TRUNCATION_MARKER: &str = "...";

#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceTaskKind {
    Build,
    Test,
    Run,
    #[default]
    Custom,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceTask {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
    pub kind: WorkspaceTaskKind,
    pub default: bool,
}

#[derive(Debug, Default, Deserialize)]
struct WorkspaceTasksFile {
    #[serde(default)]
    tasks: Vec<RawWorkspaceTask>,
}

#[derive(Debug, Deserialize)]
struct RawWorkspaceTask {
    name: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    cwd: Option<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    kind: WorkspaceTaskKind,
    #[serde(default)]
    default: bool,
}

pub fn workspace_tasks_path(root: &Path) -> PathBuf {
    root.join(".kuroya").join("tasks.toml")
}

pub fn load_workspace_tasks(root: &Path) -> anyhow::Result<Vec<WorkspaceTask>> {
    let path = workspace_tasks_path(root);
    match read_utf8_file_with_limit(&path, WORKSPACE_TASKS_FILE_MAX_BYTES) {
        Ok(text) => parse_workspace_tasks_toml(root, &text)
            .with_context(|| format!("could not parse {}", path.display())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(inferred_workspace_tasks(root)),
        Err(error) => Err(error).with_context(|| format!("could not read {}", path.display())),
    }
}

pub fn parse_workspace_tasks_toml(root: &Path, text: &str) -> anyhow::Result<Vec<WorkspaceTask>> {
    let file: WorkspaceTasksFile = toml::from_str(text)?;
    if file.tasks.len() > WORKSPACE_TASKS_MAX_TASKS {
        bail!(
            "workspace tasks file contains too many tasks ({} > {WORKSPACE_TASKS_MAX_TASKS})",
            file.tasks.len()
        );
    }

    let mut tasks = Vec::with_capacity(file.tasks.len());
    for (index, raw) in file.tasks.into_iter().enumerate() {
        tasks.push(normalize_task(root, index, raw)?);
    }
    Ok(tasks)
}

pub fn inferred_workspace_tasks(root: &Path) -> Vec<WorkspaceTask> {
    let mut tasks = Vec::with_capacity(WORKSPACE_TASKS_MAX_TASKS.min(16));
    extend_workspace_tasks_with_limit(&mut tasks, inferred_cargo_tasks(root));
    extend_workspace_tasks_with_limit(&mut tasks, inferred_package_json_tasks(root));
    extend_workspace_tasks_with_limit(&mut tasks, inferred_makefile_tasks(root));
    extend_workspace_tasks_with_limit(&mut tasks, inferred_justfile_tasks(root));
    tasks
}

fn extend_workspace_tasks_with_limit(
    tasks: &mut Vec<WorkspaceTask>,
    incoming: impl IntoIterator<Item = WorkspaceTask>,
) {
    extend_workspace_tasks_to_limit(tasks, incoming, WORKSPACE_TASKS_MAX_TASKS);
}

fn extend_workspace_tasks_to_limit(
    tasks: &mut Vec<WorkspaceTask>,
    incoming: impl IntoIterator<Item = WorkspaceTask>,
    max_tasks: usize,
) {
    let remaining = max_tasks.saturating_sub(tasks.len());
    if remaining > 0 {
        tasks.extend(incoming.into_iter().take(remaining));
    }
}

fn inferred_cargo_tasks(root: &Path) -> Vec<WorkspaceTask> {
    match CargoManifestInfo::detect(root) {
        Some(manifest) if manifest.kind == CargoManifestKind::Package => {
            let mut tasks = vec![
                cargo_task(root, "Cargo Check", ["check"], WorkspaceTaskKind::Build),
                cargo_task_with_default(
                    root,
                    "Cargo Clippy",
                    ["clippy", "--all-targets", "--", "-D", "warnings"],
                    WorkspaceTaskKind::Build,
                    false,
                ),
                cargo_task(root, "Cargo Test", ["test"], WorkspaceTaskKind::Test),
            ];
            tasks.extend(cargo_run_tasks(root, &manifest));
            tasks
        }
        Some(_) => vec![
            cargo_task(
                root,
                "Cargo Check Workspace",
                ["check", "--workspace"],
                WorkspaceTaskKind::Build,
            ),
            cargo_task_with_default(
                root,
                "Cargo Clippy Workspace",
                [
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--",
                    "-D",
                    "warnings",
                ],
                WorkspaceTaskKind::Build,
                false,
            ),
            cargo_task(
                root,
                "Cargo Test Workspace",
                ["test", "--workspace"],
                WorkspaceTaskKind::Test,
            ),
        ],
        None => Vec::new(),
    }
}

fn cargo_task<const N: usize>(
    root: &Path,
    name: &str,
    args: [&str; N],
    kind: WorkspaceTaskKind,
) -> WorkspaceTask {
    cargo_task_with_default(root, name, args, kind, true)
}

fn cargo_task_with_default<const N: usize>(
    root: &Path,
    name: &str,
    args: [&str; N],
    kind: WorkspaceTaskKind,
    default: bool,
) -> WorkspaceTask {
    let mut task_args = Vec::with_capacity(N);
    task_args.extend(args.into_iter().map(str::to_owned));

    WorkspaceTask {
        name: name.to_owned(),
        command: "cargo".to_owned(),
        args: task_args,
        cwd: Some(root.to_path_buf()),
        env: BTreeMap::new(),
        kind,
        default,
    }
}

fn cargo_run_tasks(root: &Path, manifest: &CargoManifestInfo) -> Vec<WorkspaceTask> {
    let mut bin_names = cargo_implicit_bin_names(root, manifest.package_name.as_deref());
    bin_names.extend(manifest.bin_names.iter().cloned());
    if let Some(default_run) = manifest.default_run.as_ref() {
        bin_names.insert(default_run.clone());
    }

    if bin_names.len() <= 1 && manifest.default_run.is_none() {
        return vec![cargo_task(
            root,
            "Cargo Run",
            ["run"],
            WorkspaceTaskKind::Run,
        )];
    }

    let mut tasks = Vec::with_capacity(bin_names.len());
    for bin in bin_names {
        let default = manifest.default_run.as_deref() == Some(bin.as_str());
        tasks.push(cargo_bin_run_task(root, &bin, default));
    }
    tasks
}

fn cargo_bin_run_task(root: &Path, bin: &str, default: bool) -> WorkspaceTask {
    let title = task_target_title(bin);
    WorkspaceTask {
        name: prefixed_task_display_label("Cargo Run ", &title),
        command: "cargo".to_owned(),
        args: vec!["run".to_owned(), "--bin".to_owned(), bin.to_owned()],
        cwd: Some(root.to_path_buf()),
        env: BTreeMap::new(),
        kind: WorkspaceTaskKind::Run,
        default,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CargoManifestInfo {
    kind: CargoManifestKind,
    package_name: Option<String>,
    default_run: Option<String>,
    bin_names: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CargoManifestKind {
    Package,
    VirtualWorkspace,
}

impl CargoManifestInfo {
    fn detect(root: &Path) -> Option<Self> {
        let path = root.join("Cargo.toml");
        let text = read_optional_regular_utf8_file_with_limit(&path, CARGO_MANIFEST_MAX_BYTES)?;
        let Ok(text) = text else {
            return Some(Self::package());
        };
        let Ok(manifest) = toml::from_str::<toml::Value>(&text) else {
            return Some(Self::package());
        };

        if manifest.get("package").is_some() {
            Some(Self {
                kind: CargoManifestKind::Package,
                package_name: cargo_manifest_package_name(&manifest),
                default_run: cargo_manifest_default_run(&manifest),
                bin_names: cargo_manifest_bin_names(&manifest),
            })
        } else if manifest.get("workspace").is_some() {
            Some(Self {
                kind: CargoManifestKind::VirtualWorkspace,
                package_name: None,
                default_run: None,
                bin_names: BTreeSet::new(),
            })
        } else {
            Some(Self::package())
        }
    }

    fn package() -> Self {
        Self {
            kind: CargoManifestKind::Package,
            package_name: None,
            default_run: None,
            bin_names: BTreeSet::new(),
        }
    }
}

fn cargo_manifest_package_name(manifest: &toml::Value) -> Option<String> {
    cargo_manifest_package_string(manifest, "name")
}

fn cargo_manifest_default_run(manifest: &toml::Value) -> Option<String> {
    cargo_manifest_package_string(manifest, "default-run")
}

fn cargo_manifest_package_string(manifest: &toml::Value, key: &str) -> Option<String> {
    manifest
        .get("package")
        .and_then(toml::Value::as_table)?
        .get(key)
        .and_then(toml::Value::as_str)
        .and_then(normalized_cargo_bin_name)
}

fn cargo_manifest_bin_names(manifest: &toml::Value) -> BTreeSet<String> {
    manifest
        .get("bin")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|bin| {
            bin.as_table()?
                .get("name")
                .and_then(toml::Value::as_str)
                .and_then(normalized_cargo_bin_name)
        })
        .collect()
}

fn cargo_implicit_bin_names(root: &Path, package_name: Option<&str>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    if root.join("src").join("main.rs").is_file()
        && let Some(package_name) = package_name.and_then(normalized_cargo_bin_name)
    {
        names.insert(package_name);
    }

    let bin_dir = root.join("src").join("bin");
    let Ok(entries) = fs::read_dir(bin_dir) else {
        return names;
    };

    for entry in entries.filter_map(Result::ok) {
        let file_name = entry.file_name();
        let file_name_path = Path::new(&file_name);
        if file_name_path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("rs")
        {
            if let Some(name) = file_name_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(normalized_cargo_bin_name)
            {
                names.insert(name);
            }
        } else {
            let can_have_main_rs = entry
                .file_type()
                .map(|file_type| file_type.is_dir() || file_type.is_symlink())
                .unwrap_or(true);
            if can_have_main_rs
                && entry.path().join("main.rs").is_file()
                && let Some(name) = file_name.to_str().and_then(normalized_cargo_bin_name)
            {
                names.insert(name);
            }
        }
    }

    names
}

fn normalized_cargo_bin_name(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && !value.contains('\0')).then(|| value.to_owned())
}

fn inferred_package_json_tasks(root: &Path) -> Vec<WorkspaceTask> {
    let Some(manifest) = read_package_manifest(&root.join("package.json")) else {
        return Vec::new();
    };
    let manager = PackageManager::detect(root, &manifest);
    let mut tasks = package_manifest_tasks(root, &manifest, manager, None);
    let remaining = WORKSPACE_TASKS_MAX_TASKS.saturating_sub(tasks.len());
    if remaining > 0 {
        tasks.extend(inferred_package_workspace_tasks(
            root, &manifest, manager, remaining,
        ));
    }
    tasks
}

fn inferred_package_workspace_tasks(
    root: &Path,
    manifest: &serde_json::Value,
    manager: PackageManager,
    max_tasks: usize,
) -> Vec<WorkspaceTask> {
    let patterns = package_workspace_patterns(manifest);
    if patterns.is_empty() || max_tasks == 0 {
        return Vec::new();
    }

    let package_dirs = workspace_package_dirs(root, &patterns);
    let mut tasks = Vec::with_capacity(max_tasks.min(package_dirs.len().saturating_mul(4)));
    for package_root in package_dirs {
        if tasks.len() >= max_tasks {
            break;
        }

        let Some(manifest) = read_package_manifest(&package_root.join("package.json")) else {
            continue;
        };
        let scope = package_scope_label(root, &package_root, &manifest);
        extend_workspace_tasks_to_limit(
            &mut tasks,
            package_manifest_tasks(&package_root, &manifest, manager, Some(&scope)),
            max_tasks,
        );
    }
    tasks
}

fn package_manifest_tasks(
    package_root: &Path,
    manifest: &serde_json::Value,
    manager: PackageManager,
    scope: Option<&str>,
) -> Vec<WorkspaceTask> {
    let Some(scripts) = manifest
        .get("scripts")
        .and_then(serde_json::Value::as_object)
    else {
        return Vec::new();
    };
    let script_names = package_manifest_script_names(scripts);
    let has_script = |script: &str| script_names.contains(script);
    let default_run_script = if has_script("start") {
        Some("start")
    } else if has_script("dev") {
        Some("dev")
    } else {
        None
    };
    let known_scripts = [
        ("build", WorkspaceTaskKind::Build, "Build"),
        ("test", WorkspaceTaskKind::Test, "Test"),
        ("start", WorkspaceTaskKind::Run, "Start"),
        ("dev", WorkspaceTaskKind::Run, "Dev"),
    ];

    let mut tasks = Vec::with_capacity(script_names.len().min(WORKSPACE_TASKS_MAX_TASKS));
    for (script, kind, label) in known_scripts {
        if has_script(script) {
            tasks.push(package_script_task(
                package_root,
                manager,
                script,
                &package_script_label(label, scope),
                kind,
                kind != WorkspaceTaskKind::Run || default_run_script == Some(script),
            ));
        }
    }

    let remaining = WORKSPACE_TASKS_MAX_TASKS.saturating_sub(tasks.len());
    if remaining > 0 {
        tasks.extend(
            script_names
                .iter()
                .copied()
                .filter_map(|script| {
                    let kind = infer_additional_package_script_kind(script, &script_names)?;
                    let title = package_script_title(script);
                    Some(package_script_task(
                        package_root,
                        manager,
                        script,
                        &package_script_label(&title, scope),
                        kind,
                        false,
                    ))
                })
                .take(remaining),
        );
    }

    tasks
}

fn package_manifest_script_names(
    scripts: &serde_json::Map<String, serde_json::Value>,
) -> BTreeSet<&str> {
    let mut script_names = BTreeSet::new();
    for (script, command) in scripts {
        if command.as_str().is_none() || !package_script_name_is_inferable(script) {
            continue;
        }

        if script_names.len() < PACKAGE_MANIFEST_MAX_SCRIPTS || is_known_package_script(script) {
            script_names.insert(script.as_str());
        }
    }
    script_names
}

fn package_script_name_is_inferable(script: &str) -> bool {
    let trimmed = script.trim();
    !trimmed.is_empty()
        && trimmed.len() == script.len()
        && script.chars().count() <= WORKSPACE_TASK_ARG_MAX_CHARS
        && !script.contains('\0')
        && !script
            .chars()
            .any(|ch| ch.is_control() || is_task_display_format_control(ch))
}

fn is_known_package_script(script: &str) -> bool {
    matches!(script, "build" | "test" | "start" | "dev")
}

fn package_script_task(
    package_root: &Path,
    manager: PackageManager,
    script: &str,
    label: &str,
    kind: WorkspaceTaskKind,
    default: bool,
) -> WorkspaceTask {
    let (command, args) = manager.command_for_script(script);
    WorkspaceTask {
        name: separated_task_display_label(manager.label(), label),
        command,
        args,
        cwd: Some(package_root.to_path_buf()),
        env: BTreeMap::new(),
        kind,
        default,
    }
}

fn package_script_label<'a>(label: &'a str, scope: Option<&str>) -> Cow<'a, str> {
    match scope {
        Some(scope) => {
            let mut scoped = String::with_capacity(label.len() + scope.len() + 3);
            scoped.push_str(label);
            scoped.push_str(" (");
            scoped.push_str(scope);
            scoped.push(')');
            Cow::Owned(scoped)
        }
        None => Cow::Borrowed(label),
    }
}

fn package_script_title(script: &str) -> String {
    task_target_title(script)
}

fn infer_additional_package_script_kind(
    script: &str,
    script_names: &BTreeSet<&str>,
) -> Option<WorkspaceTaskKind> {
    if matches!(script, "build" | "test" | "start" | "dev") {
        return None;
    }

    if let Some(base) = script
        .strip_prefix("pre")
        .or_else(|| script.strip_prefix("post"))
        .filter(|base| !base.is_empty())
        && script_names.contains(base)
    {
        return None;
    }

    let Some((namespace, suffix)) = script.split_once(':') else {
        return Some(WorkspaceTaskKind::Custom);
    };
    if suffix.is_empty() {
        return Some(WorkspaceTaskKind::Custom);
    }

    Some(match namespace {
        "build" => WorkspaceTaskKind::Build,
        "test" => WorkspaceTaskKind::Test,
        "start" | "dev" => WorkspaceTaskKind::Run,
        _ => WorkspaceTaskKind::Custom,
    })
}

fn task_target_title(target: &str) -> String {
    let target = strip_task_display_format_controls(target);
    let mut label = String::with_capacity(target.len());

    for part in target
        .split(['-', '_', ':'])
        .filter(|part| !part.is_empty())
    {
        if !label.is_empty() {
            label.push(' ');
        }

        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            label.extend(first.to_uppercase());
            label.push_str(chars.as_str());
        }
    }

    if label.is_empty() {
        task_display_label(&target)
    } else {
        task_display_label(&label)
    }
}

fn read_package_manifest(path: &Path) -> Option<serde_json::Value> {
    let text =
        read_optional_regular_utf8_file_with_limit(path, PACKAGE_MANIFEST_MAX_BYTES)?.ok()?;
    serde_json::from_str(&text).ok()
}

fn package_workspace_patterns(manifest: &serde_json::Value) -> Vec<Cow<'_, str>> {
    let Some(workspaces) = manifest.get("workspaces") else {
        return Vec::new();
    };
    let patterns = match workspaces {
        serde_json::Value::Array(patterns) => patterns,
        serde_json::Value::Object(object) => {
            let Some(patterns) = object.get("packages").and_then(serde_json::Value::as_array)
            else {
                return Vec::new();
            };
            patterns
        }
        _ => return Vec::new(),
    };

    let mut normalized = BTreeSet::new();
    for pattern in patterns
        .iter()
        .filter_map(serde_json::Value::as_str)
        .take(PACKAGE_WORKSPACE_PATTERNS_MAX_ITEMS)
    {
        if let Some(pattern) = normalize_package_workspace_pattern(pattern) {
            normalized.insert(pattern);
        }
    }
    let mut normalized_patterns = Vec::with_capacity(normalized.len());
    normalized_patterns.extend(normalized);
    normalized_patterns
}

fn normalize_package_workspace_pattern(pattern: &str) -> Option<Cow<'_, str>> {
    let pattern = strip_task_display_format_controls(pattern);
    let pattern = trim_package_workspace_pattern(pattern);
    (!pattern.is_empty()).then_some(pattern)
}

fn trim_package_workspace_pattern(pattern: Cow<'_, str>) -> Cow<'_, str> {
    match pattern {
        Cow::Borrowed(pattern) => Cow::Borrowed(pattern.trim().trim_end_matches(['/', '\\'])),
        Cow::Owned(pattern) => Cow::Owned(pattern.trim().trim_end_matches(['/', '\\']).to_owned()),
    }
}

fn workspace_package_dirs(root: &Path, patterns: &[Cow<'_, str>]) -> Vec<PathBuf> {
    let Some(globs) = PackageWorkspaceGlobs::from_patterns(patterns) else {
        return Vec::new();
    };
    let mut package_dirs = BTreeSet::new();

    for entry in ignore::WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .filter_entry(task_discovery_entry_is_not_pruned)
        .build()
        .filter_map(Result::ok)
    {
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }
        if entry.file_name() != "package.json" {
            continue;
        }
        let Some(package_dir) = entry.path().parent() else {
            continue;
        };
        if package_dir == root {
            continue;
        }
        let Ok(relative_dir) = package_dir.strip_prefix(root) else {
            continue;
        };
        if globs.is_match(relative_dir) {
            package_dirs.insert(package_dir.to_path_buf());
            if package_dirs.len() >= PACKAGE_WORKSPACE_DIRS_MAX_ITEMS {
                break;
            }
        }
    }

    let mut dirs = Vec::with_capacity(package_dirs.len());
    dirs.extend(package_dirs);
    dirs
}

struct PackageWorkspaceGlobs {
    includes: GlobSet,
    excludes: Option<GlobSet>,
}

impl PackageWorkspaceGlobs {
    fn from_patterns(patterns: &[Cow<'_, str>]) -> Option<Self> {
        let includes = package_workspace_glob_set(
            patterns
                .iter()
                .map(|pattern| pattern.as_ref())
                .filter(|pattern| !pattern.starts_with('!')),
        )?;
        let excludes = package_workspace_glob_set(
            patterns
                .iter()
                .map(|pattern| pattern.as_ref())
                .filter_map(|pattern| pattern.strip_prefix('!'))
                .map(str::trim)
                .filter(|pattern| !pattern.is_empty()),
        );
        Some(Self { includes, excludes })
    }

    fn is_match(&self, relative_dir: &Path) -> bool {
        self.includes.is_match(relative_dir)
            && !self
                .excludes
                .as_ref()
                .is_some_and(|excludes| excludes.is_match(relative_dir))
    }
}

fn package_workspace_glob_set<'a>(patterns: impl IntoIterator<Item = &'a str>) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    let mut added = false;
    for pattern in patterns {
        let Ok(glob) = Glob::new(pattern) else {
            continue;
        };
        builder.add(glob);
        added = true;
    }
    added.then(|| builder.build().ok()).flatten()
}

fn task_discovery_entry_is_not_pruned(entry: &ignore::DirEntry) -> bool {
    entry.depth() == 0
        || entry.file_name().to_str().is_none_or(|name| {
            !TASK_DISCOVERY_PRUNED_DIRS
                .iter()
                .any(|pruned| name.eq_ignore_ascii_case(pruned))
        })
}

fn package_scope_label(root: &Path, package_root: &Path, manifest: &serde_json::Value) -> String {
    if let Some(name) = manifest
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        return task_display_label(name);
    }

    let label = package_root
        .strip_prefix(root)
        .unwrap_or(package_root)
        .to_string_lossy();
    let label = if label.as_bytes().contains(&b'\\') {
        Cow::Owned(label.replace('\\', "/"))
    } else {
        label
    };
    task_display_label(&label)
}

fn inferred_makefile_tasks(root: &Path) -> Vec<WorkspaceTask> {
    let Some(text) = read_first_regular_utf8_file_with_limit(
        root,
        &["Makefile", "makefile"],
        MAKEFILE_MAX_BYTES,
    ) else {
        return Vec::new();
    };
    let Ok(text) = text else {
        return Vec::new();
    };

    let targets = makefile_targets(&text);
    let known_targets = [
        ("build", WorkspaceTaskKind::Build, "Make Build"),
        ("test", WorkspaceTaskKind::Test, "Make Test"),
        ("run", WorkspaceTaskKind::Run, "Make Run"),
    ];
    let mut tasks = Vec::with_capacity(targets.len().min(known_targets.len()));
    for (target, kind, name) in known_targets {
        if targets.contains(target) {
            tasks.push(WorkspaceTask {
                name: name.to_owned(),
                command: "make".to_owned(),
                args: vec![target.to_owned()],
                cwd: Some(root.to_path_buf()),
                env: BTreeMap::new(),
                kind,
                default: true,
            });
        }
    }
    tasks
}

fn inferred_justfile_tasks(root: &Path) -> Vec<WorkspaceTask> {
    let Some(text) = read_first_regular_utf8_file_with_limit(
        root,
        &["justfile", "Justfile", ".justfile"],
        JUSTFILE_MAX_BYTES,
    ) else {
        return Vec::new();
    };
    let Ok(text) = text else {
        return Vec::new();
    };

    let targets = justfile_targets(&text);
    let mut tasks = Vec::with_capacity(targets.len());
    for target in targets {
        let kind = justfile_task_kind(&target);
        let title = task_target_title(&target);
        tasks.push(WorkspaceTask {
            name: prefixed_task_display_label("Just ", &title),
            command: "just".to_owned(),
            args: vec![target],
            cwd: Some(root.to_path_buf()),
            env: BTreeMap::new(),
            kind,
            default: kind != WorkspaceTaskKind::Custom,
        });
    }
    tasks
}

fn prefixed_task_display_label(prefix: &str, label: &str) -> String {
    let mut value = String::with_capacity(prefix.len() + label.len());
    value.push_str(prefix);
    value.push_str(label);
    task_display_label(&value)
}

fn separated_task_display_label(left: &str, right: &str) -> String {
    let mut value = String::with_capacity(left.len() + 1 + right.len());
    value.push_str(left);
    value.push(' ');
    value.push_str(right);
    task_display_label(&value)
}

fn justfile_targets(text: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut targets = Vec::with_capacity(WORKSPACE_TASKS_MAX_TASKS.min(16));

    for line in text.lines() {
        if line.is_empty()
            || line.chars().next().is_some_and(char::is_whitespace)
            || line.trim_start().starts_with('#')
        {
            continue;
        }

        let Some(colon_index) = line.find(':') else {
            continue;
        };
        if line.as_bytes().get(colon_index + 1) == Some(&b'=') {
            continue;
        }

        let target = line[..colon_index]
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_start_matches('@');
        if target.starts_with('_') || !is_valid_justfile_target(target) {
            continue;
        }

        if seen.insert(target) {
            targets.push(target.to_owned());
            if targets.len() >= WORKSPACE_TASKS_MAX_TASKS {
                break;
            }
        }
    }

    targets
}

fn is_valid_justfile_target(target: &str) -> bool {
    !target.is_empty()
        && target
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

fn justfile_task_kind(target: &str) -> WorkspaceTaskKind {
    match target {
        "build" => WorkspaceTaskKind::Build,
        "test" => WorkspaceTaskKind::Test,
        "run" | "start" | "dev" => WorkspaceTaskKind::Run,
        _ => WorkspaceTaskKind::Custom,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

impl PackageManager {
    fn detect(root: &Path, manifest: &serde_json::Value) -> Self {
        if root.join("pnpm-lock.yaml").is_file() {
            Self::Pnpm
        } else if root.join("yarn.lock").is_file() {
            Self::Yarn
        } else if root.join("bun.lockb").is_file() || root.join("bun.lock").is_file() {
            Self::Bun
        } else {
            Self::from_package_manager_field(manifest).unwrap_or(Self::Npm)
        }
    }

    fn from_package_manager_field(manifest: &serde_json::Value) -> Option<Self> {
        let manager = manifest
            .get("packageManager")
            .and_then(serde_json::Value::as_str)?
            .trim();
        let name = manager.split_once('@').map_or(manager, |(name, _)| name);

        match name {
            "npm" => Some(Self::Npm),
            "pnpm" => Some(Self::Pnpm),
            "yarn" => Some(Self::Yarn),
            "bun" => Some(Self::Bun),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Npm => "NPM",
            Self::Pnpm => "PNPM",
            Self::Yarn => "Yarn",
            Self::Bun => "Bun",
        }
    }

    fn command_for_script(self, script: &str) -> (String, Vec<String>) {
        match self {
            Self::Npm => ("npm".to_owned(), vec!["run".to_owned(), script.to_owned()]),
            Self::Pnpm => ("pnpm".to_owned(), vec!["run".to_owned(), script.to_owned()]),
            Self::Yarn => ("yarn".to_owned(), vec!["run".to_owned(), script.to_owned()]),
            Self::Bun => ("bun".to_owned(), vec!["run".to_owned(), script.to_owned()]),
        }
    }
}

fn makefile_targets(text: &str) -> BTreeSet<&str> {
    let mut targets = BTreeSet::new();
    for line in text.lines() {
        let is_recipe_line = line.starts_with('\t');
        let line = line.trim_start();
        if is_recipe_line || line.starts_with('#') || line.starts_with('.') {
            continue;
        }
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let rest = rest.trim_start();
        if rest.starts_with('=') || rest.starts_with(":=") || rest.starts_with("::=") {
            continue;
        }
        targets.extend(name.split_whitespace());
    }
    targets
}

fn read_first_regular_utf8_file_with_limit(
    root: &Path,
    file_names: &[&str],
    max_bytes: u64,
) -> Option<io::Result<String>> {
    for file_name in file_names {
        let path = root.join(file_name);
        if let Some(text) = read_optional_regular_utf8_file_with_limit(&path, max_bytes) {
            return Some(text);
        }
    }
    None
}

fn read_optional_regular_utf8_file_with_limit(
    path: &Path,
    max_bytes: u64,
) -> Option<io::Result<String>> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) => {
            let metadata = fs::metadata(path).ok()?;
            if !metadata.is_file() {
                return None;
            }
            if metadata.len() > max_bytes {
                return Some(Err(task_discovery_file_too_large_error(path, max_bytes)));
            }
            return Some(Err(error));
        }
    };
    let metadata = match file.metadata() {
        Ok(metadata) => metadata,
        Err(error) => return Some(Err(error)),
    };
    if !metadata.is_file() {
        return None;
    }
    Some(read_utf8_file_with_known_metadata_limit(
        path, file, &metadata, max_bytes,
    ))
}

fn read_utf8_file_with_known_metadata_limit(
    path: &Path,
    file: fs::File,
    metadata: &fs::Metadata,
    max_bytes: u64,
) -> io::Result<String> {
    if metadata.len() > max_bytes {
        return Err(task_discovery_file_too_large_error(path, max_bytes));
    }
    read_utf8_from_reader_with_limit(path, file, max_bytes)
}

fn read_utf8_file_with_limit(path: &Path, max_bytes: u64) -> io::Result<String> {
    let file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    read_utf8_file_with_known_metadata_limit(path, file, &metadata, max_bytes)
}

fn read_utf8_from_reader_with_limit(
    path: &Path,
    reader: impl Read,
    max_bytes: u64,
) -> io::Result<String> {
    let max_read = max_bytes.saturating_add(1);
    let capacity = usize::try_from(max_read)
        .unwrap_or(usize::MAX)
        .min(64 * 1024);
    let mut bytes = Vec::with_capacity(capacity);
    reader.take(max_read).read_to_end(&mut bytes)?;

    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(task_discovery_file_too_large_error(path, max_bytes));
    }

    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn task_discovery_file_too_large_error(path: &Path, max_bytes: u64) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "{} exceeds workspace task discovery limit of {max_bytes} bytes",
            path.display()
        ),
    )
}

pub fn workspace_task_command_preview(task: &WorkspaceTask) -> String {
    let mut preview = String::with_capacity(TASK_COMMAND_PREVIEW_MAX_CHARS);
    let mut preview_chars = push_shell_preview_part(&mut preview, &task.command);

    let mut truncated = preview_chars >= TASK_COMMAND_PREVIEW_MAX_CHARS && !task.args.is_empty();
    for (index, arg) in task.args.iter().enumerate() {
        if truncated {
            break;
        }
        preview.push(' ');
        preview_chars += 1;
        preview_chars += push_shell_preview_part(&mut preview, arg);
        truncated = preview_chars > TASK_COMMAND_PREVIEW_MAX_CHARS
            || (preview_chars == TASK_COMMAND_PREVIEW_MAX_CHARS && index + 1 < task.args.len());
    }

    if truncated {
        add_display_truncation_marker(&preview, TASK_COMMAND_PREVIEW_MAX_CHARS, false)
    } else {
        truncate_task_display_text(preview, TASK_COMMAND_PREVIEW_MAX_CHARS, false)
    }
}

pub fn workspace_task_kind_label(kind: WorkspaceTaskKind) -> &'static str {
    match kind {
        WorkspaceTaskKind::Build => "build",
        WorkspaceTaskKind::Test => "test",
        WorkspaceTaskKind::Run => "run",
        WorkspaceTaskKind::Custom => "task",
    }
}

pub fn workspace_task_kind_title(kind: WorkspaceTaskKind) -> &'static str {
    match kind {
        WorkspaceTaskKind::Build => "Build",
        WorkspaceTaskKind::Test => "Test",
        WorkspaceTaskKind::Run => "Run",
        WorkspaceTaskKind::Custom => "Task",
    }
}

pub fn workspace_task_default_index(
    tasks: &[WorkspaceTask],
    kind: WorkspaceTaskKind,
) -> Option<usize> {
    tasks
        .iter()
        .position(|task| task.kind == kind && task.default)
        .or_else(|| tasks.iter().position(|task| task.kind == kind))
}

fn normalize_task(
    root: &Path,
    index: usize,
    raw: RawWorkspaceTask,
) -> anyhow::Result<WorkspaceTask> {
    let name = normalize_task_name(raw.name, index)?;
    let command = normalized_non_empty(
        raw.command,
        "task command",
        WORKSPACE_TASK_COMMAND_MAX_CHARS,
        index,
    )?;
    let args = normalize_task_args(raw.args, index)?;
    let cwd = normalize_task_cwd(root, raw.cwd.as_deref(), index)?;
    let env = normalize_task_env(raw.env, index)?;

    Ok(WorkspaceTask {
        name,
        command,
        args,
        cwd,
        env,
        kind: raw.kind,
        default: raw.default,
    })
}

fn normalize_task_name(value: String, index: usize) -> anyhow::Result<String> {
    let value = trim_owned(value);
    if value.is_empty() {
        bail!("workspace task {index} has an empty task name");
    }
    ensure_char_limit(&value, "task name", WORKSPACE_TASK_NAME_MAX_CHARS, index)?;
    ensure_no_nul(&value, "task name", index)?;
    Ok(task_display_label(&value))
}

fn normalized_non_empty(
    value: String,
    field: &str,
    max_chars: usize,
    index: usize,
) -> anyhow::Result<String> {
    let value = trim_owned(value);
    if value.is_empty() {
        bail!("workspace task {index} has an empty {field}");
    }
    ensure_char_limit(&value, field, max_chars, index)?;
    ensure_no_nul(&value, field, index)?;
    ensure_no_hidden_format_controls(&value, field, index)?;
    Ok(value)
}

fn normalize_task_args(args: Vec<String>, index: usize) -> anyhow::Result<Vec<String>> {
    if args.len() > WORKSPACE_TASK_ARGS_MAX_ITEMS {
        bail!(
            "workspace task {index} has too many arguments ({} > {WORKSPACE_TASK_ARGS_MAX_ITEMS})",
            args.len()
        );
    }

    let mut normalized = Vec::with_capacity(args.len());
    for arg in args {
        let arg = trim_owned(arg);
        if arg.is_empty() {
            continue;
        }
        ensure_char_limit(&arg, "task argument", WORKSPACE_TASK_ARG_MAX_CHARS, index)?;
        ensure_no_nul(&arg, "task argument", index)?;
        ensure_no_hidden_format_controls(&arg, "task argument", index)?;
        normalized.push(arg);
    }
    Ok(normalized)
}

fn normalize_task_env(
    env: BTreeMap<String, String>,
    index: usize,
) -> anyhow::Result<BTreeMap<String, String>> {
    if env.len() > WORKSPACE_TASK_ENV_MAX_ITEMS {
        bail!(
            "workspace task {index} has too many environment variables ({} > {WORKSPACE_TASK_ENV_MAX_ITEMS})",
            env.len()
        );
    }

    let mut normalized = BTreeMap::new();
    for (key, value) in env {
        let key = trim_owned(key);
        if key.is_empty() {
            bail!("workspace task {index} has an empty environment variable name");
        }
        ensure_char_limit(
            &key,
            "environment variable name",
            WORKSPACE_TASK_ENV_KEY_MAX_CHARS,
            index,
        )?;
        ensure_no_nul(&key, "environment variable name", index)?;
        ensure_no_hidden_format_controls(&key, "environment variable name", index)?;
        if key.contains('=') {
            bail!("workspace task {index} environment variable name must not contain `=`");
        }
        ensure_char_limit(
            &value,
            "environment variable value",
            WORKSPACE_TASK_ENV_VALUE_MAX_CHARS,
            index,
        )?;
        ensure_no_nul(&value, "environment variable value", index)?;
        ensure_no_hidden_format_controls(&value, "environment variable value", index)?;
        normalized.insert(key, value);
    }
    Ok(normalized)
}

fn normalize_task_cwd(
    root: &Path,
    cwd: Option<&str>,
    index: usize,
) -> anyhow::Result<Option<PathBuf>> {
    let Some(cwd) = cwd.map(str::trim).filter(|cwd| !cwd.is_empty()) else {
        return Ok(None);
    };
    ensure_char_limit(cwd, "task cwd", WORKSPACE_TASK_CWD_MAX_CHARS, index)?;
    ensure_no_nul(cwd, "task cwd", index)?;
    ensure_no_hidden_format_controls(cwd, "task cwd", index)?;

    let candidate = PathBuf::from(cwd);
    let Some(candidate) = normalize_child_path(root, &candidate) else {
        bail!("workspace task {index} cwd must stay inside the workspace");
    };

    Ok(Some(candidate))
}

fn ensure_char_limit(
    value: &str,
    field: &str,
    max_chars: usize,
    index: usize,
) -> anyhow::Result<()> {
    if value.chars().count() > max_chars {
        bail!("workspace task {index} {field} is too long ({max_chars} character limit)");
    }
    Ok(())
}

fn ensure_no_nul(value: &str, field: &str, index: usize) -> anyhow::Result<()> {
    if value.contains('\0') {
        bail!("workspace task {index} {field} contains a null byte");
    }
    Ok(())
}

fn ensure_no_hidden_format_controls(value: &str, field: &str, index: usize) -> anyhow::Result<()> {
    if value.chars().any(is_task_display_format_control) {
        bail!("workspace task {index} {field} contains a hidden format control");
    }
    Ok(())
}

fn trim_owned(value: String) -> String {
    let trimmed = value.trim();
    if trimmed.len() == value.len() {
        value
    } else {
        trimmed.to_owned()
    }
}

fn push_shell_preview_part(preview: &mut String, part: &str) -> usize {
    let part = sanitize_task_display_text_cow(part, TASK_COMMAND_PREVIEW_PART_MAX_CHARS, false);
    let part = part.as_ref();
    if part.is_empty() {
        preview.push_str("\"\"");
        return 2;
    }

    if part
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | '\\' | ':'))
    {
        preview.push_str(part);
        part.len()
    } else {
        preview.push('"');
        let mut chars = 2;
        for ch in part.chars() {
            if ch == '"' {
                preview.push_str("\\\"");
                chars += 2;
            } else {
                preview.push(ch);
                chars += 1;
            }
        }
        preview.push('"');
        chars
    }
}

#[cfg(test)]
mod tests;
