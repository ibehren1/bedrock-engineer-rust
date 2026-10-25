# MCP Server Configuration

Model Context Protocol (MCP) client integration allows Bedrock Engineer to connect to external MCP servers and dynamically load and use powerful external tools. This integration extends the capabilities of your AI assistant by allowing it to access and utilize the tools provided by the MCP server.

## Configuration Formats

MCP servers can be configured using the Claude Desktop-compatible `claude_desktop_config.json` format. Configuration is done on the "MCP Servers" tab of the agent editor (My Agents page). Servers are configured per agent.

MCP servers can be configured in two formats:

### 1. Command Format (Local Servers)

```json
{
  "mcpServers": {
    "server-name": {
      "command": "command-to-run",
      "args": ["arg1", "arg2"],
      "env": {
        "ENV_VAR": "value"
      }
    }
  }
}
```

### 2. URL Format (Remote Servers)

```json
{
  "mcpServers": {
    "server-name": {
      "url": "https://example.com/mcp-endpoint",
      "headers": {
        "Authorization": "Bearer <token>"
      }
    }
  }
}
```

`headers` is optional. The app connects with the Streamable HTTP transport and falls back to the
older SSE transport if that fails.

On macOS and Linux, command servers started with `npx`, `uvx` and similar see your login shell's
`PATH`, even when the app is opened from the Dock or a desktop launcher.

## Configuration Examples

```json
{
  "mcpServers": {
    "fetch": {
      "command": "uvx",
      "args": ["mcp-server-fetch"]
    },
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "~/"],
      "env": {
        "NODE_ENV": "production"
      }
    },
    "DeepWiki": {
      "url": "https://mcp.deepwiki.com/sse"
    }
  }
}
```

## Popular MCP Servers

- **fetch**: Tool for retrieving web content
- **filesystem**: File system operation tools
- **DeepWiki**: Knowledge base search tool
- **git**: Git operation tools
- **postgres**: PostgreSQL database operation tools

## Using the App's AWS Settings

A command server's `env` values can use `{{awsRegion}}` and `{{awsProfile}}`. They are filled in
from **Settings → AWS** each time the server starts:

```json
{
  "mcpServers": {
    "aws-pricing": {
      "command": "uvx",
      "args": ["awslabs.aws-pricing-mcp-server@latest"],
      "env": {
        "AWS_PROFILE": "{{awsProfile}}",
        "AWS_REGION": "{{awsRegion}}"
      }
    }
  }
}
```

- `{{awsRegion}}` becomes the app's region.
- `{{awsProfile}}` becomes the profile name when the app uses a named profile (`default` if the
  name is empty).
- When the app uses an access key instead, the entry whose value is `{{awsProfile}}` is dropped and
  `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` and (if set) `AWS_SESSION_TOKEN` are passed instead.
- Values produced by placeholders override the same variables in the app's own environment. Other
  `env` entries are still overridden by it, as before.

Only servers whose `env` uses a placeholder receive the app's credentials.

## Built-in Server for Local Knowledge Bases

An agent that has local knowledge bases (agent editor → **Knowledge Base** tab) automatically gets
one more server, `local-knowledgebase`. It runs inside the app, so there is nothing to install or
configure, and it is not part of the agent's `mcpServers` JSON: it is added when the agent runs and
removed when its last knowledge base is deleted. Its tools are `searchKnowledgeBase`,
`listKnowledgeBases` and `listKnowledgeBaseSources`.

The "MCP Servers" tab shows it read-only, marked **Built-in**, above the agent's own servers, with
a **Test connection** button; the **Tools** tab lists its tools with the other MCP tools. Manage
the knowledge bases themselves on the Knowledge Base tab and in **Settings → Knowledge Base**.

## Finding Servers

The "MCP Servers" tab has a **Find MCP servers** panel backed by the
[official MCP Registry](https://registry.modelcontextprotocol.io):

- **Search** queries the registry directly (`/v0/servers?search=…&version=latest`).
- **Suggest for this agent** asks the model to derive search terms from the agent's configuration,
  then runs those searches. The model produces search terms only — every server, version and package
  identifier shown comes from the registry.
- **MCP Registry** and **MCP Market** open the two directories in your browser.

What the suggestion reads, in order of usefulness:

| Source                              | Why it matters                                                                         |
| ----------------------------------- | -------------------------------------------------------------------------------------- |
| System prompt                       | Where the specifics live — the systems, products and data sources the agent works with |
| Scenarios                           | Concrete tasks, usually naming real tools                                              |
| Allowed shell commands              | A `kubectl` or `gh` pattern names the system directly                                  |
| Additional instruction              | Extra context appended to the generated prompt                                         |
| Description, category               | Coarse signal, used when the prompt is thin                                            |
| Enabled tools, existing MCP servers | Excluded from suggestions — no point proposing what the agent already has              |

Each term must be grounded in a phrase from that configuration, and the UI shows the phrase next to
the term. Generic words (`automation`, `devops`, `deployment`, `monitoring`, …) are rejected because
they match nothing useful in a directory. If nothing in the configuration names a system, the panel
says so instead of guessing — fill in the system prompt or search the registry directly.

Each result shows its registry name, version, how it runs, and any environment variables it requires.
"Load config" fills the JSON editor with:

- an npm package as `npx -y <identifier>@<version>`,
- a Python package as `uvx <identifier>`,
- a hosted server as its remote `url`,
- `env` keys with empty values for anything the server requires.

Servers published without package details show a note instead of a config; open their source
repository to see how to run them. Nothing is added or connected until you press "Add Server", so you
always see the command first.

[MCP Market](https://mcpmarket.com) is linked for browsing by category, but not read by the app: it
publishes no API, its `robots.txt` disallows `/api/` for all agents, and automated requests receive a
Vercel bot challenge.

## Configuration Steps

1. Open the agent editor from the My Agents page
2. Select the "MCP Servers" tab
3. Click "Add New MCP Server" button
4. Enter configuration in the JSON format above
5. Test the connection with "Test Connection"
6. Save the configuration

## Troubleshooting

**If connection errors occur:**

- Verify the command path is correct
- Check that required dependencies are installed
- Confirm environment variables are properly set

**If configuration errors occur:**

- Verify the JSON format is correct
- Ensure the `mcpServers` object is included
- Check for duplicate server names
