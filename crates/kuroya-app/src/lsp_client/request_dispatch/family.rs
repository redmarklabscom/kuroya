use super::super::commands::LspClientCommand;

pub(super) enum RequestCommandFamily {
    Navigation,
    Symbols,
    Edits,
}

pub(super) fn request_command_family(command: &LspClientCommand) -> Option<RequestCommandFamily> {
    match command {
        LspClientCommand::Hover { .. }
        | LspClientCommand::DocumentHighlights { .. }
        | LspClientCommand::Definition { .. }
        | LspClientCommand::PrepareCallHierarchy { .. }
        | LspClientCommand::CallHierarchyIncoming { .. }
        | LspClientCommand::CallHierarchyOutgoing { .. }
        | LspClientCommand::PrepareTypeHierarchy { .. }
        | LspClientCommand::TypeHierarchySupertypes { .. }
        | LspClientCommand::TypeHierarchySubtypes { .. }
        | LspClientCommand::References { .. }
        | LspClientCommand::Rename { .. } => Some(RequestCommandFamily::Navigation),
        LspClientCommand::DocumentSymbols { .. }
        | LspClientCommand::FoldingRanges { .. }
        | LspClientCommand::InlayHints { .. }
        | LspClientCommand::CodeLenses { .. }
        | LspClientCommand::ResolveCodeLens { .. }
        | LspClientCommand::ExecuteCommand { .. }
        | LspClientCommand::SemanticTokens { .. }
        | LspClientCommand::WorkspaceSymbols { .. } => Some(RequestCommandFamily::Symbols),
        LspClientCommand::Completion { .. }
        | LspClientCommand::ResolveCompletionItem { .. }
        | LspClientCommand::SignatureHelp { .. }
        | LspClientCommand::Formatting { .. }
        | LspClientCommand::CodeActions { .. }
        | LspClientCommand::ResolveCodeAction { .. } => Some(RequestCommandFamily::Edits),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::request_command_family;
    use crate::lsp_client::commands::LspClientCommand;

    #[test]
    fn request_command_family_ignores_non_request_commands() {
        assert!(request_command_family(&LspClientCommand::Shutdown).is_none());
    }
}
