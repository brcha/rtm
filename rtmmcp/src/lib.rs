//! The MCP server of rtm: lets MCP clients (Claude Desktop, Claude Code and others) read and
//! change todo.txt files over stdio. `rtmcli mcp` and `rtmapp mcp` both run it.
//!
//! The server is async (rmcp runs on tokio), but its entry point, `serve`, is synchronous: it
//! runs a current-thread runtime of its own until the client closes stdin. Nothing but the
//! protocol may reach stdout.

mod claim;
mod server;

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::ServiceExt;
use tokio::io::{AsyncRead, AsyncWrite};

use server::Server;

/// A todo.txt the server may read and change. The client never names a path of its own: it picks
/// one of these, by name or path.
#[derive(Clone, Debug)]
pub struct Source {
    pub path: PathBuf,
    /// The name shown to the client; the file name when none.
    pub name: Option<String>,
    /// Whether changes are committed and synced through the file's repository.
    pub sync: bool,
    /// The VCS to sync through when the file's directory has both Git and Pijul.
    pub vcs: Option<rtmsync::Vcs>,
}

impl Source {
    /// The name shown to the client.
    pub fn name(&self) -> String {
        self.name.clone().unwrap_or_else(|| {
            self.path
                .file_name()
                .unwrap_or(self.path.as_os_str())
                .to_string_lossy()
                .into_owned()
        })
    }
}

/// The files the server may use, asked for again at the start of every tool call, so a frontend
/// can pick up changes to its configuration. The first is the default. An `Err` is shown to the
/// client as the tool's error.
pub type Sources = Arc<dyn Fn() -> Result<Vec<Source>, String> + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not start the runtime: {0}")]
    Runtime(#[source] std::io::Error),
    #[error("the MCP session did not start: {0}")]
    Initialize(String),
    #[error("the MCP session failed: {0}")]
    Session(String),
}

/// Serves `sources` over stdin and stdout until the client closes stdin.
///
/// When another rtm server already runs for the same client (the same parent process), this one
/// offers no tools and tells the client so in its instructions, so the assistant never sees two
/// sets of the same tools.
pub fn serve(sources: Sources) -> Result<(), Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(Error::Runtime)?;
    let claim = claim::claim_client();
    let (stdin, stdout) = rmcp::transport::stdio();
    let result = runtime.block_on(run(Server::new(sources, claim.first), stdin, stdout));
    // Released before the runtime is dropped, which waits for blocking work still running (a
    // pull can take a minute): a restarted server must not find the claim held meanwhile.
    drop(claim);
    result
}

/// Serves `sources` over `read` and `write` until `read` ends, without the one-server-per-client
/// check. For tests, and for callers with a transport of their own; needs a tokio runtime.
pub async fn serve_on<R, W>(sources: Sources, read: R, write: W) -> Result<(), Error>
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    run(Server::new(sources, true), read, write).await
}

async fn run<R, W>(server: Server, read: R, write: W) -> Result<(), Error>
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    let service = server
        .serve((read, write))
        .await
        .map_err(|e| Error::Initialize(e.to_string()))?;
    service
        .waiting()
        .await
        .map_err(|e| Error::Session(e.to_string()))?;
    Ok(())
}
