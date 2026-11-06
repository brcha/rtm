//! The MCP server and its tools.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::tools::{self, Due, Filter, Status};
use crate::{Source, Sources};

/// What the assistant is told about the server.
const INSTRUCTIONS: &str = "Reads and changes the user's todo.txt files (todo.txt format: \
    priority (A), +project, @context, due:YYYY-MM-DD, rec:1w for recurrence). Only the files \
    listed by list_files can be used; the first is the default. Items are numbered by their place \
    in the file; to change one, pass its number and its line exactly as list_items showed them. \
    Every change is saved at once, and committed and synced when its file syncs (list_files \
    shows which do).";

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

    /// Runs `work` on the allowed file `file` (the default when none) on a blocking thread, as it
    /// reads files and runs git.
    async fn on_file(
        &self,
        file: Option<String>,
        work: impl FnOnce(&Source) -> Result<String, String> + Send + 'static,
    ) -> Result<CallToolResult, ErrorData> {
        let sources = self.sources.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            let source = tools::pick(sources()?, file.as_deref())?;
            work(&source)
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
        Ok(match outcome {
            Ok(result) => text(result),
            Err(e) => failure(e),
        })
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

/// Which file a tool works on.
#[derive(Deserialize, JsonSchema)]
struct FileArg {
    /// The file's name or path, as list_files shows it; the default file when left out.
    file: Option<String>,
}

#[derive(Deserialize, JsonSchema, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum StatusArg {
    /// Items not completed.
    #[default]
    Open,
    /// Completed items.
    Done,
    /// Both.
    All,
}

#[derive(Deserialize, JsonSchema, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum DueArg {
    /// Any due date, or none.
    #[default]
    All,
    /// Due today.
    Today,
    /// Due within the next 7 days, today included.
    Week,
    /// Due before today.
    Overdue,
    /// Due after today.
    Future,
}

#[derive(Deserialize, JsonSchema)]
struct ListArgs {
    /// The file's name or path, as list_files shows it; the default file when left out.
    file: Option<String>,
    /// By completion; open items when left out.
    status: Option<StatusArg>,
    /// By due date; any when left out.
    due: Option<DueArg>,
    /// Only items carrying every one of these projects (with or without the +).
    projects: Option<Vec<String>>,
    /// Only items carrying every one of these contexts (with or without the @).
    contexts: Option<Vec<String>>,
    /// Only items whose line contains this text, ignoring case.
    text: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct AddArgs {
    /// The file's name or path, as list_files shows it; the default file when left out.
    file: Option<String>,
    /// The new item as one todo.txt line, for example "(B) Call the bank +money @phone
    /// due:2026-12-01". Today's date is added as its creation date when it has none.
    line: String,
}

#[derive(Deserialize, JsonSchema)]
struct ItemArgs {
    /// The file's name or path, as list_files shows it; the default file when left out.
    file: Option<String>,
    /// The item's number, as list_items showed it.
    number: usize,
    /// The item's line, exactly as list_items showed it after the number.
    text: String,
}

#[derive(Deserialize, JsonSchema)]
struct EditArgs {
    /// The file's name or path, as list_files shows it; the default file when left out.
    file: Option<String>,
    /// The item's number, as list_items showed it.
    number: usize,
    /// The item's line, exactly as list_items showed it after the number.
    text: String,
    /// The whole new todo.txt line for the item (priority, due date, projects and so on all go
    /// through it). Its creation date is kept when the new line has none.
    new_line: String,
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

    #[tool(
        description = "Lists the items of a todo.txt file, one per line: the item's number and \
            its todo.txt line. Open items by default; filter by status, due date, projects, \
            contexts and text.",
        annotations(read_only_hint = true)
    )]
    async fn list_items(
        &self,
        Parameters(args): Parameters<ListArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let filter = Filter {
            status: match args.status.unwrap_or_default() {
                StatusArg::Open => Status::Open,
                StatusArg::Done => Status::Done,
                StatusArg::All => Status::All,
            },
            due: match args.due.unwrap_or_default() {
                DueArg::All => Due::All,
                DueArg::Today => Due::Today,
                DueArg::Week => Due::Week,
                DueArg::Overdue => Due::Overdue,
                DueArg::Future => Due::Future,
            },
            projects: args.projects.unwrap_or_default(),
            contexts: args.contexts.unwrap_or_default(),
            text: args.text,
        };
        self.on_file(args.file, move |source| tools::list_items(source, &filter))
            .await
    }

    #[tool(
        description = "Lists every +project and @context in a todo.txt file, with how many open \
            items carry each.",
        annotations(read_only_hint = true)
    )]
    async fn list_projects_and_contexts(
        &self,
        Parameters(args): Parameters<FileArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.on_file(args.file, tools::list_projects_and_contexts)
            .await
    }

    #[tool(
        description = "Adds an item, given as one todo.txt line. Returns its number and line.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    async fn add_item(
        &self,
        Parameters(args): Parameters<AddArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        self.on_file(args.file, move |source| tools::add_item(source, &args.line))
            .await
    }

    #[tool(
        description = "Marks an item done, with today's completion date. A recurring item (rec:) \
            gets its next occurrence added, which the result shows.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    async fn complete_item(
        &self,
        Parameters(args): Parameters<ItemArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        self.on_file(args.file, move |source| {
            tools::complete_item(source, args.number, &args.text)
        })
        .await
    }

    #[tool(
        description = "Marks a completed item open again.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    async fn uncomplete_item(
        &self,
        Parameters(args): Parameters<ItemArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        self.on_file(args.file, move |source| {
            tools::uncomplete_item(source, args.number, &args.text)
        })
        .await
    }

    #[tool(
        description = "Replaces an item's line with a new todo.txt line: for its text, priority, \
            due date, projects, contexts or recurrence.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    async fn edit_item(
        &self,
        Parameters(args): Parameters<EditArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        self.on_file(args.file, move |source| {
            tools::edit_item(source, args.number, &args.text, &args.new_line)
        })
        .await
    }

    #[tool(
        description = "Deletes an item and its sub-items. Returns every line it removed. Ask the \
            user before deleting; completing an item is usually what they want.",
        annotations(read_only_hint = false, destructive_hint = true)
    )]
    async fn delete_item(
        &self,
        Parameters(args): Parameters<ItemArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        self.on_file(args.file, move |source| {
            tools::delete_item(source, args.number, &args.text)
        })
        .await
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
