use std::{
    fmt,
    path::{Path, PathBuf},
};

pub mod prelude;

pub use directories;
pub use lazy_static;
pub use toml;

/// loadable cfg
pub trait StructCfg
where
    Self: Sized,
{
    fn load() -> Result<Self, Error>;
    fn path() -> &'static Path;
}

/// cfg with a specified default
pub trait StructCfgDefault: StructCfg {
    fn load_or_write_default() -> Result<Self, Error>;
    fn write_default() -> Result<Self, Error>;
}

#[derive(Debug)]
pub enum Error {
    IoRead(std::io::Error, PathBuf),
    MakeDir(std::io::Error, PathBuf),
    IoWrite(std::io::Error, PathBuf),
    Toml(toml::de::Error, PathBuf),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::IoWrite(e, _) => Some(e),
            Error::MakeDir(e, _) => Some(e),
            Error::IoRead(e, _) => Some(e),
            Error::Toml(e, _) => Some(e),
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
            Error::Toml(_, path) => write!(formatter, "Can't read config from {}", path.display()),
        }
    }
}

impl Error {
    pub fn nice(self) -> NiceError {
        NiceError(self)
    }
}

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
