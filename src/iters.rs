use std::path;

use crate::error::Error;

/// Standard iterator created from a [`Matcher`](./struct.Matcher.html).
///
/// This iterator iterates over all paths recursively without any filter. Use
/// [`IterAll::filter_entry`] to create a more efficient [`IterFilter`] out of this iterator,
/// e.g., do not walk any hidden folders such as `.git`.
#[derive(Debug)]
pub struct IterAll<P>
where
    P: AsRef<path::Path>,
{
    root: P,
    iter: walkdir::IntoIter,
    matcher: globset::GlobMatcher,
}

impl<P> IterAll<P>
where
    P: AsRef<path::Path>,
{
    pub(crate) fn new(
        root: P,
        iter: walkdir::IntoIter,
        matcher: globset::GlobMatcher,
    ) -> IterAll<P> {
        IterAll {
            root,
            iter,
            matcher,
        }
    }
}

/// Helper function for a consistent implementation of the `next` functions for
/// [`IterAll`] and [`IterFilter`].
fn match_next<P>(
    root: P,
    next: Option<Result<walkdir::DirEntry, walkdir::Error>>,
    matcher: &globset::GlobMatcher,
) -> Option<Option<Result<path::PathBuf, Error>>>
where
    P: AsRef<path::Path>,
{
    match next {
        None => Some(None),
        Some(res) => match res {
            Ok(dir) => {
                // assuming that walkdir doesn't create any paths that do not have the provided
                // prefix we can simply exclude such paths since matching on them will anyhow
                // be impossible
                let p = dir.path().strip_prefix(root).ok()?;
                // println!("checking {:?} -- {}", p, matcher.is_match(p));

                if matcher.is_match(p) {
                    return Some(Some(Ok(path::PathBuf::from(dir.path()))));
                }
                None // iterator should continue
            }
            Err(err) => Some(Some(Err(err.into()))),
        },
    }
}

impl<P> Iterator for IterAll<P>
where
    P: AsRef<path::Path>,
{
    type Item = Result<path::PathBuf, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match match_next(&self.root, self.iter.next(), &self.matcher) {
                None => continue,
                Some(entry) => {
                    return entry;
                }
            };
        }
    }
}

impl<P> IterAll<P>
where
    P: AsRef<path::Path>,
{
    /// Transform the iterator into a [`IterFilter`] using the given predicate.
    ///
    /// The provided predicate allows to efficiently filter any paths that should not be walked.
    /// The iterator will completely skip any path and its sub-paths that do not satisfy the given
    /// iterator.
    pub fn filter_entry<PrePath>(
        self,
        mut predicate: PrePath,
    ) -> IterFilter<walkdir::IntoIter, P, impl FnMut(&walkdir::DirEntry) -> bool>
    where
        PrePath: FnMut(&path::Path) -> bool,
    {
        // TODO: instead of creating an IterFilter it should be possible to swap out the
        // implementation and return an IterAll<walkdir::FilterEntry> ?
        IterFilter {
            root: self.root,
            iter: self.iter.filter_entry(move |entry| predicate(entry.path())),
            matcher: self.matcher,
        }
    }
}

/// Filtered iterator created via [`IterAll::filter_entry`].
///
/// This iterator iterates over all paths recursively but applies the configured predicate
/// to all paths.
#[derive(Debug)]
pub struct IterFilter<I, P, PreDir>
where
    PreDir: FnMut(&walkdir::DirEntry) -> bool,
    P: AsRef<path::Path>,
{
    root: P,
    iter: walkdir::FilterEntry<I, PreDir>,
    matcher: globset::GlobMatcher,
}

impl<PreDir, P> Iterator for IterFilter<walkdir::IntoIter, P, PreDir>
where
    PreDir: FnMut(&walkdir::DirEntry) -> bool,
    P: AsRef<path::Path>,
{
    type Item = Result<path::PathBuf, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match match_next(&self.root, self.iter.next(), &self.matcher) {
                None => continue,
                Some(entry) => {
                    return entry;
                }
            };
        }
    }
}
