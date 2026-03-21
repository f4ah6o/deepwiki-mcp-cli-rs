use anyhow::Result;
use serde_json::json;

use crate::mcp::{self, McpConfig, print_result};

/// `dwiki read <repo> [topic]`
///
/// - With no topic: calls `read_wiki_structure` to list available topics.
/// - With a topic:  calls `read_wiki_contents` to display that topic's documentation.
pub async fn run(config: &McpConfig, repo: &str, topic: Option<&str>) -> Result<()> {
    let client_config = mcp::McpConfig {
        url: config.url.clone(),
        token: config.token.clone(),
        timeout_secs: config.timeout_secs,
        output_format: config.output_format.clone(),
    };

    let mut client = mcp::McpClient::new(client_config)?;
    client.initialize().await?;

    let result = if let Some(t) = topic {
        let arguments = json!({
            "repoName": repo,
            "topic": t
        });
        client.call_tool("read_wiki_contents", arguments).await?
    } else {
        let arguments = json!({
            "repoName": repo
        });
        client.call_tool("read_wiki_structure", arguments).await?
    };

    print_result(&config.output_format, &result);

    Ok(())
}
