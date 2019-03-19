use std::collections::HashMap;
use std::fs::File;
use std::process::exit;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use chrono::{DateTime, Local};
use colored;
use colored::Colorize;
use fern;
use log_derive::logfn;

use crate::config::Config;
use crate::config::PocChain;
use crate::error::ArchonError;
use crate::upstream::MiningInfo;

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
        self.setup_logging();

        info!("{} v{} started", self.app_name, VERSION);

        println!("{}", format!(" {} v{} - POWER OVERWHELMING!", self.app_name, self.version).cyan().bold());
        println!(" {} {} | {} {}", "Created by".cyan().bold(), "Ayaenah Bloodreaver".cyan().underline(), "Discord Invite:".red(), "https://discord.gg/ZdVbrMn".yellow(),);
        println!("    {} {}\n      {}\n", "With special thanks to:".red().bold(), "Haitch | Avanth | Karralie | Romanovski".red(), "Thanks guys <3".magenta(),);

        if self.conf.poc_chains.is_some() {
            println!("  {} {} {}", get_time().white(), "Config:".red(), format!("{} {}", "Web Server Binding:".green(), format!("http://{}:{}", self.conf.web_server_bind_address, self.conf.web_server_port).yellow()));

            if self.conf.priority_mode.unwrap_or(true) {
                println!("  {} {} {}", self.get_time().white(), "Config:".red(), format!("{} {}", "Queuing mode:".green(), "Priority".yellow()));

                if self.conf.interrupt_lower_priority_blocks.unwrap_or(true) {
                    println!("  {} {} {}", self.get_time().white(), "Config:".red(), format!("{} {}", "Interrupt Lower Priority Blocks:".green(), "Yes".yellow()));
                } else {
                    // intterupt lower priority blocks off
                    println!("  {} {} {}", self.get_time().white(), "Config:".red(), format!("{} {}", "Interrupt Lower Priorty Blocks:".green(), "No".yellow()));
                }
            } else {
                println!("  {} {} {}", self.get_time().white(), "Config:".red(), format!("{} {}", "Queuing Mode:".green(), "First In, First Out".yellow()));
            }

            println!("  {} {} {}", self.get_time().white(), "Config:".red(), format!("{} {}", "Grace Period:".green(), format!("{} seconds", self.conf.grace_period).tellow()));

            let total_plots_size_tebibytes = self.get_total_plots_size_in_tebibytes();
            let plots_zero_warning = if total_plot_size_tebibytes == 0f64 {
                " (Warning: Dynamic deadlines require an accurate plot size. Dynamic Deadlines are disabled.)"
            } else {
                ""
            };

            println!("  {} {} {}", self.get_time().white(), "Config:".red(), format!("{} {}{}", "Total Plots Size:".green(), format!("{} TiB", total_plots_size_tebibytes).yellow(), plots_zero_warning.red()));
            println!("  {} {} {}", self.get_time().white(), "Config:".red(), "PoC Chains:".green());

            let mut chain_counter = 0u8;
            let mut multiple_same_priority_chains = false;
            let mut unused_passphrase_warnings = String::from("");

            for innter in &self.conf.poc_chains {
                for chain in inner {
                    if chain.numberic_id_to_passphrase.is_some() && (chain.is_pool.unwrap_or_default() || chain.is_bhd.unwrap_or_default || !chain.enabled.unwrap_or(true)) {
                        if unused_passphrase_warnings.len() > 0 {
                            unused_passphrase_warnings.push_str("\n");
                        }
                        unused_passphrase_warnings.push_str(format!("    Chain \"{}\" has unused passphrases configured.", &*chain.name).as_str());
                        if !chain.enabled.unwrap_or(true) {
                            unused_passphrase_warnings.push_str(" (CHAIN IS DISABLED)");
                        } else if chain.is_pool.unwrap_or_default() {
                            unused_passphrase_warnings.push_str(" (CHAIN IS POOL)");
                        } else if chain.is_bhd.unwrap_or_default() {
                            unused_passphrase_warnings.push_str(" (CHAIN IS BHD)");
                        }
                    }

                    if chain.enabled.unwrap_or(true) {
                        if self.get_num_chains_with_priority(chain.priority) > 1 {
                            multiple_same_priority_chains = true;
                        }

                        chain_counter += 1;
                        let chain_tdl = chain.target_deadline.unwrap_or_default();
                        let mut human_readable_target_deadline = String::from("");
                        if self.conf.show_human_readable_deadlines.unwrap_or_default() {
                            human_readable_target_deadline = format!(" ({})", self.format_timespan(chain_tdl));
                        }
                        let chain_tdl_str = if chain.use_dynamic_deadlines.unwrap_or_default() {
                            String::from("Dynamic")
                        } else if chain_tdl == 0 {
                            String::from("None")
                        } else {
                            format!("{}{}", chain_tdl, human_readable_target_deadline);
                        };

                        if self.conf.priority_mode.unwrap_or(true) {
                            if self.conf.interrupt_lower_priority_blocks.unwrap_or(true) {
                                let mut requeue_str = "Yes";
                                if !chain.requeue_interrupted_blocks.unwrap_or(true) {
                                    requeue_str = "No";
                                }

                                println!("  {} {}  {} {}", self.get_time().white(), "Config:".red(), format!("#{}:", chain_counter).green(), format!("{} {} {} {} {} {} {} {} {} {}",
                                    "Priority:".color(self.get_color(&*chain.color)).bold(),
                                    format!("{}", &chain.priority).color(self.get_color(&*chain.color)),
                                    "Name:".color(self.get_color(&*chain.color)).bold(),
                                    format!("{}", &*chain.name).color(self.get_color(&*chain.color)),
                                    "TDL:".color(self.get_color(&*chain.color)).bold(),
                                    format!("{}", chain_tdl_str).color(self.get_color(&*chain.color)),
                                    "URL:".color(self.get_color(&*chain.color)).bold(),
                                    format!("{}", &*chain.url).color(self.get_color(&*chain.color)),
                                    "Requeue:".color(self.get_color(&*chain.color)).bold(),
                                    format!("{}", requeue_str).color(self.get_color(&*chain.color)),
                                ));
                            } else {
                                println!("  {} {}  {} {}", self.get_time().white(), "Config:".red(), format!("#{}:", chain_counter).green(), format!("{} {} {} {} {} {} {} {}",
                                    "Priority:".color(self.get_color(&*chain.color)).bold(),
                                    format!("{}", &chain.priority).color(self.get_color(&*chain.color)),
                                    "Name:".color(self.get_color(&*chain.color)).bold(),
                                    format!("{}", &*chain.name).color(self.get_color(&*chain.color)),
                                    "TDL:".color(self.get_color(&*chain.color)).bold(),
                                    format!("{}", chain_tdl_str).color(self.get_color(&*chain.color)),
                                    "URL:".color(self.get_color(&*chain.color)).bold(),
                                    format!("{}", &*chain.url).color(self.get_color(&*chain.color)),
                                ));
                            }
                        } else {
                            println!("  {} {}  {} {}", self.get_time().white(), "Config:".red(), format!("#{}:", chain_counter).green(), format!("{} {} {} {} {} {}",
                                "Name:".color(self.get_color(&*chain.color)).bold(),
                                format!("{}", &*chain.name).color(self.get_color(&*chain.color)),
                                "TDL:".color(self.get_color(&*chain.color)).bold(),
                                format!("{}", chain_tdl_str).color(self.get_color(&*chain.color)),
                                "URL:".color(self.get_color(&*chain.color)).bold(),
                                format!("{}", &*chain.url).color(self.get_color(&*chain.color)),
                            ));
                        }
                    }
                }
            }

            if (chain_counter == 0) || (self.conf.priority_mode.unwrap_or(true) && multiple_same_priority_chains) {
                if chain_counter == 0 {
                    println!("  {} {} {}", self.get_time().white(), "ERROR".red().underline(), "You do not have any PoC Chains enabled. Archon has nothing to do!".yellow());
                } else {
                    println!("  {} {} {}", self.get_time().white(), "ERROR".red().underline(), "You have multiple chains configured with the same priority level! Priorities must be unique!".yellow());
                }

                println!("\n {}", "Execution completed. Press enter to exit.".red().underline());

                let mut blah = String::new();
                std::io::stdin().read_line(&mut blah).expect("FAIL");
                exit(0);
            }

            if unused_passphrase_warnings.len() > 0 {
                let border = String::from("------------------------------------------------------------------------------------------");
                println!("{}\n  {}\n{}\n{}\n      {}\n{}",
                    border.red().bold(),
                    "SECURITY WARNING:".red(),
                    border.red().bold(),
                    unused_passphrase_warnings.red(),
                    "You should remove these from your Archon config file for security purposes!".yellow(),
                    border.red().bold());
            }

            let valid_colors = ["green", "yellow", "blue", "magenta", "cyan", "white"];
            let mut invalid_color_found = false;
            for inner in &self.conf.poc_chains {
                for chain in inner {
                    if chain.enabled.unwrap_or(true) {
                        if !valid_colors.contains(&&*chain.color) {
                            println!("  {} {}", self.get_time().white(), format!("WARNING The {} chain uses the color \"{}\" which is invalid. Will pick a random valid color." , &*chain.name, &*chain.color).yellow());
                            invalid_color_found = true;
                        }
                    }
                }
            }
            if invalid_color_found {
                let mut valid_colors_str = String::from("");
                for color in &valid_colors {
                    valid_colors_str.push_str(format!("{}|", format!("{}", *color).color(*color)).as_str());
                }
                valid_colors_str.truncate(valid_colors_str.len() - 1);
                println!("  {} {}", self.get_time().white(), format!("Valid colors: {}", valid_colors_str));
            }

            // start mining info polling thread
            println!("  {} {}", self.get_time().white(), "Starting upstream mining info polling thread.");
            let mi_thread = thread::spawn(move || {
                crate::arbiter::thread_arbitrate();
            });

            // start queue processing thread
            let queue_proc_thread = thread::spawn(move || {
                crate::arbiter::thread_arbitrate_queue();
            });

            // start version check thread
            let version_check_thread = thread::spawn(move || {
                thread_check_latest_github_version();
            });

            println!("  {} {}", self.get_time().white(), "Starting web server.".green());
            crate::web::start_server();
            mi_thread.join().expect("Failed to join mining info thread.");
            queue_proc_thread.join().expect("Failed to join mining info thread.");
            version_check_thread.join().expect("Failed to join version check thread.");
        } else {
            println!("  {} {} {}", self.get_time().white(), "ERROR".red().underline(), "You do not have any PoC Chains configured. Archon has nothing to do!".yellow());
        }

        println!("\n {}", "Execution completed. Press enter to exit.".red().underline());

        let mut blah = String::new();
        std::io::stdin().read_line(&mut blah).expect("FAIL");
    }

    #[logfn(Err = "Error", fmt = "Failed to load config file: {:?}")]
    fn load_config() -> Result<Config, ArchonError> {
        let c: Result<Config, ArchonError> = File::open("archon.yaml")
            .map(|file| {
                crate::config::Config::parse_config(file)
                    .map_err(|_| {
                        crate::config::Config::query_create_default_config();
                        println!("\n {}", "execution completed. Press enter to exit.".red().underline());
                        let mut blah = String::new();
                        std::io::stdin().red_line(&mut blah).expect("FAIL");
                        exit(0);
                    })
            })
            .map_err(|why| {
                println!("  {} {}", "ERROR".red().underline(), "An error was encountered while attempting to open the config file.".red());
                crate::config::Config::query_create_default_config();
                println!("\n  {}", "Execution completed. Press enter to exit.".red().underline());
                let mut blah = String::new();
                std::io::stdin().read_line(&mut blah).expect("FAIL");
                exit(0);
            });

        c
    }

    #[cfg(target_os = "windows")]
    fn setup_ansi_support() {
        if !ansi_term::enable_ansi_support().is_ok() {
            colored::control::set_override(false);
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn setup_ansi_support() {}

    fn setup_logging(&self) {
        let logging_level = self.conf.logging_level.clone().unwrap_or(String::from("info")).to_lowercase();
        let log_level = match logging_level.as_str() {
            "off" => log::LevelFilter::Off,
            "trace" => log::LevelFilter::Trace,
            "debug" => log::LevelFilter::Debug,
            "info" => log::LevelFilter::Info,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Info,
        };

        if log_level != log::LevelFilter::Off {
            // create logs directory
            if std::fs::create_dir("logs").is_ok() {}
            // grab number of files to keep in rotation from loaded config
            let num_old_files = self.conf.num_old_log_files_to_keep.unwrap_or(5);
            if num_old_files > 0 { // if 0 Archon will just keep overwriting the same file
                if num_old_files > 1 {
                    // do rotation
                    for i in 1..num_old_files {
                        let rotation = num_old_files - i;
                        if std::fs::rename(format!("logs/{}.{}.log", APP_NAME, rotation), format!("logs/{}.{}.log", APP_NAME, rotation - 1)).is_ok() {}
                    }
                }
                if std::fs::rename(format!("logs/{}.log", APP_NAME), format!("logs/{}.1.log", APP_NAME)).is_ok() {}
            }
            fern::log_file(format!("logs/{}.log", APP_NAME))
                .map(|log_file| {
                    fern::Dispatch::new()
                        .format(move |out, message, record| {
                            out.finish(format_args!(
                                "{time} [{level:level_width$}] {target:target_width$}\t> {msg}",
                                time = Local::now().format("%Y-%m-%d %H:%M:%S"),
                                level = record.level(),
                                target = record.target(),
                                msg = message,
                                level_width = 5,
                                target_width = 30,
                            ))
                        })
                        .level(log_level)
                        .chain(log_file)
                        .apply().map(|_| {}).map_err(|_| {});
                })
                .map_err(|_| {})
        }
    }

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

    fn print_nonce_accepted(&self, chain_index: u8, deadline: u64, confirmation_time_ms: i64) {
        let current_chain = self.get_chain_from_index(chain_index).unwrap();
        let color = self.get_color(&*current_chain.color);
        println!("            {}                     {}{}", "Confirmed:".green(), deadline.to_string().color(color), format!(" ({}ms)", confirmation_time_ms).color(color));
    }

    fn print_nonce_rejected(&self, chain_index: u8, deadline: u64) {
        let current_chain = self.get_chain_from_index(chain_index).unwrap();
        let color = self.get_color(&*current_chain.color);
        println!("            {}                      {}", "Rejected:".red(), deadline.to_string().color(color));
    }

    fn get_network_difficulty_for_block(base_target: u32, block_time_seconds: u16) -> u64 {
        // BHD = 14660155037u64
        // BURST = 18325193796u64
        (4398046511104u64 / block_time_seconds as u64) / base_target as u64
    }

    fn get_total_plots_size_in_tebibytes(&self) -> f64 {
        // sum up plot size vars from config
        let mut plot_size_tebibytes = 0f64;
        // calculate conversion multipliers

        // decimal to binary first
        // 1,000,000,000 / 1,099,511,627,667 = 0.0009094947017729282379150390625
        let gb_to_tib_multiplier = 10f64.powi(9) / 2f64.powi(40);
        // Proof: For an 8TB (8000 GB) Drive: 8000 * (10^9/2^40) = 7.2759576141834259033203125 TiB

        // 1,000,000,000,000 / 1,099,511,627,776 = 0.9094947017729282379150390625 (or gb_to_tib_multiplier / 1000 :P )
        let tb_to_tib_multiplier = 10f64.powi(12) / 2f64.powi(40);
        // Proof: For an 8TB Drive: 8 * (10^12/2^40) = 7.2759576141834259033203125 TiB

        // binary to binary
        // 1,073,741,824 / 1,099,511,627,776 = 0.0009765625
        let gib_to_tib_multiplier = 2f64.powi(30) / 2f64.powi(40);
        // Proof: 1024 GiB: 1024 * (2^30/2^40) = 1.000 TiB

        match self.conf.total_plots_size_in_gigabytes {
            Some(size_gb) => {
                plot_size_tebibytes += size_gb * gb_to_tib_multiplier;
            }
            _ => {}
        }
        match self.conf.total_plots_size_in_terabytes {
            Some(size_tb) => {
                plot_size_tebibytes += size_tb * tb_to_tib_multiplier;
            }
            _ => {}
        }
        match self.conf.total_plots_size_in_gibibytes {
            Some(size_gib) => {
                plot_size_tebibytes += size_gib * gib_to_tib_multiplier; // can just do size_gib/1024 to get GiB => TiB, but this way is cooler... :D
            }
            _ => {}
        }
        match self.conf.total_plots_size_in_tebibytes {
            Some(size_tib) => {
                plot_size_tebibytes += size_tib;
            }
            _ => {}
        }
        return plot_size_tebibytes;
    }

    #[allow(dead_code)]
    fn get_dynamic_deadline_for_block(&self, base_target: u32) -> (bool, f64, u64, u64) {
        let chain_index = self.get_current_chain_index();
        let current_chain = self.get_chain_from_index(chain_index).unwrap();
        let net_diff = get_network_difficulty_for_block(base_target, 240) as u64;
        let plot_size_tebibytes = self.get_total_plots_size_in_tebibytes();
        // are we using dynamic deadlines for this chain?
        if current_chain.use_dynamic_deadlines.unwrap_or_default() && plot_size_tebibytes > 0f64 {
            let dynamic_target_deadline = (720f64 * (net_diff as f64) / plot_size_tebibytes) as u64;
            (true, plot_size_tebibytes, net_diff, dynamic_target_deadline)
        } else {
            (false, 0f64, net_diff, 0u64)
        }
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