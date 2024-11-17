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
