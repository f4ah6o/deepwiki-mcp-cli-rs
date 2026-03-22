use anyhow::Result;
use serde_json::json;

use crate::cli::OutputFormat;
use crate::mcp::{self, McpConfig};

/// `dwiki check <repo>`
///
/// Verifies that a repository is indexed on DeepWiki by calling the MCP
/// `read_wiki_structure` tool and checking whether the response contains
/// "Repository not found".
pub async fn run(config: &McpConfig, repo: &str) -> Result<()> {
    let client_config = mcp::McpConfig {
        url: config.url.clone(),
        token: config.token.clone(),
        timeout_secs: config.timeout_secs,
        output_format: config.output_format.clone(),
    };

    let mut client = mcp::McpClient::new(client_config)?;
    client.initialize().await?;

    let arguments = json!({ "repoName": repo });
    let result = client.call_tool("read_wiki_structure", arguments).await?;

    let indexed = !result.contains("Repository not found");

    match &config.output_format {
        OutputFormat::Json => {
            let payload = json!({
                "repo": repo,
                "indexed": indexed,
            });
            println!("{}", serde_json::to_string_pretty(&payload).unwrap());
        }
        OutputFormat::Text => {
            if indexed {
                println!("INDEXED: {repo} is indexed on DeepWiki");
            } else {
                println!("NOT_INDEXED: {repo} is not indexed on DeepWiki");
            }
        }
    }

    if !indexed {
        std::process::exit(1);
    }

    Ok(())
}
