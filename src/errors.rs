use lalrpop_util::{lexer::Token, ParseError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("LockGiveUpError")]
    LockGiveUpError,

    #[error("BufferAbortError")]
    BufferAbortError,

    #[error("TooSmallBlockError: (block_size: {0}, record_size: {1})")]
    TooSmallBlockError(usize, usize),

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_conversion() {
        let parse_error: ParseError<usize, Token<'_>, &str> = ParseError::UnrecognizedToken {
            token: (0, Token(0, "foo"), 3),
            expected: vec!["bar".to_string()],
        };

        let query_error = QueryError::from(parse_error);
        match query_error {
            QueryError::ParseError(inner) => {
                assert_eq!(
                    inner,
                    ParseError::UnrecognizedToken {
                        token: (0, "foo".to_string(), 3),
                        expected: vec!["bar".to_string()],
                    }
                );
            }
        }
    }
}
