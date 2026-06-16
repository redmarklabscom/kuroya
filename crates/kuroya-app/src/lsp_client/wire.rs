mod eligibility;
mod framing;

pub use eligibility::can_use_server_for_path;
pub(super) use framing::{LspMessageReadBuffer, lsp_version, read_message, write_message};
