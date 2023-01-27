use std::fmt;
use std::io;

/// Simple error type used by this facade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error(String);

impl Error {
    /// Creates a new error string.
    pub fn new(err: &str) -> Error {
        Error(err.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<walkdir::Error> for Error {
    fn from(item: walkdir::Error) -> Self {
        if let Some(path) = item.path() {
            let common = format!("Failed to walk path {}", path.to_string_lossy());

            if let Some(inner) = item.io_error() {
                return match inner.kind() {
                    io::ErrorKind::InvalidData => {
                        Error(format!("{common}: Invalid data encountered: {inner}"))
                    }
                    io::ErrorKind::PermissionDenied => Error(format!(
                        "{common}: Missing permissions to read entry: {inner}"
                    )),
                    _ => Error(format!("{common}: Unexpected error occurred: {inner}")),
                };
            }
            return Error(format!("{common}: Unknown error occurred"));
        }
        Error("<unknown-path>: Unknown error occurred".to_string())
    }
}
