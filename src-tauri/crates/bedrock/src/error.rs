//! Crate error type.

use aws_sdk_bedrockruntime::error::{DisplayErrorContext, ProvideErrorMetadata, SdkError};
use serde_json::{json, Map, Value};

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// A Bedrock service (or transport) failure, carrying the fields the JS SDK exposes on its
/// `ServiceException` (`name`, `message`, `$fault`, `$metadata`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceError {
    /// Error code, e.g. `ThrottlingException`, `ValidationException`. For failures that never
    /// reached the service this is a transport category (`TimeoutError`, `NetworkingError`, ...).
    pub name: String,
    pub message: String,
    /// `client` or `server` when known.
    pub fault: Option<String>,
    pub http_status_code: Option<u16>,
    pub request_id: Option<String>,
}

impl ServiceError {
    pub fn new(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            message: message.into(),
            fault: None,
            http_status_code: None,
            request_id: None,
        }
    }

    /// True for the errors `ConverseService.handleError` retries
    /// (`ThrottlingException` / `ServiceUnavailableException`).
    pub fn is_retryable(&self) -> bool {
        self.name == "ThrottlingException" || self.name == "ServiceUnavailableException"
    }

    /// The JSON body the Express routes sent for this error (`{ ...error, message }`):
    /// `name`, `$fault`, `$metadata` and `message`.
    pub fn to_json(&self) -> Value {
        let mut meta = Map::new();
        if let Some(code) = self.http_status_code {
            meta.insert("httpStatusCode".into(), json!(code));
        }
        if let Some(id) = &self.request_id {
            meta.insert("requestId".into(), json!(id));
        }
        let mut out = Map::new();
        out.insert("name".into(), json!(self.name));
        if let Some(fault) = &self.fault {
            out.insert("$fault".into(), json!(fault));
        }
        out.insert("$metadata".into(), Value::Object(meta));
        out.insert("message".into(), json!(self.message));
        Value::Object(out)
    }

    /// Build from any SDK error (operation errors and event-stream errors alike).
    pub fn from_sdk<E, R>(err: &SdkError<E, R>) -> Self
    where
        E: ProvideErrorMetadata + std::error::Error + 'static,
        R: RawResponse + std::fmt::Debug,
    {
        match err {
            SdkError::ServiceError(ctx) => {
                let e = ctx.err();
                let status = ctx.raw().status_code();
                // Event-stream exceptions carry no error code metadata; the SDK error enum variant
                // (`ModelStreamErrorException(..)`) names the shape instead.
                let name = e
                    .code()
                    .map(str::to_string)
                    .or_else(|| variant_name(e))
                    .unwrap_or_else(|| "UnknownError".to_string());
                let fault = status
                    .map(|s| if s >= 500 { "server" } else { "client" }.to_string())
                    .or_else(|| Some(fault_for_name(&name).to_string()));
                Self {
                    name,
                    message: e
                        .message()
                        .map(str::to_string)
                        .unwrap_or_else(|| DisplayErrorContext(e).to_string()),
                    fault,
                    http_status_code: status,
                    request_id: e.meta().extra("aws_request_id").map(str::to_string),
                }
            }
            SdkError::TimeoutError(_) => {
                Self::new("TimeoutError", DisplayErrorContext(err).to_string())
            }
            SdkError::DispatchFailure(_) => {
                Self::new("NetworkingError", DisplayErrorContext(err).to_string())
            }
            SdkError::ConstructionFailure(_) => {
                Self::new("ConstructionFailure", DisplayErrorContext(err).to_string())
            }
            SdkError::ResponseError(_) => {
                Self::new("ResponseError", DisplayErrorContext(err).to_string())
            }
            _ => Self::new(
                err.code().unwrap_or("UnknownError"),
                DisplayErrorContext(err).to_string(),
            ),
        }
    }
}

/// `Foo` from the Debug output `Foo(..)` of a generated SDK error enum (not `Unhandled`).
fn variant_name<E: std::fmt::Debug>(e: &E) -> Option<String> {
    let debug = format!("{e:?}");
    let name = debug.split(['(', ' ', '{']).next()?;
    (name.ends_with("Exception")).then(|| name.to_string())
}

/// `$fault` of the Bedrock Runtime error shapes (used when there is no HTTP status, i.e. for
/// errors delivered inside the event stream).
fn fault_for_name(name: &str) -> &'static str {
    match name {
        "InternalServerException" | "ServiceUnavailableException" => "server",
        _ => "client",
    }
}

/// Raw response types an [`SdkError`] can carry: the HTTP response for operation errors and the
/// event-stream message for in-stream errors (which has no status code).
pub trait RawResponse {
    fn status_code(&self) -> Option<u16>;
}

impl RawResponse for aws_sdk_bedrockruntime::config::http::HttpResponse {
    fn status_code(&self) -> Option<u16> {
        Some(self.status().as_u16())
    }
}

impl RawResponse for aws_smithy_types::event_stream::RawMessage {
    fn status_code(&self) -> Option<u16> {
        None
    }
}

/// Errors produced by this crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Bedrock rejected or failed the request.
    #[error("{}: {}", .0.name, .0.message)]
    Service(ServiceError),
    /// The request JSON could not be converted into a Converse request.
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    /// Client/credential configuration failed.
    #[error("AWS configuration error: {0}")]
    Config(String),
    /// The caller cancelled the request via its `CancellationToken`.
    #[error("request cancelled")]
    Cancelled,
    /// The event sink (e.g. a Tauri `Channel`) refused an event; streaming stopped.
    #[error("stream consumer closed: {0}")]
    Sink(String),
    /// An error the TS code raised itself: a plain `new Error(message)` (`name: "Error"`), a
    /// custom error class (`StructuredOutputError`), or a thrown object (`TranslationError`).
    /// `fields` are the extra enumerable properties it carried (`code`, `details`, ...).
    #[error("{message}")]
    Custom {
        name: String,
        message: String,
        fields: Map<String, Value>,
    },
}

impl Error {
    /// `new Error(message)`.
    pub fn plain(message: impl Into<String>) -> Self {
        Error::Custom {
            name: "Error".into(),
            message: message.into(),
            fields: Map::new(),
        }
    }

    /// The error's `message` as the TS code would have seen it (`error.message`).
    pub fn message(&self) -> String {
        match self {
            Error::Service(e) => e.message.clone(),
            Error::Custom { message, .. } => message.clone(),
            other => other.to_string(),
        }
    }

    /// The error's `name` (`error.name`).
    pub fn name(&self) -> String {
        self.to_json()["name"]
            .as_str()
            .unwrap_or("Error")
            .to_string()
    }

    /// The service error name if this is a service error.
    pub fn service_name(&self) -> Option<&str> {
        match self {
            Error::Service(e) => Some(&e.name),
            _ => None,
        }
    }

    /// JSON body equivalent to what the Express `/converse*` routes sent on failure, so the app
    /// layer can hand the renderer the same error text it used to get from `res.text()`.
    pub fn to_json(&self) -> Value {
        match self {
            Error::Service(e) => e.to_json(),
            Error::InvalidRequest(m) => json!({ "name": "ValidationException", "message": m }),
            Error::Config(m) => json!({ "name": "CredentialsProviderError", "message": m }),
            Error::Cancelled => json!({ "name": "AbortError", "message": "request cancelled" }),
            Error::Sink(m) => json!({ "name": "StreamClosed", "message": m }),
            Error::Custom {
                name,
                message,
                fields,
            } => {
                let mut out = Map::new();
                out.insert("name".into(), json!(name));
                out.insert("message".into(), json!(message));
                for (k, v) in fields {
                    out.insert(k.clone(), v.clone());
                }
                Value::Object(out)
            }
        }
    }
}

impl<E, R> From<SdkError<E, R>> for Error
where
    E: ProvideErrorMetadata + std::error::Error + 'static,
    R: RawResponse + std::fmt::Debug,
{
    fn from(err: SdkError<E, R>) -> Self {
        Error::Service(ServiceError::from_sdk(&err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_names_match_handle_error() {
        assert!(ServiceError::new("ThrottlingException", "x").is_retryable());
        assert!(ServiceError::new("ServiceUnavailableException", "x").is_retryable());
        assert!(!ServiceError::new("ValidationException", "x").is_retryable());
        assert!(!ServiceError::new("ModelTimeoutException", "x").is_retryable());
    }

    #[test]
    fn service_error_json_matches_express_body_shape() {
        let e = ServiceError {
            name: "ValidationException".into(),
            message: "bad".into(),
            fault: Some("client".into()),
            http_status_code: Some(400),
            request_id: Some("rid".into()),
        };
        assert_eq!(
            e.to_json(),
            json!({
                "name": "ValidationException",
                "$fault": "client",
                "$metadata": { "httpStatusCode": 400, "requestId": "rid" },
                "message": "bad"
            })
        );
    }

    #[test]
    fn variant_name_from_debug() {
        #[derive(Debug)]
        #[allow(dead_code)]
        enum E {
            ModelStreamErrorException(u8),
            Unhandled(u8),
        }
        assert_eq!(
            variant_name(&E::ModelStreamErrorException(1)).as_deref(),
            Some("ModelStreamErrorException")
        );
        assert_eq!(variant_name(&E::Unhandled(1)), None);
    }

    #[test]
    fn fault_for_stream_errors() {
        assert_eq!(fault_for_name("InternalServerException"), "server");
        assert_eq!(fault_for_name("ServiceUnavailableException"), "server");
        assert_eq!(fault_for_name("ThrottlingException"), "client");
        assert_eq!(fault_for_name("ModelStreamErrorException"), "client");
    }

    #[test]
    fn non_service_errors_have_json() {
        assert_eq!(Error::Cancelled.to_json()["name"], "AbortError");
        assert_eq!(Error::InvalidRequest("m".into()).to_json()["message"], "m");
    }

    #[test]
    fn custom_errors_carry_their_fields() {
        let e = Error::plain("boom");
        assert_eq!(e.to_json(), json!({ "name": "Error", "message": "boom" }));
        assert_eq!(e.to_string(), "boom");
        let mut fields = Map::new();
        fields.insert("code".into(), json!("MISSING_OUTPUT"));
        let e = Error::Custom {
            name: "StructuredOutputError".into(),
            message: "m".into(),
            fields,
        };
        assert_eq!(
            e.to_json(),
            json!({ "name": "StructuredOutputError", "message": "m", "code": "MISSING_OUTPUT" })
        );
        assert_eq!(e.name(), "StructuredOutputError");
        assert_eq!(
            Error::Service(ServiceError::new("ValidationException", "bad")).message(),
            "bad"
        );
    }
}
