# dwiki — Unofficial DeepWiki CLI

> **Disclaimer:** `dwiki` is an independent, third-party tool and is **not**
> affiliated with or endorsed by [DeepWiki](https://deepwiki.com) or Cognition AI.
> It wraps the publicly available DeepWiki MCP API. Use at your own risk.

`dwiki` is a command-line interface for querying [DeepWiki](https://deepwiki.com)
via the [DeepWiki MCP server](https://mcp.deepwiki.com). DeepWiki indexes public
GitHub repositories and exposes their documentation through an MCP server. `dwiki`
makes that server accessible from your terminal and from coding-agent pipelines.

## Installation

```bash
cargo install dwiki
```

Or build from source:

```bash
git clone https://github.com/fu2hito/dwiki
cd dwiki
cargo build --release
# binary at: ./target/release/dwiki
```

## Quick Start

```bash
# 1. Confirm the repo is indexed on DeepWiki
dwiki check tokio-rs/tokio

# 2. List all available wiki topics
dwiki read tokio-rs/tokio

# 3. Read a specific topic page
dwiki read tokio-rs/tokio "Runtime"
```

## Command Reference

### `dwiki check <OWNER/REPO>`

Verifies that a repository is indexed on DeepWiki by sending an HTTP GET
to `https://deepwiki.com/<owner>/<repo>`.

| Detail | Value |
|--------|-------|
| Exit code 0 | Repository **is** indexed — safe to proceed |
| Exit code 1 | Repository is **not** indexed (or unreachable) |
| MCP tool | *(HTTP GET — not an MCP call)* |

```bash
dwiki check tokio-rs/tokio
# INDEXED: tokio-rs/tokio is indexed on DeepWiki (200 OK)

dwiki check myorg/unindexed-repo
# NOT_INDEXED: myorg/unindexed-repo is not indexed on DeepWiki (404 Not Found)
# exit code: 1
```

**JSON output (`--output json`):**
```json
{
  "repo": "tokio-rs/tokio",
  "indexed": true,
  "status": 200,
  "url": "https://deepwiki.com/tokio-rs/tokio"
}
```

---

### `dwiki read <OWNER/REPO> [TOPIC]`

| Variant | MCP tool called | Description |
|---------|-----------------|-------------|
| `dwiki read <repo>` | `read_wiki_structure` | Lists all available documentation topics |
| `dwiki read <repo> <topic>` | `read_wiki_contents` | Loads the full text of a specific topic page |

```bash
# List all topics
dwiki read tokio-rs/tokio

# Read a specific page (quote multi-word topics)
dwiki read tokio-rs/tokio "Getting Started"
dwiki read tokio-rs/tokio "Runtime"
dwiki read tokio-rs/tokio "Runtime" --output json
```

**JSON output (topic list):**
```json
{
  "result": "# tokio-rs/tokio\n\n## Topics\n- Getting Started\n- Runtime\n- ..."
}
```

---

### `dwiki ask <OWNER/REPO> <QUESTION>`

Calls the `ask_question` MCP tool. The response is AI-generated and grounded
in the repository's indexed documentation.

| Detail | Value |
|--------|-------|
| MCP tool | `ask_question` |
| Latency | 10–40 s (use `--timeout 60` for complex queries) |
| Best for | Targeted questions, implementation details, debugging |

```bash
dwiki ask tokio-rs/tokio "What is the difference between spawn and spawn_blocking?"
dwiki ask rust-lang/rust "Where is the borrow checker implemented?" --output json
```

**JSON output:**
```json
{
  "result": "The borrow checker is implemented in `compiler/rustc_borrowck`..."
}
```

---

### `dwiki search <OWNER/REPO> <QUERY>`

A convenience wrapper around `ask_question` that frames the request as a
keyword search, asking the model to enumerate all matching topics, functions,
modules, and documentation sections.

| Detail | Value |
|--------|-------|
| MCP tool | `ask_question` (search-oriented prompt) |
| Best for | Open-ended keyword exploration, symbol lookup |

```bash
dwiki search tokio-rs/tokio "channel"
dwiki search rust-lang/rust "lifetime elision" --output json
```

---

## Global Options

| Flag | Env var | Default | Description |
|------|---------|---------|-------------|
| `--url <URL>` | `DEEPWIKI_URL` | `https://mcp.deepwiki.com/mcp` | MCP server URL |
| `--token <TOKEN>` | `DEEPWIKI_TOKEN` | *(none)* | Bearer token for private-repo endpoints |
| `--output text\|json` | — | `text` | Output format |
| `--timeout <SECONDS>` | — | `30` | Request timeout in seconds |

All global options can be placed before or after the subcommand.

## Configuration

### Environment variables

```bash
# Override the MCP server URL (optional; default is the public DeepWiki server)
export DEEPWIKI_URL=https://mcp.deepwiki.com/mcp

# Bearer token — required only for private-repo endpoints
export DEEPWIKI_TOKEN=your_token_here
```

Avoid passing `--token` as a shell argument to keep credentials out of
process lists and shell history. Use the environment variable instead.

### Public repositories (no auth required)

```bash
dwiki ask tokio-rs/tokio "How does the runtime work?"
```

### Private repositories via Devin

```bash
export DEEPWIKI_URL=https://mcp.devin.ai/mcp
export DEEPWIKI_TOKEN=your_token_here

dwiki check myorg/private-repo
dwiki ask myorg/private-repo "How does the auth module work?"
```

## JSON Output for Agents

All commands support `--output json`. This is the recommended mode for
coding agents and shell pipelines:

```bash
# Guard check before running expensive queries
result=$(dwiki check tokio-rs/tokio --output json)
indexed=$(echo "$result" | jq -r .indexed)
[ "$indexed" = "true" ] || exit 1

# Get topic list
dwiki read tokio-rs/tokio --output json | jq -r .result

# Ask a question and extract the answer
dwiki ask tokio-rs/tokio "How does spawn work?" --output json | jq -r .result
```

See [AGENTS.md](AGENTS.md) for the full agent workflow documentation, output
schemas, and exit codes.

## License

MIT — see [LICENSE](LICENSE).
