use std::fs::File;
use std::io::{BufReader, Read};

/// Representation of server's configuration.
#[derive(serde::Deserialize)]
pub struct Config {
    /// Server's port.
    pub port: u16,
    /// Database file name.
    pub database_name: String,
}

impl Config {
    /// Reads the configuration parameters from file.
    /// 
    /// The file `config.toml` that should be located in the current working directory.
    /// 
    /// # Errors
    /// Returns error if:
    /// - Opening configuration file fail
    /// - Reading from configuration file fail
    /// - Configuration file is not a valid TOML
    /// - The file does not include all the configuration parameters
    pub fn read() -> Result<Self, Error> {
        let file = File::open("config.toml").map_err(Error::Open)?;
        let mut reader = BufReader::new(file);

        let mut content = String::new();
        reader.read_to_string(&mut content).map_err(Error::Read)?;

        toml::from_str(&content).map_err(Error::Parse)
    }
}

#[derive(thiserror::Error, Debug)]
/// Errors related to reading the configuration file.
pub enum Error {
    #[error("could not open config file")]
    Open(std::io::Error),
    #[error("could not read config file")]
    Read(std::io::Error),
    #[error("could not parse config file")]
    Parse(toml::de::Error),
}
