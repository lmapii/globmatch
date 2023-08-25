use std::io;
use std::path;

/// Resolves the root for the pattern and the given path prefix.
///
/// E.g., for the prefix `/home/some/folder` and pattern `../../*.c` this function will resolve
/// the root folder to `/home/some/folder/../../` and removes the relative path components from
/// the pattern, resulting in the remainder `*.c`.
///
/// Both, the resolved root path and the remaining pattern are provided as tuple `Some(root, rest)`.
/// If the provided `prefix` is not a valid path this function returns an `io::Error`.
#[allow(clippy::needless_lifetimes)]
pub fn resolve_root<'a, P>(
    prefix: P,
    pattern: &'a str,
) -> Result<(path::PathBuf, &'a str), io::Error>
where
    P: AsRef<path::Path>,
{
    // TODO: is there such a thing as Cow for Path?
    let mut root = path::PathBuf::from(prefix.as_ref());
    let mut rest = path::PathBuf::new();

    if pattern.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "empty pattern"));
    }

    if !root.as_path().exists() {
        return Err(io::Error::from(io::ErrorKind::NotFound));
    }

    if path::Path::new(pattern).is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("'{pattern}' is an absolute path"),
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

    // Workaround for empty patterns: Keep the path component within the pattern such that
    // it will be matched. globset is not able to match empty patterns.
    if rest.components().count() == 0 {
        if let Some(c) = root.components().next_back() {
            if let path::Component::Normal(_) = c {
                rest.push(c);
                root.pop();
            }
        }
    }

    // do not canonicalize the root directory, the user can decide to do this. otherwise
    // matching against patterns that also use relative paths will be impossible.
    // let root = root.canonicalize()?;
    // println!(" -- root {:?}\n    rest {}", root, rest.to_str().unwrap());

    // patterns can have no relative paths (after selectors) since it would be possible to
    // "move" out of the pattern using "../" and changing into a directory outside of root.
    // do not allow such patterns (though the levels could be checked).
    if rest
        .components()
        .any(|c| matches!(c, path::Component::ParentDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "pattern remainder '{}' contains unresolved relative path components",
                rest.to_str().unwrap()
            ),
        ));
    }

    // notice that calling unwrap() is safe since we created the PathBuf from the pattern,
    let rest = &pattern[pattern.len() - rest.to_str().unwrap().len()..];
    Ok((root, rest))
}

/// Transforms the first character of a string to uppercase.
pub(crate) fn to_upper(s: String) -> String {
    let mut c = s.chars();
    match c.next() {
        None => s,
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// Checks if the provided path is a hidden "entry".
///
/// An entry is hidden if its final path component (filename or directory name) starts with a dot,
/// e.g., `.git` or `.clang-format` are hidden entries.
///
/// This function can be used in [IterAll::filter_entry](./struct.IterAll.html#method.filter_entry)
/// to avoid iterating through hidden paths.
pub fn is_hidden_entry<P>(path: P) -> bool
where
    P: AsRef<path::Path>,
{
    let is_hidden = path
        .as_ref()
        .file_name()
        .unwrap_or_else(|| path.as_ref().as_os_str())
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false);
    is_hidden
}

/// Checks if the provided path has a hidden path component.
///
/// A path is hidden if one of its path component (filename or directory name) starts with a dot.
pub fn is_hidden_path<P>(path: P) -> bool
where
    P: AsRef<path::Path>,
{
    let has_hidden = path.as_ref().components().find(|c| {
        c.as_os_str()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
    });

    has_hidden.is_some()
}

#[cfg(test)]
mod tests {
    // use super::*;

    use super::resolve_root;
    use std::{io, path};

    #[test]
    /// This test just demonstrates that this crate "gracefully" handles relative paths that
    /// would go outside of the file system (go back more levels than exist in the actual path)
    /// just like `ls` does: `ls` will return the root path (`/` on unix) in case a relative
    /// path goes back too many levels.
    ///
    /// On windows
    #[cfg_attr(target_os = "windows", ignore)]
    fn outside_root() -> Result<(), std::io::Error> {
        let root = path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let levels = vec!["../"; root.components().count() * 2];
        let pattern = levels.join("") + "*.txt";

        // let root_first = path::Path::new(root.components().next().unwrap().as_os_str());
        // let root_first = root_first
        //     .to_str()
        //     .ok_or(io::Error::from(io::ErrorKind::Other))?;

        let (root, rest) = resolve_root(root, pattern.as_str())?;
        let root = root.canonicalize()?;
        let root = root
            .to_str()
            .ok_or_else(|| io::Error::from(io::ErrorKind::Other))?;

        if !cfg!(windows) {
            // assert_eq!(root, root_first); // cannot test against "/" on windows
            // test demonstrates that we still don't get a panic.
            assert_eq!(root, "/");
            assert_eq!(rest, "*.txt");
        }
        Ok(())
    }

    #[test]
    fn patterns() -> Result<(), String> {
        fn tst(root: &str, pattern: &str, exp_root: &str, exp_pattern: &str) -> Result<(), String> {
            let root = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), root);

            let (root, pattern) = resolve_root(root, pattern).map_err(|err| err.to_string())?;

            let root = root.canonicalize().map_err(|err| err.to_string())?;
            let root = root
                .to_str()
                .ok_or_else(|| io::Error::from(io::ErrorKind::Other))
                .map_err(|err| err.to_string())?;

            // forward slash won't work
            // let exp_root = format!(
            //     "{}{}",
            //     env!("CARGO_MANIFEST_DIR"),
            //     match exp_root {
            //         "" => "".to_string(),
            //         p => format!("/{}", p),
            //     }
            // );

            let exp_root = format!(
                "{}{}",
                env!("CARGO_MANIFEST_DIR"),
                match exp_root {
                    "" => "".to_string(),
                    p => format!("/{p}"),
                }
            );
            let exp_root = path::PathBuf::from(exp_root)
                .canonicalize()
                .map_err(|err| err.to_string())?;
            let exp_root = exp_root
                .to_str()
                .ok_or_else(|| io::Error::from(io::ErrorKind::Other))
                .map_err(|err| err.to_string())?;

            assert_eq!(root, exp_root);
            assert_eq!(pattern, exp_pattern);
            Ok(())
        }
        // notice how a relative path can result in an empty pattern, workaround implemented!
        // err(tst("test-files/c-simple", "../test-files/c-simple", "test-files/c-simple", ""))?;
        tst("test-files/c-simple/a", "../a", "test-files/c-simple", "a")?;

        // if !cfg!(windows) {
        tst(
            "test-files/c-simple",
            "*.txt",
            "test-files/c-simple",
            "*.txt",
        )?;
        // }

        tst("test-files/c-simple/a/a0", "../../../../*.txt", "", "*.txt")?;

        tst(
            "test-files/c-simple/a/a0",
            "a0_0.txt",
            "test-files/c-simple/a/a0",
            "a0_0.txt",
        )?;

        tst(
            "test-files/c-simple/a/a0",
            "../a0/a0_0.txt",
            "test-files/c-simple/a/a0",
            "a0_0.txt",
        )?;
        Ok(())
    }
}
