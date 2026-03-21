use anyhow::{Context, Result};
use serde_json::json;
use std::time::Duration;

use crate::cli::OutputFormat;
use crate::mcp::McpConfig;

/// `dwiki check <repo>`
///
/// Verifies that a repository is indexed on DeepWiki by issuing an HTTP GET
/// request to `https://deepwiki.com/<repo>` and checking the response status.
pub async fn run(config: &McpConfig, repo: &str) -> Result<()> {
    let url = format!("https://deepwiki.com/{repo}");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.timeout_secs))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .context("Failed to build HTTP client")?;

    let response = client
        .get(&url)
        .send()
        .await
        .context("Failed to reach deepwiki.com")?;

    let status = response.status();
    let indexed = status.is_success();

    let message = if indexed {
        format!("INDEXED: {repo} is indexed on DeepWiki ({status})")
    } else {
        format!("NOT_INDEXED: {repo} is not indexed on DeepWiki ({status})")
    };

    match &config.output_format {
        OutputFormat::Json => {
            let payload = json!({
                "repo": repo,
                "indexed": indexed,
                "status": status.as_u16(),
                "url": url
            });
            println!("{}", serde_json::to_string_pretty(&payload).unwrap());
        }
        OutputFormat::Text => {
            println!("{message}");
        }
    }

    if !indexed {
        std::process::exit(1);
    }

    Ok(())
}
