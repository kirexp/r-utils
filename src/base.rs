
pub mod base {
    #[derive(Debug)]
    pub enum AppError {
        Db(String),
        Network(String),
        // etc...
    }
    pub type GenericError = Box<dyn std::error::Error + Send + Sync + 'static>;
    pub type GenericResult<T> = Result<T, GenericError>;
}
