//! This module implements common usecases.
//!
//! When specifying globs it is sometimes useful to be able to simply specify a set of globs to
//! determine paths, and a common set of filters that are applied to all of these globs. The
//! functions in this module cover this common usecase as demonstrated by the example below.
//!
//! # Example
//!
//! ```
//! /*
//!     Example files:
//!     globmatch/test-files/c-simple/.hidden
//!     globmatch/test-files/c-simple/.hidden/h_1.txt
//!     globmatch/test-files/c-simple/.hidden/h_0.txt
//!     globmatch/test-files/c-simple/a/a2/a2_0.txt
//!     globmatch/test-files/c-simple/a/a0/a0_0.txt
//!     globmatch/test-files/c-simple/a/a0/a0_1.txt
//!     globmatch/test-files/c-simple/a/a0/A0_3.txt
//!     globmatch/test-files/c-simple/a/a0/a0_2.md
//!     globmatch/test-files/c-simple/a/a1/a1_0.txt
//!     globmatch/test-files/c-simple/some_file.txt
//!     globmatch/test-files/c-simple/b/b_0.txt
//!  */
//!
//! use globmatch;
//!
//! # fn example_usecase() -> Result<(), String> {
//! let root = env!("CARGO_MANIFEST_DIR");
//! let patterns = vec![
//!     "test-files/c-simple/**/[aA]*.txt",
//!     "test-files/c-simple/**/*.md",
//! ];
//!
//! let filter_entry = Some(vec![".*"]);
//! let filter_post = Some(vec![
//!     "test-files/c-simple/**/a1/*.txt",
//!     "test-files/c-simple/**/a0/*.*",
//! ]);
//!
//! let candidates = globmatch::wrappers::build_matchers(&patterns, &root)?;
//! let filter_pre = globmatch::wrappers::build_glob_set(&filter_entry, false)?;
//! let filter_post = globmatch::wrappers::build_glob_set(&filter_post, false)?;
//! let (paths, filtered) = globmatch::wrappers::match_paths(candidates, filter_pre, filter_post);
//!
//! /*
//! paths = [
//!     "/test-files/c-simple/a/a2/a2_0.txt"
//! ];
//! filtered = [
//!     "/test-files/c-simple/a/a0/A3_0.txt",
//!     "/test-files/c-simple/a/a0/a0_0.txt",
//!     "/test-files/c-simple/a/a0/a0_1.txt",
//!     "/test-files/c-simple/a/a0/a0_2.md",
//!     "/test-files/c-simple/a/a1/a1_0.txt",
//! ];
//! */
//!
//! assert_eq!(1, paths.len());
//! assert_eq!(5, filtered.len());
//! # Ok(())
//! # }
//! # example_usecase().unwrap();
//! ```

use std::path;

use crate::{utils, Builder, GlobSet, Matcher};

fn extract_patterns<T>(candidates: Vec<Result<T, String>>) -> Result<Vec<T>, String> {
    let failures: Vec<_> = candidates
        .iter()
        .filter_map(|f| match f {
            Ok(_) => None,
            Err(e) => Some(e),
        })
        .collect();

    if !failures.is_empty() {
        return Err(format!(
            "Failed to compile patterns: \n{}",
            failures
                .iter()
                .map(|err| err.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        ));
    }
    Ok(candidates.into_iter().flatten().collect())
}

/// Builds a set of [`Matcher`]s for the list of `globs` relative to `root`.
///
/// This function creates multiple [`Matcher`]s by calling the [`Builder::build`] for each of the
/// provided globs. It then checks if any failures have occured while building the [`Matcher`]
/// instances; if for any of the provided globs the build fails an error is returned.
///
/// # Errors
///
/// Refer to [`Builder::build`]. Error checks are performed for each glob.
pub fn build_matchers<'a, P>(
    globs: &[&'a str],
    root: P,
) -> Result<Vec<Matcher<'a, path::PathBuf>>, String>
where
    P: AsRef<path::Path>,
{
    let candidates: Vec<Result<_, String>> = globs
        .iter()
        .map(|pattern| {
            Builder::new(pattern)
                .case_sensitive(!cfg!(windows))
                .build(root.as_ref())
        })
        .collect();

    let candidates = extract_patterns(candidates)?;
    Ok(candidates)
}

/// Builds a set of [`GlobSet`]s for the list of provided `paths`.
///
/// This function creates multiple [`GlobSet`]s by calling the [`Builder::build_glob_set`] function
/// for each provided glob. It then checks if any failures have occured while building the
/// [`GlobSet`] instances; if for any of the provided paths the build fails an error is returned.
///
/// # Errors
///
/// Refer to [`Builder::build_glob_set`]. Error checks are performed for each glob.
pub fn build_glob_set<'a>(
    paths: &Option<Vec<&'a str>>,
    case_sensitive: bool,
) -> Result<Option<Vec<GlobSet<'a>>>, String> {
    let paths = match paths {
        None => None,
        Some(paths_) => {
            let candidates: Vec<Result<_, String>> = paths_
                .iter()
                .map(|pattern| {
                    Builder::new(pattern)
                        .case_sensitive(case_sensitive)
                        .build_glob_set()
                })
                .collect();
            Some(extract_patterns(candidates)?)
        }
    };
    Ok(paths)
}

/// Collects all paths using a set of [`Matcher`]s and optional filters.
///
/// This function iterates over all `candidates` to resolve the paths for each [`Matcher`] in the
/// list of candidates. A common set of filters is applied to each candidate.
///
/// # Filters
///
/// The optional `filter_entry` will be passed to the [`crate::IterAll::filter_entry`] call,
/// filtering files and folders *before* matching any of the paths of each candidate. If no
/// `filter_entry` is provided this function filters all hidden paths by applying the
/// [`crate::is_hidden_entry`] utility function.
///
/// The optional `filter_post` is used to apply a filter *after* matching the paths.
pub fn match_paths<P>(
    candidates: Vec<Matcher<'_, P>>,
    filter_entry: Option<Vec<GlobSet<'_>>>,
    filter_post: Option<Vec<GlobSet<'_>>>,
) -> (Vec<path::PathBuf>, Vec<path::PathBuf>)
where
    P: AsRef<path::Path>,
{
    let mut filtered = vec![];

    let paths = candidates
        .into_iter()
        .flat_map(|m| {
            m.into_iter()
                .filter_entry(|path| {
                    match &filter_entry {
                        // yield all entries if no pattern have been provided
                        // but try_for_each yields all elements for an empty vector (see test)
                        // Some(patterns) if patterns.is_empty() => true,
                        // Some(patterns) if !patterns.is_empty() => {
                        Some(patterns) => {
                            let do_filter = patterns
                                .iter()
                                .try_for_each(|glob| match glob.is_match(path) {
                                    true => None,      // path is a match, abort on first match
                                    false => Some(()), // path is not a match, continue with 'ok'
                                })
                                .is_none(); // the value remains "Some" if no match was encountered
                            !do_filter
                        }
                        _ => !utils::is_hidden_entry(path), // yield entries that are not hidden
                    }
                })
                .flatten()
                .collect::<Vec<_>>()
        })
        // .filter(|path| path.as_path().is_file()) // accept only files
        .filter(|path| match &filter_post {
            None => true,
            Some(patterns) => {
                let do_filter = patterns
                    .iter()
                    .try_for_each(|glob| match glob.is_match(path) {
                        true => None,      // path is a match, abort on first match in filter_post
                        false => Some(()), // path is not a match, continue with 'ok'
                    })
                    .is_none(); // the value remains "Some" if no match was encountered
                if do_filter {
                    filtered.push(path::PathBuf::from(path));
                }
                !do_filter
            }
        });

    let mut paths: Vec<_> = paths.collect();
    paths.sort_unstable();
    paths.dedup();

    filtered.sort_unstable();
    filtered.dedup();

    (paths, filtered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_foreach() {
        let items = vec![0u8, 1u8, 2u8];
        let filter: Vec<u8> = vec![];

        // show that an empty filter list yields all elements
        let filter_zero: Vec<_> = items
            .iter()
            .filter(|item| {
                let do_filter = filter
                    .iter()
                    .try_for_each(|filter_item| {
                        if *filter_item == **item {
                            None // abort on first match
                        } else {
                            Some(()) // no match, continue
                        }
                    })
                    .is_none(); // the value remains "Some" if no match was encountered
                !do_filter
            })
            .cloned()
            .collect();

        assert_eq!(filter_zero, items);
    }

    #[test]
    fn test_usecase() -> Result<(), String> {
        fn log_paths<P>(paths: &[P])
        where
            P: AsRef<path::Path>,
        {
            println!(
                "paths:\n{}",
                paths
                    .iter()
                    .map(|p| format!("{}", p.as_ref().to_string_lossy()))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }

        let root = env!("CARGO_MANIFEST_DIR");
        let patterns = vec![
            "test-files/c-simple/**/[aA]*.txt",
            "test-files/c-simple/**/*.md",
        ];
        let filter_entry = Some(vec![".*"]);

        let filter_post = Some(vec![
            "test-files/c-simple/**/a1/*.txt",
            "test-files/c-simple/**/a0/*.*",
        ]);

        let candidates = build_matchers(&patterns, root)?;
        let filter_pre = build_glob_set(&filter_entry, !cfg!(windows))?;
        let filter_post = build_glob_set(&filter_post, !cfg!(windows))?;

        let (paths, filtered) = match_paths(candidates, filter_pre, filter_post);

        log_paths(&paths);
        log_paths(&filtered);

        // paths = [
        //     "/test-files/c-simple/a/a2/a2_0.txt"
        // ];
        // filtered = [
        //     "/test-files/c-simple/a/a0/A3_0.txt",
        //     "/test-files/c-simple/a/a0/a0_0.txt",
        //     "/test-files/c-simple/a/a0/a0_1.txt",
        //     "/test-files/c-simple/a/a0/a0_2.md",
        //     "/test-files/c-simple/a/a1/a1_0.txt",
        // ];

        assert_eq!(1, paths.len());
        assert_eq!(5, filtered.len());
        Ok(())
    }
}
