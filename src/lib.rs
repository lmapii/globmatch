use globset;
use std::io;
use std::path;

// TODO: extend for utility functions for Vec of patterns and a common root path

// TODO: resolve paths before matching with filter -> consolidate()
// to ensure that patterns can be matched easier

pub use crate::error::Error;
pub use util::is_hidden;

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

    fn glob_matcher_for(&self, glob: &str) -> Result<globset::GlobMatcher, String> {
        Ok(globset::GlobBuilder::new(glob)
            .case_insensitive(self.case_insensitive)
            .build()
            .map_err(|err| format!("{}: {}", self.glob.to_string(), err.kind().to_string(),))?
            .compile_matcher())
    }

    // TODO: document that this is an optimized builder
    // this item moves relative paths into the root such that patterns can contain relative paths
    // which would otherwise not be possible
    pub fn build<P>(&self, root: P) -> Result<Matcher<'a, path::PathBuf>, String>
    where
        P: AsRef<path::Path>,
    {
        let (root, rest) = util::resolve_root(root, self.glob)
            .map_err(|err| format!("Root folder not found: {}", err))?;
        let matcher = self.glob_matcher_for(rest)?;
        Ok(Matcher {
            glob: self.glob,
            root,
            rest,
            matcher,
        })
    }

    pub fn build_raw<P>(&self, root: P) -> Result<Matcher<'a, path::PathBuf>, String>
    where
        P: AsRef<path::Path>,
    {
        if !root.as_ref().exists() {
            return Err(format!(
                "Root folder not found: {}",
                io::Error::from(io::ErrorKind::NotFound)
            ));
        }
        let matcher = self.glob_matcher_for(self.glob)?;
        Ok(Matcher {
            glob: self.glob,
            root: path::PathBuf::from(root.as_ref()),
            rest: "",
            matcher,
        })
    }

    // for building globs - iterators won't work properly
    pub fn build_glob(&self, strict: bool) -> Result<Matcher<'a, path::PathBuf>, String> {
        match path::PathBuf::from(self.glob).components().next() {
            None => Ok(()),
            Some(_) if !strict => Ok(()),
            Some(c) => match c {
                path::Component::Normal(_) => Ok(()),
                _ => Err("Absolute or relative paths are not allowed for raw globs"),
            },
        }?;
        let matcher = self.glob_matcher_for(self.glob)?;
        Ok(Matcher {
            glob: self.glob,
            root: path::PathBuf::from(""),
            rest: "",
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
        // println!(
        //     "matching {} -> {} (original {})",
        //     self.root.as_ref().to_string_lossy(),
        //     self.rest,
        //     self.glob
        // );
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
    pub fn is_match(&self, p: P) -> bool {
        self.matcher.is_match(p)
    }
    // TODO: can_iter() -> bool:false on empty root
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
                        // println!("IterAll: matching {}", dir.path().to_string_lossy());
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
            .flatten()
            .filter(|p| !is_hidden(p))
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

    #[test]
    fn match_filter_glob() -> Result<(), String> {
        let root = env!("CARGO_MANIFEST_DIR");
        let pattern = "test-files/**/*.txt";

        let glob = Builder::new("/**/test-files/a/a[0]/**").build_glob(false)?;

        let paths: Vec<_> = Builder::new(pattern)
            .build(root)?
            .into_iter()
            .flatten()
            .filter(|p| !is_hidden(p))
            .filter(|p| !glob.is_match(p.into()))
            .collect();

        println!(
            "paths \n{}",
            paths
                .iter()
                .map(|p| format!("{}", p.to_string_lossy()))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert_eq!(2, paths.len());
        Ok(())
    }
}
