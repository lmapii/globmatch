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
    let mut root = path::PathBuf::from(prefix.as_ref());
    let mut rest = path::PathBuf::new();

    if pattern.is_empty() {
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

    // try to found a common root path from which the recursive search would start. notice that
    // it may happen that the relative path component of the pattern is not a valid path, e.g.,
    // the prefix might go back many levels and then to a folder that doesn not exist. such
    // an error is not caught here since we do not differ between path names and patterns and will
    // only lead to zero matches during the matching procedure.

    // println!("resolve root for {:?} -> {}", prefix.as_ref(), pattern);
    let mut push_root = true;
    path::Path::new(pattern).components().for_each(|c| {
        if push_root {
            root.push(c);

            // notice that a path exists even if the number of "../" is beyond the root.
            // thus all superfluous "../" will simply be consumed by this iterator.
            if !root.exists() {
                root.pop();
                rest.push(c);
                push_root = false;
            }
        } else {
            rest.push(c);
        }
    });

    // println!(" -- root {:?}\n    rest {}", root, rest.to_str().unwrap());
    rest.components().map_while(|c| match c {
        path::Component::ParentDir => None,
        _ => Some(()),
    });

    // let rest = &pattern[rest.to_str().unwrap().len()..];
    // notice that calling unwrap() is safe since we created the PathBuf from the pattern,
    let rest = rest.to_str().unwrap().to_string();

    // TODO: check remainder for relative path components (not allowed)

    // in case we moved parts from our pattern into the root (because there was a relative path
    // component) we now need to pre-pend "**" since this might not have been part of the original
    // pattern. e.g.,
    // `root = /home/someone/a/b/c`, `pattern = ../../*.txt` would be resolved to
    // `root = /home/someone/a/`, `pattern = *.txt`
    // clearly applying the pattern `*.txt` to these search paths will do us no good since it
    // won't match any full paths.

    // if rest.as_str() != pattern {
    //     if path::Path::new(rest.as_str()).has_root() {
    //         rest = "**".to_owned() + &rest;
    //     } else {
    //         rest = "**/".to_owned() + &rest;
    //     }
    // }

    Ok((root, rest))
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

#[cfg(test)]
mod tests {
    // use super::*;

    use super::resolve_root;
    use std::path;

    #[test]
    /// This test just demonstrates that this crate "gracefully" handles relative paths that
    /// would go outside of the file system (go back more levels than exist in the actual path)
    /// just like `ls` does: `ls` will return the root path (`/` on unix) in case a relative
    /// path goes back too many levels.
    fn outside_root() -> Result<(), std::io::Error> {
        let root = path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let levels = vec!["../"; root.components().count() * 2];
        let pattern = levels.join("") + "*.txt";

        let (root, rest) = resolve_root(root, pattern.as_str())?;
        println!("root-rest {:?}  {:?}", root.canonicalize(), rest);
        Ok(())
    }

    #[test]
    fn dummy_b() -> Result<(), std::io::Error> {
        let root = format!("{}{}", env!("CARGO_MANIFEST_DIR"), "/test-files/a");
        let pattern = "../../../../../../../../../../../*.txt";

        let (root, rest) = resolve_root(root, pattern)?;
        println!("root-rest {:?}  {:?}", root.canonicalize(), rest);
        Ok(())
    }
}
