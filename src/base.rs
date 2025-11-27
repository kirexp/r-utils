
pub mod base {
    use thiserror::Error;

    pub type GenericError = anyhow::Error;
    pub type GenericResult<T> = anyhow::Result<T>;

    pub type BotResult<T> = Result<T, BotError>;

    #[derive(Debug, Error)]
    pub enum BotError {
        #[error("Unknown Error: {1}")]
        UnknownError(i64, String),
        #[error("BusinessError Error: {0}")]
        BusinessError(i64, String),
        #[error("ChatlessError: {0}")]
        ChatlessError(String)
    }
}
