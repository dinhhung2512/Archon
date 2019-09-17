#![feature(vec_remove_item)]

use ansi_term::{Colour, Colour::RGB, Colour::Fixed};
use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::fs::File;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread;
use fern;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate log;

#[macro_use]
extern crate lazy_static;

pub mod arbiter;
pub mod config;
pub mod upstream;
pub mod web;
pub mod error;
use crate::config::Config;
use crate::config::PocChain;
use crate::upstream::MiningInfo;
use arbiter::HDPoolSubmitNonceInfo;

const APP_NAME: &'static str = env!("CARGO_PKG_NAME");
const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
enum TargetDeadlineType {
    PoolMaximum(u64),
    ConfigChainLevel(u64),
    ConfigOverriddenByID(u64),
    Dynamic(u64),
    Default,
}

#[derive(Debug)]
enum LastBlockInfo {
    Requeued(Option<(u8, u8)>, u64, u8),
    Superseded(u64, u8),
    Forked(u64, u8),
    Completed(u64, u8),
    Interrupted(u64, u8),
}

lazy_static! {
    static ref CHAIN_MINING_INFOS: Arc<Mutex<HashMap<u8, (MiningInfo, DateTime<Local>)>>> = {
        let chain_mining_infos = HashMap::new();
        Arc::new(Mutex::new(chain_mining_infos))
    };
    static ref MINING_INFO_CACHE: Arc<Mutex<HashMap<u8, (u32, String, u64)>>> = {
        let mining_info_cached_map = HashMap::new();
        Arc::new(Mutex::new(mining_info_cached_map))
    };
    static ref BLOCK_START_PRINTED: Arc<Mutex<HashMap<u8, u32>>> = {
        let block_start_printed_map = HashMap::new();
        Arc::new(Mutex::new(block_start_printed_map))
    };
    static ref HDPOOL_SUBMIT_NONCE_SENDER_BHD: Arc<Mutex<Option<crossbeam::channel::Sender<HDPoolSubmitNonceInfo>>>> = Arc::new(Mutex::new(None));
    static ref HDPOOL_SUBMIT_NONCE_SENDER_LHD: Arc<Mutex<Option<crossbeam::channel::Sender<HDPoolSubmitNonceInfo>>>> = Arc::new(Mutex::new(None));
    // Key = block height, Value = tuple (account_id, best_deadline)
    static ref BEST_DEADLINES: Arc<Mutex<HashMap<u32, Vec<(u64, u64)>>>> = {
        let best_deadlines = HashMap::new();
        Arc::new(Mutex::new(best_deadlines))
    };
    static ref CHAIN_QUEUE_STATUS: Arc<Mutex<HashMap<u8, (u32, DateTime<Local>)>>> = {
        let chain_queue_status = HashMap::new();
        Arc::new(Mutex::new(chain_queue_status))
    };
    static ref CURRENT_CHAIN_INDEX: Arc<Mutex<u8>> = Arc::new(Mutex::new(0u8));
    static ref LAST_MINING_INFO: Arc<Mutex<String>> = Arc::new(Mutex::new(String::from("")));
    static ref CHAIN_NONCE_SUBMISSION_CLIENTS: Arc<Mutex<HashMap<u8, reqwest::Client>>> = {
        let chain_nonce_submission_clients = HashMap::new();
        Arc::new(Mutex::new(chain_nonce_submission_clients))
    };
    static ref CHAIN_REQUEUE_TIMES: Arc<Mutex<HashMap<u8, (u32, u8)>>> = {
        let chain_requeue_times_map = HashMap::new();
        Arc::new(Mutex::new(chain_requeue_times_map))
    };
    static ref CONNECTED_MINER_DATA: Arc<Mutex<HashMap<u32, (f64, String, DateTime<Local>)>>> = {
        let connected_miner_data = HashMap::new();
        Arc::new(Mutex::new(connected_miner_data))
    };
    #[derive(Serialize)]
    pub static ref CONF: Config = {
        let c: Config = match File::open("archon.yaml").map(|file| {
            Config::parse_config(file).map_err(|why| {
                println!("  {}", why);
                query_create_default_config();
                println!("\n  {}", Colour::Red.underline().paint("Execution completed. Press enter to exit."));
                let mut blah = String::new();
                std::io::stdin().read_line(&mut blah).expect("FAIL");
                exit(0);
            })
        }).map_err(|why| {
            println!("  {} {}\n  {}", Colour::Red.underline().paint("ERROR"), Colour::Red.paint("An error was encountered while attempting to open the config file."), why);
            query_create_default_config();
            println!("\n  {}", Colour::Red.underline().paint("Execution completed. Press enter to exit."));
            let mut blah = String::new();
            std::io::stdin().read_line(&mut blah).expect("FAIL");
            exit(0)
        }) {
            Ok(data) => data.unwrap(),
            Err(_) => { unreachable!(); },
        };

        c
    };
}

fn main() {
    // set up ansi support if user is running windows
    setup_ansi_support();

    let app_name = uppercase_first(APP_NAME);

    // setup logging
    let (archon_logging_info, archon_log_warn, dep_logging_info, dep_log_warn) = setup_logging();

    info!("{} v{} started", app_name, VERSION);

    println!("{}", Colour::Cyan.bold().paint(format!("  {} v{} - POWER OVERWHELMING!", app_name, VERSION)));
    println!("  {} {} | {} {}",
        Colour::Cyan.bold().paint("Created by"),
        Colour::Cyan.underline().paint("Ayaenah Bloodreaver"),
        Colour::Red.paint("Discord Invite:"),
        Colour::Yellow.paint("https://discord.gg/ZdVbrMn"),
    );
    println!("    {} {}",
        Colour::Red.bold().paint("With special thanks to:"),
        Colour::Red.paint("Haitch | Avanth | Karralie | Romanovski")
    );
    println!("      {}",
        Colour::Purple.paint("Thanks guys! <3"),
    );
    if crate::CONF.poc_chains.is_some() {
        let archon_logging_warning = match archon_log_warn {
            true => "\n    WARNING: Log files can get very large, very quickly using this level!",
            false => "",
        };
        println!("\n  {} {} {}", 
            get_time(), 
            Colour::Red.paint("Config:"),
            format!("{} {}{}",
                Colour::Green.paint("Archon Logging Level:"),
                Colour::Yellow.paint(archon_logging_info),
                Colour::Red.paint(archon_logging_warning)
            )
        );

        if dep_logging_info.len() > 0 {
            let dep_logging_warning = match dep_log_warn {
                true => "\n    WARNING: Log files can get very large, very quickly using this level!",
                false => "",
            };
            println!("  {} {} {}", 
                get_time(), 
                Colour::Red.paint("Config:"), 
                format!("{} {}{}",
                    Colour::Green.paint("Dependency Logging Level:"),
                    Colour::Yellow.paint(dep_logging_info),
                    Colour::Red.paint(dep_logging_warning)
                )
            );
        }

        println!("  {} {} {}",
            get_time(),
            Colour::Red.paint("Config:"),
            format!("{} {}",
                Colour::Green.paint("Web Server Binding:"),
                Colour::Yellow.paint(format!("http://{}:{}",
                    crate::CONF.web_server_bind_address,
                    crate::CONF.web_server_port
                ))
            )
        );
        if crate::CONF.priority_mode.unwrap_or(true) {
            println!("  {} {} {}",
                get_time(),
                Colour::Red.paint("Config:"),
                format!("{} {}", Colour::Green.paint("Queuing Mode:"), Colour::Yellow.paint("Priority"))
            );
            if crate::CONF.interrupt_lower_priority_blocks.unwrap_or(true) {
                println!("  {} {} {}",
                    get_time(),
                    Colour::Red.paint("Config:"),
                    format!("{} {}", Colour::Green.paint("Interrupt Lower Priority Blocks:"), Colour::Yellow.paint("Yes"))
                );
            } else {
                // interrupt lower priority blocks off
                println!("  {} {} {}",
                    get_time(),
                    Colour::Red.paint("Config:"),
                    format!("{} {}", Colour::Green.paint("Interrupt Lower Priority Blocks:"), Colour::Yellow.paint("No"))
                );
            }
        } else {
            println!("  {} {} {}",
                get_time(),
                Colour::Red.paint("Config:"),
                format!("{} {}", Colour::Green.paint("Queuing Mode:"), Colour::Yellow.paint("First In, First Out"))
            );
        }
        println!("  {} {} {}",
            get_time(),
            Colour::Red.paint("Config:"),
            format!("{} {}",
                Colour::Green.paint("Grace Period:"),
                Colour::Yellow.paint(format!("{} seconds", crate::CONF.grace_period))
            )
        );
        println!("  {} {} {}",
            get_time(),
            Colour::Red.paint("Config:"),
            Colour::Green.paint("PoC Chains:")
        );
        let mut chain_counter = 0u8;
        let mut multiple_same_priority_chains = false;
        let mut unused_passphrase_warnings = String::from("");
        let mut account_key_warnings = String::from("");
        let mut invalid_url_warnings = String::from("");
        for inner in &crate::CONF.poc_chains {
            for chain in inner {
                // check for configured passphrases which will not be used
                if chain.numeric_id_to_passphrase.is_some()
                    && (chain.is_pool.unwrap_or_default()
                        || chain.is_bhd.unwrap_or_default()
                        || chain.is_lhd.unwrap_or_default()
                        || !chain.enabled.unwrap_or(true))
                {
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
                    } else if chain.is_lhd.unwrap_or_default() {
                        unused_passphrase_warnings.push_str(" (CHAIN IS LHD)");
                    }
                }
                if chain.enabled.unwrap_or(true) {
                    let chain_color = get_color(chain.clone());
                    if get_num_chains_with_priority(chain.priority) > 1 {
                        multiple_same_priority_chains = true;
                    }
                    if (chain.is_hdpool.unwrap_or_default() || chain.is_hdpool_eco.unwrap_or_default()) && chain.is_hpool.unwrap_or_default() {
                        // fatal error - can't have chain defined as for both HPOOL and HDPOOL
                        println!("\n  {}", Colour::Red.underline().paint(format!("FATAL ERROR: The chain \"{}\" is defined as both HDPOOL and HPOOL. Pick one!", &*chain.name)));
                        error!("The chain \"{}\" is defined as both HDPOOL and HPOOL. Pick one!", &*chain.name);

                        println!("\n  {}", Colour::Red.underline().paint("Execution completed. Press enter to exit."));

                        let mut blah = String::new();
                        std::io::stdin().read_line(&mut blah).expect("FAIL");
                        exit(0);
                    }
                    if chain.is_hdpool.unwrap_or_default() && chain.is_hdpool_eco.unwrap_or_default() {
                        // fatal error - can't have chain defined as for both HDPOOL and HDPOOL ECO
                        println!("\n  {}", Colour::Red.underline().paint(format!("FATAL ERROR: The chain \"{}\" is defined as both HDPOOL and HDPOOL ECO. Pick one!", &*chain.name)));
                        error!("The chain \"{}\" is defined as both HDPOOL and HDPOOL ECO. Pick one!", &*chain.name);

                        println!("\n  {}", Colour::Red.underline().paint("Execution completed. Press enter to exit."));

                        let mut blah = String::new();
                        std::io::stdin().read_line(&mut blah).expect("FAIL");
                        exit(0);
                    }
                    // check if account key is defined if the chain is for mining via hpool or hdpool
                    let account_key = chain.account_key.clone().unwrap_or(String::from(""));
                    if chain.account_key.is_none() || account_key.len() == 0 {
                        if chain.is_hpool.unwrap_or_default() {
                            warn!("Chain \"{}\" is set for HPOOL mining, but has no account key defined!\n", &*chain.name);
                            account_key_warnings.push_str(format!("    Chain \"{}\" is set for HPOOL mining, but has no account key defined!\n", &*chain.name).as_str());
                        } else if chain.is_hdpool.unwrap_or_default() || chain.is_hdpool_eco.unwrap_or_default() {
                            warn!("Chain \"{}\" is set for HDPOOL mining, but has no account key defined!\n", &*chain.name);
                            account_key_warnings.push_str(format!("    Chain \"{}\" is set for HDPOOL mining, but has no account key defined!\n", &*chain.name).as_str());
                        }
                    }
                    // check if URL is present if this chain is NOT for HDPool Direct
                    if !((chain.is_hdpool.unwrap_or_default() || chain.is_hdpool_eco.unwrap_or_default()) && chain.account_key.is_some()) && chain.url.clone().len() == 0 {
                        invalid_url_warnings.push_str(format!("    Chain \"{}\" has no URL set when one is required! If you are trying to use HDPool direct mining, ensure your config names are correct, as they are CaSe SeNsItIvE!\n", &*chain.name).as_str());
                    }
                    // check if both payout address and account key are set
                    if chain.account_key.is_some() && chain.foxypool_payout_address.is_some() {
                        error!(r#"The chain "{}" has both an account key and payout address defined. If mining to foxy pool, only set Payout Address! If mining to HDPool / HPool / BPool, only set Account Key!"#, &*chain.name);
                        println!("\n  {}", Colour::Red.underline().paint(format!(r#"FATAL ERROR: The chain "{}" has both an account key and payout address defined. If mining to foxy pool, only set Payout Address! If mining to HDPool / HPool / BPool, only set Account Key!"#, &*chain.name)));

                        println!("\n  {}", Colour::Red.underline().paint("Execution completed. Press enter to exit."));

                        let mut blah = String::new();
                        std::io::stdin().read_line(&mut blah).expect("FAIL");
                        exit(0);
                    }
                    // check if account key is set for chains other than hpool / hdpool
                    if chain.account_key.is_some() && !(chain.is_hdpool.unwrap_or_default() || chain.is_hdpool_eco.unwrap_or_default() || chain.is_hpool.unwrap_or_default()) {
                        error!(r#"The chain "{}" has an accountKey defined, but the accountKey option is ONLY valid for HPool/BPool or HDPool. If mining to a Foxy Pool, use foxypoolPayoutAddress! If mining to HDPool / HPool / BPool, only set Account Key!"#, &*chain.name);
                        println!("\n  {}", Colour::Red.underline().paint(format!(r#"FATAL ERROR: The chain "{}" has an accountKey defined, but the accountKey option is ONLY valid for HPool/BPool or HDPool. If mining to a Foxy Pool, use foxypoolPayoutAddress. If mining to HDPool / HPool / BPool, use accountKey, and ensure you !"#, &*chain.name)));

                        println!("\n  {}", Colour::Red.underline().paint("Execution completed. Press enter to exit."));

                        let mut blah = String::new();
                        std::io::stdin().read_line(&mut blah).expect("FAIL");
                        exit(0);
                    }
                    chain_counter += 1;
                    let chain_tdl = chain.target_deadline.unwrap_or_default();
                    let mut human_readable_target_deadline = String::from("");
                    if crate::CONF.show_human_readable_deadlines.unwrap_or_default() {
                        human_readable_target_deadline = format!(" ({})", format_timespan(chain_tdl));
                    }
                    let chain_tdl_str;
                    if chain.use_dynamic_deadlines.unwrap_or_default() {
                        chain_tdl_str = String::from("Dynamic");
                    } else if chain_tdl == 0 {
                        chain_tdl_str = String::from("Not Set");
                    } else {
                        chain_tdl_str = format!("{}{}", chain_tdl, human_readable_target_deadline);
                    }
                    let chain_url;
                    if chain.is_hdpool.unwrap_or_default() && chain.account_key.is_some() {
                        chain_url = "HDPOOL (WebSocket Direct)";
                    } else if chain.is_hdpool_eco.unwrap_or_default() && chain.account_key.is_some() {
                        chain_url = "HDPOOL ECO (WebSocket Direct)";
                    } else {
                        chain_url = &chain.url;
                    }
                    if crate::CONF.priority_mode.unwrap_or(true) {
                        if crate::CONF.interrupt_lower_priority_blocks.unwrap_or(true) {
                            let requeue_str;
                            if !chain.requeue_interrupted_blocks.unwrap_or(true) {
                                requeue_str = Colour::Red.paint(String::from("No"));
                            } else {
                                let requeue_times_str = match chain.maximum_requeue_times {
                                    Some(max_requeues) => format!(" ({}x)", max_requeues),
                                    None => String::from("")
                                };
                                requeue_str = chain_color.paint(format!("Yes{}", requeue_times_str));
                            }
                            println!("{}",
                                format!("    {}\n      {} @ {}\n        {} {} | {} {} | {} {}",
                                    chain_color.paint(format!("Chain #{}:", chain_counter)),
                                    chain_color.bold().paint(format!("{}", &*chain.name)),
                                    chain_color.paint(format!("{}", &*chain_url)),
                                    chain_color.bold().paint("Priority:"),
                                    chain_color.paint(format!("#{}", &chain.priority + 1)),
                                    chain_color.bold().paint("Target Deadline:"),
                                    chain_color.paint(format!("{}", chain_tdl_str)),
                                    chain_color.bold().paint("Requeue:"),
                                    format!("{}", requeue_str),
                                )
                            );
                        } else {
                            println!("{}",
                                format!("    {}\n      {} @ {}\n        {} {} | {} {}",
                                    chain_color.paint(format!("Chain #{}:", chain_counter)),
                                    chain_color.bold().paint(format!("{}", &*chain.name)),
                                    chain_color.paint(format!("{}", &*chain_url)),
                                    chain_color.bold().paint("Priority:"),
                                    chain_color.paint(format!("#{}", &chain.priority + 1)),
                                    chain_color.bold().paint("Target Deadline:"),
                                    chain_color.paint(format!("{}", chain_tdl_str)),
                                )
                            );
                        }
                    } else {
                        println!("{}",
                            format!("    {}\n      {} @ {}\n        {} {}",
                                chain_color.paint(format!("Chain #{}:", chain_counter)),
                                chain_color.bold().paint(format!("{}", &*chain.name)),
                                chain_color.paint(format!("{}", &*chain_url)),
                                chain_color.bold().paint("Target Deadline:"),
                                chain_color.paint(format!("{}", chain_tdl_str)),
                            )
                        );
                    }
                }
            }
        }

        if chain_counter == 0
            || (CONF.priority_mode.unwrap_or(true) && multiple_same_priority_chains)
            || invalid_url_warnings.len() > 0
        {
            if chain_counter == 0 {
                println!("  {} {} {}",
                    get_time(),
                    Colour::Red.underline().paint("ERROR"),
                    Colour::Yellow.paint("You do not have any PoC Chains enabled. Archon has nothing to do!")
                );
                error!("There are no PoC Chains configured.");
            } else if CONF.priority_mode.unwrap_or(true) && multiple_same_priority_chains {
                println!("  {} {} {}",
                    get_time(),
                    Colour::Red.underline().paint("ERROR"),
                    Colour::Yellow.paint("You have multiple chains configured with the same priority level! Priorities must be unique!")
                );
                error!("Multiple PoC Chains are configured with the same priority level. Priority levels must be unique.");
            } else if invalid_url_warnings.len() > 0 {
                println!("\n  {} {} - Invalid Chain URL found\n{}",
                    get_time(),
                    Colour::Red.underline().paint("ERROR"),
                    Colour::Red.paint(&invalid_url_warnings),
                    );
                error!("Invalid chain url found: {}", &invalid_url_warnings);
            }

            println!("\n  {}", Colour::Red.underline().paint("Execution completed. Press enter to exit."));

            let mut blah = String::new();
            std::io::stdin().read_line(&mut blah).expect("FAIL");
            exit(0);
        }
        if unused_passphrase_warnings.len() > 0 {
            let border = String::from("------------------------------------------------------------------------------------------");
            println!("{}\n  {}\n{}\n{}\n      {}\n{}",
                Colour::Red.bold().paint(&border),
                Colour::Red.paint("SECURITY WARNING:"),
                Colour::Red.bold().paint(&border),
                Colour::Red.paint(&unused_passphrase_warnings),
                Colour::Yellow.paint("You should remove these from your Archon config file for security purposes!"),
                Colour::Red.bold().paint(&border),
            );
            warn!("Unused passphrases found in archon.yaml: {}", &unused_passphrase_warnings);
        }
        if account_key_warnings.len() > 0 {
            println!("\n  {}", Colour::Red.paint(format!("WARNING:\n{}", account_key_warnings)));
        }

        // start mining info polling thread
        println!("  {} {}", get_time(), "Starting upstream mining info polling thread.");
        let mi_thread = thread::spawn(move || {
            arbiter::thread_arbitrate();
        });
        // start queue processing thread
        let queue_proc_thread = thread::spawn(move || {
            arbiter::thread_arbitrate_queue();
        });
        // start version check thread
        let version_check_thread = thread::spawn(move || {
            thread_check_latest_githib_version();
        });
        // start capacity monitoring thread
        let capacity_monitor_thread = thread::spawn(move || {
            thread_monitor_capacity();
        });

        println!("  {} {}", get_time(), Colour::Green.paint("Starting web server.") );
        web::start_server();
        mi_thread.join().expect("Failed to join mining info thread.");
        queue_proc_thread.join().expect("Failed to join queue processing thread.");
        version_check_thread.join().expect("Failed to join version check thread.");
        capacity_monitor_thread.join().expect("Failed to join capacity monitor thread.");
    } else {
        println!("  {} {} {}", get_time(), Colour::Red.underline().paint("ERROR"), Colour::Yellow.paint("You do not have any PoC Chains configured. Archon has nothing to do!"));
    }

    println!("\n  {}", Colour::Red.underline().paint("Execution completed. Press enter to exit."));

    let mut blah = String::new();
    std::io::stdin().read_line(&mut blah).expect("FAIL");
}

#[cfg(target_os = "windows")]
fn setup_ansi_support() {
    if ansi_term::enable_ansi_support().is_ok() {}
    }

#[cfg(not(target_os = "windows"))]
fn setup_ansi_support() {}

fn get_logging_level_from_string(level_string: &str, default_level: Option<log::LevelFilter>) -> log::LevelFilter {
    match level_string {
        "off" => {
            log::LevelFilter::Off
        },
        "trace" => {
            log::LevelFilter::Trace
        },
        "debug" => {
            log::LevelFilter::Debug
        },
        "info" => {
            log::LevelFilter::Info
        },
        "warn" => {
            log::LevelFilter::Warn
        },
        "error" => {
            log::LevelFilter::Error
        },
        _ => default_level.unwrap_or(log::LevelFilter::Info),
    }
}

fn setup_logging() -> (String, bool, String, bool) {
    let logging_level = CONF.logging_level.clone().unwrap_or(String::from("info")).to_lowercase();
    let log_level = get_logging_level_from_string(&logging_level, None);
    // set warning if debug|trace level
    let logging_level_warning = log_level == log::LevelFilter::Debug || log_level == log::LevelFilter::Trace;
    let console_logging_message = format!("{}", Colour::Yellow.paint(format!("{}", uppercase_first(logging_level.as_str()))));
    if log_level != log::LevelFilter::Off {
        // setup dependency logging level
        let dep_logging_level = CONF.dependency_logging_level.clone().unwrap_or(String::from("info")).to_lowercase();
        let dependency_log_level = get_logging_level_from_string(&dep_logging_level, None);
        // set warning if debug|trace level
        let dep_logging_level_warning = dependency_log_level == log::LevelFilter::Debug || dependency_log_level == log::LevelFilter::Trace;
        let dependency_console_logging_message = format!("{}", Colour::Yellow.paint(format!("{}", uppercase_first(dep_logging_level.as_str()))));
        // create logs directory
        if std::fs::create_dir("logs").is_ok() {}
        // grab number of files to keep in rotation from loaded config
        let num_old_files = CONF.num_old_log_files_to_keep.unwrap_or(5);
        if num_old_files > 0 { // if 0 Archon will just keep overwriting the same file
            if num_old_files > 1 {
                // do rotation
                for i in 1..num_old_files {
                    let rotation = num_old_files - i;
                    if std::fs::rename(format!("logs/{}.{}.log", APP_NAME, rotation), format!("logs/{}.{}.log", APP_NAME, rotation + 1)).is_ok() {}
                }
            }
            if std::fs::rename(format!("logs/{}.log", APP_NAME), format!("logs/{}.1.log", APP_NAME)).is_ok() {}
        }
        match fern::log_file(format!("logs/{}.log", APP_NAME)) {
            Ok(log_file) => {
                match fern::Dispatch::new()
                    .format(move |out, message, record| {
                    out.finish(format_args!(
                        "{time}   [{level:level_width$}] {target:target_width$}\t> {msg}",
                        time = Local::now().format("%Y-%m-%d %H:%M:%S"),
                        level = record.level(),
                        target = record.target(),
                        msg = message,
                        level_width = 5,
                        target_width = 30
                    ))
                })
                .level(dependency_log_level)
                .level_for("archon", log_level)
                .level_for("archon::web", log_level)
                .level_for("archon::arbiter", log_level)
                .level_for("archon::upstream", log_level)
                .level_for("archon::config", log_level)
                .level_for("archon::error", log_level)
                .chain(log_file)
                .apply() {
                    Ok(_) => {},
                    Err(_) => {},
                };
            },
            Err(_) => {}
        }
        (console_logging_message, logging_level_warning, dependency_console_logging_message, dep_logging_level_warning)
    } else {
        (console_logging_message, logging_level_warning, String::from(""), false)
    }
}

fn thread_check_latest_githib_version() {
    use semver::Version;
    let current_version = Version::parse(VERSION).unwrap();
    // load github api - https://api.github.com/repos/Bloodreaver/Archon/releases
    let version_check_client = reqwest::Client::new();
    loop {
        let response: Result<serde_json::Value, String> = version_check_client.get("https://api.github.com/repos/Bloodreaver/Archon/releases")
            .send()
            .map(|mut response| {
                response.text().map(|json| {
                    serde_json::from_str(&json).unwrap()
                }).unwrap()
            })
            .map_err(|e| e.to_string() );
        match response {
            Ok(data) => {
                let mut tag_name_string = data[0]["tag_name"].to_string().clone();
                tag_name_string = tag_name_string.trim_matches('"').to_string();
                if tag_name_string.starts_with("v") {
                    tag_name_string = tag_name_string.trim_start_matches("v").to_string();
                }
                match Version::parse(tag_name_string.as_str()) {
                    Ok(latest) => {
                        if current_version < latest {
                            // if latest version is pre-release and current version is not
                            if current_version < latest && !current_version.is_prerelease() && latest.is_prerelease() {
                                let border = "------------------------------------------------------------------------------------------";
                                let headline = "  NEW PRE-RELEASE VERSION AVAILABLE ==> ";
                                println!("{}", Colour::Red.paint(format!("\n{}\n{}v{}\n{}\n    There is a new pre-release available on GitHub.\n      https://github.com/Bloodreaver/Archon/releases\n{}\n",
                                    border, headline, tag_name_string, border, border)));
                                info!("VERSION CHECK: Pre-release v{} is available on GitHub - See https://github.com/Bloodreaver/Archon/releases", tag_name_string);
                            } else // if latest is not pre-release or current & latest are both pre-release
                            if !latest.is_prerelease() || (latest.is_prerelease() && current_version.is_prerelease()) {
                                let border = "------------------------------------------------------------------------------------------";
                                let headline = "  NEW VERSION AVAILABLE ==> ";
                                println!("{}", Colour::Red.paint(format!("\n{}\n{}v{}\n{}\n    There is a new release available on GitHub. Please update ASAP!\n      https://github.com/Bloodreaver/Archon/releases\n{}\n",
                                    border, headline, tag_name_string, border, border)));
                                info!("VERSION CHECK: v{} is available on GitHub - See https://github.com/Bloodreaver/Archon/releases", tag_name_string);
                            }
                        } else {
                            info!("VERSION CHECK: Up to date. (Current = {}, Latest = {})", VERSION, tag_name_string);
                        }
                    },
                    Err(why) => warn!("VERSION CHECK: Unable to parse github version. (Current = {}, GH Tag = {} | Error={:?})", VERSION, tag_name_string, why),
                }
            },
            Err(why) => warn!("VERSION CHECK: Unable to check github version: {:?}", why)
        };
        // sleep for 30 mins
        std::thread::sleep(std::time::Duration::from_secs(1800));
    }
}

fn uppercase_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn get_color(chain: PocChain) -> Colour {
    // if using poc chain colors is disabled in config, return white here
    if !crate::CONF.use_poc_chain_colors.unwrap_or(true) {
        return Colour::White;
    }
    if chain.color.is_some() {
        match chain.color.unwrap().to_lowercase().as_str() {
            "black" => return Colour::Black,
            "red" => return Colour::Red,
            "green" => return Colour::Green,
            "yellow" => return Colour::Yellow,
            "blue" => return Colour::Blue,
            "magenta" => return Colour::Purple,
            "purple" => return Colour::Purple,
            "cyan" => return Colour::Cyan,
            "white" => return Colour::White,
            _ => {}
        }
    }
    if chain.color_num.is_some() {
        return Fixed(chain.color_num.unwrap());
    }
    if chain.color_rgb.is_some() {
        match chain.color_rgb.unwrap() {
            (r, g, b) => return RGB(r, g, b)
        }
    }
    if chain.color_hex.is_some() {
        // get values for red, green and blue
        let (r, g, b) = hex_to_rgb(chain.color_hex.unwrap());
        return RGB(r, g, b);
    }
    return RGB(255, 255, 255);
}

fn hex_to_rgb(hex: String) -> (u8, u8, u8) {
    if hex.len() == 6 || hex.len() == 7 {
        let mut hex_str = hex.clone();
        hex_str.retain(|c| is_hex_char(c));
        if hex_str.len() == 6 {
            let mut i = 0;
            let mut r = 0u8;
            let mut g = 0u8;
            let mut b = 0u8;
            for c in hex_str.chars() {
                let val = get_hex_value(c, i % 2 == 0);
                match i {
                    0..=1 => r += val,
                    2..=3 => g += val,
                    4..=5 => b += val,
                    _ => {}
                }
                i += 1;
            }
            return (r, g, b);
        }
    }
    return (255, 255, 255);
}

fn get_hex_value(c: char, sixteenth: bool) -> u8 {
    let base_value = match c {
        '0' => 0,
        '1' => 1,
        '2' => 2,
        '3' => 3,
        '4' => 4,
        '5' => 5,
        '6' => 6,
        '7' => 7,
        '8' => 8,
        '9' => 9,
        'a' => 10,
        'A' => 10,
        'b' => 11,
        'B' => 11,
        'c' => 12,
        'C' => 12,
        'd' => 13,
        'D' => 13,
        'e' => 14,
        'E' => 14,
        'f' => 15,
        'F' => 15,
        _ => 0,
    };
    if sixteenth {
        base_value * 16
    } else {
        base_value
    }
}

fn is_hex_char(c: char) -> bool {
    let valid_chars = String::from("0123456789abcdefABCDEF");
    valid_chars.contains(c)
}

fn get_cached_mining_info() -> Option<(u8, u32, String, u64)> {
    let cache_map = MINING_INFO_CACHE.lock().unwrap();
    let index = CURRENT_CHAIN_INDEX.lock().unwrap();
    match cache_map.get(&index) {
        Some((height, json, tdl)) => {
            Some((*index, *height, json.clone(), *tdl))
        },
        None => None,
    }
}

fn add_mining_info_to_cache(index: u8, mining_info: MiningInfo) -> String {
    let mut mod_mi = mining_info.clone();
    let tdl = match get_target_deadline(None, mod_mi.base_target, index, None) {
        TargetDeadlineType::PoolMaximum(val) => val,
        TargetDeadlineType::ConfigChainLevel(val) => val,
        TargetDeadlineType::ConfigOverriddenByID(val) => val,
        TargetDeadlineType::Dynamic(val) => val,
        TargetDeadlineType::Default => u64::max_value(),
    };
    mod_mi.target_deadline = tdl;
    let mining_info_json = mod_mi.to_json().to_string();
    debug!("CACHE - Chain #{} Block #{}: {:?}", index, mod_mi.height, mod_mi);
    let mut cache_map = MINING_INFO_CACHE.lock().unwrap();
    cache_map.insert(index, (mod_mi.height, mining_info_json.clone(), tdl));
    mining_info_json
}

fn is_block_start_printed(index: u8, height: u32) -> bool {
    let block_start_printed_map = BLOCK_START_PRINTED.lock().unwrap();
    match block_start_printed_map.get(&index) {
        Some(matched_height) => {
            *matched_height == height
        },
        _ => {
            false
        }
    }
}

/// Return previously cached mining info if present, or create a cache for current mining info and return that
fn get_current_mining_info_json() -> String {
    match get_cached_mining_info() {
        Some((index, height, mining_info_json, tdl)) => {
            let mi = match arbiter::get_current_chain_mining_info(index) {
                Some((mi, _)) => mi,
                _ => MiningInfo::empty()
            };
            let calculated_tdl = match get_target_deadline(None, mi.clone().base_target, index, None) {
                TargetDeadlineType::ConfigChainLevel(val) => val,
                TargetDeadlineType::ConfigOverriddenByID(val) => val,
                TargetDeadlineType::Default => u64::max_value(),
                TargetDeadlineType::Dynamic(val) => val,
                TargetDeadlineType::PoolMaximum(val) => val,
            };
            if tdl != calculated_tdl {
                add_mining_info_to_cache(index, mi);
            }
            // check if block start has been printed for this index & height
            //   TRUE: Go ahead and return the mining info
            //  FALSE: Return the previous mining info
            if is_block_start_printed(index, height) {
                mining_info_json.clone()
            } else {
                match get_current_mining_info() {
                    Some(mining_info) => {
                        if is_block_start_printed(index, mining_info.height) {
                            add_mining_info_to_cache(index, mining_info.clone())
                        } else {
                            let last_mining_info = LAST_MINING_INFO.lock().unwrap();
                            debug!("Chain #{} Block #{} is not printed to console yet, sending last mining info: {}", index, height, last_mining_info.clone());
                            last_mining_info.clone()
                        }
                    },
                    _ => {
                        let last_mining_info = LAST_MINING_INFO.lock().unwrap();
                        info!("Chain #{} Block #{} is not printed to console yet, sending last mining info: {}", index, height, last_mining_info.clone());
                        last_mining_info.clone()
                    }
                }
            }
        },
        None => {
            let chain_map = CHAIN_MINING_INFOS.lock().unwrap();
            let index = CURRENT_CHAIN_INDEX.lock().unwrap();
            match chain_map.get(&index) {
                Some((mining_info, _)) => {
                    // ^!@$IN' DEADLOCKS!!!1!
                    let mi = mining_info.clone();
                    drop(chain_map);
                    add_mining_info_to_cache(*index, mi)
                },
                None => {
                    drop(chain_map);
                    r#"{"result":"failure","reason":"Haven't found any mining info!"}"#.to_string()
                },
            }
        }
    }
}

fn get_current_mining_info() -> Option<MiningInfo> {
    let chain_map = CHAIN_MINING_INFOS.lock().unwrap();
    let index = CURRENT_CHAIN_INDEX.lock().unwrap();
    match chain_map.get(&index) {
        Some((mining_info, _)) => Some(mining_info.clone()),
        None => None,
    }
}

fn query_create_default_config() {
    println!("\n  Would you like to create a default configuration file?");
    println!(
        "  {}",
        Colour::Yellow.paint("WARNING: THIS WILL OVERWRITE AN EXISTING FILE AND CANNOT BE UNDONE!")
    );
    println!(
        "  {}",
        Colour::Cyan.paint("Type \"y\" and <Enter> to create the file, or just hit <Enter> to exit:")
    );
    let mut resp = String::new();
    match std::io::stdin().read_line(&mut resp) {
        Ok(_) => {
            if resp.trim().to_lowercase() == "y" {
                let default_config_yaml = Config::create_default();
                //let default_config_yaml = Config::to_yaml(&default_config);
                match File::create("archon.yaml") {
                    Ok(mut file) => {
                        use std::io::Write;
                        match file.write_all(&default_config_yaml.as_bytes()) {
                            Ok(_) => {
                                println!(
                                    "  {}",
                                    Colour::Green.paint("Default config file saved to archon.yaml")
                                );
                            }
                            Err(_) => {}
                        };
                    }
                    Err(err) => {
                        println!("  {}", Colour::Red.paint(format!("Error saving config file: {}", err)))
                    }
                };
            }
        }
        Err(_) => {
            return;
        }
    };
}

fn print_forked_and_queued(chain_index: u8, height: u32) {
    let chain = get_chain_from_index(chain_index);
    let chain_color = get_color(chain.clone());
    let border = String::from("==========================================================================================");
    println!("{}\n  {} {}\n{}",
        Colour::Yellow.paint(&border),
        get_time(),
        format!("{} => FORK DETECTED: BLOCK {} REQUEUED",
            chain_color.paint(&*chain.name),
            chain_color.paint(format!("#{}", height)),
        ),
        Colour::Yellow.paint(&border),
    );
}

fn print_block_started(
    chain_index: u8,
    height: u32,
    base_target: u64,
    gen_sig: String,
    last_block_info: Option<LastBlockInfo>,
) {
    if !is_block_start_printed(chain_index, height) {
        let current_chain = get_chain_from_index(chain_index);
        let mut new_block_message = String::from("");
        let border = String::from("------------------------------------------------------------------------------------------");
        let border2 = String::from("==========================================================================================");
        let chain_color = get_color(current_chain.clone());
        let last_block_time_str;
        let last_block_chain_color;
        match last_block_info {
            Some(LastBlockInfo::Completed(last_block_time, prev_block_chain_index)) => {
                let prev_chain = get_chain_from_index(prev_block_chain_index);
                last_block_chain_color = get_color(prev_chain.clone());
                if last_block_time > 0 {
                    let human_time;
                    if CONF.show_human_readable_deadlines.unwrap_or(true) { human_time = format!(" ({})", format_timespan(last_block_time)); } else { human_time = String::from(""); }
                    let plural = match last_block_time { 1 => "", _ => "s", }; 
                    last_block_time_str = format!("{}\n  Block completed after {} second{}{}\n", border, last_block_time, plural, human_time);
                } else {
                    last_block_time_str = String::from("")
                }
            },
            Some(LastBlockInfo::Interrupted(last_block_time, prev_block_chain_index)) => {
                let prev_chain = get_chain_from_index(prev_block_chain_index);
                last_block_chain_color = get_color(prev_chain.clone());
                let human_time;
                if CONF.show_human_readable_deadlines.unwrap_or(true) { human_time = format!(" ({})", format_timespan(last_block_time)); } else { human_time = String::from(""); }
                let plural = match last_block_time { 1 => "", _ => "s", };
                last_block_time_str = format!("{}\n  Block interrupted after {} second{}{}\n", border, last_block_time, plural, human_time);
            },
            Some(LastBlockInfo::Requeued(requeue_info, last_block_time, prev_block_chain_index)) => {
                let prev_chain = get_chain_from_index(prev_block_chain_index);
                last_block_chain_color = get_color(prev_chain.clone());
                match requeue_info {
                    Some((num_times, max_times)) => {
                        if last_block_time > 0 {
                            let human_time;
                            if CONF.show_human_readable_deadlines.unwrap_or(true) { human_time = format!(" ({})", format_timespan(last_block_time)); } else { human_time = String::from(""); }
                            let plural = match last_block_time { 1 => "", _ => "s", };
                            last_block_time_str = format!("{}\n  Block requeued (#{}/{}) after {} second{}{}\n", border, num_times + 1, max_times, last_block_time, plural, human_time);
                        } else {
                            last_block_time_str = format!("{}\n Block requeued\n", border);
                        }
                    },
                    _ => {
                        if last_block_time > 0 {
                            let human_time;
                            if CONF.show_human_readable_deadlines.unwrap_or(true) { human_time = format!(" ({})", format_timespan(last_block_time)); } else { human_time = String::from(""); }
                            let plural = match last_block_time { 1 => "", _ => "s", }; 
                            last_block_time_str = format!("{}\n  Block requeued after {} second{}{}\n", border, last_block_time, plural, human_time);
                        } else {
                            last_block_time_str = format!("{}\n  Block requeued\n", border);
                        }
                    }
                }
            },
            Some(LastBlockInfo::Superseded(last_block_time, prev_block_chain_index)) => {
                let prev_chain = get_chain_from_index(prev_block_chain_index);
                last_block_chain_color = get_color(prev_chain.clone());
                let human_time;
                if CONF.show_human_readable_deadlines.unwrap_or(true) { human_time = format!(" ({})", format_timespan(last_block_time)); } else { human_time = String::from(""); }
                let plural = match last_block_time { 1 => "", _ => "s", }; 
                last_block_time_str = format!("{}\n  Block superseded after {} second{}{}\n", border, last_block_time, plural, human_time);
            },
            Some(LastBlockInfo::Forked(last_block_time, prev_block_chain_index)) => {
                let prev_chain = get_chain_from_index(prev_block_chain_index);
                last_block_chain_color = get_color(prev_chain.clone());
                let human_time;
                if CONF.show_human_readable_deadlines.unwrap_or(true) { human_time = format!(" ({})", format_timespan(last_block_time)); } else { human_time = String::from(""); }
                let plural = match last_block_time { 1 => "", _ => "s", }; 
                last_block_time_str = format!("{}\n  Block forked after {} second{}{}\n", border, last_block_time, plural, human_time);
            },
            _ => {
                last_block_time_str = String::from("");
                last_block_chain_color = chain_color;
            },
        };
        /*let mut prev_block_time = 0;
        if height > 0 {
            prev_block_time = arbiter::get_time_since_block_start(height - 1).unwrap_or(0);
        }
        let prev_block_time_str;
        if prev_block_time > 0 {
            if CONF.show_human_readable_deadlines.unwrap_or(true) {
                prev_block_time_str = format!("[#{} Run time: {} secs ({})]", height - 1, prev_block_time, format_timespan(prev_block_time));
            } else {
                prev_block_time_str = format!("[#{} Run time: {} secs", height - 1, prev_block_time);
            }
        } else {
            prev_block_time_str = String::from("");
        }*/
        new_block_message.push_str(
            format!("{}{}\n  {} {} => {} | {}\n{}\n  {}       {}\n  {}          {}\n",
                last_block_chain_color.bold().paint(last_block_time_str),
                chain_color.bold().paint(&border2),
                format!("{}", get_time()),
                chain_color.paint(" STARTED BLOCK"),
                chain_color.paint(format!("{}", &*current_chain.name)),
                chain_color.paint(format!("#{}", height)),
                //prev_block_time_str.color(color).bold(),
                chain_color.bold().paint(&border2),
                chain_color.bold().paint("Total Capacity:"),
                chain_color.paint(format!("{:.4} TiB", get_total_plots_size_in_tebibytes())),
                chain_color.bold().paint("Base Target:"),
                chain_color.paint(base_target.to_string()),
            ).as_str(),
        );
        let mut human_readable_target_deadline = String::from("");
        let is_bhd = current_chain.is_bhd.unwrap_or_default();
        let is_lhd = current_chain.is_lhd.unwrap_or_default();
        match get_target_deadline(None, base_target, chain_index, Some(current_chain.clone())) {
            TargetDeadlineType::ConfigChainLevel(chain_tdl) => {
                if crate::CONF.show_human_readable_deadlines.unwrap_or_default() {
                    human_readable_target_deadline = format!(" ({})", format_timespan(chain_tdl));
                }
                new_block_message.push_str(
                    format!("  {}      {}\n",
                        chain_color.bold().paint("Target Deadline:"),
                        chain_color.paint(format!("{}{} (Chain Config)", chain_tdl, human_readable_target_deadline)),
                    ).as_str(),
                );
            },
            TargetDeadlineType::Default => {
                new_block_message.push_str(
                    format!("  {}      {}\n",
                        chain_color.bold().paint("Target Deadline:"),
                        chain_color.paint("None")
                    ).as_str(),
                );
            },
            TargetDeadlineType::Dynamic(dynamic_tdl) => {
                if crate::CONF.show_human_readable_deadlines.unwrap_or_default() {
                    human_readable_target_deadline = format!(" ({})", format_timespan(dynamic_tdl));
                }
                new_block_message.push_str(
                    format!("  {}      {}\n",
                        chain_color.bold().paint("Target Deadline:"),
                        chain_color.paint(format!("{}{} (Dynamic @ {}%)", dynamic_tdl, human_readable_target_deadline, current_chain.submit_probability.unwrap_or(95))),
                    ).as_str(),
                );
            },
            TargetDeadlineType::PoolMaximum(pool_tdl) => {
                if crate::CONF.show_human_readable_deadlines.unwrap_or_default() {
                    human_readable_target_deadline = format!(" ({})", format_timespan(pool_tdl));
                }
                new_block_message.push_str(
                    format!("  {}      {}\n",
                        chain_color.bold().paint("Target Deadline:"),
                        chain_color.paint(format!("{}{} (Pool Maximum)", pool_tdl, human_readable_target_deadline))
                    ).as_str(),
                );
            },
            _ => {} // no point having a match for TargetDeadlineType::ConfigOverriddenByID(base_tdl, id, override_tdl), since we don't provide an ID to get_target_deadline
        };
        let net_difficulty = get_network_difficulty_for_block(base_target, 240);
        let net_difficulty_fmt = fmt_capacity(net_difficulty, None);
        if is_bhd {
            let bhd_net_diff = get_network_difficulty_for_block(base_target, 180);
            let bhd_net_difficulty_fmt = fmt_capacity(bhd_net_diff, None);
            new_block_message.push_str(
                format!("  {}   {}\n",
                    chain_color.bold().paint("Network Difficulty:"),
                    chain_color.paint(format!("{} ({})", net_difficulty_fmt, bhd_net_difficulty_fmt))
                ).as_str(),
            );
        } else if is_lhd {
            let lhd_net_diff = get_network_difficulty_for_block(base_target, 300);
            let lhd_net_difficulty_fmt = fmt_capacity(lhd_net_diff, None);
            new_block_message.push_str(
                format!("  {}   {}\n",
                    chain_color.bold().paint("Network Difficulty:"),
                    chain_color.paint(format!("{} ({})", net_difficulty_fmt, lhd_net_difficulty_fmt))
                ).as_str(),
            );
        } else {
            new_block_message.push_str(
                format!("  {}   {}\n",
                    chain_color.bold().paint("Network Difficulty:"),
                    chain_color.paint(format!("{}", net_difficulty_fmt))
                ).as_str(),
            );
        }
        new_block_message.push_str(
            format!("  {} {}\n{}",
                chain_color.bold().paint("Generation Signature:"),
                chain_color.paint(gen_sig),
                chain_color.bold().paint(border)
            ).as_str(),
        );
        let mut block_start_printed_map = BLOCK_START_PRINTED.lock().unwrap();
        block_start_printed_map.insert(chain_index, height);
        println!("{}", new_block_message);
    }
}

fn fmt_capacity(capacity: u64, num_decimals: Option<usize>) -> String {
    let mut new_capacity = capacity as f64;
    let mut iterations = 0u8;
    // keep dividing the value by 1024 until it is <= 1024, keeping track of how many iterations were performed
    loop {
        if new_capacity > 1024f64 {
            new_capacity /= 1024f64;
            iterations += 1;
        } else {
            break;
        }
    }
    let units = match iterations {
        0u8 => "TiB",
        1u8 => "PiB",
        2u8 => "EiB",
        3u8 => "ZiB", // god help us if we get to this point...
        4u8 => "YiB",
        _ => "?",
    };
    format!("{:.*} {}", num_decimals.unwrap_or(4), new_capacity, units)
}

fn print_nonce_submission(
    index: u8,
    height: u32,
    account_id: u64,
    deadline: u64,
    user_agent: &str,
    target_deadline: u64,
    id_override: bool,
    remote_addr: String,
    time_since_block_started: u64,
) {
    let current_chain = get_chain_from_index(index);

    // check if this is a submission for the actual current chain we're mining
    let actual_current_chain_index = arbiter::get_current_chain_index();
    let actual_current_chain_height = arbiter::get_latest_chain_info(actual_current_chain_index).0;

    if actual_current_chain_index == index && actual_current_chain_height == height {
        //let scoop_num = rand::thread_rng().gen_range(0, 4097);
        let chain_color = get_color(current_chain.clone());
        let mut deadline_string = deadline.to_string();
        if crate::CONF.show_human_readable_deadlines.unwrap_or_default()
        {
            deadline_string.push_str(format!(" ({})", format_timespan(deadline)).as_str());
        }
        let deadline_color = match deadline {
            0..=600 => RGB(255, 95, 0),
            601..=3600 => Colour::Green,
            3601..=86400 => Colour::Yellow,
            _ => Colour::White,
        };
        // remote_addr is an endpoint, need to truncate the port and just leave the hostname/ip
        let remote_address = match CONF.show_miner_addresses.unwrap_or_default() {
            true => {
                let mut addr = remote_addr;
                let mut port_index = 0;
                let mut i = 0;
                for c in addr.chars() {
                    if c == ':' {
                        port_index = i;
                        break;
                    }
                    i += 1;
                }
                if port_index > 0 {
                    addr.truncate(port_index);
                }
                addr.push_str(": ");
                addr
            },
            false => String::from(""),
        };
        let time_since_block_started_str = match time_since_block_started {
            0 => String::from(""),
            _ => format!(" ({}ms)", time_since_block_started),
        };
        if !id_override {
            println!("    {}{} ==> {}{} ==> {} {}\n        {}                          {}{}",
                remote_address,
                chain_color.bold().paint(user_agent.to_string()),
                chain_color.bold().paint("Block #"),
                chain_color.paint(height.to_string()),
                chain_color.bold().paint("Numeric ID:"),
                chain_color.paint(censor_account_id(account_id)),
                chain_color.bold().paint("Deadline:"),
                deadline_color.paint(deadline_string),
                chain_color.paint(time_since_block_started_str),
            );
        } else {
            println!("    {}{} ==> {}{} ==> {} {}{}\n        {}                          {}{}",
                remote_address,
                chain_color.bold().paint(user_agent.to_string()),
                chain_color.bold().paint("Block #"),
                chain_color.paint(height.to_string()),
                chain_color.bold().paint("Numeric ID:"),
                chain_color.paint(censor_account_id(account_id)),
                Colour::Red.paint(format!(" [TDL: {}]", target_deadline)),
                chain_color.bold().paint("Deadline:"),
                deadline_color.paint(deadline_string),
                chain_color.paint(time_since_block_started_str),
            );
        }
    }
}

fn get_num_chains_with_priority(priority: u8) -> u8 {
    if CONF.poc_chains.is_some() {
        let mut count = 0;
        for inner in &CONF.poc_chains {
            for chain in inner {
                if chain.priority == priority && chain.enabled.unwrap_or(true) {
                    count += 1;
                }
            }
        }
        return count;
    }
    return 0u8;
}

fn print_nonce_accepted(chain_index: u8, block_height: u32, deadline: u64, confirmation_time_ms: i64) {
    let current_chain = get_chain_from_index(chain_index);

    // check if this is a submission for the actual current chain we're mining
    let actual_current_chain_index = arbiter::get_current_chain_index();
    let actual_current_chain_height = arbiter::get_latest_chain_info(actual_current_chain_index).0;

    if actual_current_chain_index == chain_index && actual_current_chain_height == block_height {
        let chain_color = get_color(current_chain.clone());
        let is_hdpool = (current_chain.is_hdpool.unwrap_or_default() || current_chain.is_hdpool_eco.unwrap_or_default()) && current_chain.account_key.is_some();
        let confirm_text = if is_hdpool { "Submitted:" } else { "Confirmed:" };
        println!("            {}                     {}{}",
            Colour::Green.paint(confirm_text),
            chain_color.paint(deadline.to_string()),
            chain_color.paint(format!(" ({}ms)", confirmation_time_ms))
        );
    }
}

fn print_nonce_rejected(
    chain_index: u8,
    block_height: u32,
    deadline: u64,
    rejection_time_ms: i64,
    attempt: u8,
    attempts: u8,
    failure_message: Option<String>,
    is_timeout: bool,
) {
    // check if this is a submission for the actual current chain we're mining
    let actual_current_chain_index = arbiter::get_current_chain_index();
    let actual_current_chain_height = arbiter::get_latest_chain_info(actual_current_chain_index).0;
    let attempts_num = if attempts == 0 { 1 } else { attempts };
    let attempt_num = if attempt == 0 { 1 } else if attempt > attempts_num { attempts_num } else { attempt };

    if actual_current_chain_index == chain_index && actual_current_chain_height == block_height {
        let current_chain = get_chain_from_index(chain_index);
        let chain_color = get_color(current_chain.clone());
        let rejected_text = if is_timeout { format!("Timeout ({}/{}): ", attempt_num, attempts_num) } else { format!("Rejected ({}/{}):", attempt_num, attempts_num) };
        if is_timeout {
                println!("            {}                {}{}",
                    Colour::Red.paint(rejected_text),
                    chain_color.paint(deadline.to_string()),
                    chain_color.paint(format!(" ({}ms)", rejection_time_ms))
                );
        } else {
            if failure_message.is_none() {
                println!("            {}                {}{}\n              (No reason given)",
                    Colour::Red.paint(rejected_text),
                    chain_color.paint(deadline.to_string()),
                    chain_color.paint(format!(" ({}ms)", rejection_time_ms))
                );
            } else {
                println!("            {}                {}{}\n              ({})",
                    Colour::Red.paint(rejected_text),
                    chain_color.paint(deadline.to_string()),
                    chain_color.paint(format!(" ({}ms)", rejection_time_ms)),
                    Colour::Red.paint(format!("{}", failure_message.unwrap())),
                );
            }
        }
    }
}

fn get_network_difficulty_for_block(base_target: u64, block_time_seconds: u16) -> u64 {
    // BHD = 14660155037u64
    // BURST = 18325193796u64
    return (4398046511104u64 / u64::from(block_time_seconds)) / u64::from(base_target);
}

fn get_total_plots_size_in_tebibytes() -> f64 {
    let connected_miners_map = CONNECTED_MINER_DATA.lock().unwrap();
    let mut plots_capacity = 0f64;
    let current_time = Local::now();
    for (val, _, last_updated) in connected_miners_map.values() {
        if (current_time - *last_updated).num_seconds() < i64::from(CONF.miner_update_timeout.unwrap_or(1800)) {
            plots_capacity += *val;
        }
    }
    plots_capacity
}

fn get_target_deadline(
    account_id: Option<u64>,
    base_target: u64,
    chain_index: u8,
    chain: Option<PocChain>,
) -> TargetDeadlineType {
    let chain_obj = if chain.is_some() { chain.clone().unwrap() } else { get_chain_from_index(chain_index) };

    // get max deadline from upstream if present
    let tdl_last_value;
    let upstream_target_deadline;
    let mut target_deadline = match arbiter::get_current_chain_mining_info(chain_index) {
        Some((mining_info, _)) => {
            if mining_info.target_deadline == 0 || mining_info.target_deadline == u64::max_value() {
                tdl_last_value = u64::max_value();
                TargetDeadlineType::Default
            } else {
                tdl_last_value = mining_info.target_deadline;
                TargetDeadlineType::PoolMaximum(tdl_last_value)
            }
        },
        _ => {
            tdl_last_value = u64::max_value();
            TargetDeadlineType::Default
        },
    };
    upstream_target_deadline = tdl_last_value;

    // get chain's global target deadline, if set
    if chain_obj.clone().target_deadline.is_some() {
        target_deadline = TargetDeadlineType::ConfigChainLevel(chain_obj.clone().target_deadline.unwrap());
    }

    // calculate the dynamic deadline
    if chain_obj.clone().use_dynamic_deadlines.unwrap_or_default() {
        let (_, dynamic_target_deadline) = get_dynamic_deadline_for_block(base_target, chain_obj.clone().submit_probability.unwrap_or(95));
        if dynamic_target_deadline < upstream_target_deadline {
            target_deadline = TargetDeadlineType::Dynamic(dynamic_target_deadline);
        }
    }

    // check if there is a target deadline specified for this account id in this chain's config, if so, override all other deadlines with it
    if account_id.is_some() {
        if let Some(num_id_to_tdls_map) = chain_obj.numeric_id_to_target_deadline {
            for (id, overridden_tdl) in num_id_to_tdls_map {
                if id == account_id.unwrap() && overridden_tdl < upstream_target_deadline {
                    target_deadline = TargetDeadlineType::ConfigOverriddenByID(overridden_tdl);
                }
            }
        }
    }
    return target_deadline;
}

fn get_dynamic_deadline_for_block(base_target: u64, submit_probability: u16) -> (u64, u64) {
    let net_diff = get_network_difficulty_for_block(base_target, 240) as u64;
    let plot_size_tebibytes = get_total_plots_size_in_tebibytes();
    // are we using dynamic deadlines for this chain?
    if plot_size_tebibytes > 0f64 {
        let dynamic_target_deadline = ((720f64 * (net_diff as f64) / plot_size_tebibytes) * (f64::from(submit_probability) / 95f64)) as u64;
        (net_diff, dynamic_target_deadline)
    } else {
        (net_diff, u64::max_value())
    }
}

fn get_time() -> String {
    let local_time: DateTime<Local> = Local::now();
    if crate::CONF.use_24_hour_time.unwrap_or_default() {
        return local_time.format("%H:%M:%S").to_string();
    }
    return local_time.format("%I:%M:%S%P").to_string();
}

fn get_chain_from_index(index: u8) -> PocChain {
    let mut i = 0;
    let mut last_chain = PocChain {
        name: String::from(""),
        enabled: None,
        priority: 0,
        is_bhd: None,
        is_lhd: None,
        is_boomcoin: None,
        is_pool: None,
        is_hpool: None,
        is_hdpool: None,
        is_hdpool_eco: None,
        account_key: None,
        miner_name: None,
        url: String::from(""),
        numeric_id_to_passphrase: None,
        numeric_id_to_target_deadline: None,
        historical_rounds: None,
        target_deadline: None,
        color: None,
        color_num: None,
        color_rgb: None,
        color_hex: None,
        get_mining_info_interval: None,
        use_dynamic_deadlines: None,
        submit_probability: None,
        allow_lower_block_heights: None,
        requeue_interrupted_blocks: None,
        maximum_requeue_times: None,
        append_version_to_miner_name: None,
        miner_alias: None,
        payout_address: None,
        timeout: None,
        submit_attempts: None,
    };
    for inner in &crate::CONF.poc_chains {
        for chain in inner {
            last_chain = chain.clone();
            if chain.enabled.unwrap_or(true) {
                if i == index {
                    return chain.clone();
                }
                i += 1;
            }
        }
    }
    return last_chain;
}

fn get_chain_index(chain_url: &str, chain_name: &str) -> u8 {
    let mut index = 0;
    for inner in &crate::CONF.poc_chains {
        for chain in inner {
            if chain.enabled.unwrap_or(true) {
                if chain.url == chain_url && chain.name == chain_name {
                    return index;
                }
                index += 1;
            }
        }
    }
    return 0;
}

fn format_timespan(timespan: u64) -> String {
    if !crate::CONF
        .show_human_readable_deadlines
        .unwrap_or_default()
    {
        return String::from("");
    }
    if timespan == 0u64 {
        return String::from("00:00:00");
    }
    let (has_years, years, mdhms) = modulus(timespan, 31536000);
    let (has_months, months, dhms) = modulus(mdhms, 86400 * 30);
    let (has_days, days, hms) = modulus(dhms, 86400);
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
    let hms_str = format!(
        "{}:{}:{}",
        pad_left(hours, 2),
        pad_left(mins, 2),
        pad_left(secs, 2)
    );
    let mut gap_str = String::from("");
    if has_years || has_months || has_days {
        gap_str.push_str(" ");
    }
    return format!(
        "{}{}{}{}{}",
        years_str, months_str, days_str, gap_str, hms_str
    );
}

fn censor_account_id(account_id: u64) -> String {
    let as_string = account_id.to_string();
    if CONF.mask_account_ids_in_console.unwrap_or_default() {
        let mut masked_account_id = String::from("");
        let mut i = 0i8;
        for c in as_string.chars() {
            if i % 3 == 2 || i == 0 || i >= as_string.len() as i8 - 3 {
                masked_account_id.push(c);
            } else if i % 2 == 0 {
                let c_u8 = c.to_string().parse::<i8>().unwrap_or(3i8);
                masked_account_id.push_str(format!("{}", c_u8 + i / 1 + (5 + (c_u8 - c_u8 * 2 - i)).abs()).as_str());
            }
            i += 1;
        }
        return masked_account_id;
    }
    return as_string;
}

fn modulus(numerator: u64, denominator: u64) -> (bool, u64, u64) {
    return (
        numerator / denominator > 0,
        numerator / denominator,
        numerator % denominator,
    );
}

fn pad_left(num: u64, desired_length: usize) -> String {
    let mut padded = format!("{}", num);
    while padded.len() < desired_length {
        padded = format!("0{}", padded);
    }
    return padded;
}

#[allow(dead_code)]
fn get_current_capacity(ip_address: u32) -> f64 {
    // lookup value in the map
    let connected_miners_map = CONNECTED_MINER_DATA.lock().unwrap();
    match connected_miners_map.get(&ip_address) {
        Some((value, _, last_updated)) => {
            // check it hasn't been more than 30 minutes since receiving an update from this miner
            if (Local::now() - *last_updated).num_seconds() < i64::from(CONF.miner_update_timeout.unwrap_or(1800)) {
                *value
            } else {
                0f64
            }
        },
        None => 0f64
    }
}

fn update_connected_miners(endpoint: &str, data: crate::web::MiningHeaderData) {
    let mut ip_address = endpoint.to_string();
    let mut port_index = 0;
    let mut i = 0;
    for c in ip_address.chars() {
        if c == ':' {
            port_index = i;
            break;
            }
        i += 1;
    }
    if port_index > 0 {
        ip_address.truncate(port_index);
    }
    let ip_as_u32 = ip_to_u32(ip_address.as_str());
    let capacity_tib = data.capacity / 1024f64;
    let stored_capacity = get_current_capacity(ip_as_u32);
    let capacity_to_store = if capacity_tib > 0f64 && capacity_tib != stored_capacity { capacity_tib } else { stored_capacity };
    if data.capacity > 0f64 && capacity_tib != stored_capacity {
        debug!("Miner from IP {} capacity changed from {} TiB => {} TiB", ip_address, stored_capacity, capacity_tib);
    }
    let mut connected_miners_map_guard = CONNECTED_MINER_DATA.lock().unwrap();
    connected_miners_map_guard.insert(ip_as_u32, (capacity_to_store, ip_address, Local::now()));
}

fn ip_to_u32(ip_address: &str) -> u32 {
    match ipaddress::IPAddress::split_to_u32(&ip_address.to_string()) {
        Ok(ip_as_u32) => ip_as_u32,
        _ => 0u32
    }
}

fn thread_monitor_capacity() {
    let mut last_plot_capacity = get_total_plots_size_in_tebibytes();
    let mut last_timeout_check = Local::now();
    let border = "==========================================================================================";
    loop {
        // check if the plot capacity has changed at all since the last loop iteration, log any changes if so
        let new_plot_capacity = get_total_plots_size_in_tebibytes();
        if new_plot_capacity != last_plot_capacity {
            let difference = new_plot_capacity - last_plot_capacity;
            let sign;
            let difference_color = if difference > 0f64 { sign = "+"; Colour::Green } else { sign = ""; Colour::Red }; 
            println!("{}\n  {} {} {} {}\n{}",
                Colour::Green.bold().paint(border),
                get_time(), 
                Colour::Yellow.paint("TOTAL PLOT CAPACITY IS NOW"),
                format!("{:.4} TiB", new_plot_capacity),
                difference_color.paint(format!("({}{:.4} TiB)", sign, difference)),
                Colour::Green.bold().paint(border)
            );
            info!("TOTAL PLOT CAPACITY CHANGED FROM {} TiB => {} TiB ({}{} TiB)", last_plot_capacity, new_plot_capacity, sign, difference);
        }
        last_plot_capacity = new_plot_capacity;

        // if 5 mins has elapsed since the last check and the user has miner offline warnings on, do a miner timeout check
        if CONF.miner_offline_warnings.unwrap_or(true) && (Local::now() - last_timeout_check).num_seconds() >= 300 {
            let connected_miners_map_guard = CONNECTED_MINER_DATA.lock().unwrap();
            let current_time = Local::now();
            let mut offline_warnings = String::from("");
            let mut num_offline_miners = 0;
            for (_, ip, last_updated) in connected_miners_map_guard.values() {
                let time_since_last_updated = (current_time - *last_updated).num_seconds();
                if time_since_last_updated >= i64::from(CONF.miner_update_timeout.unwrap_or(1000) / 2) {
                    let human_readable_time = if CONF.show_human_readable_deadlines.unwrap_or_default() { format!(" ({})", format_timespan(time_since_last_updated as u64)) } else { String::from("") };
                    let new_line = if !offline_warnings.is_empty() { "\n" } else { "" };
                    warn!("MINER OFFLINE - Miner @ IP {} last seen {} seconds{} ago.", ip, time_since_last_updated, &human_readable_time);
                    offline_warnings.push_str(format!("{}  - {} => Last seen {} seconds{} ago.", new_line, ip, time_since_last_updated, human_readable_time).as_str());
                    num_offline_miners += 1;
                }
            }
            drop(connected_miners_map_guard);
            if !offline_warnings.is_empty() {
                let miners_plural = if num_offline_miners == 1 { "" } else { "S" };
                println!("{}\n  {} {}\n{}\n{}\n{}",
                    Colour::Red.bold().paint(border),
                    get_time(),
                    Colour::Red.paint(format!("MINER{} OFFLINE!", miners_plural)),
                    Colour::Red.bold().paint(border),
                    Colour::Red.paint(format!("{}", offline_warnings)),
                    Colour::Red.bold().paint("------------------------------------------------------------------------------------------"),
                );
            }
            last_timeout_check = Local::now();
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
