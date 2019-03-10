use std::fs::File;

use log_derive::logfn;

use crate::Config;
use crate::error::ArchonError;

pub struct App {
    conf: Config,
}

impl App {
    //Constructor
    pub fn new() -> App {}

    //Utilities
    #[logfn(Err = "Error", fmt = "Failed to load config file: {:?}")]
    fn load_config() -> Result<Config, ArchonError> {
        let c: Config = match File::open("archon.yaml") {
            Ok(file) => {
                let (conf, err) = Config::try_parse_config(file);
                if !err {
                }
            }
            Err(why) => {
                Err(ArchonError::new(format!("{:?} - New default config will be created.", why.kind()).red()))
            }
        };
    }
}