/*!
This crate allows you to derive commonly used functions for configuration files.
It will derive [Cfgen](Cfgen) and optionally [CfgenDefault](CfgenDefault)
depending on the args passed to cfgen.

# Usage
```
use cfgen::prelude::*;

const DEFAULT_CONFIG: &str = r#"
foo = 4
bar = "bar"
"#;

# #[cfg(feature = "with-toml")]
#[derive(Cfgen, serde::Deserialize)]
#[cfgen(app_name = "test",
        org = "Big Company",
        format = "toml",
        qualifier = "com",
        filename = "configuration.toml",
        default = "DEFAULT_CONFIG")]
struct Config {
    foo: i32,
    bar: String,
}
```

All keys for the derive macro are optional
, cfgen tries to guess a good value from cargo env vars.

- `app_name`: The name of your program, defaults to the crate name in Cargo.toml
- `org`: Name of your organisation, defaults to the first author in Cargo.toml without email
- `qualifier`: Standard qualifier like "com" and "org", defaults to "org"
- `format`: Configuration file format, defaults depend on enabled crate features
- `filename`: Name of the configuration file, defaults to `config.$CONFIG_FORMAT_EXTENSION`
- `default`: const string containing your default config. If no default given, only derives
             Cfgen and without generating an CfgenDefault for your struct.
             Also generates a test that asserts that the default config parses.
- `generate_test`: Wether to generate an automatic test that tests if the default
                   config is deserializeable. Defaults to true

All config formats are optional cargo features, if you want to use toml/yaml configuration
add "with-toml"/"yaml" to the enabled features of this crate.

# Config path construction
The config path is constructed with the
[ProjectDirs::from](https://docs.rs/directories/2.0.0/directories/struct.ProjectDirs.html#method.from)
from the [directories](https://crates.io/crates/directories) crate.
app_name, org and qualifier map to application, organization and qualifier, respectively.
*/
#![deny(missing_docs)]

/// Reexports all traits and derive macros of this crate for use with * imports.
pub mod prelude;

use std::{
    fmt,
    path::{Path, PathBuf},
};

#[doc(hide)]
pub use directories;
#[doc(hide)]
pub use once_cell;
#[doc(hide)]
#[cfg(feature = "yaml")]
pub use serde_yaml;
#[doc(hide)]
#[cfg(feature = "with-toml")]
pub use toml;

/// Basic functions to read a config.
pub trait Cfgen
where
    Self: Sized,
{
    /// Path to config, see the top level crate documentation for more information.
    fn path() -> &'static Path;

    /// Load config from config dir. Errors with IoRead when the file can't be read, and with
    /// either Toml or Yaml when the file can't be deserialized.
    fn load() -> Result<Self, Error>;
}

/// Configuration file with a default config
pub trait CfgenDefault: Cfgen {
    /// Writes the default config to Cfgen::path(), errors with IoWrite when config can't be
    /// written.
    fn write_default() -> Result<Self, Error>;

    /// If the config file doesn't exist, writes the default to Cfgen::path() and then tries to
    /// load the default config on disk. Returns a [ConfigLoad](ConfigLoad)
    fn load_or_write_default() -> Result<(ConfigLoad, Self), Error>;
}

/// All possible errors this crate can return.
#[derive(Debug)]
pub enum Error {
    /// Io error when reading default config
    IoRead(std::io::Error, PathBuf),

    /// Couldn't create directory for default config
    MakeDir(std::io::Error, PathBuf),

    /// Failed to write default config to fs
    IoWrite(std::io::Error, PathBuf),

    /// Error caused by failed toml deserialization
    #[cfg(feature = "with-toml")]
    Toml(toml::de::Error, PathBuf),

    /// Error caused by failed yaml deserialization
    #[cfg(feature = "yaml")]
    Yaml(serde_yaml::Error, PathBuf),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::IoWrite(e, _) => Some(e),
            Error::MakeDir(e, _) => Some(e),
            Error::IoRead(e, _) => Some(e),
            #[cfg(feature = "with-toml")]
            Error::Toml(e, _) => Some(e),
            #[cfg(feature = "yaml")]
            Error::Yaml(e, _) => Some(e),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::MakeDir(e, path) => write!(
                formatter,
                "Can't create directory {}: {}",
                path.display(),
                e
            ),
            Error::IoWrite(e, path) => {
                write!(formatter, "Can't write config to {}: {}", path.display(), e)
            }
            Error::IoRead(e, path) => write!(
                formatter,
                "Can't read config from {}: {}",
                path.display(),
                e
            ),
            #[cfg(feature = "with-toml")]
            Error::Toml(e, path) => write!(
                formatter,
                "Can't read config from {}: {}",
                path.display(),
                e
            ),
            #[cfg(feature = "yaml")]
            Error::Yaml(e, path) => write!(
                formatter,
                "Can't read config from {}: {}",
                path.display(),
                e
            ),
        }
    }
}

/// Enum representing the outcome of load_or_write_default
pub enum ConfigLoad {
    /// Default config was written to the config path, contains default
    DefaultWritten,

    /// Config was loaded without writing default
    Loaded,
}

use serde::de::{Deserialize, Deserializer};

// TODO: better doc, link to tilde-expand
// TODO: Can expandpath and expandpath_opt be unified?
/// Convenience function for deserializing `PathBuf`s. Automatically expands `PathBuf`s like "~/thing" to
/// "/home/your_name/thing"
/// Example use:
/// ```rust
/// use cfgen::prelude::*;
///
/// #[derive(Cfgen, serde::Deserialize)]
/// struct Config {
///     #[serde(deserialize_with = "cfgen::expandpath")]
///     a_directory: std::path::PathBuf,
/// }
/// ```
pub fn expandpath<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    PathBuf::deserialize(deserializer).map(tilde_expand::tilde_expand)
}

/// Same thing is `expandpath`, but for Option<PathBuf>
/// ```rust
/// use cfgen::prelude::*;
///
/// #[derive(Cfgen, serde::Deserialize)]
/// struct Config {
///     #[serde(deserialize_with = "cfgen::expandpath_opt")]
///     a_directory: Option<std::path::PathBuf>,
/// }
/// ```
pub fn expandpath_opt<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<PathBuf>::deserialize(deserializer).map(|buf| buf.map(tilde_expand::tilde_expand))
}
