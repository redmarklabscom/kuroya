use crate::{
    terminal::TerminalProcessSessionState,
    workspace_tasks_panel::{
        workspace_task_cancel_command, workspace_task_is_running, workspace_task_label,
        workspace_task_run_command,
    },
    workspace_tasks_runtime::{
        RunningWorkspaceTask, WorkspaceTaskCompletion, WorkspaceTaskKindRunPlan,
        close_running_workspace_task_sessions, prune_finished_workspace_task_records,
        prune_stale_workspace_task_records, workspace_task_completed_status,
        workspace_task_display_text, workspace_task_fingerprint, workspace_task_kind_run_plan,
        workspace_task_loading_status, workspace_task_missing_kind_status,
        workspace_task_not_running_status, workspace_task_started_status,
        workspace_tasks_loaded_status, workspace_tasks_loading_status,
        workspace_tasks_restricted_status,
    },
};
use kuroya_core::{Command, WorkspaceTask, WorkspaceTaskKind};
use std::{collections::BTreeMap, path::PathBuf};

#[test]
fn workspace_task_labels_include_kind_name_and_command() {
    let task = task();

    assert_eq!(
        workspace_task_label(&task, false),
        "test default  Test All  cargo test --all"
    );
    assert_eq!(
        workspace_task_label(&task, true),
        "test default running  Test All  cargo test --all"
    );
}

#[test]
fn workspace_task_statuses_report_lifecycle() {
    let task = task();

    assert_eq!(
        workspace_tasks_loaded_status(0),
        "No workspace tasks configured"
    );
    assert_eq!(workspace_tasks_loaded_status(1), "Loaded 1 workspace task");
    assert_eq!(workspace_tasks_loaded_status(2), "Loaded 2 workspace tasks");
    assert_eq!(
        workspace_tasks_restricted_status(),
        "Trust this workspace before loading tasks"
    );
    assert_eq!(workspace_tasks_loading_status(), "Loading workspace tasks");
    assert_eq!(
        workspace_task_loading_status(WorkspaceTaskKind::Build),
        "Loading build workspace task"
    );
    assert_eq!(
        workspace_task_started_status(&task),
        "Started task `Test All` in app: cargo test --all"
    );
    assert_eq!(
        workspace_task_not_running_status(&task),
        "Task `Test All` is not running"
    );
    assert_eq!(
        workspace_task_completed_status(&task, WorkspaceTaskCompletion::Finished),
        "Task `Test All` finished"
    );
    assert_eq!(
        workspace_task_completed_status(&task, WorkspaceTaskCompletion::Finished),
        "Task `Test All` finished"
    );
    assert_eq!(
        workspace_task_completed_status(&task, WorkspaceTaskCompletion::FailedExitCode(17)),
        "Task `Test All` failed with exit code 17"
    );
    assert_eq!(
        workspace_task_completed_status(&task, WorkspaceTaskCompletion::TerminalError),
        "Task `Test All` failed in terminal"
    );
}

#[test]
fn workspace_task_display_text_sanitizes_controls_bidi_and_blank_values() {
    assert_eq!(
        workspace_task_display_text("alpha\tbeta\u{2028}gamma\u{202e}", 80),
        "alpha beta gamma"
    );
    assert_eq!(workspace_task_display_text("\n\u{202e}\t", 80), ".");
    assert_eq!(
        workspace_task_display_text("abcdefghijklmnopqrstuvwxyz", 12),
        "abcd...vwxyz"
    );
}

#[test]
fn workspace_task_labels_and_statuses_bound_user_visible_task_text() {
    let mut task = task();
    task.name = format!("Task\n{}\u{202e}", "x".repeat(200));
    task.command = format!("cargo\n{}\u{202e}", "y".repeat(200));
    task.args = vec!["test\targ".to_owned()];
    task.cwd = Some(PathBuf::from(format!(
        "workspace/dir\n{}\u{202e}",
        "z".repeat(200)
    )));

    let panel_label = workspace_task_label(&task, true);
    let started = workspace_task_started_status(&task);
    let completed =
        workspace_task_completed_status(&task, WorkspaceTaskCompletion::FailedExitCode(17));

    for display in [&panel_label, &started, &completed] {
        assert!(!display.chars().any(char::is_control));
        assert!(!display.contains('\u{202e}'));
        assert!(display.contains("..."));
    }
}

#[test]
fn workspace_task_run_command_requires_trust_and_valid_row() {
    let tasks = vec![task()];

    assert_eq!(
        workspace_task_run_command(&tasks, 0, true, false),
        Some(Command::RunWorkspaceTaskSnapshot {
            index: 0,
            fingerprint: workspace_task_fingerprint(&tasks[0]),
        })
    );
    assert_eq!(workspace_task_run_command(&tasks, 1, true, false), None);
    assert_eq!(workspace_task_run_command(&tasks, 0, false, false), None);
    assert_eq!(workspace_task_run_command(&tasks, 0, true, true), None);

    let mut changed = tasks[0].clone();
    changed.args.push("--changed".to_owned());
    assert_ne!(
        workspace_task_fingerprint(&tasks[0]),
        workspace_task_fingerprint(&changed)
    );
}

#[test]
fn workspace_task_cancel_command_requires_trust_and_valid_row() {
    let tasks = vec![task()];
    let running_tasks = vec![RunningWorkspaceTask {
        task_index: 0,
        fingerprint: workspace_task_fingerprint(&tasks[0]),
        session_id: 7,
    }];
    let unrelated_running_tasks = vec![RunningWorkspaceTask {
        task_index: 0,
        fingerprint: 123,
        session_id: 8,
    }];

    assert_eq!(
        workspace_task_cancel_command(&tasks, 0, true, false, &running_tasks),
        Some(Command::CancelWorkspaceTaskSnapshot {
            index: 0,
            fingerprint: workspace_task_fingerprint(&tasks[0]),
        })
    );
    assert!(workspace_task_is_running(0, &tasks[0], &running_tasks));
    assert!(!workspace_task_is_running(
        0,
        &tasks[0],
        &unrelated_running_tasks
    ));
    assert_eq!(
        workspace_task_cancel_command(&tasks, 0, true, false, &unrelated_running_tasks),
        None
    );
    assert_eq!(
        workspace_task_cancel_command(&tasks, 1, true, false, &running_tasks),
        None
    );
    assert_eq!(
        workspace_task_cancel_command(&tasks, 0, false, false, &running_tasks),
        None
    );
    assert_eq!(
        workspace_task_cancel_command(&tasks, 0, true, true, &running_tasks),
        None
    );
}

#[test]
fn workspace_task_running_state_distinguishes_duplicate_rows() {
    let tasks = vec![task(), task()];
    assert_eq!(
        workspace_task_fingerprint(&tasks[0]),
        workspace_task_fingerprint(&tasks[1])
    );
    let running_tasks = vec![RunningWorkspaceTask {
        task_index: 0,
        fingerprint: workspace_task_fingerprint(&tasks[0]),
        session_id: 7,
    }];

    assert!(workspace_task_is_running(0, &tasks[0], &running_tasks));
    assert!(!workspace_task_is_running(1, &tasks[1], &running_tasks));
    assert_eq!(
        workspace_task_cancel_command(&tasks, 0, true, false, &running_tasks),
        Some(Command::CancelWorkspaceTaskSnapshot {
            index: 0,
            fingerprint: workspace_task_fingerprint(&tasks[0]),
        })
    );
    assert_eq!(
        workspace_task_cancel_command(&tasks, 1, true, false, &running_tasks),
        None
    );
}

#[test]
fn workspace_task_pruning_reports_successful_process_status() {
    let tasks = vec![task()];
    let mut running_tasks = vec![RunningWorkspaceTask {
        task_index: 0,
        fingerprint: workspace_task_fingerprint(&tasks[0]),
        session_id: 7,
    }];

    let status = prune_finished_workspace_task_records(&mut running_tasks, &tasks, |session_id| {
        assert_eq!(session_id, 7);
        Some(TerminalProcessSessionState::Exited(0))
    });

    assert!(running_tasks.is_empty());
    assert_eq!(status.as_deref(), Some("Task `Test All` finished"));
}

#[test]
fn workspace_task_pruning_reports_failed_process_status() {
    let tasks = vec![task()];
    let mut running_tasks = vec![RunningWorkspaceTask {
        task_index: 0,
        fingerprint: workspace_task_fingerprint(&tasks[0]),
        session_id: 7,
    }];

    let status = prune_finished_workspace_task_records(&mut running_tasks, &tasks, |session_id| {
        assert_eq!(session_id, 7);
        Some(TerminalProcessSessionState::Exited(17))
    });

    assert!(running_tasks.is_empty());
    assert_eq!(
        status.as_deref(),
        Some("Task `Test All` failed with exit code 17")
    );
}

#[test]
fn workspace_task_pruning_prefers_failed_process_status() {
    let tasks = vec![task(), build_task()];
    let mut running_tasks = vec![
        RunningWorkspaceTask {
            task_index: 0,
            fingerprint: workspace_task_fingerprint(&tasks[0]),
            session_id: 7,
        },
        RunningWorkspaceTask {
            task_index: 1,
            fingerprint: workspace_task_fingerprint(&tasks[1]),
            session_id: 8,
        },
    ];

    let status = prune_finished_workspace_task_records(&mut running_tasks, &tasks, |session_id| {
        match session_id {
            7 => Some(TerminalProcessSessionState::Exited(17)),
            8 => Some(TerminalProcessSessionState::Exited(0)),
            _ => panic!("unexpected task session id {session_id}"),
        }
    });

    assert!(running_tasks.is_empty());
    assert_eq!(
        status.as_deref(),
        Some("Task `Test All` failed with exit code 17")
    );
}

#[test]
fn workspace_task_pruning_reports_terminal_failure_status() {
    let tasks = vec![task()];
    let mut running_tasks = vec![RunningWorkspaceTask {
        task_index: 0,
        fingerprint: workspace_task_fingerprint(&tasks[0]),
        session_id: 7,
    }];

    let status = prune_finished_workspace_task_records(&mut running_tasks, &tasks, |session_id| {
        assert_eq!(session_id, 7);
        Some(TerminalProcessSessionState::TerminalError)
    });

    assert!(running_tasks.is_empty());
    assert_eq!(
        status.as_deref(),
        Some("Task `Test All` failed in terminal")
    );
}

#[test]
fn workspace_task_pruning_keeps_running_tasks() {
    let tasks = vec![task()];
    let mut running_tasks = vec![RunningWorkspaceTask {
        task_index: 0,
        fingerprint: workspace_task_fingerprint(&tasks[0]),
        session_id: 7,
    }];

    let status = prune_finished_workspace_task_records(&mut running_tasks, &tasks, |_| {
        Some(TerminalProcessSessionState::Running)
    });

    assert_eq!(running_tasks.len(), 1);
    assert_eq!(running_tasks[0].session_id, 7);
    assert_eq!(status, None);
}

#[test]
fn workspace_task_pruning_drops_duplicate_session_records_without_duplicate_terminal_queries() {
    let tasks = vec![task()];
    let current = RunningWorkspaceTask {
        task_index: 0,
        fingerprint: workspace_task_fingerprint(&tasks[0]),
        session_id: 7,
    };
    let mut running_tasks = vec![
        current,
        RunningWorkspaceTask {
            task_index: 0,
            fingerprint: workspace_task_fingerprint(&tasks[0]),
            session_id: 7,
        },
    ];
    let mut queried = Vec::new();

    let status = prune_finished_workspace_task_records(&mut running_tasks, &tasks, |session_id| {
        queried.push(session_id);
        Some(TerminalProcessSessionState::Running)
    });

    assert_eq!(queried, vec![7]);
    assert_eq!(running_tasks, vec![current]);
    assert_eq!(status, None);
}

#[test]
fn workspace_task_stale_pruning_keeps_current_fingerprint_snapshots() {
    let tasks = vec![task()];
    let mut changed_task = task();
    changed_task.args.push("--changed".to_owned());
    let current = RunningWorkspaceTask {
        task_index: 0,
        fingerprint: workspace_task_fingerprint(&tasks[0]),
        session_id: 7,
    };
    let mut running_tasks = vec![
        current,
        RunningWorkspaceTask {
            task_index: 0,
            fingerprint: workspace_task_fingerprint(&changed_task),
            session_id: 8,
        },
        RunningWorkspaceTask {
            task_index: 1,
            fingerprint: workspace_task_fingerprint(&tasks[0]),
            session_id: 9,
        },
    ];

    prune_stale_workspace_task_records(&mut running_tasks, &tasks);

    assert_eq!(
        running_tasks,
        vec![
            current,
            RunningWorkspaceTask {
                task_index: 0,
                fingerprint: workspace_task_fingerprint(&tasks[0]),
                session_id: 9,
            },
        ]
    );
}

#[test]
fn workspace_task_stale_pruning_remaps_reordered_task_snapshots() {
    let build = build_task();
    let test = task();
    let tasks = vec![test, build.clone()];
    let mut running_tasks = vec![RunningWorkspaceTask {
        task_index: 0,
        fingerprint: workspace_task_fingerprint(&build),
        session_id: 7,
    }];

    prune_stale_workspace_task_records(&mut running_tasks, &tasks);

    assert_eq!(
        running_tasks,
        vec![RunningWorkspaceTask {
            task_index: 1,
            fingerprint: workspace_task_fingerprint(&build),
            session_id: 7,
        }]
    );
    assert!(!workspace_task_is_running(0, &tasks[0], &running_tasks));
    assert!(workspace_task_is_running(1, &tasks[1], &running_tasks));
}

#[test]
fn workspace_task_stale_pruning_preserves_duplicate_row_identity() {
    let tasks = vec![task(), task()];
    let mut running_tasks = vec![RunningWorkspaceTask {
        task_index: 1,
        fingerprint: workspace_task_fingerprint(&tasks[1]),
        session_id: 7,
    }];

    prune_stale_workspace_task_records(&mut running_tasks, &tasks);

    assert_eq!(
        running_tasks,
        vec![RunningWorkspaceTask {
            task_index: 1,
            fingerprint: workspace_task_fingerprint(&tasks[1]),
            session_id: 7,
        }]
    );
}

#[test]
fn workspace_task_stale_pruning_prefers_current_session_identity_over_remap() {
    let build = build_task();
    let test = task();
    let tasks = vec![build.clone(), test.clone()];
    let mut running_tasks = vec![
        RunningWorkspaceTask {
            task_index: 9,
            fingerprint: workspace_task_fingerprint(&build),
            session_id: 7,
        },
        RunningWorkspaceTask {
            task_index: 1,
            fingerprint: workspace_task_fingerprint(&test),
            session_id: 7,
        },
    ];

    prune_stale_workspace_task_records(&mut running_tasks, &tasks);

    assert_eq!(
        running_tasks,
        vec![RunningWorkspaceTask {
            task_index: 1,
            fingerprint: workspace_task_fingerprint(&test),
            session_id: 7,
        }]
    );
}

#[test]
fn workspace_task_stale_pruning_keeps_one_remapped_record_per_session() {
    let build = build_task();
    let test = task();
    let tasks = vec![build.clone(), test.clone()];
    let mut running_tasks = vec![
        RunningWorkspaceTask {
            task_index: 9,
            fingerprint: workspace_task_fingerprint(&build),
            session_id: 7,
        },
        RunningWorkspaceTask {
            task_index: 8,
            fingerprint: workspace_task_fingerprint(&test),
            session_id: 7,
        },
    ];

    prune_stale_workspace_task_records(&mut running_tasks, &tasks);

    assert_eq!(
        running_tasks,
        vec![RunningWorkspaceTask {
            task_index: 0,
            fingerprint: workspace_task_fingerprint(&build),
            session_id: 7,
        }]
    );
}

#[test]
fn workspace_task_pruning_drops_stale_records_without_terminal_queries() {
    let tasks = vec![task()];
    let mut changed_task = task();
    changed_task.args.push("--changed".to_owned());
    let mut running_tasks = vec![
        RunningWorkspaceTask {
            task_index: 0,
            fingerprint: workspace_task_fingerprint(&changed_task),
            session_id: 7,
        },
        RunningWorkspaceTask {
            task_index: 1,
            fingerprint: workspace_task_fingerprint(&tasks[0]),
            session_id: 8,
        },
    ];

    let status = prune_finished_workspace_task_records(&mut running_tasks, &tasks, |_| {
        panic!("stale task records should not query terminal session state")
    });

    assert!(running_tasks.is_empty());
    assert_eq!(status, None);
}

#[test]
fn workspace_task_pruning_ignores_missing_or_changed_stopped_sessions() {
    let tasks = vec![task()];
    let mut missing_session = vec![RunningWorkspaceTask {
        task_index: 0,
        fingerprint: workspace_task_fingerprint(&tasks[0]),
        session_id: 7,
    }];

    let missing_status =
        prune_finished_workspace_task_records(&mut missing_session, &tasks, |_| None);

    assert!(missing_session.is_empty());
    assert_eq!(missing_status, None);

    let mut changed_task = vec![RunningWorkspaceTask {
        task_index: 0,
        fingerprint: 123,
        session_id: 8,
    }];

    let changed_status = prune_finished_workspace_task_records(&mut changed_task, &tasks, |_| {
        Some(TerminalProcessSessionState::Exited(0))
    });

    assert!(changed_task.is_empty());
    assert_eq!(changed_status, None);
}

#[test]
fn workspace_task_restricted_cleanup_closes_recorded_sessions_and_clears_records() {
    let task = task();
    let fingerprint = workspace_task_fingerprint(&task);
    let mut running_tasks = vec![
        RunningWorkspaceTask {
            task_index: 0,
            fingerprint,
            session_id: 7,
        },
        RunningWorkspaceTask {
            task_index: 0,
            fingerprint,
            session_id: 8,
        },
        RunningWorkspaceTask {
            task_index: 0,
            fingerprint,
            session_id: 9,
        },
    ];
    let mut requested = Vec::new();

    let closed = close_running_workspace_task_sessions(&mut running_tasks, |session_id| {
        requested.push(session_id);
        session_id != 8
    });

    assert_eq!(requested, vec![7, 8, 9]);
    assert_eq!(closed, 2);
    assert!(running_tasks.is_empty());
}

#[test]
fn workspace_task_restricted_cleanup_closes_duplicate_session_ids_once() {
    let task = task();
    let fingerprint = workspace_task_fingerprint(&task);
    let mut running_tasks = vec![
        RunningWorkspaceTask {
            task_index: 0,
            fingerprint,
            session_id: 7,
        },
        RunningWorkspaceTask {
            task_index: 0,
            fingerprint,
            session_id: 7,
        },
        RunningWorkspaceTask {
            task_index: 0,
            fingerprint,
            session_id: 8,
        },
    ];
    let mut requested = Vec::new();

    let closed = close_running_workspace_task_sessions(&mut running_tasks, |session_id| {
        requested.push(session_id);
        true
    });

    assert_eq!(requested, vec![7, 8]);
    assert_eq!(closed, 2);
    assert!(running_tasks.is_empty());
}

fn task() -> WorkspaceTask {
    WorkspaceTask {
        name: "Test All".to_owned(),
        command: "cargo".to_owned(),
        args: vec!["test".to_owned(), "--all".to_owned()],
        cwd: Some(PathBuf::from("crates/app")),
        env: BTreeMap::new(),
        kind: WorkspaceTaskKind::Test,
        default: true,
    }
}

fn build_task() -> WorkspaceTask {
    WorkspaceTask {
        name: "Build".to_owned(),
        command: "cargo".to_owned(),
        args: vec!["build".to_owned()],
        cwd: Some(PathBuf::from("crates/app")),
        env: BTreeMap::new(),
        kind: WorkspaceTaskKind::Build,
        default: true,
    }
}

#[test]
fn workspace_task_missing_kind_status_names_requested_kind() {
    assert_eq!(
        workspace_task_missing_kind_status(WorkspaceTaskKind::Build),
        "No build workspace task configured"
    );
    assert_eq!(
        workspace_task_missing_kind_status(WorkspaceTaskKind::Run),
        "No run configuration configured"
    );
}

#[test]
fn workspace_task_kind_run_plan_waits_for_loading_tasks_before_missing() {
    assert_eq!(
        workspace_task_kind_run_plan(&[], WorkspaceTaskKind::Build, false, false),
        WorkspaceTaskKindRunPlan::Load
    );
    assert_eq!(
        workspace_task_kind_run_plan(&[], WorkspaceTaskKind::Build, false, true),
        WorkspaceTaskKindRunPlan::Load
    );
    assert_eq!(
        workspace_task_kind_run_plan(&[], WorkspaceTaskKind::Build, true, false),
        WorkspaceTaskKindRunPlan::Missing
    );

    let tasks = vec![task()];
    assert_eq!(
        workspace_task_kind_run_plan(&tasks, WorkspaceTaskKind::Test, true, false),
        WorkspaceTaskKindRunPlan::Run(0)
    );
    assert_eq!(
        workspace_task_kind_run_plan(&tasks, WorkspaceTaskKind::Test, true, true),
        WorkspaceTaskKindRunPlan::Load
    );
    assert_eq!(
        workspace_task_kind_run_plan(&tasks, WorkspaceTaskKind::Build, true, false),
        WorkspaceTaskKindRunPlan::Missing
    );
}
