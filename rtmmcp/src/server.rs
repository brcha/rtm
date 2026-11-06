//! The MCP server and its tools.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};

use crate::{Source, Sources};

/// What the assistant is told about the server.
const INSTRUCTIONS: &str = "Reads and changes the user's todo.txt files (todo.txt format: \
    priority (A), +project, @context, due:YYYY-MM-DD). Only the files listed by list_files can be \
    used; the first is the default.";

/// What the assistant is told by a second rtm server of the same client.
const INACTIVE: &str = "Another rtm server is already running for this client; use that one. \
    This one offers no tools.";

#[derive(Clone)]
pub(crate) struct Server {
    sources: Sources,
    /// False for a second rtm server of the same client: it offers no tools.
    active: bool,
    router: ToolRouter<Server>,
}

impl Server {
    pub(crate) fn new(sources: Sources, active: bool) -> Self {
        let router = if active {
            Self::tool_router()
        } else {
            ToolRouter::new()
        };
        Server {
            sources,
            active,
            router,
        }
    }

    /// The allowed files, read on a blocking thread (a frontend may read its configuration).
    async fn sources(&self) -> Result<Vec<Source>, String> {
        let sources = self.sources.clone();
        tokio::task::spawn_blocking(move || sources())
            .await
            .map_err(|e| e.to_string())?
    }
}

/// A tool's text result.
fn text(text: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(text)])
}

/// A tool's failure: a result with `isError`, which the assistant reads, not a protocol error.
fn failure(text: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(text)])
}

#[tool_router]
impl Server {
    #[tool(
        description = "Lists the todo.txt files this server can use, one per line: the name to \
            pass as `file` to the other tools, its path, and whether changes are synced through \
            its repository. The first is the default.",
        annotations(read_only_hint = true)
    )]
    async fn list_files(&self) -> Result<CallToolResult, ErrorData> {
        Ok(match self.sources().await {
            Ok(sources) if sources.is_empty() => failure("No todo.txt files are configured."),
            Ok(sources) => text(
                sources
                    .iter()
                    .enumerate()
                    .map(|(i, source)| {
                        format!(
                            "{}: {} ({}{})",
                            source.name(),
                            source.path.display(),
                            if source.sync { "synced" } else { "not synced" },
                            if i == 0 { ", default" } else { "" }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            Err(e) => failure(e),
        })
    }
}

#[tool_handler(router = self.router)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("rtm", env!("CARGO_PKG_VERSION")))
            .with_instructions(if self.active { INSTRUCTIONS } else { INACTIVE })
    }
}

#[cfg(test)]
#[path = "tests/server_duplicate_tests.rs"]
mod duplicate_tests;
