//! Node.js-style filesystem error rendering.
//!
//! The TS tools embed `error.message` from `fs/promises` in their results, e.g.
//! `ENOENT: no such file or directory, open '/x'`, and `JSON.stringify` of the error
//! (used for the `cause` metadata) yields `{errno, code, syscall, path[, dest]}`. The model
//! sees both, so they are reproduced from `std::io::Error`.

use serde_json::{json, Map, Value};
use std::io;

/// An `std::io::Error` annotated with the Node syscall name and path(s).
#[derive(Debug, Clone)]
pub struct NodeIoError {
    pub code: String,
    pub errno: i64,
    pub message: String,
    pub syscall: String,
    pub path: String,
    pub dest: Option<String>,
}

impl NodeIoError {
    pub fn new(err: &io::Error, syscall: &str, path: &str, dest: Option<&str>) -> Self {
        let (code, desc) = code_and_description(err);
        let errno = err.raw_os_error().map(|n| -(n as i64)).unwrap_or(-1);
        let message = match dest {
            Some(d) => format!("{code}: {desc}, {syscall} '{path}' -> '{d}'"),
            None => format!("{code}: {desc}, {syscall} '{path}'"),
        };
        NodeIoError {
            code: code.to_string(),
            errno,
            message,
            syscall: syscall.to_string(),
            path: path.to_string(),
            dest: dest.map(str::to_string),
        }
    }

    /// `JSON.stringify(error)` for a Node system error.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("errno".into(), json!(self.errno));
        m.insert("code".into(), json!(self.code));
        m.insert("syscall".into(), json!(self.syscall));
        m.insert("path".into(), json!(self.path));
        if let Some(d) = &self.dest {
            m.insert("dest".into(), json!(d));
        }
        Value::Object(m)
    }
}

impl std::fmt::Display for NodeIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// libuv error code name and description for an io error.
fn code_and_description(err: &io::Error) -> (&'static str, &'static str) {
    #[cfg(unix)]
    if let Some(raw) = err.raw_os_error() {
        let known = match raw {
            libc::ENOENT => Some(("ENOENT", "no such file or directory")),
            libc::EACCES => Some(("EACCES", "permission denied")),
            libc::EEXIST => Some(("EEXIST", "file already exists")),
            libc::EISDIR => Some(("EISDIR", "illegal operation on a directory")),
            libc::ENOTDIR => Some(("ENOTDIR", "not a directory")),
            libc::ENOTEMPTY => Some(("ENOTEMPTY", "directory not empty")),
            libc::EXDEV => Some(("EXDEV", "cross-device link not permitted")),
            libc::EPERM => Some(("EPERM", "operation not permitted")),
            libc::EBUSY => Some(("EBUSY", "resource busy or locked")),
            libc::EMFILE => Some(("EMFILE", "too many open files")),
            libc::ENOSPC => Some(("ENOSPC", "no space left on device")),
            libc::EROFS => Some(("EROFS", "read-only file system")),
            libc::EINVAL => Some(("EINVAL", "invalid argument")),
            libc::ELOOP => Some(("ELOOP", "too many symbolic links encountered")),
            libc::ENAMETOOLONG => Some(("ENAMETOOLONG", "name too long")),
            _ => None,
        };
        if let Some(k) = known {
            return k;
        }
    }
    use io::ErrorKind::*;
    match err.kind() {
        NotFound => ("ENOENT", "no such file or directory"),
        PermissionDenied => ("EACCES", "permission denied"),
        AlreadyExists => ("EEXIST", "file already exists"),
        IsADirectory => ("EISDIR", "illegal operation on a directory"),
        NotADirectory => ("ENOTDIR", "not a directory"),
        DirectoryNotEmpty => ("ENOTEMPTY", "directory not empty"),
        CrossesDevices => ("EXDEV", "cross-device link not permitted"),
        InvalidInput => ("EINVAL", "invalid argument"),
        _ => ("EIO", "i/o error"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enoent_message_matches_node() {
        let err = std::fs::read("/definitely/not/here").unwrap_err();
        let e = NodeIoError::new(&err, "open", "/definitely/not/here", None);
        assert_eq!(
            e.message,
            "ENOENT: no such file or directory, open '/definitely/not/here'"
        );
        assert_eq!(e.to_json()["code"], "ENOENT");
        #[cfg(unix)]
        assert_eq!(e.errno, -2);
    }

    #[test]
    fn two_path_message() {
        let err = io::Error::from(io::ErrorKind::NotFound);
        let e = NodeIoError::new(&err, "rename", "/a", Some("/b"));
        assert_eq!(
            e.message,
            "ENOENT: no such file or directory, rename '/a' -> '/b'"
        );
        assert_eq!(e.to_json()["dest"], "/b");
    }
}
