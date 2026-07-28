use thiserror::Error;

pub mod policy;

#[derive(Debug, Error)]
pub enum Error {
    #[error("internal error")]
    Internal,
}

pub type Result<T> = std::result::Result<T, Error>;
