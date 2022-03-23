use std::io;
use std::path;

/// Resolves the root for the pattern and the given path prefix.
///
/// E.g., for the prefix `/home/some/folder` and pattern `../../*.c` this function will resolve
/// the root folder to `/home` and removes the relative path components from the pattern, resulting
/// in the leftover pattern `*.c`.
///
/// Both, the resolved root path and the remaining pattern are provided as tuple `Some(root, rest)`.
/// If the provided `prefix` is not a valid path this function returns an error.
pub fn resolve_root<'a, P>(
    prefix: P,
    pattern: &'a str,
) -> Result<(path::PathBuf, &'a str), io::Error>
where
    P: AsRef<path::Path>,
{
    let mut found = false;
    let mut root = path::PathBuf::from(prefix.as_ref());
    let mut rest = path::PathBuf::new();

    if !root.as_path().exists() {
        return Err(io::Error::from(io::ErrorKind::NotFound));
    }

    // TODO: there must be a better solution to consume "the remainder" via iterators ?
    for c in path::PathBuf::from(pattern).components() {
        root.push(c);
        if !root.as_path().exists() {
            root.pop();
            found = true;
        }
        if !found {
            rest.push(c);
        }
    }
    // notice that calling unwrap() is safe since we created the PathBuf from the pattern.
    Ok((root, &pattern[rest.to_str().unwrap().len()..]))
}

pub fn is_hidden<P>(path: P) -> bool
where
    P: AsRef<path::Path>,
{
    path.as_ref()
        .file_name()
        .unwrap_or_else(|| path.as_ref().as_os_str())
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}
