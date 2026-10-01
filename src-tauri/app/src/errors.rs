//! Error strings at the command boundary (BRIDGE.md rule 3). Commands return `Err(String)`; a
//! JSON object string `{"name", "message", ...}` becomes an `Error` with that `name` and
//! `message` (plus the extra fields) in the renderer shim (`toError` in `tauriBridge.ts`), a
//! plain string becomes `new Error(string)`.

#[cfg(test)]
use serde_json::Value;

/// A `bedrock` crate error: the JSON body the Express routes sent (`name`, `message`, `$fault`,
/// `$metadata`, ...). `Cancelled` becomes `{ name: "AbortError" }`.
pub fn bedrock(e: bedrock::Error) -> String {
    e.to_json().to_string()
}

/// A tool failure as `{ name, message }`, rethrown as `Error` by the shim, like the preload's
/// `executeTool` rejecting.
pub fn tool(e: tools::ToolError) -> String {
    e.to_js_error().to_string()
}

/// `new Error(message)`: a plain message (the shim keeps `name: "Error"`).
pub fn plain(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Parse an error string back the way the shim does (tests and logging).
#[cfg(test)]
pub fn parse(err: &str) -> (String, String, Value) {
    match serde_json::from_str::<Value>(err) {
        Ok(v @ Value::Object(_)) if v["message"].is_string() => (
            v["name"].as_str().unwrap_or("Error").to_string(),
            v["message"].as_str().unwrap_or_default().to_string(),
            v,
        ),
        _ => ("Error".into(), err.to_string(), Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bedrock::ServiceError;
    use serde_json::json;

    #[test]
    fn cancelled_is_abort_error() {
        let (name, _, _) = parse(&bedrock(bedrock::Error::Cancelled));
        assert_eq!(name, "AbortError");
    }

    #[test]
    fn service_errors_keep_sdk_name_message_and_metadata() {
        let e = bedrock::Error::Service(ServiceError {
            name: "ThrottlingException".into(),
            message: "Too many requests".into(),
            fault: Some("client".into()),
            http_status_code: Some(429),
            request_id: Some("rid".into()),
        });
        let (name, message, v) = parse(&bedrock(e));
        assert_eq!(name, "ThrottlingException");
        assert_eq!(message, "Too many requests");
        assert_eq!(v["$fault"], "client");
        assert_eq!(v["$metadata"]["httpStatusCode"], 429);
    }

    #[test]
    fn in_stream_exceptions_are_named_errors() {
        // modelStreamErrorException & co. are thrown, not sent as events.
        let e = bedrock::Error::Service(ServiceError::new(
            "ModelStreamErrorException",
            "stream broke",
        ));
        let (name, message, _) = parse(&bedrock(e));
        assert_eq!(name, "ModelStreamErrorException");
        assert_eq!(message, "stream broke");
    }

    #[test]
    fn custom_errors_carry_extra_fields() {
        let mut fields = serde_json::Map::new();
        fields.insert("code".into(), json!("MISSING_OUTPUT"));
        let e = bedrock::Error::Custom {
            name: "StructuredOutputError".into(),
            message: "no output".into(),
            fields,
        };
        let (name, _, v) = parse(&bedrock(e));
        assert_eq!(name, "StructuredOutputError");
        assert_eq!(v["code"], "MISSING_OUTPUT");
    }

    #[test]
    fn config_and_validation_errors() {
        let (name, message, _) = parse(&bedrock(bedrock::Error::Config("no creds".into())));
        assert_eq!(name, "CredentialsProviderError");
        assert_eq!(message, "no creds");
        let (name, _, _) = parse(&bedrock(bedrock::Error::InvalidRequest("bad".into())));
        assert_eq!(name, "ValidationException");
    }

    #[test]
    fn tool_errors_are_name_and_message() {
        let e = tools::ToolError::not_found("nope");
        let (name, message, v) = parse(&tool(e.clone()));
        assert_eq!(name, e.name);
        assert_eq!(message, e.message);
        assert_eq!(v.as_object().unwrap().len(), 2);
    }

    #[test]
    fn plain_messages() {
        assert_eq!(
            parse(&plain("boom")),
            ("Error".into(), "boom".into(), Value::Null)
        );
    }
}
