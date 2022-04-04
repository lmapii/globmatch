//! This crate provides cross platform matching for globs with relative path prefixes.
//!
//! For CLI utilities it can be a common pattern to operate on a set of files. Such a set of files
//! is either provided directly, as parameter to the tool - or via configuration files. The use of
//! a configuration file makes it easier to determine the location of a file since the path
//! can be specified relative to the configuration. Consider, e.g., the following `.json` input:
//!
//! ```ignore
//! {
//!   "globs": [
//!     "../../../some/text-files/**/*.txt",
//!     "other/inputs/*.md",
//!     "paths/from/dir[0-9]/*.*"
//!   ]
//! }
//! ```
//!
//! Specifying these paths in a dedicated configuration file allows to resolve the paths
//! independent of the invocation of the script operating on these files, the location of the
//! configuration file is used as base directory.
//!
//! This crate combines the features of the existing crates [globset][globset] and
//! [walkdir][walkdir] to implement a *relative glob matcher*:
//!
//! - A [`Builder`] is created for each glob in the same style as in `globset::Glob`.
//! - A [`Matcher`] is created from the [`Builder`] using [`Builder::build`]. This call resolves
//!   the relative path components within the glob by "moving" it to the specified root directory.
//! - The [`Matcher`] is then transformed into an iterator yielding `path::PathBuf`.
//!
//! For the previous example it would be sufficient to use one builder per glob and to specify
//! the root folder when building the pattern (see examples below).
//!
//! # Globs
//!
//! Please check the documentation of [globset][globset] for the available glob format.
//!
//! # Example: A simple match.
//!
//! The following example uses the files stored in the `test-files` folder, we're trying to match
//! all the `.txt` files using the glob `test-files/**/*.txt` (where `test-files` is the only
//! relative path component).
//!
//! ```
//! /*
//!     Example files:
//!     globmatch/test-files/.hidden
//!     globmatch/test-files/.hidden/h_1.txt
//!     globmatch/test-files/.hidden/h_0.txt
//!     globmatch/test-files/a/a2/a2_0.txt
//!     globmatch/test-files/a/a0/a0_0.txt
//!     globmatch/test-files/a/a0/a0_1.txt
//!     globmatch/test-files/a/a0/A0_3.txt
//!     globmatch/test-files/a/a0/a0_2.md
//!     globmatch/test-files/a/a1/a1_0.txt
//!     globmatch/test-files/some_file.txt
//!     globmatch/test-files/b/b_0.txt
//!  */
//!
//! use globmatch;
//!
//! # fn example_a() -> Result<(), String> {
//! let builder = globmatch::Builder::new("test-files/**/*.txt")
//!     .build(env!("CARGO_MANIFEST_DIR"))?;
//!
//! let paths: Vec<_> = builder.into_iter()
//!     .flatten()
//!     .collect();
//!
//! println!(
//!     "paths:\n{}",
//!     paths
//!         .iter()
//!         .map(|p| format!("{}", p.to_string_lossy()))
//!         .collect::<Vec<_>>()
//!         .join("\n")
//! );
//!
//! assert_eq!(6 + 2 + 1, paths.len());
//! # Ok(())
//! # }
//! # example_a().unwrap();
//! ```
//!
//! # Example: Specifying options and using `.filter_entry`.
//!
//! Similar to the builder pattern in [globset][globset] when using `globset::GlobBuilder`, this
//! crate allows to pass options (currently just case sensitivity) to the builder.
//!
//! In addition, the [`filter_entry`][filter_entry] function from [walkdir][walkdir] is accessible,
//! but only as a single call (this crate does not implement a recursive iterator). This function
//! allows filter files and folders *before* matching against the provided glob and therefore
//! to efficiently exclude files and folders, e.g., hidden folders:
//!
//! ```
//! use globmatch;
//!
//! # fn example_b() -> Result<(), String> {
//! let root = env!("CARGO_MANIFEST_DIR");
//! let pattern = "test-files/**/[ah]*.txt";
//!
//! let builder = globmatch::Builder::new(pattern)
//!     .case_sensitive(true)
//!     .build(root)?;
//!
//! let paths: Vec<_> = builder
//!     .into_iter()
//!     .filter_entry(|p| !globmatch::is_hidden_entry(p))
//!     .flatten()
//!     .collect();
//!
//! assert_eq!(4, paths.len());
//! # Ok(())
//! # }
//! # example_b().unwrap();
//! ```
//!
//! # Example: Filtering with `.build_glob`.
//!
//! The above examples demonstrated how to search for paths using this crate. Two more builder
//! functions are available for additional matching on the paths yielded by the iterator, e.g.,
//! to further limit the files (e.g., based on a global blacklist).
//!
//! - [`Builder::build_glob`] to create a single [`Glob`] (caution: the builder only checks
//!    that the pattern is not empty, but allows absolute paths).
//! - [`Builder::build_glob_set`] to create a [`Glob`] matcher that contains two globs
//!   `[glob, **/glob]` out of the specified `glob` parameter of [`Builder::new`]. The pattern
//!    must not be an absolute path.
//!
//! ```
//! use globmatch;
//!
//! # fn example_c() -> Result<(), String> {
//! let root = env!("CARGO_MANIFEST_DIR");
//! let pattern = "test-files/**/a*.*";
//!
//! let builder = globmatch::Builder::new(pattern)
//!     .case_sensitive(true)
//!     .build(root)?;
//!
//! let glob = globmatch::Builder::new("*.txt").build_glob_set()?;
//!
//! let paths: Vec<_> = builder
//!     .into_iter()
//!     .filter_entry(|p| !globmatch::is_hidden_entry(p))
//!     .flatten()
//!     .filter(|p| glob.is_match(p))
//!     .collect();
//!
//! assert_eq!(4, paths.len());
//! # Ok(())
//! # }
//! # example_c().unwrap();
//! ```
//!
//! [globset]: https://docs.rs/globset
//! [walkdir]: https://docs.rs/walkdir
//! [filter_entry]: #IterFilter::filter_entry

#[cfg(doctest)]
doc_comment::doctest!("../readme.md");

use std::path;

mod error;
mod iters;
mod utils;

pub use crate::error::Error;
pub use crate::iters::{IterAll, IterFilter};
pub use crate::utils::{is_hidden_entry, is_hidden_path};

/// Asterisks `*` in a glob do not match path separators (e.g., `/` in unix).
/// Only a double asterisk `**` match multiple folder levels.
const REQUIRE_PATHSEP: bool = true;

/// A builder for a matcher or globs.
///
/// This builder can be configured to match case sensitive (default) or case insensitive.
/// A single asterisk will not match path separators, e.g., `*/*.txt` does not match the file
/// `path/to/file.txt`. Use `**` to match across directory boundaries.
///
/// The lifetime `'a` refers to the lifetime of the glob string.
pub struct Builder<'a> {
    glob: &'a str,
    case_sensitive: bool,
}

impl<'a> Builder<'a> {
    /// Create a new builder for the given glob.
    ///
    /// The glob is not compiled until any of the `build` methods is called.
    pub fn new(glob: &'a str) -> Builder<'a> {
        Builder {
            glob,
            case_sensitive: true,
        }
    }

    /// Toggle whether the glob matches case sensitive or not.
    ///
    /// The default setting is to match case **sensitive***.
    pub fn case_sensitive(&mut self, yes: bool) -> &mut Builder<'a> {
        self.case_sensitive = yes;
        self
    }

    /// The actual facade for `globset::Glob`.
    #[doc(hidden)]
    fn glob_for(&self, glob: &str) -> Result<globset::Glob, String> {
        globset::GlobBuilder::new(glob)
            .literal_separator(REQUIRE_PATHSEP)
            .case_insensitive(!self.case_sensitive)
            .build()
            .map_err(|err| {
                format!(
                    "'{}': {}",
                    self.glob,
                    utils::to_upper(err.kind().to_string())
                )
            })
    }

    /// Builds a [`Matcher`] for the given [`Builder`] relative to `root`.
    ///
    /// Resolves the relative path prefix for the `glob` that has been provided when creating the
    /// builder for the given root directory, e.g.,
    ///
    /// For the root directory `/path/to/some/folder` and glob `../../*.txt`, this function will
    /// move the relative path components to the root folder, resulting in only `*.txt` for the
    /// glob, and `/path/to/some/folder/../../` for the root directory.
    ///
    /// Notice that the relative path components will **not** be resolved. The caller of the
    /// function can map and consolidate each path yielded by the iterator, if required.
    ///
    /// # Errors
    ///
    /// Simple error messages will be provided in case of failures, e.g., for empty patterns or
    /// patterns for which the compilation failed; as well as for invalid root directories.
    pub fn build<P>(&self, root: P) -> Result<Matcher<'a, path::PathBuf>, String>
    where
        P: AsRef<path::Path>,
    {
        // notice that resolve_root does not return empty patterns
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

    /// Builds a [`Glob`].
    ///
    /// This [`Glob`] that can be used for filtering paths provided by a [`Matcher`] (created
    /// using the `build` function).
    pub fn build_glob(&self) -> Result<Glob<'a>, String> {
        if self.glob.is_empty() {
            return Err("Empty glob".to_string());
        }

        let matcher = self.glob_for(self.glob)?.compile_matcher();
        Ok(Glob {
            glob: self.glob,
            matcher,
        })
    }

    /// Builds a combined [`GlobSet`].
    ///
    /// A globset extends the provided `pattern` to `[pattern, **/pattern]`. This is useful, e.g.,
    /// for blacklists, where only the file type is important.
    ///
    /// Yes, it would be sufficient to use the pattern `**/pattern` in the first place. This is
    /// a simple commodity function.
    pub fn build_glob_set(&self) -> Result<GlobSet<'a>, String> {
        if self.glob.is_empty() {
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
                    self.glob,
                    utils::to_upper(err.kind().to_string())
                )
            })?;

        Ok(GlobSet {
            glob: self.glob,
            matcher,
        })
    }
}

/// Matcher type for transformation into an iterator.
///
/// This type exists such that [`Builder::build`] can return a result type (whereas `into_iter`
/// cannot). Notice that `iter()` is not implemented due to the use of references.
pub struct Matcher<'a, P>
where
    P: AsRef<path::Path>,
{
    glob: &'a str,
    /// Original glob-pattern
    root: P,
    /// Root path of a resolved pattern
    rest: &'a str,
    /// Remaining pattern after root has been resolved
    matcher: globset::GlobMatcher,
}

impl<'a, P> IntoIterator for Matcher<'a, P>
where
    P: AsRef<path::Path>,
{
    type Item = Result<path::PathBuf, Error>;
    type IntoIter = IterAll<P>;

    /// Transform the [`Matcher`] into a recursive directory iterator.
    fn into_iter(self) -> Self::IntoIter {
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
    /// Provides the original glob-pattern used to create this [`Matcher`].
    ///
    /// This is the unchanged glob, i.e., no relative path components have been resolved.
    pub fn glob(&self) -> &str {
        self.glob
    }

    /// Provides the resolved root folder used by the [`Matcher`].
    ///
    /// This directory already contains the path components from the original glob. The main
    /// intention of this function is to for debugging or logging (thus a String).
    pub fn root(&self) -> String {
        let path = path::PathBuf::from(self.root.as_ref());
        String::from(path.to_str().unwrap())
    }

    /// Provides the resolved glob used by the [`Matcher`].
    ///
    /// All relative path components have been resolved for this glob. The glob is of type &str
    /// since all globs are input parameters and specified as strings (and not paths).
    pub fn rest(&self) -> &str {
        self.rest
    }

    /// Checks whether the provided path is a match for the stored glob.
    pub fn is_match(&self, p: P) -> bool {
        self.matcher.is_match(p)
    }
}

/// Wrapper type for glob matching.
///
/// This type is created by [`Builder::build_glob`] for a single glob on which no transformations
/// or path resolutions have been performed.
pub struct Glob<'a> {
    glob: &'a str,
    pub matcher: globset::GlobMatcher,
}

impl<'a> Glob<'a> {
    /// Provides the original glob-pattern used to create this [`Glob`].
    pub fn glob(&self) -> &str {
        self.glob
    }

    /// Checks whether the provided path is a match for the stored glob.
    pub fn is_match<P>(&self, p: P) -> bool
    where
        P: AsRef<path::Path>,
    {
        self.matcher.is_match(p)
    }
}

/// Comfort type for glob matching.
///
/// This type is created by [`Builder::build_glob_set`] (refer to the function documentation). The
/// matcher stores two globs created from the original pattern as `[**/pattern, pattern]` for
/// easy matching on multiple paths.
pub struct GlobSet<'a> {
    glob: &'a str,
    pub matcher: globset::GlobSet,
}

impl<'a> GlobSet<'a> {
    /// Provides the original glob-pattern used to create this [`GlobSet`].
    pub fn glob(&self) -> &str {
        self.glob
    }

    /// Checks whether the provided path is a match for any of the two stored globs.
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
    #[cfg_attr(target_os = "windows", ignore)]
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
            "some_file.txt",
        ];

        // function declaration within function. yay this starts to feel like python :D
        fn match_glob<'a>(f: &'a str, m: &globset::GlobMatcher) -> Option<&'a str> {
            match m.is_match(f) {
                true => Some(f),
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
                .literal_separator(REQUIRE_PATHSEP)
                .build()?
                .compile_matcher())
        }

        fn test_for(glob: &str, len: usize, files: &[&str], case_sensitive: bool) {
            let glob = glob_for(glob, case_sensitive).unwrap();
            let matches = files
                .iter()
                .filter_map(|f| match_glob(f, &glob))
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

        // notice that **/*.txt also matches zero recursive levels and thus also "some_file.txt"
        test_for("**/*.txt", 7, &files, false);
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
    #[cfg(not(target_os = "windows"))]
    fn match_absolute_pattern() -> Result<(), String> {
        let root = format!("{}/test-files", env!("CARGO_MANIFEST_DIR"));
        match Builder::new("/test-files/**/*.txt").build(root) {
            Err(_) => Ok(()),
            Ok(_) => Err("Expected failure".to_string()),
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn match_absolute_pattern() -> Result<(), String> {
        let root = format!("{}/test-files", env!("CARGO_MANIFEST_DIR"));
        match Builder::new("C:/test-files/**/*.txt").build(root) {
            Err(_) => Ok(()),
            Ok(_) => Err("Expected failure".to_string()),
        }
    }

    /*
    some helper functions for testing
    */

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

    fn log_paths_and_assert<P>(paths: &[P], expected_len: usize)
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
        log_paths_and_assert(&paths, 6 + 2 + 1); // this also matches `some_file.txt`
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
    fn match_with_glob() -> Result<(), String> {
        let root = env!("CARGO_MANIFEST_DIR");
        let pattern = "test-files/**/*.txt";

        let glob = Builder::new("**/test-files/a/a[0]/**").build_glob()?;
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
    fn match_with_glob_all() -> Result<(), String> {
        let root = env!("CARGO_MANIFEST_DIR");
        let pattern = "test-files/**/*.*";

        // build_glob creates a ["**/pattern", "pattern"] glob such that the user two separate
        // patterns when scanning for files, e.g., using "*.txt" (which would need "**/*.txt"
        // as well), but also when specifying paths within this glob.
        let glob = Builder::new("*.txt").build_glob_set()?;
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
        // TODO: implememnt tests for different relative pattern styles
        // TODO: also provide failing tests for relative parts in the rest/remainder glob
        Ok(())
    }
}
