use crate::lsp_client::commands::LspClientCommand;

pub(super) enum EditRequestFamily {
    Position,
    Actions,
}

pub(super) fn edit_request_family(command: &LspClientCommand) -> Option<EditRequestFamily> {
    match command {
        LspClientCommand::Completion { .. }
        | LspClientCommand::ResolveCompletionItem { .. }
        | LspClientCommand::SignatureHelp { .. } => Some(EditRequestFamily::Position),
        LspClientCommand::Formatting { .. }
        | LspClientCommand::CodeActions { .. }
        | LspClientCommand::ResolveCodeAction { .. } => Some(EditRequestFamily::Actions),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::edit_request_family;
    use crate::lsp_client::commands::LspClientCommand;

    #[test]
    fn edit_request_routing_ignores_non_edit_commands() {
        assert!(edit_request_family(&LspClientCommand::Shutdown).is_none());
    }
}
