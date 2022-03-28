
# globmatch

[![Build status](https://github.com/lmapii/globmatch/workflows/ci/badge.svg)](https://github.com/lmapii/globmatch/actions)

Rust crate for resolving globs relative to a specified directory. Based on [globset][globset] and [walkdir][walkdir].

<!--
## Documentation

[https://docs.rs/globmatch](https://docs.rs/globmatch)

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
globmatch = "0.1"
``` -->

## Examples and concept


For CLI utilities it can be a common pattern to operate on a set of files. Such a set of files is either provided directly, as parameter to the tool - or via configuration files.

The use of a configuration file makes it easier to determine the location of a file since the path can be specified relative to the configuration. Consider, e.g., the following `.json` input:

```json,no_run
{
  "globs": [
    "../../../some/text-files/**/*.txt",
    "other/inputs/*.md",
    "paths/from/dir[0-9]/*.*"
  ]
}
```

Specifying these paths in a dedicated configuration file allows to resolve the paths independent of the invocation of the script operating on these files, the location of the configuration file is used as base directory. This crate combines the features of the existing crates [globset][globset] and [walkdir][walkdir] to implement a *relative glob matcher*.

### Example: A simple match.

The following example uses the files stored in the `test-files` folder, we're trying to match all the `.txt` files using the glob `test-files/**/*.txt` (where `test-files` is the only relative path component).

```rust
use globmatch;

fn example_a() -> Result<(), String> {
    let builder = globmatch::Builder::new("test-files/**/*.txt")
        .build(env!("CARGO_MANIFEST_DIR"))?;

    let paths: Vec<_> = builder.into_iter()
        .flatten()
        .collect();

    println!(
        "paths:\n{}",
        paths
            .iter()
            .map(|p| format!("{}", p.to_string_lossy()))
            .collect::<Vec<_>>()
            .join("\n")
    );

    assert_eq!(6 + 2 + 1, paths.len());
    Ok(())
}

example_a().unwrap();
```

### Example: Specifying options and using `.filter_entry`.

Similar to the builder pattern in [globset][globset] when using `globset::GlobBuilder`, this crate allows to pass options (currently just case sensitivity) to the builder.

In addition, the `filter_entry` function from [walkdir][walkdir] is accessible, but only as a single call (this crate does not implement a recursive iterator). This function allows filter files and folders *before* matching against the provided glob and therefore to efficiently exclude files and folders, e.g., hidden folders:

 ```rust
use globmatch;

fn example_b() -> Result<(), String> {
    let root = env!("CARGO_MANIFEST_DIR");
    let pattern = "test-files/**/[ah]*.txt";

    let builder = globmatch::Builder::new(pattern)
        .case_sensitive(true)
        .build(root)?;

    let paths: Vec<_> = builder
        .into_iter()
        .filter_entry(|p| !globmatch::is_hidden_entry(p))
        .flatten()
        .collect();

    assert_eq!(4, paths.len());
    Ok(())
}

example_b().unwrap();
 ```

[globset]: https://docs.rs/globset
[walkdir]: https://docs.rs/walkdir