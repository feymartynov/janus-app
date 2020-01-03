use std::error::Error as StdError;
use std::fmt::{self, Display};

#[derive(Debug)]
pub struct Error(String);

impl Error {
    pub fn new(detail: &str) -> Self {
        Self(detail.to_owned())
    }
}

impl StdError for Error {}

impl Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, fmt)
    }
}
