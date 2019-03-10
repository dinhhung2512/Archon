use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct ArchonError {
    details: String,
}

impl ArchonError {
    pub fn new(msg: &str) -> ArchonError {
        ArchonError { details: msg.to_string() }
    }
}

impl fmt::Display for ArchonError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for ArchonError {
    fn description(&self) -> &str {
        &self.details
    }
}