/*!
This crate allows you to derive commonly used functions for configuration files.
It will derive [Cfgen](Cfgen) and optionally [CfgenDefault](CfgenDefault)
depending on the args passed to cfgen.

# Usage
```
use cfgen::prelude::*;
use serde_derive::Deserialize;

const DEFAULT_CONFIG: &str = r#"
foo = 4
bar = "bar"
"#;

# #[cfg(feature = "with-toml")]
#[derive(Cfgen, Deserialize)]
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

All config formats are optional cargo features, if you want to use toml/yaml configuration
add "with-toml"/"yaml" to the enabled features of this crate.

# Config path construction
The config path is constructed with the
[ProjectDirs::from](https://docs.rs/directories/1.0.2/directories/struct.ProjectDirs.html#method.from)
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
pub use lazy_static;
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
            Error::MakeDir(_, path) => {
                write!(formatter, "Can't create directory {}", path.display())
            }
            Error::IoWrite(_, path) => {
                write!(formatter, "Can't write config to {}", path.display())
            }
            Error::IoRead(_, path) => {
                write!(formatter, "Can't read config from {}", path.display())
            }
            #[cfg(feature = "with-toml")]
            Error::Toml(_, path) => write!(formatter, "Can't read config from {}", path.display()),
            #[cfg(feature = "yaml")]
            Error::Yaml(_, path) => write!(formatter, "Can't read config from {}", path.display()),
        }
    }
}

impl Error {
    /// Returns a nicely formatted error message for use in applications that don't use
    /// Error::source chaining.
    pub fn nice(self) -> NiceError {
        NiceError(self)
    }
}

/// Error with a display implementation that looks nice without source chaining.
#[derive(Debug)]
pub struct NiceError(pub Error);

impl std::error::Error for NiceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl fmt::Display for NiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(formatter)?;
        use std::error::Error;
        write!(formatter, ": {}", self.source().unwrap())
    }
}

/// Enum representing the outcome of load_or_write_default
pub enum ConfigLoad {
    /// Default config was written to the config path, contains default
    DefaultWritten,

    /// Config was loaded without writing default
    Loaded,
}
