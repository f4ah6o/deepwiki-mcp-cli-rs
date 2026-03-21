# AGENTS.md — dwiki Agent Integration Guide

`dwiki` is designed to be called by coding agents (LLM-based or scripted)
as a documentation lookup tool. This document describes the tool overview,
output schemas, exit codes, error handling, recommended workflows, and
best practices for agent use.

---

## Tool Overview

`dwiki` wraps the [DeepWiki MCP server](https://mcp.deepwiki.com) and exposes
it as a set of CLI subcommands optimized for agent pipelines. It supports
structured JSON output (`--output json`) on all commands for reliable parsing.

| Command | MCP tool | Purpose |
|---------|----------|---------|
| `dwiki check <repo>` | HTTP GET | Verify repository is indexed |
| `dwiki read <repo>` | `read_wiki_structure` | List all wiki topics |
| `dwiki read <repo> <topic>` | `read_wiki_contents` | Read a topic page in full |
| `dwiki ask <repo> <question>` | `ask_question` | AI-powered Q&A |
| `dwiki search <repo> <query>` | `ask_question` | Keyword/symbol search |

**Always use `--output json`** in agent pipelines for stable, parseable output.

---

## Typical Workflow

The recommended order of operations is:

1. **check** — verify the repository is indexed before spending tokens
2. **read** (no topic) — discover available topics; cache the list
3. **read** (with topic) — load specific topic pages for deterministic content
4. **ask** — targeted AI questions when you need synthesis across topics
5. **search** — open-ended keyword exploration of an unfamiliar codebase

---

## Output Schemas

### `dwiki check` — JSON

```json
{
  "repo":    "owner/repo",
  "indexed": true,
  "status":  200,
  "url":     "https://deepwiki.com/owner/repo"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `repo` | `string` | The repository slug passed as argument |
| `indexed` | `boolean` | `true` if HTTP 200; `false` otherwise |
| `status` | `integer` | Raw HTTP status code from deepwiki.com |
| `url` | `string` | The URL that was fetched |

### `dwiki read`, `dwiki ask`, `dwiki search` — JSON

When the content is plain text (the normal case):

```json
{
  "result": "<full text of the response>"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `result` | `string` | Full text of the MCP tool response |

When the MCP server already returns JSON (rare):

```json
{
  "topics": ["Getting Started", "Runtime", "..."]
}
```

Parse defensively: always check for `result` first, then fall through
to treating the entire object as the payload.

---

## Exit Codes

| Code | Command | Meaning |
|------|---------|---------|
| `0` | all | Success — stdout contains valid output |
| `1` | `check` | Repository not indexed on DeepWiki |
| `1` | all others | Network error, timeout, or MCP protocol error |

`dwiki check` is the only command that exits `1` for a "soft" failure
(unindexed repo). All other commands exit `1` only on hard errors.

There is no exit code `2`. All errors collapse to `1` for simplicity.
Agents should inspect stderr for the specific error message.

---

## Error Handling

Errors are written to **stderr**. Stdout is always either empty or valid
output (text or JSON). This makes it safe to capture stdout independently.

Common error patterns on stderr:

```
Error: Failed to reach deepwiki.com: connection refused
Error: MCP error -32601: Method not found
Error: Server returned HTTP 404: ...
Error: No data received from SSE stream
Error: HTTP request failed: operation timed out
```

### Recommended error handling in shell

```bash
output=$(dwiki ask owner/repo "question" --output json 2>/tmp/dwiki_err)
exit_code=$?

if [ $exit_code -ne 0 ]; then
  echo "dwiki failed: $(cat /tmp/dwiki_err)" >&2
  exit 1
fi

echo "$output" | jq -r .result
```

---

## Recommended Agent Workflow

### Step 1 — Guard: check before calling

Always verify the repository is indexed. Unindexed repos return MCP errors
that waste tokens and time.

```bash
result=$(dwiki check owner/repo --output json)
if [ "$(echo "$result" | jq -r .indexed)" != "true" ]; then
  echo "Repository not indexed — skipping documentation lookup"
  exit 0
fi
```

### Step 2 — Discover: get the topic list

Load the topic list once per session and cache it. It rarely changes.

```bash
topics=$(dwiki read owner/repo --output json | jq -r .result)
```

Scan topic titles to identify the most relevant pages before loading them.

### Step 3 — Read: load specific topic pages

Prefer `read <repo> <topic>` over `ask` when you know the topic name.
`read_wiki_contents` returns deterministic, full-text documentation;
it is faster and cheaper than `ask_question`.

```bash
dwiki read owner/repo "Architecture" --output json | jq -r .result
dwiki read owner/repo "Getting Started" --output json | jq -r .result
```

### Step 4 — Ask: targeted AI questions

Use `ask` for questions that require synthesis across multiple pages,
or when the relevant topic is unknown.

```bash
dwiki ask owner/repo "Where is the database connection pool initialised?" \
  --output json | jq -r .result
```

Keep questions specific. Broad questions ("tell me everything") produce
less useful answers than focused ones ("how does X call Y?").

### Step 5 — Search: symbol or keyword lookup

Use `search` when exploring an unfamiliar codebase or looking for a symbol.

```bash
dwiki search owner/repo "TcpListener" --output json | jq -r .result
dwiki search owner/repo "retry logic" --output json | jq -r .result
```

---

## Full Workflow Example

```bash
REPO="tokio-rs/tokio"

# 1. Guard
check=$(dwiki check "$REPO" --output json)
[ "$(echo "$check" | jq -r .indexed)" = "true" ] || { echo "Not indexed"; exit 1; }

# 2. Topic list
topics=$(dwiki read "$REPO" --output json | jq -r .result)
echo "$topics"

# 3. Read the most relevant topic
dwiki read "$REPO" "Runtime" --output json | jq -r .result

# 4. Ask a specific question
dwiki ask "$REPO" "What is the difference between current_thread and multi_thread runtimes?" \
  --output json | jq -r .result

# 5. Symbol search
dwiki search "$REPO" "JoinHandle" --output json | jq -r .result
```

---

## Best Practices

- **Run `check` once per repo** per session, not before every command. Index
  status rarely changes during a session.
- **Cache topic lists** from `read <repo>` (no topic). The structure is stable
  within a session.
- **Prefer `read` over `ask`** when you know the topic name. `read_wiki_contents`
  is deterministic, faster, and does not consume AI tokens on the server side.
- **Use `--timeout 60`** for `ask` calls on complex questions. The default 30 s
  can time out on slow networks or long answers.
- **Extract `.result`** from JSON output before passing content to an LLM to
  avoid wrapping the JSON envelope in the prompt.
- **Set environment variables** rather than passing `--url` / `--token` as flags
  to keep credentials out of process listings and shell history.

---

## Performance Notes

| Tip | Detail |
|-----|--------|
| Cache topic lists | `read_wiki_structure` results are stable within a session |
| Prefer `read` over `ask` | `read_wiki_contents` is deterministic and faster |
| Increase timeout for `ask` | Set `--timeout 60` for complex AI queries |
| Run `check` once per repo | Not on every call; index status rarely changes |

---

## Environment Configuration

Set these once in the agent's environment:

```bash
# Public repos (default — no token required)
export DEEPWIKI_URL=https://mcp.deepwiki.com/mcp

# Private repos via Devin
export DEEPWIKI_URL=https://mcp.devin.ai/mcp
export DEEPWIKI_TOKEN=<token>
```

Do **not** pass `--token` as a CLI flag — it will appear in process listings
and shell history. Use the environment variable.

---

## Disclaimer

`dwiki` is an unofficial, third-party tool and is not affiliated with or
endorsed by DeepWiki or Cognition AI.
