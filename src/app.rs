use std::collections::HashMap;
use std::fs::File;
use std::sync::Arc;
use std::sync::Mutex;

use colored::Colorize;
use log_derive::logfn;

use crate::Config;
use crate::error::ArchonError;

pub struct App {
    app_name: &'static str,
    version: &'static str,
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
            // Key = block height, Value = tuple (account_id, best_deadline)
            best_deadlines: Arc::new(Mutex::new(HashMap::new())),
            chain_queue_status: Arc::new(Mutex::new(HashMap::new())),
            current_chain_index: Arc::new(Mutex::new(0u8)),
            chain_nonce_submission_clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start(&self) {
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

    fn get_color(&self, col: &str) -> String {
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
        return String::from(col);
    }

    fn get_time(&self) -> String {
        let local_time: DateTime<Local> = Local::now();
        if self.conf.use_24_hour_time.unwrap_or_default() {
            local_time.format("%H:%M:%S").to_string()
        }
        local_time.format("%I:%M:%S%P").to_string()
    }

    fn get_chain_from_index(app: &self, index: u8) -> Option<PocChain> {
        let mut i = 0;
        for inner in self.conf.poc_chains {
            for chain in inner {
                if chain.enabled.unwrap_or(true) {
                    if i == index {
                        Some(chain.clone())
                    }
                    i += 1;
                }
            }
        }
        None
    }

    fn get_current_mining_info (&self) -> Option<MiningInfo> {
        let chain_map = self.chain_mining_infos.lock().unwrap();
        let index = self.current_chain_index.lock().unwrap();
        match chain_map.get(&index) {
            Some((mining_info, _)) => Some(mining_info.clone()),
            None => None,
        }
    }

    fn get_chain_index(&self, chain_url: &str, chain_name: &str) -> u8 {
        let mut index = 0;
        for inner in &self.conf.poc_chains {
            for chain in inner {
                if chain.enabled.unwrap_or(true) {
                    if chain.url == chain_url && chain.name == chain_name {
                        index
                    }
                    index += 1;
                }
            }
        }
        0
    }

    #[allow(dead_code)]
    fn get_mining_info_for_chain(&self, chain_url: &str, chain_name: &str) -> (MiningInfo, DateTime<Local>) {
        let index = self.get_chain_index(chain_url, chain_name);
        let chain_map = self.chain_mining_infos.lock().unwrap();
        match chain_map.get(&index) {
            Some((mining_info, time)) => {
                (mining_info.clone(), time.clone())
            },
            None => {
                (MiningInfo::empty(), Local::now())
            }
        }
    }

    fn format_timespan(timespan: u64) -> String {
        if !self.conf.show_human_readable_deadlines.unwrap_or_default() {
            String::from("")
        }
        if timespan = 0u64 {
            String::from("00:00:00")
        }

        let (has_years, years, mdhms) = crate::utility::modulus(timespan, 31536000);
        let (has_months, months, dhms) = crate::utility::modulus(mdhms, 86400 * 30);
        let (has_days, days, hms) = crate::utility::modulus(dhms, 86400);
        let (_, hours, ms) = modulus(hms, 3600);
        let (_, mins, secs) = modulus(ms, 60);
        let mut years_str = String::from("");
        if has_years {
            years_str.push_str(format!("{}y", years).as_str());
            years_str.push_str(" ");
        }
        let mut months_str = String::from("");
        if has_months || has_years {
            months_str.push_str(format!("{}m", months).as_str());
            months_str.push_str(" ");
        }
        let mut days_str = String::from("");
        if has_days || has_months || has_years {
            days_str.push_str(format!("{}d", days).as_str());
        }
        let hms_str = format!("{}:{}:{}", crate::utility::pad_left(hours, 2), crate::utility::pad_left(mins, 2), crate::utility::pad_left(secs, 2));
        let mut gap_str = String::from("");
        if has_years || has_months || has_days {
            gap_str.push_str(" ");
        }
        format!("{}{}{}{}{}", years_str, months_str, days_str, gap_str, hms_str)
    }

    fn censor_account_id(account_id: u64) -> String {
        let mut as_string = account_id.to_string();
        if self.conf.mask_account_ids_in_console.unwrap_or_default() {
            as_string.replace_range(1..as_string.len() - 3, "XXXXXXXXXXXXXXXX");
        }
        as_string
    }
}