use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("internal error")]
    Internal,
}

pub type Result<T> = std::result::Result<T, Error>;
