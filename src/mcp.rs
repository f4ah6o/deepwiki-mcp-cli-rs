use anyhow::{anyhow, bail, Context, Result};
use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};
use std::time::Duration;

use crate::cli::OutputFormat;

/// Runtime configuration passed from CLI flags / env vars.
pub struct McpConfig {
    pub url: String,
    pub token: Option<String>,
    pub timeout_secs: u64,
    pub output_format: OutputFormat,
}

// ---------------------------------------------------------------------------
// MCP client
// ---------------------------------------------------------------------------

pub struct McpClient {
    client: reqwest::Client,
    config: McpConfig,
    session_id: Option<String>,
}

impl McpClient {
    pub fn new(config: McpConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            client,
            config,
            session_id: None,
        })
    }

    /// Build request headers, optionally including a session ID.
    fn build_headers(&self, session_id: Option<&str>) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/event-stream"),
        );

        if let Some(token) = &self.config.token {
            let auth = format!("Bearer {token}");
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&auth).context("Invalid token value")?,
            );
        }

        if let Some(sid) = session_id {
            headers.insert(
                "mcp-session-id",
                HeaderValue::from_str(sid).context("Invalid session ID")?,
            );
        }

        Ok(headers)
    }

    /// Send a raw JSON-RPC request and return the parsed response.
    /// Handles both plain JSON and Server-Sent Events (SSE) responses.
    async fn send(&self, body: Value, session_id: Option<&str>) -> Result<Value> {
        let headers = self.build_headers(session_id)?;

        let response = self
            .client
            .post(&self.config.url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .context("HTTP request failed")?;

        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            bail!("Server returned HTTP {status}: {text}");
        }

        if content_type.contains("text/event-stream") {
            self.parse_sse_response(response).await
        } else {
            let text = response.text().await.context("Failed to read response body")?;
            serde_json::from_str(&text).context("Failed to parse JSON response")
        }
    }

    /// Parse a Server-Sent Events response and collect the final result.
    async fn parse_sse_response(&self, response: reqwest::Response) -> Result<Value> {
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut last_result: Option<Value> = None;

        while let Some(chunk) = stream.next().await {
            let chunk: Bytes = chunk.context("SSE stream error")?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete SSE events (delimited by double newlines)
            while let Some(event_end) = buffer.find("\n\n") {
                let event_str = buffer[..event_end].to_string();
                buffer = buffer[event_end + 2..].to_string();

                let event = parse_sse_event(&event_str);

                if let Some(data) = event.data {
                    if data == "[DONE]" {
                        break;
                    }
                    if let Ok(parsed) = serde_json::from_str::<Value>(&data) {
                        last_result = Some(parsed);
                    }
                }
            }
        }

        // Handle remaining buffer
        if !buffer.is_empty() {
            let event = parse_sse_event(&buffer);
            if let Some(data) = event.data {
                if data != "[DONE]" {
                    if let Ok(parsed) = serde_json::from_str::<Value>(&data) {
                        last_result = Some(parsed);
                    }
                }
            }
        }

        last_result.ok_or_else(|| anyhow!("No data received from SSE stream"))
    }

    /// Perform the MCP initialize handshake and store the session ID.
    pub async fn initialize(&mut self) -> Result<()> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "dwiki",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });

        // Use reqwest directly here to capture the response headers for session ID
        let headers = self.build_headers(None)?;
        let response = self
            .client
            .post(&self.config.url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .context("Initialize request failed")?;

        if let Some(sid) = response.headers().get("mcp-session-id") {
            if let Ok(sid_str) = sid.to_str() {
                self.session_id = Some(sid_str.to_string());
            }
        }

        // Drain the response body
        let status = response.status();
        let _body = response.text().await.ok();

        if !status.is_success() {
            // Some MCP servers return 4xx on initialize if the session is already active;
            // treat this as non-fatal and continue.
        }

        Ok(())
    }

    /// Call an MCP tool and return the text content of the result.
    pub async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<String> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        let response = self.send(body, self.session_id.as_deref()).await?;

        extract_tool_result(&response)
    }
}

// ---------------------------------------------------------------------------
// SSE helpers
// ---------------------------------------------------------------------------

struct SseEvent {
    data: Option<String>,
}

fn parse_sse_event(raw: &str) -> SseEvent {
    let mut data: Option<String> = None;

    for line in raw.lines() {
        if let Some(value) = line.strip_prefix("data:") {
            let value = value.trim();
            data = Some(
                data.map(|d| format!("{d}\n{value}"))
                    .unwrap_or_else(|| value.to_string()),
            );
        }
    }

    SseEvent { data }
}

// ---------------------------------------------------------------------------
// Result extraction
// ---------------------------------------------------------------------------

/// Extract text content from an MCP tools/call JSON-RPC response.
fn extract_tool_result(response: &Value) -> Result<String> {
    // JSON-RPC error
    if let Some(err) = response.get("error") {
        let code = err.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
        let msg = err
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        bail!("MCP error {code}: {msg}");
    }

    let result = response
        .get("result")
        .ok_or_else(|| anyhow!("Response has no 'result' field"))?;

    // MCP tools/call result format: { content: [ { type: "text", text: "..." } ] }
    if let Some(content_arr) = result.get("content").and_then(|v| v.as_array()) {
        let mut parts: Vec<String> = Vec::new();
        for item in content_arr {
            if item.get("type").and_then(|v| v.as_str()) == Some("text") {
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    parts.push(text.to_string());
                }
            }
        }
        if !parts.is_empty() {
            return Ok(parts.join("\n"));
        }
    }

    // Fallback: return the raw result as pretty JSON
    Ok(serde_json::to_string_pretty(result)
        .unwrap_or_else(|_| result.to_string()))
}

// ---------------------------------------------------------------------------
// Public helper: create a ready-to-use client
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub async fn connect(config: McpConfig) -> Result<McpClient> {
    let mut client = McpClient::new(config)?;
    client.initialize().await?;
    Ok(client)
}

// ---------------------------------------------------------------------------
// Output helpers
// ---------------------------------------------------------------------------

/// Print text or JSON depending on the configured output format.
pub fn print_result(output_format: &OutputFormat, text: &str) {
    match output_format {
        OutputFormat::Text => println!("{text}"),
        OutputFormat::Json => {
            // Try to detect if the text is already JSON; if so, pretty-print it.
            if let Ok(v) = serde_json::from_str::<Value>(text) {
                println!("{}", serde_json::to_string_pretty(&v).unwrap_or_else(|_| text.to_string()));
            } else {
                let wrapper = json!({ "result": text });
                println!("{}", serde_json::to_string_pretty(&wrapper).unwrap());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sse_event_single_data() {
        let raw = "data: {\"foo\":\"bar\"}";
        let event = parse_sse_event(raw);
        assert_eq!(event.data, Some("{\"foo\":\"bar\"}".to_string()));
    }

    #[test]
    fn test_parse_sse_event_multiline_data() {
        let raw = "data: line1\ndata: line2";
        let event = parse_sse_event(raw);
        assert_eq!(event.data, Some("line1\nline2".to_string()));
    }

    #[test]
    fn test_parse_sse_event_done() {
        let raw = "data: [DONE]";
        let event = parse_sse_event(raw);
        assert_eq!(event.data, Some("[DONE]".to_string()));
    }

    #[test]
    fn test_extract_tool_result_text_content() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "content": [
                    { "type": "text", "text": "Hello, world!" }
                ]
            }
        });
        let result = extract_tool_result(&response).unwrap();
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn test_extract_tool_result_error() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "error": { "code": -32601, "message": "Method not found" }
        });
        let result = extract_tool_result(&response);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Method not found"));
    }

    #[test]
    fn test_print_result_json_wraps_plain_text() {
        // smoke test: should not panic
        print_result(&OutputFormat::Json, "plain text");
    }
}
