use std::fs::File;
use std::io::{BufReader, Read};

#[derive(serde::Deserialize)]
pub struct Config {
    pub port: u16,
    pub database_name: String,
}

impl Config {
    pub fn read() -> Result<Self, Error> {
        let file = File::open("config.toml").map_err(Error::Open)?;
        let mut reader = BufReader::new(file);

        let mut content = String::new();
        reader.read_to_string(&mut content).map_err(Error::Read)?;

        toml::from_str(&content).map_err(Error::Parse)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("could not open config file")]
    Open(std::io::Error),
    #[error("could not read config file")]
    Read(std::io::Error),
    #[error("could not parse config file")]
    Parse(toml::de::Error),
}
