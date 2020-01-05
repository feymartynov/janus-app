use std::error::Error as StdError;
use std::fmt::{self, Display};

#[derive(Debug)]
pub struct Error(String);

impl Error {
    pub fn new(detail: &str) -> Self {
        Self(detail.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl StdError for Error {}

impl Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, fmt)
    }
}
