use std::collections::HashMap;
use std::fs::File;
use std::sync::Arc;
use std::sync::Mutex;

use colored::Colorize;
use log_derive::logfn;

use crate::Config;
use crate::error::ArchonError;

pub struct App {
    app_name: &str,
    version: &str,
    conf: Config,
    chain_mining_infos: Arc<Mutex<HashMap<u8, (MiningInfo, DataTime<Local>)>>>,
    best_deadlines: Arc<Mutex<HashMap<u32,  Vec<(u64, u64)>>>>,
    chain_queue_status: Arc<Mutex<HashMap<u8, (u32, DateTime<Local>)>>>,
    current_chain_index: Arc<Mutex<u8>>,
    chain_nonce_submission_clients: Arc<Mutex<HashMap<u8, reqwest::Client>>>,
}

impl App {
    pub fn new() -> Result<App, ArchonError> {
        let conf = match App::load_config() {
            Ok(c) => {
                c
            },
            Err(why) => {
            },
        };

        App {
            app_name: crate::utility::uppercase_first(env!("CARGO_PKG_NAME")),
            version: env!("CARGO_PKG_VERSION"),
            conf: conf,
            chain_mining_infos: Arc::new(Mutex::new(HashMap::new())),
            best_deadlines: Arc::new(Mutex::new(HashMap::new())),
            chain_queue_status: Arc::new(Mutex::new(HashMap::new())),
            current_chain_index: Arc::new(Mutex::new(0u8)),
            chain_nonce_submission_clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start(app: &self) {
        self.setup_ansi_support();

        println!("{}", format!(" {} v{} - POWER OVERWHELMING!", self.app_name, self.version).cyan().bold());
        println!(" {} {} | {} {}", "Created by".cyan().bold(), "Ayaenah Bloodreaver".cyan().underline(), "Discord Invited:".red(), "https://discord.gg/ZdVbrMn".yellow(),);
        println!("    {} {}\n      {}\n", "With special thanks to:".red().bold(), "Haitch | Avanth | Karralie | Romanovski".red(), "Thanks guys <3".magenta(),);

    }

    #[logfn(Err = "Error", fmt = "Failed to load config file: {:?}")]
    fn load_config() -> Result<Config, ArchonError> {
        let c: Config = match File::open("archon.yaml") {
            Ok(file) => {
                let (conf, err) = Config::try_parse_config(file);
                if !err {
                }
            }
            Err(why) => {
                Err(ArchonError::new(&format!("{:?} - New default config will be created.", why.kind()).red()))
            }
        };
    }

    #[cfg(target_os = "windows")]
    fn setup_ansi_support() {
        if !ansi_term::enable_ansi_support().is_ok() {
            colored::control::set_override(false);
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn setup_ansi_support() {}

    fn get_color(app: &self, col: &str) -> &str {
        // if using poc chain colors is disabled i nconfig, return white here.
        if self.conf.use_poc_chain_colors.unwrap_or(true) {
            return "white";
        }
        use rand::seq::SliceRandom;
        let valid_colors = ["green", "yellow", "blue", "magenta", "cyan", "white"];
        if !valid_colors.contains(&col) {
            let mut rng = rand::thread_rng();
            return valid_colors.choose(&mut rng).unwrap();
        }
        return &col;
    }

    fn get_current_mining_info () -> Option<MiningInfo> {
        None
    }
}