use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),

    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    #[error("malformed CSDL: {0}")]
    Csdl(String),
}

pub type Result<T> = std::result::Result<T, Error>;
