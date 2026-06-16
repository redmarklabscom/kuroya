use crate::save_lifecycle::{LspSaveSyncPlan, plan_lsp_save_sync};

#[test]
fn lsp_save_sync_plan_sends_save_for_clean_saved_buffers() {
    assert_eq!(
        plan_lsp_save_sync(false, false, false),
        LspSaveSyncPlan {
            save: true,
            ..LspSaveSyncPlan::default()
        }
    );
    assert_eq!(
        plan_lsp_save_sync(false, true, false),
        LspSaveSyncPlan {
            change: true,
            save: true,
            ..LspSaveSyncPlan::default()
        }
    );
}

#[test]
fn lsp_save_sync_plan_reschedules_dirty_newer_edits_without_false_save() {
    assert_eq!(
        plan_lsp_save_sync(false, true, true),
        LspSaveSyncPlan {
            reschedule: true,
            ..LspSaveSyncPlan::default()
        }
    );
    assert_eq!(
        plan_lsp_save_sync(true, true, true),
        LspSaveSyncPlan {
            open: true,
            ..LspSaveSyncPlan::default()
        }
    );
}

#[test]
fn lsp_save_sync_plan_opens_renamed_or_save_as_paths_before_save() {
    assert_eq!(
        plan_lsp_save_sync(true, false, false),
        LspSaveSyncPlan {
            open: true,
            save: true,
            ..LspSaveSyncPlan::default()
        }
    );
}
