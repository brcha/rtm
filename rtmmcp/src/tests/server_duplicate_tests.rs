use super::*;
use rmcp::ServiceExt;
use std::sync::Arc;

/// A client connected over an in-memory pipe to `server`.
async fn connect(server: Server) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let (read, write) = tokio::io::split(server_io);
    tokio::spawn(crate::run(server, read, write));
    ().serve(client_io).await.unwrap()
}

#[tokio::test]
async fn test_a_second_server_offers_no_tools() {
    let sources: Sources = Arc::new(|| Ok(Vec::new()));
    let client = connect(Server::new(sources, false)).await;
    assert!(client.list_all_tools().await.unwrap().is_empty());
    let info = client.peer_info().unwrap();
    assert_eq!(info.server_info.as_ref().unwrap().name, "rtm");
    assert!(
        info.instructions
            .as_deref()
            .unwrap()
            .contains("Another rtm server")
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn test_the_first_server_offers_its_tools() {
    let sources: Sources = Arc::new(|| Ok(Vec::new()));
    let client = connect(Server::new(sources, true)).await;
    let tools = client.list_all_tools().await.unwrap();
    assert!(tools.iter().any(|tool| tool.name == "list_files"));
    client.cancel().await.unwrap();
}
