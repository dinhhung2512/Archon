use log_derive::logfn;

use std::error::Error;
use std::fmt;

use crate::Config;

#[derive(Debug)]
struct AppError {
    details: String,
}

impl AppError {
    fn new(msg: &str) -> AppError {
        AppError { defaults: msg.to_string() }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for AppError {
    fn description(&self) -> &str {
        &self.details
    }
}

pub struct App {
    conf: Config,
}

impl App {
    //Constructor
    pub fn new() -> App {}

    //Utilities
    #[logfn(Err = "Error", fmt = "Failed to load config file: {:?}")]
    fn load_config() -> Result<Config, AppError> {
        let c: Config = match File::open("archon.yaml") {
            Ok(file) => {
                let (conf, err) = Config::try_parse_config(file);
                if !err {
                }
            }
            Err(why) => {
                Err(AppError::new(format!("{:?} - New default config will be created.", why.kind()).red()))
            }
        }
    }
}