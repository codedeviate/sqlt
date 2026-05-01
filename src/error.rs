use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("parse error: {0}")]
    Parse(#[from] sqlparser::parser::ParserError),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("unknown dialect: {0}")]
    UnknownDialect(String),

    #[error("unknown encoding: {0}")]
    UnknownEncoding(String),

    #[error("encoding error: {0}")]
    Encoding(String),

    #[error("translation produced warnings (--strict)")]
    StrictWarnings,
}

pub type Result<T> = std::result::Result<T, Error>;
