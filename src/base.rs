
pub mod base {
    use thiserror::Error;

    pub type GenericError = anyhow::Error;
    pub type GenericResult<T> = anyhow::Result<T>;

    pub type BotResult<T> = Result<T, BotError>;

    #[derive(Debug, Error)]
    pub enum BotError {
        #[error("wrong input error: {0}")]
        UnknownError(i64, String),
    }
}
