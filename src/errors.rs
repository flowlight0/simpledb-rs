use lalrpop_util::{lexer::Token, ParseError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("LockGiveUpError")]
    LockGiveUpError,

    #[error("BufferAbortError")]
    BufferAbortError,

    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum QueryError {
    #[error("ParseError: {0}")]
    ParseError(ParseError<usize, String, String>),
}

impl From<ParseError<usize, Token<'_>, &str>> for QueryError {
    fn from(value: ParseError<usize, Token, &str>) -> Self {
        QueryError::ParseError(
            value
                .map_token(|token| token.1.to_string())
                .map_error(|error| error.to_string()),
        )
    }
}

#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("TransactionError: {0}")]
    TransactionError(#[from] TransactionError),

    #[error("QueryError: {0}")]
    QueryError(#[from] QueryError),
}
