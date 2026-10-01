/// Errors raised by attachment operations. `Display` strings match the TS `Error` messages.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No project directory is configured. Set one in Settings before attaching files.")]
    NoProjectPath,
    #[error("Invalid attachment name: \"{0}\"")]
    InvalidName(String),
    #[error("Attachment name \"{0}\" resolves outside the chat's attachments folder.")]
    OutsideFolder(String),
    /// An image rejected by [`crate::image_validation::validate_image_bytes`].
    #[error("{0}")]
    Image(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
