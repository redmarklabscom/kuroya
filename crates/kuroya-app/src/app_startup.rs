use crate::{KuroyaApp, app_startup_context::AppStartupContext};

impl KuroyaApp {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        let mut context = AppStartupContext::load(cc)?;
        let saved_session = context.saved_session.take();
        let mut app = Self::from_startup_context(context);
        if app.workspace_placeholder {
            let _ = app.save_app_state();
            return Ok(app);
        }
        if let Some(session) = saved_session {
            app.restore_session(session);
        } else {
            app.record_recent_project(app.workspace.root.clone());
        }
        let _ = app.save_app_state();
        app.spawn_index();
        app.spawn_git_scan();
        app.spawn_workspace_task_load();
        app.spawn_plugin_discovery();
        Ok(app)
    }
}
