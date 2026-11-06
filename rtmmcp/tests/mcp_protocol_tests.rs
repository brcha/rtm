//! The server over the MCP protocol, through rmcp's client on an in-memory pipe.

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, ContentBlock};
use rtmmcp::{Source, Sources};

/// A todo.txt in a fresh scratch directory under the system temp directory.
fn scratch_file() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmmcp-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let file = dir.join("todo.txt");
    std::fs::write(&file, "Buy milk\n").unwrap();
    file
}

#[tokio::test]
async fn test_initialize_list_and_call_a_tool() {
    let file = scratch_file();
    let sources: Sources = {
        let file = file.clone();
        Arc::new(move || {
            Ok(vec![Source {
                path: file.clone(),
                name: Some("home".to_owned()),
                sync: false,
                vcs: None,
            }])
        })
    };
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let (read, write) = tokio::io::split(server_io);
    let server = tokio::spawn(rtmmcp::serve_on(sources, read, write));
    let client = ().serve(client_io).await.unwrap();

    let info = client.peer_info().unwrap();
    assert_eq!(info.server_info.as_ref().unwrap().name, "rtm");
    let tools = client.list_all_tools().await.unwrap();
    let list_files = tools.iter().find(|tool| tool.name == "list_files").unwrap();
    assert_eq!(
        list_files.annotations.as_ref().unwrap().read_only_hint,
        Some(true)
    );

    let result = client
        .call_tool(CallToolRequestParams::new("list_files"))
        .await
        .unwrap();
    assert_ne!(result.is_error, Some(true));
    let ContentBlock::Text(text) = &result.content[0] else {
        panic!("not text: {:?}", result.content);
    };
    assert_eq!(
        text.text,
        format!("home: {} (not synced, default)", file.display())
    );

    client.cancel().await.unwrap();
    server.await.unwrap().unwrap();
    std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
}
