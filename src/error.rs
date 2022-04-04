use std::fmt;
use std::io;

/// Simple error type used by this facade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error(String);

impl Error {
    pub fn new(err: &str) -> Error {
        Error(err.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl From<walkdir::Error> for Error {
    fn from(item: walkdir::Error) -> Self {
        if let Some(path) = item.path() {
            let common = format!("Failed to walk path {}", path.to_string_lossy());

            if let Some(inner) = item.io_error() {
                return match inner.kind() {
                    io::ErrorKind::InvalidData => {
                        Error(format!("{}: Invalid data encountered: {}", common, inner))
                    }
                    io::ErrorKind::PermissionDenied => Error(format!(
                        "{}: Missing permissions to read entry: {}",
                        common, inner
                    )),
                    _ => Error(format!("{}: Unexpected error occurred: {}", common, inner)),
                };
            }
            return Error(format!("{}: Unknown error occurred", common));
        }
        Error("<unknown-path>: Unknown error occurred".to_string())
    }
}
