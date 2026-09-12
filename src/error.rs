use thiserror::Error;

#[derive(Debug, Error)]
#[error("{code}: {message}")]
pub struct Error {
    pub code: &'static str,
    pub message: String,
}
pub type Result<T> = std::result::Result<T, Error>;
impl Error {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::new(
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                "PERMISSION_DENIED"
            } else {
                "IO_ERROR"
            },
            e.to_string(),
        )
    }
}
impl From<rustix::io::Errno> for Error {
    fn from(e: rustix::io::Errno) -> Self {
        let code = match e {
            rustix::io::Errno::LOOP => "SYMLINK_REJECTED",
            rustix::io::Errno::XDEV => "CROSS_FILESYSTEM",
            rustix::io::Errno::EXIST => "DESTINATION_EXISTS",
            rustix::io::Errno::ACCESS | rustix::io::Errno::PERM => "PERMISSION_DENIED",
            _ => "IO_ERROR",
        };
        Self::new(code, e.to_string())
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Self::new("DATABASE_ERROR", e.to_string())
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::new("INVALID_DATA", e.to_string())
    }
}
