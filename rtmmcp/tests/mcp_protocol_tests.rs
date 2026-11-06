//! The server over the MCP protocol, through rmcp's client on an in-memory pipe.

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, ContentBlock};
use rtmmcp::{Source, Sources};
use serde_json::{Value, json};

/// A todo.txt in a fresh scratch directory under the system temp directory.
fn scratch_file() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmmcp-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let file = dir.join("todo.txt");
    std::fs::write(&file, "Buy milk\n").unwrap();
    file
}

fn object(value: Value) -> serde_json::Map<String, Value> {
    value.as_object().unwrap().clone()
}

/// Calls tool `name` with `arguments` and returns its text, which must not be an error.
async fn call(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    name: &'static str,
    arguments: Value,
) -> String {
    let result = client
        .call_tool(CallToolRequestParams::new(name).with_arguments(object(arguments)))
        .await
        .unwrap();
    assert_ne!(result.is_error, Some(true), "{result:?}");
    let ContentBlock::Text(text) = &result.content[0] else {
        panic!("not text: {:?}", result.content);
    };
    text.text.clone()
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

    let hints = |name: &str| {
        let tool = tools.iter().find(|tool| tool.name == name).unwrap();
        tool.annotations.as_ref().unwrap().destructive_hint
    };
    assert_eq!(hints("add_item"), Some(false));
    assert_eq!(hints("edit_item"), Some(false));
    assert_eq!(hints("delete_item"), Some(true));

    let added = call(
        &client,
        "add_item",
        json!({"line": "2026-01-01 Pay rent +home"}),
    )
    .await;
    assert_eq!(added, "Added 2 2026-01-01 Pay rent +home");
    let listed = call(&client, "list_items", json!({"projects": ["home"]})).await;
    assert_eq!(listed, "2 2026-01-01 Pay rent +home");
    let refused = client
        .call_tool(
            CallToolRequestParams::new("delete_item")
                .with_arguments(object(json!({"number": 2, "text": "Pay the rent"}))),
        )
        .await
        .unwrap();
    assert_eq!(refused.is_error, Some(true));
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 2);

    client.cancel().await.unwrap();
    server.await.unwrap().unwrap();
    std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
}
