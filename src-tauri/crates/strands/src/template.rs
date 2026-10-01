//! Port of `src/main/services/strandsAgentsConverter/templateEngine.ts`: the output templates
//! and the helpers that fill them.
//!
//! The templates are the TS template literals with their escapes resolved (`\\n` in TS is the
//! two characters `\n` in the generated Python). The legacy `PYTHON_AGENT_TEMPLATE` is not
//! ported: only `generatePythonCode` used it, and nothing calls that.

/// `MCP_INTEGRATED_TEMPLATE` (`agent.py`).
pub const MCP_INTEGRATED_TEMPLATE: &str = r##"#!/usr/bin/env python3
"""
Generated Strands Agent with MCP Server Integration
Agent: {{agentName}}
Description: {{agentDescription}}
Generated on: {{generationDate}}
"""

{{imports}}

# System prompt
SYSTEM_PROMPT = """{{systemPrompt}}"""

# AWS configuration
session = boto3.Session(
    region_name="{{awsRegion}}",
)

{{mcpClientSetup}}

def setup_basic_tools():
    """Configure basic tools used by the agent"""
    tools = []
    {{basicToolsSetup}}
    {{specialSetupCode}}
    return tools

def create_model():
    """Create Bedrock model"""
    bedrock_model = BedrockModel(
        model_id="{{modelConfig}}",
        temperature=0.3,
        top_p=0.8,
        boto_session=session
    )
    return bedrock_model

def main():
    """Main agent execution function"""
    try:
        # Setup basic tools
        basic_tools = setup_basic_tools()

        # Connect to MCP servers and collect tools
        all_tools = basic_tools.copy()

        {{mcpContextManager}}:
{{mcpToolsCollection}}

            # Create and configure agent
            agent = Agent(
                system_prompt=SYSTEM_PROMPT,
                tools=all_tools,
                model=create_model()
            )

            # Interactive chat loop
            print(f"🤖 {{agentName}} agent is ready!")
            print("Type 'quit' or 'exit' to terminate the session")

            while True:
                try:
                    user_input = input("\n👤 You: ")
                    if user_input.lower().strip() in ['quit', 'exit']:
                        break

                    if not user_input.strip():
                        continue

                    response = agent(user_input)
                    print(f"🤖 Agent: {response}")

                except KeyboardInterrupt:
                    print("\n👋 Goodbye!")
                    break
                except Exception as e:
                    print(f"❌ Error occurred: {e}")

    except Exception as e:
        print(f"❌ Agent initialization error: {e}")
        return 1

    return 0

if __name__ == "__main__":
    exit(main())
"##;

/// `REQUIREMENTS_TEMPLATE` (`requirements.txt`).
pub const REQUIREMENTS_TEMPLATE: &str = r##"# Strands Agents dependencies
strands-agents>=1.0.0
strands-agents-tools>=0.2.0

# MCP dependencies (if MCP servers are used)
{{mcpDependencies}}

# AWS dependencies (if needed)
boto3>=1.26.0
botocore>=1.29.0

# Additional dependencies based on tools used
{{additionalDependencies}}
"##;

/// `CONFIG_TEMPLATE` (`config.yaml`).
pub const CONFIG_TEMPLATE: &str = r##"# Strands Agent Configuration
agent:
  name: "{{agentName}}"
  description: "{{agentDescription}}"
  model_provider: "{{modelProvider}}"

tools:
  supported:
{{supportedTools}}

  unsupported:
{{unsupportedTools}}

environment:
{{environmentVars}}

notes: |
  This agent was automatically converted from a Bedrock Engineer CustomAgent.
  If detailed configuration or adjustments are needed, please edit the generated Python code directly.
"##;

/// `README_TEMPLATE` (`README.md`).
pub const README_TEMPLATE: &str = r##"# {{agentName}}

{{agentDescription}}

## Overview

This agent was automatically converted from a Bedrock Engineer CustomAgent.

## Usage

### 1. Create Virtual Environment

Create and activate a Python virtual environment:

```bash
# Create virtual environment
python -m venv .venv

# Activate virtual environment
# On Windows:
.venv\Scripts\activate
# On macOS/Linux:
source .venv/bin/activate
```

### 2. Install Dependencies

```bash
pip install -r requirements.txt
```

### 3. Set Environment Variables

Set the required environment variables:

```bash
{{environmentSetup}}
```

### 4. Run the Agent

```bash
python agent.py
```

### 5. Programmatic Usage

```python
from agent import create_agent

agent = create_agent()
response = agent("Enter your question or task here")
print(response)
```

## Available Tools

{{toolsList}}

## Unsupported Tools

The following tools are not supported in automatic conversion:

{{unsupportedToolsList}}

## Notes

- When using AWS-related tools, appropriate AWS credentials must be configured
- Some tools may require environment-specific configuration
- Customize the generated code as needed

## Conversion Information

- Source: Bedrock Engineer CustomAgent
- Conversion Date: {{conversionDate}}
- Supported Tools: {{supportedToolsCount}}/{{totalToolsCount}}
"##;

/// `renderTemplate`: for each variable in order, replace every `{{key}}` with the value. Later
/// variables also apply to text inserted by earlier ones, and the value is a JS replacement
/// string (`$$`, `$&`, `` $` ``, `$'` are expanded), exactly as `String.prototype.replace` with a
/// global regex did.
pub fn render_template(template: &str, variables: &[(&str, String)]) -> String {
    let mut result = template.to_string();
    for (key, value) in variables {
        result = js_replace_all(&result, &format!("{{{{{key}}}}}"), value);
    }
    result
}

/// `haystack.replace(/<pattern>/g, replacement)` for a literal pattern without capture groups.
fn js_replace_all(haystack: &str, pattern: &str, replacement: &str) -> String {
    if pattern.is_empty() || !haystack.contains(pattern) {
        return haystack.to_string();
    }
    let mut out = String::with_capacity(haystack.len());
    let mut last = 0;
    for (pos, matched) in haystack.match_indices(pattern) {
        out.push_str(&haystack[last..pos]);
        let end = pos + matched.len();
        expand_replacement(
            &mut out,
            replacement,
            matched,
            &haystack[..pos],
            &haystack[end..],
        );
        last = end;
    }
    out.push_str(&haystack[last..]);
    out
}

/// ECMAScript `GetSubstitution` with no captures: `$1`… and `$<` stay literal.
fn expand_replacement(
    out: &mut String,
    replacement: &str,
    matched: &str,
    before: &str,
    after: &str,
) {
    let mut chars = replacement.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('$') => {
                out.push('$');
                chars.next();
            }
            Some('&') => {
                out.push_str(matched);
                chars.next();
            }
            Some('`') => {
                out.push_str(before);
                chars.next();
            }
            Some('\'') => {
                out.push_str(after);
                chars.next();
            }
            _ => out.push('$'),
        }
    }
}

/// `generateToolsSetupCode`
pub fn generate_tools_setup_code(supported_tools: &[&str]) -> String {
    if supported_tools.is_empty() {
        return "# No basic tools".to_string();
    }
    format!("tools.extend([{}])", supported_tools.join(", "))
}

/// `combineSpecialSetupCode`
pub fn combine_special_setup_code(codes: &[String]) -> String {
    codes
        .iter()
        .filter(|c| !js_trim(c).is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// `generateYamlList`
pub fn generate_yaml_list(items: &[String]) -> String {
    if items.is_empty() {
        return "    []".to_string();
    }
    items
        .iter()
        .map(|item| format!("    - \"{item}\""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `generateEnvironmentSetup`
pub fn generate_environment_setup(env: &[(String, String)]) -> String {
    if env.is_empty() {
        return "# No environment variables required".to_string();
    }
    env.iter()
        .map(|(k, v)| format!("export {k}=\"{v}\""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `generateMcpClientSetup`
pub fn generate_mcp_client_setup(strands_codes: &[&str]) -> String {
    if strands_codes.is_empty() {
        return "# No MCP server configuration".to_string();
    }
    strands_codes.join("\n\n")
}

/// `generateMcpContextManager`
pub fn generate_mcp_context_manager(client_names: &[String]) -> String {
    if client_names.is_empty() {
        return "# No MCP servers".to_string();
    }
    format!("with {}", client_names.join(", "))
}

/// `generateMcpToolsCollection`: `(server name, client variable name)` pairs.
pub fn generate_mcp_tools_collection(servers: &[(String, String)]) -> String {
    if servers.is_empty() {
        return "            # No MCP tools".to_string();
    }
    servers
        .iter()
        .map(|(name, var)| {
            format!(
                "            # Get tools from {name}\n            {var}_tools = {var}_client.list_tools_sync()\n            all_tools.extend({var}_tools)"
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `generateMcpDependencies`
pub fn generate_mcp_dependencies(has_mcp_servers: bool) -> String {
    if has_mcp_servers {
        "mcp>=1.12.1".to_string()
    } else {
        String::new()
    }
}

/// JS `String.prototype.trim`.
pub(crate) fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_replaces_every_occurrence_in_variable_order() {
        let out = render_template(
            "{{a}} and {{a}}, {{b}}",
            &[("a", "x{{b}}".into()), ("b", "y".into())],
        );
        // `{{b}}` inserted by `a` is replaced afterwards, as in the TS loop.
        assert_eq!(out, "xy and xy, y");
    }

    #[test]
    fn render_expands_js_replacement_patterns() {
        let t = "pre {{v}} post";
        assert_eq!(render_template(t, &[("v", "$$".into())]), "pre $ post");
        assert_eq!(render_template(t, &[("v", "$&".into())]), "pre {{v}} post");
        assert_eq!(render_template(t, &[("v", "$`".into())]), "pre pre  post");
        assert_eq!(render_template(t, &[("v", "$'".into())]), "pre  post post");
        // No capture groups: `$1`, `$<`, a lone `$` stay literal.
        assert_eq!(
            render_template(t, &[("v", "$1 $<x> $ $z".into())]),
            "pre $1 $<x> $ $z post"
        );
    }

    #[test]
    fn helpers_match_ts() {
        assert_eq!(generate_tools_setup_code(&[]), "# No basic tools");
        assert_eq!(
            generate_tools_setup_code(&["shell", "think"]),
            "tools.extend([shell, think])"
        );
        assert_eq!(
            combine_special_setup_code(&["a".into(), "  ".into(), "b".into()]),
            "a\n\nb"
        );
        assert_eq!(generate_yaml_list(&[]), "    []");
        assert_eq!(
            generate_yaml_list(&["x".into(), "y".into()]),
            "    - \"x\"\n    - \"y\""
        );
        assert_eq!(
            generate_environment_setup(&[]),
            "# No environment variables required"
        );
        assert_eq!(
            generate_environment_setup(&[("A".into(), "1".into()), ("B".into(), "2".into())]),
            "export A=\"1\"\nexport B=\"2\""
        );
        assert_eq!(
            generate_mcp_client_setup(&[]),
            "# No MCP server configuration"
        );
        assert_eq!(generate_mcp_client_setup(&["a", "b"]), "a\n\nb");
        assert_eq!(generate_mcp_context_manager(&[]), "# No MCP servers");
        assert_eq!(
            generate_mcp_context_manager(&["a_client".into(), "b_client".into()]),
            "with a_client, b_client"
        );
        assert_eq!(
            generate_mcp_tools_collection(&[]),
            "            # No MCP tools"
        );
        assert_eq!(
            generate_mcp_tools_collection(&[("Srv".into(), "srv".into())]),
            "            # Get tools from Srv\n            srv_tools = srv_client.list_tools_sync()\n            all_tools.extend(srv_tools)"
        );
        assert_eq!(generate_mcp_dependencies(false), "");
        assert_eq!(generate_mcp_dependencies(true), "mcp>=1.12.1");
    }

    #[test]
    fn templates_keep_python_escapes() {
        // TS `\\n` inside the template literal is a literal backslash-n in the Python source.
        assert!(MCP_INTEGRATED_TEMPLATE.contains(r#"input("\n👤 You: ")"#));
        assert!(README_TEMPLATE.contains(r".venv\Scripts\activate"));
        assert!(MCP_INTEGRATED_TEMPLATE.ends_with("    exit(main())\n"));
    }
}
