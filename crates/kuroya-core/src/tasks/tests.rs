use super::*;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

mod discovery;

#[test]
fn parse_workspace_tasks_normalizes_fields() {
    let root = PathBuf::from("workspace");
    let tasks = parse_workspace_tasks_toml(
        &root,
        r#"
                [[tasks]]
                name = " Test "
                command = " cargo "
                args = [" test ", "", "--all"]
                cwd = " crates/app "
                kind = "test"
                default = true

                [tasks.env]
                " RUST_BACKTRACE " = "1"
            "#,
    )
    .unwrap();

    assert_eq!(
        tasks,
        vec![WorkspaceTask {
            name: "Test".to_owned(),
            command: "cargo".to_owned(),
            args: vec!["test".to_owned(), "--all".to_owned()],
            cwd: Some(PathBuf::from("workspace").join("crates/app")),
            env: BTreeMap::from([("RUST_BACKTRACE".to_owned(), "1".to_owned())]),
            kind: WorkspaceTaskKind::Test,
            default: true,
        }]
    );
}

#[test]
fn parse_workspace_tasks_rejects_empty_names_and_commands() {
    let root = PathBuf::from("workspace");
    let empty_name = parse_workspace_tasks_toml(
        &root,
        r#"
                [[tasks]]
                name = " "
                command = "cargo"
            "#,
    );
    let empty_command = parse_workspace_tasks_toml(
        &root,
        r#"
                [[tasks]]
                name = "build"
                command = " "
            "#,
    );

    assert!(empty_name.is_err());
    assert!(empty_command.is_err());
}

#[test]
fn normalize_workspace_tasks_rejects_dense_structural_inputs() {
    let root = PathBuf::from("workspace");
    let mut text = String::new();
    for index in 0..=WORKSPACE_TASKS_MAX_TASKS {
        text.push_str("[[tasks]]\nname = \"task");
        text.push_str(&index.to_string());
        text.push_str("\"\ncommand = \"cargo\"\n");
    }

    let error = parse_workspace_tasks_toml(&root, &text)
        .unwrap_err()
        .to_string();
    assert!(error.contains("too many tasks"), "{error}");

    let mut args = raw_workspace_task();
    args.args = (0..=WORKSPACE_TASK_ARGS_MAX_ITEMS)
        .map(|index| format!("arg{index}"))
        .collect();
    assert_normalize_task_error(args, "too many arguments");

    let mut env = raw_workspace_task();
    env.env = (0..=WORKSPACE_TASK_ENV_MAX_ITEMS)
        .map(|index| (format!("KEY_{index}"), "1".to_owned()))
        .collect();
    assert_normalize_task_error(env, "too many environment variables");
}

#[test]
fn normalize_workspace_tasks_rejects_nul_task_metadata() {
    let mut command = raw_workspace_task();
    command.command = "cargo\0".to_owned();
    assert_normalize_task_error(command, "task command contains a null byte");

    let mut arg = raw_workspace_task();
    arg.args = vec!["test\0".to_owned()];
    assert_normalize_task_error(arg, "task argument contains a null byte");

    let mut cwd = raw_workspace_task();
    cwd.cwd = Some("crates\0/app".to_owned());
    assert_normalize_task_error(cwd, "task cwd contains a null byte");

    let mut env_key = raw_workspace_task();
    env_key.env.insert("BAD\0KEY".to_owned(), "1".to_owned());
    assert_normalize_task_error(env_key, "environment variable name contains a null byte");

    let mut env_value = raw_workspace_task();
    env_value
        .env
        .insert("RUSTFLAGS".to_owned(), "-D\0warnings".to_owned());
    assert_normalize_task_error(env_value, "environment variable value contains a null byte");
}

#[test]
fn normalize_workspace_tasks_rejects_hidden_format_controls_in_runtime_metadata() {
    let mut command = raw_workspace_task();
    command.command = "cargo\u{202e}".to_owned();
    assert_normalize_task_error(command, "task command contains a hidden format control");

    let mut arg = raw_workspace_task();
    arg.args = vec!["test\u{200b}".to_owned()];
    assert_normalize_task_error(arg, "task argument contains a hidden format control");

    let mut cwd = raw_workspace_task();
    cwd.cwd = Some("crates\u{2066}/app".to_owned());
    assert_normalize_task_error(cwd, "task cwd contains a hidden format control");

    let mut env_key = raw_workspace_task();
    env_key
        .env
        .insert("BAD\u{200f}KEY".to_owned(), "1".to_owned());
    assert_normalize_task_error(
        env_key,
        "environment variable name contains a hidden format control",
    );

    let mut env_value = raw_workspace_task();
    env_value
        .env
        .insert("RUSTFLAGS".to_owned(), "-D\u{202a}warnings".to_owned());
    assert_normalize_task_error(
        env_value,
        "environment variable value contains a hidden format control",
    );
}

#[test]
fn normalize_workspace_tasks_rejects_oversized_task_metadata_strings() {
    let mut name = raw_workspace_task();
    name.name = "n".repeat(WORKSPACE_TASK_NAME_MAX_CHARS + 1);
    assert_normalize_task_error(name, "task name is too long");

    let mut command = raw_workspace_task();
    command.command = "c".repeat(WORKSPACE_TASK_COMMAND_MAX_CHARS + 1);
    assert_normalize_task_error(command, "task command is too long");

    let mut arg = raw_workspace_task();
    arg.args = vec!["a".repeat(WORKSPACE_TASK_ARG_MAX_CHARS + 1)];
    assert_normalize_task_error(arg, "task argument is too long");

    let mut cwd = raw_workspace_task();
    cwd.cwd = Some("c".repeat(WORKSPACE_TASK_CWD_MAX_CHARS + 1));
    assert_normalize_task_error(cwd, "task cwd is too long");

    let mut env_key = raw_workspace_task();
    env_key.env.insert(
        "K".repeat(WORKSPACE_TASK_ENV_KEY_MAX_CHARS + 1),
        "1".to_owned(),
    );
    assert_normalize_task_error(env_key, "environment variable name is too long");

    let mut env_value = raw_workspace_task();
    env_value.env.insert(
        "RUSTFLAGS".to_owned(),
        "v".repeat(WORKSPACE_TASK_ENV_VALUE_MAX_CHARS + 1),
    );
    assert_normalize_task_error(env_value, "environment variable value is too long");
}

#[test]
fn normalize_workspace_task_name_sanitizes_and_bounds_display_text() {
    let root = PathBuf::from("workspace");
    let mut raw = raw_workspace_task();
    raw.name = format!(
        " Build\n\u{202e}{}\u{2028}Release ",
        "x".repeat(TASK_DISPLAY_LABEL_MAX_CHARS * 2)
    );

    let task = normalize_task(&root, 0, raw).unwrap();

    assert!(!task.name.chars().any(|ch| ch.is_control()));
    assert!(!task.name.chars().any(is_task_display_format_control));
    assert!(task.name.chars().count() <= TASK_DISPLAY_LABEL_MAX_CHARS);
    assert!(task.name.starts_with("Build "));
    assert!(task.name.ends_with(TASK_DISPLAY_TRUNCATION_MARKER));
}

#[test]
fn task_display_text_fast_path_preserves_simple_ascii_rules() {
    assert_eq!(
        sanitize_task_display_text("  Cargo Build  ", TASK_DISPLAY_LABEL_MAX_CHARS, true),
        "Cargo Build"
    );
    assert_eq!(
        sanitize_task_display_text("arg ", TASK_COMMAND_PREVIEW_PART_MAX_CHARS, false),
        "arg "
    );
    assert_eq!(
        sanitize_task_display_text(" arg  value", TASK_COMMAND_PREVIEW_PART_MAX_CHARS, false),
        "arg value"
    );
}

#[test]
fn task_display_text_fast_path_borrows_simple_display_text() {
    match sanitize_task_display_text_cow("  Cargo Build  ", TASK_DISPLAY_LABEL_MAX_CHARS, true) {
        Cow::Borrowed(label) => assert_eq!(label, "Cargo Build"),
        Cow::Owned(label) => panic!("simple task label was allocated: {label}"),
    }

    match sanitize_task_display_text_cow("hello world", TASK_COMMAND_PREVIEW_PART_MAX_CHARS, false)
    {
        Cow::Borrowed(part) => assert_eq!(part, "hello world"),
        Cow::Owned(part) => panic!("simple command preview part was allocated: {part}"),
    }
}

#[test]
fn task_display_text_fast_path_borrows_clean_unicode_display_text() {
    match sanitize_task_display_text_cow(
        "  \u{7f16}\u{8bd1} \u{53d1}\u{5e03}  ",
        TASK_DISPLAY_LABEL_MAX_CHARS,
        true,
    ) {
        Cow::Borrowed(label) => assert_eq!(label, "\u{7f16}\u{8bd1} \u{53d1}\u{5e03}"),
        Cow::Owned(label) => panic!("clean Unicode task label was allocated: {label}"),
    }

    match sanitize_task_display_text_cow(
        "\u{30b3}\u{30f3}\u{30d1}\u{30a4}\u{30eb} \u{5f15}\u{6570} ",
        TASK_COMMAND_PREVIEW_PART_MAX_CHARS,
        false,
    ) {
        Cow::Borrowed(part) => assert_eq!(
            part,
            "\u{30b3}\u{30f3}\u{30d1}\u{30a4}\u{30eb} \u{5f15}\u{6570} "
        ),
        Cow::Owned(part) => panic!("clean Unicode command preview part was allocated: {part}"),
    }
}

#[test]
fn task_display_text_unicode_dirty_paths_still_allocate_and_sanitize() {
    match sanitize_task_display_text_cow(
        "\u{7f16}\u{8bd1}\u{00a0}\u{53d1}\u{5e03}",
        TASK_DISPLAY_LABEL_MAX_CHARS,
        true,
    ) {
        Cow::Owned(label) => {
            assert_eq!(label, "\u{7f16}\u{8bd1} \u{53d1}\u{5e03}")
        }
        Cow::Borrowed(label) => panic!("dirty Unicode whitespace was borrowed: {label}"),
    }

    match sanitize_task_display_text_cow(
        "\u{7f16}\u{8bd1}\u{200b}\u{53d1}\u{5e03}",
        TASK_DISPLAY_LABEL_MAX_CHARS,
        true,
    ) {
        Cow::Owned(label) => assert_eq!(label, "\u{7f16}\u{8bd1}\u{53d1}\u{5e03}"),
        Cow::Borrowed(label) => panic!("Unicode format control was borrowed: {label}"),
    }

    let overlong = "\u{754c}".repeat(TASK_DISPLAY_LABEL_MAX_CHARS + 1);
    match sanitize_task_display_text_cow(&overlong, TASK_DISPLAY_LABEL_MAX_CHARS, true) {
        Cow::Owned(label) => {
            assert!(label.ends_with(TASK_DISPLAY_TRUNCATION_MARKER));
            assert_eq!(label.chars().count(), TASK_DISPLAY_LABEL_MAX_CHARS);
        }
        Cow::Borrowed(label) => panic!("overlong Unicode display text was borrowed: {label}"),
    }
}

#[test]
fn parse_workspace_tasks_rejects_environment_names_with_equals() {
    let root = PathBuf::from("workspace");
    let result = parse_workspace_tasks_toml(
        &root,
        r#"
                [[tasks]]
                name = "build"
                command = "cargo"

                [tasks.env]
                "BAD=KEY" = "1"
            "#,
    );

    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("environment variable name must not contain `=`"),
        "{error}"
    );
}

#[test]
fn parse_workspace_tasks_rejects_cwd_outside_workspace() {
    let root = PathBuf::from("workspace");
    let result = parse_workspace_tasks_toml(
        &root,
        r#"
                [[tasks]]
                name = "bad"
                command = "cargo"
                cwd = "../outside"
            "#,
    );

    assert!(result.is_err());
}

#[test]
fn parse_workspace_tasks_rejects_cwd_parent_reentry() {
    let root = PathBuf::from("workspace");
    for cwd in ["../workspace", "tools/../../workspace/tools"] {
        let text = format!(
            r#"
                    [[tasks]]
                    name = "bad"
                    command = "cargo"
                    cwd = "{cwd}"
                "#
        );

        assert!(parse_workspace_tasks_toml(&root, &text).is_err(), "{cwd}");
    }
}

#[cfg(windows)]
#[test]
fn parse_workspace_tasks_normalizes_windows_cwd_containment() {
    let root = PathBuf::from(r"C:\Repo\Project");
    let tasks = parse_workspace_tasks_toml(
        &root,
        r#"
                [[tasks]]
                name = "build"
                command = "cargo"
                cwd = 'c:\repo\project\crates\app'
            "#,
    )
    .unwrap();

    assert_eq!(
        tasks[0].cwd,
        Some(PathBuf::from(r"c:\repo\project\crates\app"))
    );

    for cwd in [
        r"\Repo\Project\crates\app",
        r"C:crates\app",
        r"C:\Repo\ProjectSibling\app",
    ] {
        let text = format!(
            r#"
                    [[tasks]]
                    name = "bad"
                    command = "cargo"
                    cwd = '{cwd}'
                "#
        );
        assert!(parse_workspace_tasks_toml(&root, &text).is_err(), "{cwd}");
    }
}

#[test]
fn load_workspace_tasks_infers_cargo_tasks_when_config_is_missing() {
    let root = unique_test_dir("kuroya-cargo-tasks");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();

    let tasks = load_workspace_tasks(&root).unwrap();

    assert_eq!(
        tasks
            .iter()
            .map(|task| (
                task.name.as_str(),
                task.kind,
                task.args.iter().map(String::as_str).collect::<Vec<_>>(),
                task.default
            ))
            .collect::<Vec<_>>(),
        vec![
            ("Cargo Check", WorkspaceTaskKind::Build, vec!["check"], true),
            (
                "Cargo Clippy",
                WorkspaceTaskKind::Build,
                vec!["clippy", "--all-targets", "--", "-D", "warnings"],
                false
            ),
            ("Cargo Test", WorkspaceTaskKind::Test, vec!["test"], true),
            ("Cargo Run", WorkspaceTaskKind::Run, vec!["run"], true),
        ]
    );
    assert_eq!(
        workspace_task_default_index(&tasks, WorkspaceTaskKind::Build),
        Some(0)
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_workspace_tasks_infers_explicit_cargo_bin_run_configs() {
    let root = unique_test_dir("kuroya-cargo-bin-tasks");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        concat!(
            "[package]\n",
            "name = \"demo\"\n",
            "\n",
            "[[bin]]\n",
            "name = \"server\"\n",
            "path = \"src/server.rs\"\n",
            "\n",
            "[[bin]]\n",
            "name = \"worker\"\n",
            "path = \"src/worker.rs\"\n",
        ),
    )
    .unwrap();

    let tasks = load_workspace_tasks(&root).unwrap();

    assert_eq!(
        tasks
            .iter()
            .filter(|task| task.kind == WorkspaceTaskKind::Run)
            .map(|task| (
                task.name.as_str(),
                task.args.iter().map(String::as_str).collect::<Vec<_>>(),
                task.default
            ))
            .collect::<Vec<_>>(),
        vec![
            ("Cargo Run Server", vec!["run", "--bin", "server"], false),
            ("Cargo Run Worker", vec!["run", "--bin", "worker"], false),
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_workspace_tasks_marks_cargo_default_run_bin() {
    let root = unique_test_dir("kuroya-cargo-default-run-task");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        concat!(
            "[package]\n",
            "name = \"demo\"\n",
            "default-run = \"worker\"\n",
            "\n",
            "[[bin]]\n",
            "name = \"server\"\n",
            "path = \"src/server.rs\"\n",
            "\n",
            "[[bin]]\n",
            "name = \"worker\"\n",
            "path = \"src/worker.rs\"\n",
        ),
    )
    .unwrap();

    let tasks = load_workspace_tasks(&root).unwrap();

    assert_eq!(
        tasks
            .iter()
            .filter(|task| task.kind == WorkspaceTaskKind::Run)
            .map(|task| (
                task.name.as_str(),
                task.args.iter().map(String::as_str).collect::<Vec<_>>(),
                task.default
            ))
            .collect::<Vec<_>>(),
        vec![
            ("Cargo Run Server", vec!["run", "--bin", "server"], false),
            ("Cargo Run Worker", vec!["run", "--bin", "worker"], true),
        ]
    );
    assert_eq!(
        workspace_task_default_index(&tasks, WorkspaceTaskKind::Run),
        Some(4)
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_workspace_tasks_infers_implicit_cargo_bin_run_configs() {
    let root = unique_test_dir("kuroya-cargo-implicit-bin-tasks");
    fs::create_dir_all(root.join("src/bin/tools")).unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo-app\"\n").unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(root.join("src/bin/server.rs"), "fn main() {}\n").unwrap();
    fs::write(root.join("src/bin/tools/main.rs"), "fn main() {}\n").unwrap();
    fs::write(root.join("src/bin/readme.txt"), "ignored\n").unwrap();

    let tasks = load_workspace_tasks(&root).unwrap();

    assert_eq!(
        tasks
            .iter()
            .filter(|task| task.kind == WorkspaceTaskKind::Run)
            .map(|task| (
                task.name.as_str(),
                task.args.iter().map(String::as_str).collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("Cargo Run Demo App", vec!["run", "--bin", "demo-app"]),
            ("Cargo Run Server", vec!["run", "--bin", "server"]),
            ("Cargo Run Tools", vec!["run", "--bin", "tools"]),
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn cargo_implicit_bin_names_ignore_non_directory_non_rs_entries() {
    let root = unique_test_dir("kuroya-cargo-implicit-bin-entry-types");
    fs::create_dir_all(root.join("src/bin/tools")).unwrap();
    fs::write(root.join("src/bin/server.rs"), "fn main() {}\n").unwrap();
    fs::write(root.join("src/bin/tools/main.rs"), "fn main() {}\n").unwrap();
    fs::write(root.join("src/bin/readme.txt"), "ignored\n").unwrap();
    fs::write(root.join("src/bin/helper"), "ignored\n").unwrap();

    let names = cargo_implicit_bin_names(&root, None);

    assert_eq!(
        names,
        BTreeSet::from(["server".to_owned(), "tools".to_owned()])
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn cargo_manifest_detection_ignores_non_file_manifest_candidate() {
    let root = unique_test_dir("kuroya-cargo-manifest-directory");
    fs::create_dir_all(root.join("Cargo.toml")).unwrap();

    assert_eq!(CargoManifestInfo::detect(&root), None);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_workspace_tasks_infers_virtual_cargo_workspace_without_run() {
    let root = unique_test_dir("kuroya-virtual-cargo-workspace-tasks");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/app\"]\n",
    )
    .unwrap();

    let tasks = load_workspace_tasks(&root).unwrap();

    assert_eq!(
        tasks
            .iter()
            .map(|task| (
                task.name.as_str(),
                task.kind,
                task.args.iter().map(String::as_str).collect::<Vec<_>>(),
                task.default
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "Cargo Check Workspace",
                WorkspaceTaskKind::Build,
                vec!["check", "--workspace"],
                true
            ),
            (
                "Cargo Clippy Workspace",
                WorkspaceTaskKind::Build,
                vec![
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--",
                    "-D",
                    "warnings"
                ],
                false
            ),
            (
                "Cargo Test Workspace",
                WorkspaceTaskKind::Test,
                vec!["test", "--workspace"],
                true
            ),
        ]
    );
    assert!(!tasks.iter().any(|task| task.kind == WorkspaceTaskKind::Run));
    assert_eq!(
        workspace_task_default_index(&tasks, WorkspaceTaskKind::Build),
        Some(0)
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_workspace_tasks_rejects_oversized_config_file() {
    let root = unique_test_dir("kuroya-oversized-tasks-config");
    let config_dir = root.join(".kuroya");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("tasks.toml"),
        vec![b'a'; usize::try_from(WORKSPACE_TASKS_FILE_MAX_BYTES + 1).unwrap()],
    )
    .unwrap();

    let error = format!("{:#}", load_workspace_tasks(&root).unwrap_err());

    assert!(error.contains("workspace task discovery limit"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_workspace_tasks_infers_package_json_scripts_when_config_is_missing() {
    let root = unique_test_dir("kuroya-package-tasks");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("pnpm-lock.yaml"), "").unwrap();
    fs::write(
        root.join("package.json"),
        r#"{
                "scripts": {
                    "build": "vite build",
                    "test": "vitest run",
                    "start": "node server.js",
                    "dev": "vite",
                    "lint": "eslint ."
                }
            }"#,
    )
    .unwrap();

    let tasks = load_workspace_tasks(&root).unwrap();

    assert_eq!(tasks.len(), 5);
    assert_eq!(
        tasks
            .iter()
            .map(|task| (
                task.name.as_str(),
                task.kind,
                task.command.as_str(),
                task.args.iter().map(String::as_str).collect::<Vec<_>>(),
                task.default
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "PNPM Build",
                WorkspaceTaskKind::Build,
                "pnpm",
                vec!["run", "build"],
                true
            ),
            (
                "PNPM Test",
                WorkspaceTaskKind::Test,
                "pnpm",
                vec!["run", "test"],
                true
            ),
            (
                "PNPM Start",
                WorkspaceTaskKind::Run,
                "pnpm",
                vec!["run", "start"],
                true
            ),
            (
                "PNPM Dev",
                WorkspaceTaskKind::Run,
                "pnpm",
                vec!["run", "dev"],
                false
            ),
            (
                "PNPM Lint",
                WorkspaceTaskKind::Custom,
                "pnpm",
                vec!["run", "lint"],
                false
            ),
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_workspace_tasks_uses_package_manager_field_when_lockfile_is_missing() {
    let root = unique_test_dir("kuroya-package-manager-field-tasks");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{
                "packageManager": "yarn@4.5.0",
                "scripts": {
                    "build": "vite build",
                    "test": "vitest run"
                }
            }"#,
    )
    .unwrap();

    let tasks = load_workspace_tasks(&root).unwrap();

    assert_eq!(
        tasks
            .iter()
            .map(|task| (
                task.name.as_str(),
                task.kind,
                task.command.as_str(),
                task.args.iter().map(String::as_str).collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "Yarn Build",
                WorkspaceTaskKind::Build,
                "yarn",
                vec!["run", "build"]
            ),
            (
                "Yarn Test",
                WorkspaceTaskKind::Test,
                "yarn",
                vec!["run", "test"]
            ),
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn inferred_package_tasks_include_custom_scripts_in_stable_order() {
    let root = unique_test_dir("kuroya-package-custom-tasks");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{
                "scripts": {
                    "type-check": "tsc --noEmit",
                    "dev": "vite",
                    "lint:fix": "eslint . --fix",
                    "": "ignored",
                    " lint ": "edge padding is ignored",
                    "lint\tunsafe": "control characters are ignored",
                    "invalid": { "not": "a string" }
                }
            }"#,
    )
    .unwrap();

    let tasks = load_workspace_tasks(&root).unwrap();

    assert_eq!(
        tasks
            .iter()
            .map(|task| (
                task.name.as_str(),
                task.kind,
                task.args.iter().map(String::as_str).collect::<Vec<_>>(),
                task.default
            ))
            .collect::<Vec<_>>(),
        vec![
            ("NPM Dev", WorkspaceTaskKind::Run, vec!["run", "dev"], true),
            (
                "NPM Lint Fix",
                WorkspaceTaskKind::Custom,
                vec!["run", "lint:fix"],
                false
            ),
            (
                "NPM Type Check",
                WorkspaceTaskKind::Custom,
                vec!["run", "type-check"],
                false
            ),
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn inferred_package_tasks_bound_dense_script_manifests() {
    let mut scripts = serde_json::Map::new();
    scripts.insert(
        "build".to_owned(),
        serde_json::Value::String("vite build".to_owned()),
    );
    for index in 0..WORKSPACE_TASKS_MAX_TASKS + 64 {
        scripts.insert(
            format!("script-{index:03}"),
            serde_json::Value::String("node task.js".to_owned()),
        );
    }
    let mut manifest = serde_json::Map::new();
    manifest.insert("scripts".to_owned(), serde_json::Value::Object(scripts));

    let tasks = package_manifest_tasks(
        Path::new("workspace"),
        &serde_json::Value::Object(manifest),
        PackageManager::Npm,
        None,
    );

    assert_eq!(tasks.len(), WORKSPACE_TASKS_MAX_TASKS);
    assert_eq!(tasks[0].name, "NPM Build");
    assert!(
        tasks
            .iter()
            .all(|task| task.args.first().map(String::as_str) == Some("run"))
    );
}

#[test]
fn inferred_package_tasks_classify_namespaced_lifecycle_scripts() {
    let root = unique_test_dir("kuroya-package-namespaced-lifecycle-tasks");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{
                "scripts": {
                    "build:web": "vite build",
                    "test:unit": "vitest run",
                    "dev:electron": "electron .",
                    "start:api": "node api.js",
                    "lint:fix": "eslint . --fix"
                }
            }"#,
    )
    .unwrap();

    let tasks = load_workspace_tasks(&root).unwrap();

    assert_eq!(
        tasks
            .iter()
            .map(|task| (
                task.name.as_str(),
                task.kind,
                task.args.iter().map(String::as_str).collect::<Vec<_>>(),
                task.default
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "NPM Build Web",
                WorkspaceTaskKind::Build,
                vec!["run", "build:web"],
                false
            ),
            (
                "NPM Dev Electron",
                WorkspaceTaskKind::Run,
                vec!["run", "dev:electron"],
                false
            ),
            (
                "NPM Lint Fix",
                WorkspaceTaskKind::Custom,
                vec!["run", "lint:fix"],
                false
            ),
            (
                "NPM Start Api",
                WorkspaceTaskKind::Run,
                vec!["run", "start:api"],
                false
            ),
            (
                "NPM Test Unit",
                WorkspaceTaskKind::Test,
                vec!["run", "test:unit"],
                false
            ),
        ]
    );
    assert_eq!(
        workspace_task_default_index(&tasks, WorkspaceTaskKind::Build),
        Some(0)
    );
    assert_eq!(
        workspace_task_default_index(&tasks, WorkspaceTaskKind::Test),
        Some(4)
    );
    assert_eq!(
        workspace_task_default_index(&tasks, WorkspaceTaskKind::Run),
        Some(1)
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn additional_package_script_kind_filters_and_classifies_once() {
    let script_names = BTreeSet::from([
        "build",
        "build:web",
        "dev:electron",
        "lint:fix",
        "postbuild:web",
        "prebuild",
        "predeploy",
        "start:api",
        "test:unit",
    ]);

    assert_eq!(
        infer_additional_package_script_kind("build", &script_names),
        None
    );
    assert_eq!(
        infer_additional_package_script_kind("prebuild", &script_names),
        None
    );
    assert_eq!(
        infer_additional_package_script_kind("postbuild:web", &script_names),
        None
    );
    assert_eq!(
        infer_additional_package_script_kind("predeploy", &script_names),
        Some(WorkspaceTaskKind::Custom)
    );
    assert_eq!(
        infer_additional_package_script_kind("build:web", &script_names),
        Some(WorkspaceTaskKind::Build)
    );
    assert_eq!(
        infer_additional_package_script_kind("test:unit", &script_names),
        Some(WorkspaceTaskKind::Test)
    );
    assert_eq!(
        infer_additional_package_script_kind("start:api", &script_names),
        Some(WorkspaceTaskKind::Run)
    );
    assert_eq!(
        infer_additional_package_script_kind("dev:electron", &script_names),
        Some(WorkspaceTaskKind::Run)
    );
    assert_eq!(
        infer_additional_package_script_kind("build:", &script_names),
        Some(WorkspaceTaskKind::Custom)
    );
    assert_eq!(
        infer_additional_package_script_kind("rebuild:web", &script_names),
        Some(WorkspaceTaskKind::Custom)
    );
    assert_eq!(
        infer_additional_package_script_kind("lint:fix", &script_names),
        Some(WorkspaceTaskKind::Custom)
    );
}

#[test]
fn inferred_package_tasks_skip_pre_and_post_hooks_for_present_scripts() {
    let root = unique_test_dir("kuroya-package-lifecycle-hook-tasks");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{
                "scripts": {
                    "build": "vite build",
                    "prebuild": "node before-build.js",
                    "postbuild": "node after-build.js",
                    "build:web": "vite build --mode web",
                    "prebuild:web": "node before-build-web.js",
                    "postbuild:web": "node after-build-web.js",
                    "lint": "eslint .",
                    "prelint": "node before-lint.js",
                    "postlint": "node after-lint.js",
                    "predeploy": "node standalone-predeploy.js"
                }
            }"#,
    )
    .unwrap();

    let tasks = load_workspace_tasks(&root).unwrap();

    assert_eq!(
        tasks
            .iter()
            .map(|task| (
                task.name.as_str(),
                task.kind,
                task.args.iter().map(String::as_str).collect::<Vec<_>>(),
                task.default
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "NPM Build",
                WorkspaceTaskKind::Build,
                vec!["run", "build"],
                true
            ),
            (
                "NPM Build Web",
                WorkspaceTaskKind::Build,
                vec!["run", "build:web"],
                false
            ),
            (
                "NPM Lint",
                WorkspaceTaskKind::Custom,
                vec!["run", "lint"],
                false
            ),
            (
                "NPM Predeploy",
                WorkspaceTaskKind::Custom,
                vec!["run", "predeploy"],
                false
            ),
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn inferred_package_tasks_include_package_workspace_scripts() {
    let root = unique_test_dir("kuroya-package-workspace-tasks");
    fs::create_dir_all(root.join("packages/app")).unwrap();
    fs::create_dir_all(root.join("packages/tools")).unwrap();
    fs::create_dir_all(root.join("packages/bad")).unwrap();
    fs::create_dir_all(root.join("node_modules/ignored")).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{
                "workspaces": { "packages": ["packages/*", "node_modules/*"] },
                "scripts": {
                    "build": "turbo build"
                }
            }"#,
    )
    .unwrap();
    fs::write(
        root.join("packages/app/package.json"),
        r#"{
                "name": "@demo/app",
                "scripts": {
                    "build": "vite build",
                    "lint:fix": "eslint . --fix"
                }
            }"#,
    )
    .unwrap();
    fs::write(
        root.join("packages/tools/package.json"),
        r#"{
                "scripts": {
                    "dev": "node index.js"
                }
            }"#,
    )
    .unwrap();
    fs::write(root.join("packages/bad/package.json"), "{ invalid").unwrap();
    fs::write(
        root.join("node_modules/ignored/package.json"),
        r#"{
                "scripts": {
                    "build": "should not be inferred"
                }
            }"#,
    )
    .unwrap();

    let tasks = load_workspace_tasks(&root).unwrap();

    assert_eq!(
        tasks
            .iter()
            .map(|task| (
                task.name.as_str(),
                task.kind,
                task.args.iter().map(String::as_str).collect::<Vec<_>>(),
                task_cwd_label(&root, task),
                task.default
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "NPM Build",
                WorkspaceTaskKind::Build,
                vec!["run", "build"],
                String::new(),
                true
            ),
            (
                "NPM Build (@demo/app)",
                WorkspaceTaskKind::Build,
                vec!["run", "build"],
                "packages/app".to_owned(),
                true
            ),
            (
                "NPM Lint Fix (@demo/app)",
                WorkspaceTaskKind::Custom,
                vec!["run", "lint:fix"],
                "packages/app".to_owned(),
                false
            ),
            (
                "NPM Dev (packages/tools)",
                WorkspaceTaskKind::Run,
                vec!["run", "dev"],
                "packages/tools".to_owned(),
                true
            ),
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn inferred_package_tasks_respect_workspace_negation_patterns() {
    let root = unique_test_dir("kuroya-package-workspace-negation-tasks");
    fs::create_dir_all(root.join("packages/app")).unwrap();
    fs::create_dir_all(root.join("packages/ignored")).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{
                "workspaces": ["packages/*", "!packages/ignored"]
            }"#,
    )
    .unwrap();
    fs::write(
        root.join("packages/app/package.json"),
        r#"{
                "scripts": {
                    "build": "vite build"
                }
            }"#,
    )
    .unwrap();
    fs::write(
        root.join("packages/ignored/package.json"),
        r#"{
                "scripts": {
                    "build": "should not be inferred"
                }
            }"#,
    )
    .unwrap();

    let tasks = load_workspace_tasks(&root).unwrap();

    assert_eq!(
        tasks
            .iter()
            .map(|task| (task.name.as_str(), task_cwd_label(&root, task)))
            .collect::<Vec<_>>(),
        vec![("NPM Build (packages/app)", "packages/app".to_owned())]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn package_workspace_patterns_borrow_clean_patterns() {
    let manifest = serde_json::json!({
        "workspaces": ["packages/*"]
    });

    let patterns = package_workspace_patterns(&manifest);

    assert_eq!(
        patterns
            .iter()
            .map(|pattern| pattern.as_ref())
            .collect::<Vec<_>>(),
        vec!["packages/*"]
    );
    match &patterns[0] {
        Cow::Borrowed(pattern) => assert_eq!(*pattern, "packages/*"),
        Cow::Owned(pattern) => panic!("clean workspace pattern was allocated: {pattern}"),
    }
}

#[test]
fn package_workspace_patterns_strip_hidden_format_controls() {
    let manifest = serde_json::json!({
        "workspaces": [
            "\u{202e}packages/*/\u{2069}",
            "\u{200b}"
        ]
    });

    let patterns = package_workspace_patterns(&manifest);

    assert_eq!(
        patterns
            .iter()
            .map(|pattern| pattern.as_ref())
            .collect::<Vec<_>>(),
        vec!["packages/*"]
    );
    match &patterns[0] {
        Cow::Owned(pattern) => assert_eq!(pattern, "packages/*"),
        Cow::Borrowed(pattern) => panic!("dirty workspace pattern was borrowed: {pattern}"),
    }
}

#[test]
fn inferred_package_workspace_tasks_skip_unsafe_scripts_and_sanitize_display() {
    let root = unique_test_dir("kuroya-unsafe-package-workspace-tasks");
    let package_root = root.join("packages/app");
    let script = "lint:fix".to_owned();
    let overlong_script = "x".repeat(WORKSPACE_TASK_ARG_MAX_CHARS + 1);
    fs::create_dir_all(&package_root).unwrap();

    let root_manifest = serde_json::json!({
        "workspaces": ["\u{202e}packages/*/\u{2069}"]
    });
    fs::write(root.join("package.json"), root_manifest.to_string()).unwrap();

    let mut scripts = serde_json::Map::new();
    scripts.insert(
        script.clone(),
        serde_json::Value::String("eslint . --fix".to_owned()),
    );
    scripts.insert(
        "lint\nunsafe".to_owned(),
        serde_json::Value::String("eslint .".to_owned()),
    );
    scripts.insert(
        "\u{202e}lint:unsafe\u{2069}".to_owned(),
        serde_json::Value::String("eslint .".to_owned()),
    );
    scripts.insert(
        overlong_script,
        serde_json::Value::String("node task.js".to_owned()),
    );
    let mut manifest = serde_json::Map::new();
    manifest.insert(
        "name".to_owned(),
        serde_json::Value::String("\u{2066}@demo/app\u{200b}\u{2069}".to_owned()),
    );
    manifest.insert("scripts".to_owned(), serde_json::Value::Object(scripts));
    fs::write(
        package_root.join("package.json"),
        serde_json::Value::Object(manifest).to_string(),
    )
    .unwrap();

    let tasks = load_workspace_tasks(&root).unwrap();

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].name, "NPM Lint Fix (@demo/app)");
    assert_eq!(tasks[0].args, vec!["run".to_owned(), script]);
    assert_eq!(task_cwd_label(&root, &tasks[0]), "packages/app");
    assert!(!tasks[0].name.chars().any(is_task_display_format_control));
    assert!(tasks[0].args.iter().all(|arg| {
        !arg.chars()
            .any(|ch| ch.is_control() || is_task_display_format_control(ch))
    }));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn inferred_package_run_task_defaults_to_dev_when_start_is_missing() {
    let root = unique_test_dir("kuroya-package-dev-task");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{
                "scripts": {
                    "dev": "vite",
                    "start": { "invalid": true }
                }
            }"#,
    )
    .unwrap();

    let tasks = load_workspace_tasks(&root).unwrap();

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].name, "NPM Dev");
    assert_eq!(tasks[0].kind, WorkspaceTaskKind::Run);
    assert!(tasks[0].default);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn inferred_package_tasks_sanitize_script_and_scope_display_text() {
    let root = PathBuf::from("workspace");
    let package_root = root.join("packages/app");
    let script = "lint fix".to_owned();
    let mut scripts = serde_json::Map::new();
    scripts.insert(
        script.clone(),
        serde_json::Value::String("eslint .".to_owned()),
    );
    let mut manifest = serde_json::Map::new();
    manifest.insert(
        "name".to_owned(),
        serde_json::Value::String("@demo\t\u{202e}app".to_owned()),
    );
    manifest.insert("scripts".to_owned(), serde_json::Value::Object(scripts));
    let manifest = serde_json::Value::Object(manifest);
    let scope = package_scope_label(&root, &package_root, &manifest);

    let tasks = package_manifest_tasks(&package_root, &manifest, PackageManager::Npm, Some(&scope));

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].name, "NPM Lint fix (@demo app)");
    assert_eq!(tasks[0].args, vec!["run".to_owned(), script]);
    assert!(!tasks[0].name.chars().any(|ch| ch.is_control()));
    assert!(!tasks[0].name.chars().any(is_task_display_format_control));
}

#[test]
fn inferred_package_tasks_bound_huge_script_display_without_rewriting_arg() {
    let root = PathBuf::from("workspace");
    let package_root = root.join("packages/app");
    let script = format!("lint-{}", "x".repeat(TASK_DISPLAY_LABEL_MAX_CHARS * 2));
    let mut scripts = serde_json::Map::new();
    scripts.insert(
        script.clone(),
        serde_json::Value::String("eslint .".to_owned()),
    );
    let mut manifest = serde_json::Map::new();
    manifest.insert("scripts".to_owned(), serde_json::Value::Object(scripts));
    let manifest = serde_json::Value::Object(manifest);

    let tasks = package_manifest_tasks(&package_root, &manifest, PackageManager::Npm, None);

    assert_eq!(tasks.len(), 1);
    assert!(!tasks[0].name.chars().any(|ch| ch.is_control()));
    assert!(tasks[0].name.chars().count() <= TASK_DISPLAY_LABEL_MAX_CHARS);
    assert!(tasks[0].name.ends_with(TASK_DISPLAY_TRUNCATION_MARKER));
    assert_eq!(tasks[0].args, vec!["run".to_owned(), script]);
}

#[test]
fn inferred_tasks_ignore_invalid_package_json() {
    let root = unique_test_dir("kuroya-invalid-package-tasks");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("package.json"), "{ invalid").unwrap();

    assert!(load_workspace_tasks(&root).unwrap().is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_task_command_preview_quotes_spaced_args() {
    let task = WorkspaceTask {
        name: "Run".to_owned(),
        command: "cargo".to_owned(),
        args: vec!["run".to_owned(), "--".to_owned(), "hello world".to_owned()],
        cwd: None,
        env: BTreeMap::new(),
        kind: WorkspaceTaskKind::Run,
        default: false,
    };

    assert_eq!(
        workspace_task_command_preview(&task),
        "cargo run -- \"hello world\""
    );
}

#[test]
fn workspace_task_command_preview_sanitizes_controls_and_bounds_output() {
    let task = WorkspaceTask {
        name: "Run".to_owned(),
        command: "cargo\n\u{202e}run".to_owned(),
        args: vec![
            "--message".to_owned(),
            format!(
                "hello\t\u{2066}{}",
                "x".repeat(TASK_COMMAND_PREVIEW_MAX_CHARS * 2)
            ),
        ],
        cwd: None,
        env: BTreeMap::new(),
        kind: WorkspaceTaskKind::Run,
        default: false,
    };

    let preview = workspace_task_command_preview(&task);

    assert!(!preview.chars().any(|ch| ch.is_control()));
    assert!(!preview.chars().any(is_task_display_format_control));
    assert!(preview.chars().count() <= TASK_COMMAND_PREVIEW_MAX_CHARS);
    assert!(preview.contains(TASK_DISPLAY_TRUNCATION_MARKER));
    assert!(preview.starts_with("\"cargo run\" --message \"hello "));
}

#[test]
fn workspace_task_command_preview_stops_after_display_budget() {
    let task = WorkspaceTask {
        name: "Run".to_owned(),
        command: "tool".to_owned(),
        args: (0..10_000).map(|index| format!("arg{index}")).collect(),
        cwd: None,
        env: BTreeMap::new(),
        kind: WorkspaceTaskKind::Run,
        default: false,
    };

    let preview = workspace_task_command_preview(&task);

    assert!(preview.chars().count() <= TASK_COMMAND_PREVIEW_MAX_CHARS);
    assert!(preview.ends_with(TASK_DISPLAY_TRUNCATION_MARKER));
    assert!(!preview.contains("arg9999"));
}

#[test]
fn workspace_task_default_index_prefers_explicit_default_for_kind() {
    let tasks = vec![
        WorkspaceTask {
            name: "Test Quick".to_owned(),
            command: "cargo".to_owned(),
            args: vec!["test".to_owned()],
            cwd: None,
            env: BTreeMap::new(),
            kind: WorkspaceTaskKind::Test,
            default: false,
        },
        WorkspaceTask {
            name: "Test All".to_owned(),
            command: "cargo".to_owned(),
            args: vec!["test".to_owned(), "--all".to_owned()],
            cwd: None,
            env: BTreeMap::new(),
            kind: WorkspaceTaskKind::Test,
            default: true,
        },
    ];

    assert_eq!(
        workspace_task_default_index(&tasks, WorkspaceTaskKind::Test),
        Some(1)
    );
    assert_eq!(
        workspace_task_default_index(&tasks, WorkspaceTaskKind::Build),
        None
    );
}

#[test]
fn workspace_task_default_index_falls_back_to_first_task_of_kind() {
    let tasks = vec![
        WorkspaceTask {
            name: "Build Debug".to_owned(),
            command: "cargo".to_owned(),
            args: vec!["build".to_owned()],
            cwd: None,
            env: BTreeMap::new(),
            kind: WorkspaceTaskKind::Build,
            default: false,
        },
        WorkspaceTask {
            name: "Build Release".to_owned(),
            command: "cargo".to_owned(),
            args: vec!["build".to_owned(), "--release".to_owned()],
            cwd: None,
            env: BTreeMap::new(),
            kind: WorkspaceTaskKind::Build,
            default: false,
        },
    ];

    assert_eq!(
        workspace_task_default_index(&tasks, WorkspaceTaskKind::Build),
        Some(0)
    );
}

fn unique_test_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{name}-{nanos}"))
}

fn task_cwd_label(root: &Path, task: &WorkspaceTask) -> String {
    task.cwd
        .as_deref()
        .and_then(|cwd| cwd.strip_prefix(root).ok())
        .map(|cwd| cwd.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn raw_workspace_task() -> RawWorkspaceTask {
    RawWorkspaceTask {
        name: "task".to_owned(),
        command: "cargo".to_owned(),
        args: Vec::new(),
        cwd: None,
        env: BTreeMap::new(),
        kind: WorkspaceTaskKind::Custom,
        default: false,
    }
}

fn assert_normalize_task_error(raw: RawWorkspaceTask, expected: &str) {
    let root = PathBuf::from("workspace");
    let error = normalize_task(&root, 3, raw).unwrap_err().to_string();
    assert!(error.contains(expected), "{error}");
}
