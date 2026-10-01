//! Port of `src/common/mcp/utils.ts`.

use common::agent::{ConnectionType, McpServerConfig};

/// `inferConnectionType`: explicit `connectionType`, else `command` if a command is set, else
/// `url` if a URL is set, else `command` (legacy format).
pub fn infer_connection_type(server: &McpServerConfig) -> ConnectionType {
    if let Some(t) = server.connection_type {
        return t;
    }
    if server.command.as_deref().is_some_and(|c| !c.is_empty()) {
        return ConnectionType::Command;
    }
    if server.url.as_deref().is_some_and(|u| !u.is_empty()) {
        return ConnectionType::Url;
    }
    ConnectionType::Command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_connection_type() {
        let mut s = McpServerConfig::default();
        assert_eq!(infer_connection_type(&s), ConnectionType::Command);
        s.url = Some("https://x".into());
        assert_eq!(infer_connection_type(&s), ConnectionType::Url);
        s.command = Some("node".into());
        assert_eq!(infer_connection_type(&s), ConnectionType::Command);
        s.connection_type = Some(ConnectionType::Url);
        assert_eq!(infer_connection_type(&s), ConnectionType::Url);
    }
}
