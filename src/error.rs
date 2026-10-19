#[derive(Debug, thiserror::Error)]
#[error("unsupported changelog: {message}")]
pub struct ParseError {
    pub message: String,
}
impl ParseError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error("merge validation failed: {0}")]
    Validation(String),
}
