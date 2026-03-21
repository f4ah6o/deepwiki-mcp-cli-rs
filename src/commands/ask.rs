use anyhow::Result;
use serde_json::json;

use crate::mcp::{self, McpConfig, print_result};

/// `dwiki ask <repo> <question>`
///
/// Calls the MCP `ask_question` tool and prints the answer.
pub async fn run(config: &McpConfig, repo: &str, question: &str) -> Result<()> {
    let client_config = mcp::McpConfig {
        url: config.url.clone(),
        token: config.token.clone(),
        timeout_secs: config.timeout_secs,
        output_format: config.output_format.clone(),
    };

    let mut client = mcp::McpClient::new(client_config)?;
    client.initialize().await?;

    let arguments = json!({
        "repoName": repo,
        "question": question
    });

    let result = client.call_tool("ask_question", arguments).await?;
    print_result(&config.output_format, &result);

    Ok(())
}
