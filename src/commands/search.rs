use anyhow::Result;
use serde_json::json;

use crate::mcp::{self, McpConfig, print_result};

/// `dwiki search <repo> <query>`
///
/// Searches within a repository's documentation by phrasing the query as
/// a search-oriented question and calling the `ask_question` MCP tool.
pub async fn run(config: &McpConfig, repo: &str, query: &str) -> Result<()> {
    let client_config = mcp::McpConfig {
        url: config.url.clone(),
        token: config.token.clone(),
        timeout_secs: config.timeout_secs,
        output_format: config.output_format.clone(),
    };

    let mut client = mcp::McpClient::new(client_config)?;
    client.initialize().await?;

    // Phrase as a search question to leverage the AI-powered ask_question tool.
    let search_question = format!(
        "Search for information about: {query}. \
         List all relevant topics, functions, modules, and documentation sections that match this query."
    );

    let arguments = json!({
        "repoName": repo,
        "question": search_question
    });

    let result = client.call_tool("ask_question", arguments).await?;
    print_result(&config.output_format, &result);

    Ok(())
}
