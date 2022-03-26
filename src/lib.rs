use globset;
use std::path;

// TODO: extend for utility functions for Vec of patterns and a common root path
// TODO: rust serde convention - The message should not be capitalized and should not end with a period.

pub use crate::error::Error;
pub use util::{is_hidden_entry, is_hidden_path};

mod error;
mod util;

pub struct Builder<'a> {
    glob: &'a str,
    case_sensitive: bool,
}

impl<'a> Builder<'a> {
    pub fn new(glob: &'a str) -> Builder<'a> {
        Builder {
            glob,
            case_sensitive: true,
        }
    }

    pub fn case_sensitive(&mut self, yes: bool) -> &mut Builder<'a> {
        self.case_sensitive = yes;
        self
    }

    // TODO: document that this turns it into an is an optimized builder
    // this item moves relative paths into the root such that patterns can contain relative paths
    // which would otherwise not be possible. this makes a 1:1 mapping for builder and glob,
    // which in some cases makes matching less efficient (globs on the same root path)
    // but since it is impossible to know which paths are actually part of this and for different
    // sub-paths it is better than to have a far-off root path.
    // TODO: document: in case of doubt resolve yielded paths using consolidate()
    // to ensure that patterns can be matched easier

    fn glob_matcher_for(&self, glob: &str) -> Result<globset::GlobMatcher, String> {
        Ok(globset::GlobBuilder::new(glob)
            .literal_separator(true)
            .case_insensitive(!self.case_sensitive)
            .build()
            .map_err(|err| {
                format!("'{}': {}", self.glob.to_string(), {
                    let str = err.kind().to_string();
                    let mut c = str.chars();
                    match c.next() {
                        None => String::from("Unknown error"),
                        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                    }
                },)
            })?
            .compile_matcher())
    }

    pub fn build<P>(&self, root: P) -> Result<Matcher<'a, path::PathBuf>, String>
    where
        P: AsRef<path::Path>,
    {
        // notice that resolve_root doesnot return empty patterns
        let (root, rest) = util::resolve_root(root, self.glob)
            .map_err(|err| format!("Failed to resolve paths: {}", err))?;

        let matcher = self.glob_matcher_for(rest)?;
        Ok(Matcher {
            glob: self.glob,
            root,
            rest,
            matcher,
        })
    }

    pub fn build_glob(&self, strict: bool) -> Result<Glob<'a>, String> {
        match path::PathBuf::from(self.glob).components().next() {
            None => Ok(()),
            Some(_) if !strict => Ok(()),
            Some(c) => match c {
                path::Component::Normal(_) => Ok(()),
                _ => Err("Absolute or relative paths are not allowed for raw globs"),
            },
        }?;
        let matcher = self.glob_matcher_for(self.glob)?;
        Ok(Glob {
            glob: self.glob,
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
    type IntoIter = IterAll<P>;

    fn into_iter(self) -> Self::IntoIter {
        // println!(
        //     "matching {} -> {} (original {})",
        //     self.root.as_ref().to_string_lossy(),
        //     self.rest,
        //     self.glob
        // );
        let walk_root = path::PathBuf::from(self.root.as_ref());
        IterAll::new(
            self.root,
            walkdir::WalkDir::new(walk_root).into_iter(),
            self.matcher,
        )
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
}

pub struct Glob<'a> {
    glob: &'a str,
    matcher: globset::GlobMatcher,
}

impl<'a> Glob<'a> {
    pub fn glob(&self) -> &str {
        self.glob
    }

    pub fn is_match<P>(&self, p: P) -> bool
    where
        P: AsRef<path::Path>,
    {
        self.matcher.is_match(p)
    }
}

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
    fn new(root: P, iter: walkdir::IntoIter, matcher: globset::GlobMatcher) -> IterAll<P> {
        println!("matcher: {:?}", matcher);
        IterAll {
            root,
            iter,
            matcher,
        }
    }
}

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
                println!("checking {:?}", p);

                if matcher.is_match(p) {
                    return Some(Some(Ok(path::PathBuf::from(dir.path()))));
                }
                return None; // iterator should continue
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

            // let entry = match self.iter.next() {
            //     None => None,
            //     Some(res) => match res {
            //         Ok(dir) => {
            //             let p = dir.path().strip_prefix(&self.root).unwrap();
            //             println!("checking {:?}", p);

            //             if self.matcher.is_match(dir.path()) {
            //                 return Some(Ok(path::PathBuf::from(dir.path())));
            //             }
            //             continue;
            //         }
            //         Err(err) => Some(Err(err.into())),
            //     },
            // };
            // return entry;
        }
    }
}

impl<P> IterAll<P>
where
    P: AsRef<path::Path>,
{
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

// TODO: implement recursive filter_entry?

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path() {
        let path = path::Path::new("");
        assert!(!path.is_absolute());
    }

    #[test]
    fn match_globset() {
        // yes, it is on purpose that this is a simple list and not read from the test-files
        let files = vec![
            "/some/path/test-files/a",
            "/some/path/test-files/a/a0",
            "/some/path/test-files/a/a0/a0_0.txt",
            "/some/path/test-files/a/a0/a0_1.txt",
            "/some/path/test-files/a/a0/A0_3.txt",
            "/some/path/test-files/a/a0/a0_2.md",
            "/some/path/test-files/a/a1",
            "/some/path/test-files/a/a1/a1_0.txt",
            "/some/path/test-files/a/a2",
            "/some/path/test-files/a/a2/a2_0.txt",
            "/some/path/test-files/b/b_0.txt",
        ];

        // function declaration within function. yay this starts to feel like python :D
        fn match_glob<'a>(f: &'a str, m: &globset::GlobMatcher) -> Option<&'a str> {
            match m.is_match(f) {
                true => Some(f.as_ref()),
                false => None,
            }
        }

        fn glob_for(
            glob: &str,
            case_sensitive: bool,
        ) -> Result<globset::GlobMatcher, globset::Error> {
            Ok(globset::GlobBuilder::new(glob)
                .case_insensitive(!case_sensitive)
                .backslash_escape(true)
                .literal_separator(true)
                .build()?
                .compile_matcher())
        }

        fn test_for(glob: &str, len: usize, files: &Vec<&str>, case_sensitive: bool) {
            let glob = glob_for(glob, case_sensitive).unwrap();
            let matches = files
                .iter()
                .map(|f| match_glob(f, &glob))
                .flatten()
                .collect::<Vec<_>>();
            println!(
                "matches for {}:\n'{}'",
                glob.glob(),
                matches
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            assert_eq!(len, matches.len());
        }

        test_for("/test-files/**/*.txt", 0, &files, true);
        test_for("test-files/**/*.txt", 0, &files, true);
        test_for("**/test-files/**/*.txt", 6, &files, true);
        test_for("**/test-files/**/a*.txt", 4, &files, true);
        test_for("**/test-files/**/a*.txt", 5, &files, false);
        test_for("**/test-files/a/a*/a*.txt", 5, &files, false);
        test_for("**/test-files/a/a[01]/a*.txt", 4, &files, false);

        // this is important, an empty pattern does not match anything
        test_for("", 0, &files, false);
    }

    #[test]
    fn builder_build() -> Result<(), String> {
        let root = env!("CARGO_MANIFEST_DIR");
        let pattern = "**/*.txt";

        let _builder = Builder::new(pattern).build(root)?;
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
    // #[should_panic]
    fn match_absolute_pattern() -> Result<(), String> {
        let root = format!("{}/test-files", env!("CARGO_MANIFEST_DIR"));
        match Builder::new("/test-files/**/*.txt").build(root) {
            Err(_) => Ok(()),
            Ok(_) => Err("Expected failure".to_string()),
        }
    }

    /*
    some helper functions for testing
    */

    fn collect_paths<P>(builder: Matcher<P>) -> Vec<path::PathBuf>
    where
        P: AsRef<path::Path>,
    {
        let paths: Vec<_> = builder.into_iter().flatten().collect();
        println!(
            "paths:\n{}",
            paths
                .iter()
                .map(|p| format!("{}", p.to_string_lossy()))
                .collect::<Vec<_>>()
                .join("\n")
        );
        paths
    }

    fn collect_paths_and_assert<P>(builder: Matcher<P>, expected_len: usize)
    where
        P: AsRef<path::Path>,
    {
        let paths = collect_paths(builder);
        assert_eq!(expected_len, paths.len());
    }

    #[test]
    fn match_all() -> Result<(), String> {
        // the following resolves to `<package-root>/test-files/**/*.txt` and therefore
        // successfully matches all files
        let builder = Builder::new("test-files/**/*.txt").build(env!("CARGO_MANIFEST_DIR"))?;
        collect_paths_and_assert(builder, 6 + 2);
        Ok(())
    }

    #[test]
    fn match_flavours() -> Result<(), String> {
        // TODO: continue here for different relative pattern styles
        // TODO: for that the utils.rs function must be fixed. rest is not required, the glob is root + pattern
        // the important part was only to figure out the root directory to start searching from
        // TODO: the util function should simply check that there is no relative parts in the REMAINDER
        // meaning in the actual path
        // patterns can have no relative paths (after selectors) since it is possible to move out of the pattern
        // and then there is a loop. (though the levels of back and forth could be checked).
        Ok(())
    }

    #[test]
    fn match_case() -> Result<(), String> {
        let root = env!("CARGO_MANIFEST_DIR");
        let pattern = "test-files/a/a?/a*.txt";

        // default is case_sensitive(true)
        let builder = Builder::new(pattern).build(root)?;
        println!(
            "working on root {} with glob {:?}",
            builder.root(),
            builder.rest()
        );
        collect_paths_and_assert(builder, 4);
        Ok(())
    }

    #[test]
    fn match_filter_entry() -> Result<(), String> {
        let root = env!("CARGO_MANIFEST_DIR");
        let pattern = "test-files/**/*.txt";

        let builder = Builder::new(pattern).build(root)?;
        let paths: Vec<_> = builder
            .into_iter()
            .filter_entry(|p| !is_hidden_entry(p))
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
        assert_eq!(6, paths.len());
        Ok(())
    }

    #[test]
    fn match_filter() -> Result<(), String> {
        let root = env!("CARGO_MANIFEST_DIR");
        let pattern = "test-files/**/*.txt";

        // this is slower than filter_entry since it matches all hidden paths
        let builder = Builder::new(pattern).build(root)?;
        let paths: Vec<_> = builder
            .into_iter()
            .flatten()
            .filter(|p| !is_hidden_path(p))
            .collect();

        println!(
            "paths \n{}",
            paths
                .iter()
                .map(|p| format!("{}", p.to_string_lossy()))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert_eq!(6, paths.len());
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
            .filter(|p| !is_hidden_path(p))
            .filter(|p| !glob.is_match(p))
            .collect();

        println!(
            "paths \n{}",
            paths
                .iter()
                .map(|p| format!("{}", p.to_string_lossy()))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert_eq!(3, paths.len());
        Ok(())
    }
}
