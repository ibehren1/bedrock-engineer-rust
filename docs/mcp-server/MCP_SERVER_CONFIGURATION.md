# MCP Server Configuration

Model Context Protocol (MCP) client integration allows Bedrock Engineer to connect to external MCP servers and dynamically load and use powerful external tools. This integration extends the capabilities of your AI assistant by allowing it to access and utilize the tools provided by the MCP server.

## Configuration Formats

MCP servers can be configured using the Claude Desktop-compatible `claude_desktop_config.json` format. Configuration can be done from the "MCP Servers" section in the agent edit modal.

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
      "url": "https://example.com/mcp-endpoint"
    }
  }
}
```

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
