use globset;
use std::path;

pub use crate::error::Error;

mod error;
mod util;

pub struct Builder<'a> {
    glob: &'a str,
    case_insensitive: bool,
}

impl<'a> Builder<'a> {
    pub fn new(glob: &'a str) -> Builder<'a> {
        Builder {
            glob,
            case_insensitive: true,
        }
    }

    pub fn case_insensitive(&mut self, yes: bool) -> &mut Builder<'a> {
        self.case_insensitive = yes;
        self
    }

    pub fn build<P>(&self, root: P) -> Result<Matcher<'a, path::PathBuf>, String>
    where
        P: AsRef<path::Path>,
    {
        let (root, rest) = util::resolve_root(root, self.glob)
            .map_err(|err| format!("Root folder not found: {}", err))?;
        let matcher = globset::GlobBuilder::new(rest)
            .case_insensitive(self.case_insensitive)
            .build()
            .map_err(|err| format!("{}: {}", self.glob.to_string(), err.kind().to_string(),))?
            .compile_matcher();
        Ok(Matcher {
            glob: self.glob,
            root,
            rest,
            matcher,
        })
    }
}

pub struct Matcher<'a, P>
where
    P: AsRef<path::Path>,
{
    glob: &'a str, // original glob-pattern
    root: P,       // root path of a resolved pattern
    rest: &'a str, // remaining pattern after root has been resolved
    matcher: globset::GlobMatcher,
}

impl<'a, P> IntoIterator for Matcher<'a, P>
where
    P: AsRef<path::Path>,
{
    type Item = Result<path::PathBuf, Error>;
    type IntoIter = IterAll;

    fn into_iter(self) -> Self::IntoIter {
        println!(
            "matching {} -> {} (original {})",
            self.root.as_ref().to_string_lossy(),
            self.rest,
            self.glob
        );
        IterAll::new(walkdir::WalkDir::new(self.root).into_iter(), self.matcher)
    }
}

impl<'a, P> Matcher<'a, P>
where
    P: AsRef<path::Path>,
{
    pub fn glob(&self) -> &str {
        self.glob
    }
    pub fn root(&self) -> String {
        let path = path::PathBuf::from(self.root.as_ref());
        String::from(path.to_str().unwrap())
    }
    pub fn rest(&self) -> &str {
        self.rest
    }
}

pub struct IterAll {
    iter: walkdir::IntoIter,
    matcher: globset::GlobMatcher,
}

impl IterAll {
    fn new(iter: walkdir::IntoIter, matcher: globset::GlobMatcher) -> IterAll {
        IterAll { iter, matcher }
    }
}

impl Iterator for IterAll {
    type Item = Result<path::PathBuf, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let entry = match self.iter.next() {
                None => None,
                Some(res) => match res {
                    Ok(dir) => {
                        println!("IterAll: matching {}", dir.path().to_string_lossy());
                        let p = path::PathBuf::from(dir.path());
                        if self.matcher.is_match(dir.path()) {
                            return Some(Ok(p));
                        }
                        continue; // don't list files that didn't match'
                    }
                    Err(err) => Some(Err(err.into())),
                },
            };
            return entry;
            //return Some(Ok((dent, is_match)));
        }
    }
}

impl IterAll {
    pub fn filter_entry<Q>(
        self,
        mut predicate: Q,
    ) -> IterFilter<walkdir::IntoIter, impl FnMut(&walkdir::DirEntry) -> bool>
    where
        Q: FnMut(&path::Path) -> bool,
    {
        IterFilter {
            iter: self.iter.filter_entry(move |entry| predicate(entry.path())),
            matcher: self.matcher,
        }
    }
}

pub struct IterFilter<I, P>
where
    P: FnMut(&walkdir::DirEntry) -> bool,
{
    iter: walkdir::FilterEntry<I, P>,
    matcher: globset::GlobMatcher,
}

impl<P> Iterator for IterFilter<walkdir::IntoIter, P>
where
    P: FnMut(&walkdir::DirEntry) -> bool,
{
    type Item = Result<path::PathBuf, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let entry = match self.iter.next() {
                None => None,
                Some(res) => match res {
                    Ok(dir) => {
                        // TODO: some way to have the original IterFilter iterate?
                        println!("IterFilter: matching {}", dir.path().to_string_lossy());
                        let p = path::PathBuf::from(dir.path());
                        if self.matcher.is_match(dir.path()) {
                            return Some(Ok(p));
                        }
                        continue; // don't list files that didn't match'
                    }
                    Err(err) => Some(Err(err.into())),
                },
            };
            return entry;
        }
    }
}

// TODO: calling filter_entry twice won't work
// impl<P> IterFilter<walkdir::IntoIter, P>
// where
//     P: FnMut(&walkdir::DirEntry) -> bool,
// {
//     pub fn filter_entry<Q>(
//         self,
//         mut predicate: Q,
//     ) -> IterFilter<
//         walkdir::FilterEntry<walkdir::IntoIter, impl FnMut(&walkdir::DirEntry) -> bool>,
//         impl FnMut(&walkdir::DirEntry) -> bool,
//     >
//     where
//         Q: FnMut(&path::Path) -> bool,
//     {
//         IterFilter {
//             iter: self
//                 .iter
//                 .filter_entry(move |entry: &walkdir::DirEntry| predicate(entry.path())),
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_build() -> Result<(), String> {
        let root = env!("CARGO_MANIFEST_DIR");
        let pattern = "**/*.txt";

        let _builder = Builder::new(pattern).case_insensitive(true).build(root)?;
        Ok(())
    }

    #[test]
    fn builder_err() -> Result<(), String> {
        let root = env!("CARGO_MANIFEST_DIR");
        let pattern = "a[";

        match Builder::new(pattern).build(root) {
            Ok(_) => Err("Expected pattern to fail".to_string()),
            Err(_) => Ok(()),
        }
    }

    #[test]
    fn match_all() -> Result<(), String> {
        let root = env!("CARGO_MANIFEST_DIR");
        let pattern = "test-files/**/*.txt";

        let builder = Builder::new(pattern).build(root)?;
        let paths: Vec<_> = builder.into_iter().flatten().collect();

        println!(
            "paths \n{}",
            paths
                .iter()
                .map(|p| format!("{}", p.to_string_lossy()))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert_eq!(4, paths.len());
        Ok(())
    }

    #[test]
    fn match_filter() -> Result<(), String> {
        let root = env!("CARGO_MANIFEST_DIR");
        let pattern = "test-files/**/*.txt";

        let builder = Builder::new(pattern).build(root)?;
        let paths: Vec<_> = builder
            .into_iter()
            .filter_entry(|p| !{
                p.file_name()
                    .unwrap_or_else(|| p.as_os_str())
                    .to_str()
                    .map(|s| s.starts_with("."))
                    .unwrap_or(false)
            })
            // TODO: can't have filter_entry twice
            // .filter_entry(|p| !{
            //     p.file_name()
            //         .unwrap_or_else(|| p.as_os_str())
            //         .to_str()
            //         .map(|s| s.starts_with("."))
            //         .unwrap_or(false)
            // })
            .flatten()
            .collect();

        println!(
            "paths \n{}",
            paths
                .iter()
                .map(|p| format!("{}", p.to_string_lossy()))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert_eq!(4, paths.len());
        Ok(())
    }
}
