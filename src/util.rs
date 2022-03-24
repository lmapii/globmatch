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
) -> Result<(path::PathBuf, String), io::Error>
where
    P: AsRef<path::Path>,
{
    let mut found = false;
    let mut root = path::PathBuf::from(prefix.as_ref());
    let mut rest = path::PathBuf::new();

    if pattern.len() == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Empty pattern"));
    }

    if !root.as_path().exists() {
        return Err(io::Error::from(io::ErrorKind::NotFound));
    }

    if path::Path::new(pattern).is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("'{}' is an absolute path", pattern),
        ));
    }

    println!("resolve root for {:?} -> {}", prefix.as_ref(), pattern);
    for c in path::PathBuf::from(pattern).components() {
        root.push(c);
        println!("  ? {:?}", root);
        if !root.as_path().exists() {
            root.pop();
            found = true;
        }
        if !found {
            rest.push(c);
        }
    }
    println!(
        " -- found: {} {:?}, rest is {}",
        found,
        root,
        rest.to_str().unwrap()
    );

    // TODO: if we did push and the new pattern starts with a RootDir then we have to place
    // a "**" in front of it in oder to match anything
    // TODO: the glob should be the combined path and NOT just the rest (!)
    // but that is not possible since the glob is a &str and root is a path.

    // notice that calling unwrap() is safe since we created the PathBuf from the pattern,
    // which is a &str, and we only removed components so far
    let rest = &pattern[rest.to_str().unwrap().len()..];

    if found && path::Path::new(&rest).has_root() {
        // this seems to be impossible ...
        // TODO: why can i not create a &str with a well defined lifetime?
        // let pattern: &'a str = format!("**{}", rest_str).as_str();
        // this is also not possible since the value is owned by the function
        // let pattern = String::from(rest_str);
        let rest = "**".to_owned() + rest;
        // star.push_str(rest);
        return Ok((root, rest));
    }

    Ok((root, rest.to_string()))
}

pub fn is_hidden_entry<P>(path: P) -> bool
where
    P: AsRef<path::Path>,
{
    let is_hidden = path
        .as_ref()
        .file_name()
        .unwrap_or_else(|| path.as_ref().as_os_str())
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false);

    // println!(
    //     "check hidden: {} - {}",
    //     path.as_ref().to_string_lossy(),
    //     is_hidden
    // );
    is_hidden
}

pub fn is_hidden_path<P>(path: P) -> bool
where
    P: AsRef<path::Path>,
{
    let has_hidden = path.as_ref().components().find_map(|c| {
        match c
            .as_os_str()
            .to_str()
            .map(|s| s.starts_with("."))
            .unwrap_or(false)
        {
            true => Some(c),
            _ => None,
        }
    });

    let is_hidden = match has_hidden {
        None => false,
        _ => true,
    };
    is_hidden
}
