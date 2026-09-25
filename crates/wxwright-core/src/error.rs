//! Core error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("theme error: {0}")]
    Theme(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("image error: {0}")]
    Image(String),

    #[error("clipboard error: {0}")]
    Clipboard(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("html rewrite error: {0}")]
    Rewrite(String),

    #[error("upload error: {0}")]
    Upload(String),
}

pub type Result<T> = std::result::Result<T, Error>;
