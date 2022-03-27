use globset;
use std::path;

// TODO: extend for utility functions for Vec of patterns and a common root path
// TODO: rust serde convention - The message should not be capitalized and should not end with a period.

mod error;
mod iters;
mod utils;

pub use crate::error::Error;
pub use crate::iters::{IterAll, IterFilter};
pub use crate::utils::{is_hidden_entry, is_hidden_path};

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

    fn glob_for(&self, glob: &str) -> Result<globset::Glob, String> {
        Ok(globset::GlobBuilder::new(glob)
            .literal_separator(true)
            .case_insensitive(!self.case_sensitive)
            .build()
            .map_err(|err| {
                format!(
                    "'{}': {}",
                    self.glob.to_string(),
                    utils::to_upper(err.kind().to_string())
                )
            })?)
    }

    pub fn build<P>(&self, root: P) -> Result<Matcher<'a, path::PathBuf>, String>
    where
        P: AsRef<path::Path>,
    {
        // notice that resolve_root doesnot return empty patterns
        let (root, rest) = utils::resolve_root(root, self.glob).map_err(|err| {
            format!(
                "'Failed to resolve paths': {}",
                utils::to_upper(err.to_string())
            )
        })?;

        let matcher = self.glob_for(rest)?.compile_matcher();
        Ok(Matcher {
            glob: self.glob,
            root,
            rest,
            matcher,
        })
    }

    pub fn build_glob_raw(&self) -> Result<Glob<'a>, String> {
        let matcher = self.glob_for(self.glob)?.compile_matcher();
        Ok(Glob {
            glob: self.glob,
            matcher,
        })
    }

    pub fn build_glob(&self) -> Result<GlobSet<'a>, String> {
        if self.glob.len() == 0 {
            return Err("Empty glob".to_string());
        }

        let p = path::Path::new(self.glob);
        if p.is_absolute() {
            return Err(format!("{}' is an absolute path", self.glob));
        }

        let glob_sub = "**/".to_string() + self.glob;

        let matcher = globset::GlobSetBuilder::new()
            .add(self.glob_for(self.glob)?)
            .add(self.glob_for(&glob_sub)?)
            .build()
            .map_err(|err| {
                format!(
                    "'{}': {}",
                    self.glob.to_string(),
                    utils::to_upper(err.kind().to_string())
                )
            })?;

        Ok(GlobSet {
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
    pub matcher: globset::GlobMatcher,
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

pub struct GlobSet<'a> {
    glob: &'a str,
    pub matcher: globset::GlobSet,
}

impl<'a> GlobSet<'a> {
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

    fn log_paths<P>(paths: &Vec<P>)
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

    fn log_paths_and_assert<P>(paths: &Vec<P>, expected_len: usize)
    where
        P: AsRef<path::Path>,
    {
        log_paths(paths);
        assert_eq!(expected_len, paths.len());
    }

    #[test]
    fn match_all() -> Result<(), String> {
        // the following resolves to `<package-root>/test-files/**/*.txt` and therefore
        // successfully matches all files
        let builder = Builder::new("test-files/**/*.txt").build(env!("CARGO_MANIFEST_DIR"))?;

        let paths: Vec<_> = builder.into_iter().flatten().collect();
        log_paths_and_assert(&paths, 6 + 1 + 2);
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

        let paths: Vec<_> = builder.into_iter().flatten().collect();
        log_paths_and_assert(&paths, 4);
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

        log_paths_and_assert(&paths, 6 + 1);
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

        log_paths_and_assert(&paths, 6 + 1);
        Ok(())
    }

    #[test]
    fn match_with_raw() -> Result<(), String> {
        let root = env!("CARGO_MANIFEST_DIR");
        let pattern = "test-files/**/*.txt";

        let glob = Builder::new("**/test-files/a/a[0]/**").build_glob_raw()?;
        let paths: Vec<_> = Builder::new(pattern)
            .build(root)?
            .into_iter()
            .flatten()
            .filter(|p| !is_hidden_path(p))
            .filter(|p| glob.is_match(p))
            .collect();

        log_paths_and_assert(&paths, 3);
        Ok(())
    }

    #[test]
    fn match_with_glob() -> Result<(), String> {
        let root = env!("CARGO_MANIFEST_DIR");
        let pattern = "test-files/**/*.*";

        // build_glob creates a ["**/pattern", "pattern"] glob such that the user two separate
        // patterns when scanning for files, e.g., using "*.txt" (which would need "**/*.txt" as well
        let glob = Builder::new("*.txt").build_glob()?;
        let paths: Vec<_> = Builder::new(pattern)
            .build(root)?
            .into_iter()
            .filter_entry(|e| !is_hidden_entry(e))
            .flatten()
            .filter(|p| {
                let is_match = glob.is_match(p);
                println!("is match: {:?} - {}", p, is_match);
                is_match
            })
            .collect();

        log_paths_and_assert(&paths, 6 + 1);
        Ok(())
    }

    #[test]
    fn match_flavours() -> Result<(), String> {
        // TODO: continue here for different relative pattern styles
        // TODO: the util function checks that there are no relative parts in the REMAINDER
        Ok(())
    }
}

// TODO: checkout coverage
// https://github.com/mozilla/grcov
// https://marco-c.github.io/2020/11/24/rust-source-based-code-coverage.html
// https://github.com/marco-c/rust-code-coverage-sample/blob/main/run_gcov.sh
