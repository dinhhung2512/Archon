#![feature(vec_remove_item, proc_macro_hygiene, decl_macro)]

use colored;
use colored::Colorize;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate log;

pub mod arbiter;
pub mod config;
pub mod upstream;
pub mod web;
pub mod error;
pub mod app;
pub mod utility;

use crate::config::Config;
use crate::config::PocChain;
use crate::upstream::MiningInfo;

<<<<<<< HEAD
fn main() -> Result<(), ()> {
    app::new().map(|application| {
        app.start();
        app
    }).map_err(|why| {
        println!("  {} {}", "ERROR".red().underline(), "Archon has failed to start.".red());
    })
=======
const APP_NAME: &'static str = env!("CARGO_PKG_NAME");
const VERSION: &'static str = env!("CARGO_PKG_VERSION");

lazy_static! {
    static ref CHAIN_MINING_INFOS: Arc<Mutex<HashMap<u8, (MiningInfo, DateTime<Local>)>>> = {
        let chain_mining_infos = HashMap::new();
        Arc::new(Mutex::new(chain_mining_infos))
    };
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
    static ref CHAIN_NONCE_SUBMISSION_CLIENTS: Arc<Mutex<HashMap<u8, reqwest::Client>>> = {
        let chain_nonce_submission_clients = HashMap::new();
        Arc::new(Mutex::new(chain_nonce_submission_clients))
    };
    #[derive(Serialize)]
    pub static ref CONF: Config = {
        let r: Config = match File::open("archon.yaml") {
            Ok(file) => {
                let (conf, err) = Config::try_parse_config(file);
                if !err {
                    query_create_default_config();

                    println!(
                        "\n  {}",
                        "Execution completed. Press enter to exit."
                            .red()
                            .underline()
                    );

                    let mut blah = String::new();
                    std::io::stdin().read_line(&mut blah).expect("FAIL");
                    exit(0);
                } else {
                    conf.unwrap()
                }
            }
            Err(_) => {
                println!(
                    "  {} {}",
                    "ERROR".red().underline(),
                    "An error was encountered while attempting to open the config file.".red()
                );
                query_create_default_config();

                println!(
                    "\n  {}",
                    "Execution completed. Press enter to exit."
                        .red()
                        .underline()
                );

                let mut blah = String::new();
                std::io::stdin().read_line(&mut blah).expect("FAIL");
                exit(0);
            }
        };
        r
    };
}

fn main() {
    // set up ansi support if user is running windows
    setup_ansi_support();

    let app_name = uppercase_first(APP_NAME);

    // setup logging
    let console_logging_info = setup_logging();

    info!("{} v{} started", app_name, VERSION);

    println!(
        "{}",
        format!("  {} v{} - POWER OVERWHELMING!", app_name, VERSION)
            .cyan()
            .bold()
    );
    println!("  {} {} | {} {}",
        "Created by".cyan().bold(),
        "Ayaenah Bloodreaver".cyan().underline(),
        "Discord Invite:".red(),
        "https://discord.gg/ZdVbrMn".yellow(),
    );
    println!("    {} {}\n      {}\n",
        "With special thanks to:".red().bold(),
        "Haitch | Avanth | Karralie | Romanovski".red(),
        "Thanks guys <3".magenta(),
    );

    if crate::CONF.poc_chains.is_some() {
        println!("{}", console_logging_info);

        println!("  {} {} {}",
            get_time().white(),
            "Config:".red(),
            format!("{} {}",
                "Web Server Binding:".green(),
                format!("http://{}:{}",
                    crate::CONF.web_server_bind_address,
                    crate::CONF.web_server_port
                )
                .yellow()
            )
        );
        if crate::CONF.priority_mode.unwrap_or(true) {
            println!("  {} {} {}",
                get_time().white(),
                "Config:".red(),
                format!("{} {}", "Queuing Mode:".green(), "Priority".yellow())
            );
            if crate::CONF.interrupt_lower_priority_blocks.unwrap_or(true) {
                println!("  {} {} {}",
                    get_time().white(),
                    "Config:".red(),
                    format!("{} {}",
                        "Interrupt Lower Priority Blocks:".green(),
                        "Yes".yellow()
                    )
                );
            } else {
                // interrupt lower priority blocks off
                println!("  {} {} {}",
                    get_time().white(),
                    "Config:".red(),
                    format!("{} {}",
                        "Interrupt Lower Priority Blocks:".green(),
                        "No".yellow()
                    )
                );
            }
        } else {
            println!("  {} {} {}",
                get_time().white(),
                "Config:".red(),
                format!("{} {}",
                    "Queuing Mode:".green(),
                    "First In, First Out".yellow()
                )
            );
        }
        println!("  {} {} {}",
            get_time().white(),
            "Config:".red(),
            format!("{} {}",
                "Grace Period:".green(),
                format!("{} seconds", crate::CONF.grace_period).yellow()
            )
        );
        let total_plots_size_tebibytes = get_total_plots_size_in_tebibytes();
        let plots_zero_warning;
        if total_plots_size_tebibytes == 0f64 {
            plots_zero_warning = " (Warning: Dynamic deadlines require an accurate plot size. Dynamic Deadlines are disabled.)";
        } else {
            plots_zero_warning = "";
        }
        println!("  {} {} {}",
            get_time().white(),
            "Config:".red(),
            format!("{} {}{}",
                "Total Plots Size:".green(),
                format!("{} TiB", total_plots_size_tebibytes).yellow(),
                plots_zero_warning.red(),
            )
        );
        println!("  {} {} {}",
            get_time().white(),
            "Config:".red(),
            "PoC Chains:".green()
        );
        let mut chain_counter = 0u8;
        let mut multiple_same_priority_chains = false;
        let mut unused_passphrase_warnings = String::from("");
        let mut account_key_warnings = String::from("");
        for inner in &crate::CONF.poc_chains {
            for chain in inner {
                if chain.is_hdpool.unwrap_or_default() && chain.is_hpool.unwrap_or_default() {
                    // fatal error - can't have chain defined as for both HPOOL and HDPOOL
                    println!("\n  {}", format!("FATAL ERROR: The chain \"{}\" is defined as both HDPOOL and HPOOL. Pick one!", &*chain.name).red().underline());
                    error!("The chain \"{}\" is defined as both HDPOOL and HPOOL. Pick one!", &*chain.name);

                    println!("\n  {}", "Execution completed. Press enter to exit.".red().underline());

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
                    } else if chain.is_hdpool.unwrap_or_default() {
                        warn!("Chain \"{}\" is set for HDPOOL mining, but has no account key defined!\n", &*chain.name);
                        account_key_warnings.push_str(format!("    Chain \"{}\" is set for HDPOOL mining, but has no account key defined!\n", &*chain.name).as_str());
                    }
                }
                if chain.numeric_id_to_passphrase.is_some()
                    && (chain.is_pool.unwrap_or_default()
                        || chain.is_bhd.unwrap_or_default()
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
                    }
                }
                if chain.enabled.unwrap_or(true) {
                    if get_num_chains_with_priority(chain.priority) > 1 {
                        multiple_same_priority_chains = true;
                    }
                    chain_counter += 1;
                    let chain_tdl = chain.target_deadline.unwrap_or_default();
                    let mut human_readable_target_deadline = String::from("");
                    if crate::CONF
                        .show_human_readable_deadlines
                        .unwrap_or_default()
                    {
                        human_readable_target_deadline =
                            format!(" ({})", format_timespan(chain_tdl));
                    }
                    let chain_tdl_str;
                    if chain.use_dynamic_deadlines.unwrap_or_default() {
                        chain_tdl_str = String::from("Dynamic");
                    } else if chain_tdl == 0 {
                        chain_tdl_str = String::from("None");
                    } else {
                        chain_tdl_str = format!("{}{}", chain_tdl, human_readable_target_deadline);
                    }
                    if crate::CONF.priority_mode.unwrap_or(true) {
                        if crate::CONF.interrupt_lower_priority_blocks.unwrap_or(true) {
                            let mut requeue_str = "Yes";
                            if !chain.requeue_interrupted_blocks.unwrap_or(true) {
                                requeue_str = "No";
                            }
                            println!("  {} {}  {} {}",
                                get_time().white(),
                                "Config:".red(),
                                format!("#{}:", chain_counter).green(),
                                format!("{} {} {} {} {} {} {} {} {} {}",
                                    "Priority:".color(get_color(&*chain.color)).bold(),
                                    format!("{}", &chain.priority).color(get_color(&*chain.color)),
                                    "Name:".color(get_color(&*chain.color)).bold(),
                                    format!("{}", &*chain.name).color(get_color(&*chain.color)),
                                    "TDL:".color(get_color(&*chain.color)).bold(),
                                    format!("{}", chain_tdl_str).color(get_color(&*chain.color)),
                                    "URL:".color(get_color(&*chain.color)).bold(),
                                    format!("{}", &*chain.url).color(get_color(&*chain.color)),
                                    "Requeue:".color(get_color(&*chain.color)).bold(),
                                    format!("{}", requeue_str).color(get_color(&*chain.color)),
                                )
                            );
                        } else {
                            println!("  {} {}  {} {}",
                                get_time().white(),
                                "Config:".red(),
                                format!("#{}:", chain_counter).green(),
                                format!("{} {} {} {} {} {} {} {}",
                                    "Priority:".color(get_color(&*chain.color)).bold(),
                                    format!("{}", &chain.priority).color(get_color(&*chain.color)),
                                    "Name:".color(get_color(&*chain.color)).bold(),
                                    format!("{}", &*chain.name).color(get_color(&*chain.color)),
                                    "TDL:".color(get_color(&*chain.color)).bold(),
                                    format!("{}", chain_tdl_str).color(get_color(&*chain.color)),
                                    "URL:".color(get_color(&*chain.color)).bold(),
                                    format!("{}", &*chain.url).color(get_color(&*chain.color)),
                                )
                            );
                        }
                    } else {
                        println!("  {} {}  {} {}",
                            get_time().white(),
                            "Config:".red(),
                            format!("#{}:", chain_counter).green(),
                            format!("{} {} {} {} {} {}",
                                "Name:".color(get_color(&*chain.color)).bold(),
                                format!("{}", &*chain.name).color(get_color(&*chain.color)),
                                "TDL:".color(get_color(&*chain.color)).bold(),
                                format!("{}", chain_tdl_str).color(get_color(&*chain.color)),
                                "URL:".color(get_color(&*chain.color)).bold(),
                                format!("{}", &*chain.url).color(get_color(&*chain.color)),
                            )
                        );
                    }
                }
            }
        }

        if chain_counter == 0
            || (CONF.priority_mode.unwrap_or(true) && multiple_same_priority_chains)
        {
            if chain_counter == 0 {
                println!("  {} {} {}",
                    get_time().white(),
                    "ERROR".red().underline(),
                    "You do not have any PoC Chains enabled. Archon has nothing to do!".yellow()
                );
                error!("There are no PoC Chains configured.");
            } else {
                println!("  {} {} {}",
                    get_time().white(),
                    "ERROR".red().underline(),
                    "You have multiple chains configured with the same priority level! Priorities must be unique!".yellow()
                );
                error!("Multiple PoC Chains are configured with the same priority level. Priority levels must be unique.");
            }

            println!("\n  {}",
                "Execution completed. Press enter to exit."
                    .red()
                    .underline()
            );

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
                "You should remove these from your Archon config file for security purposes!"
                    .yellow(),
                border.red().bold(),
            );
            warn!("Unused passphrases found in archon.yaml: {}", unused_passphrase_warnings);
        }
        if account_key_warnings.len() > 0 {
            println!("\n  {}", format!("WARNING:\n{}", account_key_warnings).red());
        }

        let valid_colors = ["green", "yellow", "blue", "magenta", "cyan", "white"];
        let mut invalid_color_found = false;
        for inner in &crate::CONF.poc_chains {
            for chain in inner {
                if chain.enabled.unwrap_or(true) {
                    if !valid_colors.contains(&&*chain.color) {
                        println!("  {} {}", get_time().white(), format!("WARNING The {} chain uses the color \"{}\" which is invalid. Will pick a random valid color.", &*chain.name, &*chain.color).yellow());
                        invalid_color_found = true;
                    }
                }
            }
        }
        if invalid_color_found {
            let mut valid_colors_str = String::from("");
            for color in &valid_colors {
                valid_colors_str
                    .push_str(format!("{}|", format!("{}", *color).color(*color)).as_str());
            }
            valid_colors_str.truncate(valid_colors_str.len() - 1);
            println!(
                "  {} {}",
                get_time().white(),
                format!("Valid colors: {}", valid_colors_str)
            );
        }

        // start mining info polling thread
        println!("  {} {}", get_time().white(), "Starting upstream mining info polling thread.");
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

        println!("  {} {}", get_time().white(), "Starting web server.".green() );
        web::start_server();
        mi_thread.join().expect("Failed to join mining info thread.");
        queue_proc_thread.join().expect("Failed to join queue processing thread.");
        version_check_thread.join().expect("Failed to join version check thread.");
    } else {
        println!("  {} {} {}", get_time().white(), "ERROR".red().underline(), "You do not have any PoC Chains configured. Archon has nothing to do!".yellow());
    }

    println!("\n  {}", "Execution completed. Press enter to exit." .red().underline());

    let mut blah = String::new();
    std::io::stdin().read_line(&mut blah).expect("FAIL");
}

#[cfg(target_os = "windows")]
fn setup_ansi_support() {
    if !ansi_term::enable_ansi_support().is_ok() {
        colored::control::set_override(false);
    }
}

#[cfg(not(target_os = "windows"))]
fn setup_ansi_support() {}

fn setup_logging() -> String {
    let logging_level = CONF.logging_level.clone().unwrap_or(String::from("info")).to_lowercase();
    let logging_level_warning;
    let log_level = match logging_level.as_str() {
        "off" => {
            logging_level_warning = "";
            log::LevelFilter::Off
        },
        "trace" => {
            logging_level_warning = " (Warning: Log files will get very large, very quickly using this level!)";
            log::LevelFilter::Trace
        },
        "debug" => {
            logging_level_warning = " (Warning: Log files will get very large, very quickly using this level!)";
            log::LevelFilter::Debug
        },
        "info" => {
            logging_level_warning = "";
            log::LevelFilter::Info
        },
        "warn" => {
            logging_level_warning = "";
            log::LevelFilter::Warn
        },
        "error" => {
            logging_level_warning = "";
            log::LevelFilter::Error
        },
        _ => {
            logging_level_warning = "";
            log::LevelFilter::Info
        },
    };
    let console_logging_message = format!("  {} {} {}",
        get_time().white(),
        "Config:".red(),
        format!("{} {}{}",
            "Logging Level:".green(),
            format!("{}", uppercase_first(logging_level.as_str())).yellow(),
            logging_level_warning.red(),
        )
    );
    if log_level != log::LevelFilter::Off {
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
                .level(log_level)
                .chain(log_file)
                .apply() {
                    Ok(_) => {},
                    Err(_) => {},
                };
            },
            Err(_) => {}
        }
    }
    console_logging_message
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
                        if current_version < latest && current_version.is_prerelease() && latest.is_prerelease() {
                            let border = "------------------------------------------------------------------------------------------";
                            let headline = "  NEW VERSION AVAILABLE ==> ";
                            println!("{}", format!("\n{}\n{}v{}\n{}\n    There is a new release available on GitHub. Please update ASAP!\n      https://github.com/Bloodreaver/Archon/releases\n{}\n",
                                border, headline, tag_name_string, border, border).red());
                            info!("VERSION CHECK: v{} is available on GitHub - See https://github.com/Bloodreaver/Archon/releases", tag_name_string);
                        } else if current_version < latest && !current_version.is_prerelease() && latest.is_prerelease() {
                            let border = "------------------------------------------------------------------------------------------";
                            let headline = "  NEW PRE-RELEASE VERSION AVAILABLE ==> ";
                            println!("{}", format!("\n{}\n{}v{}\n{}\n    There is a new pre-release available on GitHub.\n      https://github.com/Bloodreaver/Archon/releases\n{}\n",
                                border, headline, tag_name_string, border, border).red());
                            info!("VERSION CHECK: Pre-release v{} is available on GitHub - See https://github.com/Bloodreaver/Archon/releases", tag_name_string);
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

fn get_color(col: &str) -> &str {
    // if using poc chain colors is disabled in config, return white here
    if !crate::CONF.use_poc_chain_colors.unwrap_or(true) {
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
        "WARNING: THIS WILL OVERWRITE AN EXISTING FILE AND CANNOT BE UNDONE!".yellow()
    );
    println!(
        "  {}",
        "Type \"y\" and <Enter> to create the file, or just hit <Enter> to exit:".cyan()
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
                                    "Default config file saved to archon.yaml".green()
                                );
                            }
                            Err(_) => {}
                        };
                    }
                    Err(err) => {
                        println!("  {}", format!("Error saving config file: {}", err).red())
                    }
                };
            }
        }
        Err(_) => {
            return;
        }
    };
>>>>>>> origin/master
}

fn print_block_requeued_or_interrupted(
    chain_name: &str,
    chain_color: &str,
    height: u32,
    requeued: bool,
) {
    let border = String::from("------------------------------------------------------------------------------------------");
    let color = get_color(chain_color);
    if requeued {
        println!("{}", border.yellow());
        println!("  {} {} => {} | {}",
            format!("{}", get_time()).white(),
            "INTERRUPTED & REQUEUED BLOCK".color(color),
            format!("{}", chain_name).color(color),
            format!("#{}", height).color(color)
        );
        println!("{}", border.yellow());
    } else {
        println!("{}", border.red());
        println!("  {} {} => {} | {}",
            format!("{}", get_time()).white(),
            "INTERRUPTED BLOCK".red(),
            format!("{}", chain_name).color(color),
            format!("#{}", height).color(color)
        );
        println!("{}", border.red());
    }
}

/*fn print_block_queued(chain_name: &str, chain_color: &str, height: u32) {
    if CONF.show_block_queued_messages.unwrap_or(true) {
        let mut queued_block_message = String::from("");
        let border = String::from("------------------------------------------------------------------------------------------");
        let color = get_color(chain_color);
        queued_block_message.push_str(
            format!(
                "{}\n  {} {} => {} | {}\n{}",
                border.color(color).bold(),
                format!("{}", get_time()).white(),
                "  QUEUED BLOCK".color(color),
                format!("{}", chain_name).color(color),
                format!("#{}", height).color(color),
                border.color(color).bold()
            )
            .as_str(),
        );
        println!("{}", queued_block_message);
    }
}*/

fn print_block_started(
    height: u32,
    base_target: u32,
    gen_sig: String,
    target_deadline: u64,
    last_block_time: Option<u64>,
) {
    let chain_index = arbiter::get_current_chain_index();
    let current_chain = get_chain_from_index(chain_index).unwrap();
    let mut new_block_message = String::from("");
    let border = String::from("------------------------------------------------------------------------------------------");
    let color = get_color(&*current_chain.color);
    let last_block_time_str;
    match last_block_time {
        Some(time) => {
            if time > 0 {
                let human_time;
                if CONF.show_human_readable_deadlines.unwrap_or(true) {
                    human_time = format!(" ({})", format_timespan(time));
                } else {
                    human_time = String::from("");
                }
                let plural = match time {
                    1 => "",
                    _ => "s",
                };
                last_block_time_str = format!(
                    "{}\n  Block finished in {} second{}{}\n",
                    border, time, plural, human_time
                );
            } else {
                last_block_time_str = String::from("")
            }
        }
        None => last_block_time_str = String::from(""),
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
        format!("{}{}\n  {} {} => {} | {}\n{}\n  {}          {}\n",
            last_block_time_str.yellow(),
            border.color(color).bold(),
            format!("{}", get_time()).white(),
            " STARTED BLOCK".color(color),
            format!("{}", &*current_chain.name).color(color),
            format!("#{}", height).color(color),
            //prev_block_time_str.color(color).bold(),
            border.color(color).bold(),
            "Base Target:".color(color).bold(),
            base_target.to_string().color(color),
        )
        .as_str(),
    );
    let mut actual_target_deadline = target_deadline;
    if actual_target_deadline == 0 {
        actual_target_deadline = u64::max_value();
    }
    if current_chain.target_deadline.is_some() {
        actual_target_deadline = current_chain.target_deadline.unwrap()
    }
    let mut human_readable_target_deadline = String::from("");
    match get_dynamic_deadline_for_block(base_target) {
        (true, _, net_difficulty, dynamic_target_deadline) => {
            let mut dynamic_target_deadline_warning = String::from("");
            if dynamic_target_deadline < actual_target_deadline {
                actual_target_deadline = dynamic_target_deadline;
            } else {
                dynamic_target_deadline_warning =
                    format!(" [Dyn > Max Target! - {}]", dynamic_target_deadline);
            }
            if crate::CONF
                .show_human_readable_deadlines
                .unwrap_or_default()
            {
                human_readable_target_deadline =
                    format!(" ({})", format_timespan(actual_target_deadline));
                if dynamic_target_deadline_warning.len() > 0 {
                    dynamic_target_deadline_warning
                        .truncate(dynamic_target_deadline_warning.len() - 1);
                    dynamic_target_deadline_warning.push_str(
                        format!(" ({})]", format_timespan(dynamic_target_deadline)).as_str(),
                    );
                }
            }
            new_block_message.push_str(
                format!("  {}      {}\n",
                    "Target Deadline:".color(color).bold(),
                    format!(
                        "{}{}{}", // (Upstream: {} | Config: {} | Dynamic: {})",
                        actual_target_deadline,
                        human_readable_target_deadline,
                        dynamic_target_deadline_warning,
                        /*target_deadline,
                        chain_tdl,
                        dynamic_target_deadline,*/
                    )
                    .color(color)
                )
                .as_str(),
            );
            if !current_chain.is_bhd.unwrap_or_default() {
                new_block_message.push_str(
                    format!("  {}   {}\n",
                        "Network Difficulty:".color(color).bold(),
                        format!("{} TiB", net_difficulty).color(color)
                    )
                    .as_str(),
                );
            } else {
                let bhd_net_diff = get_network_difficulty_for_block(base_target, 300);
                new_block_message.push_str(
                    format!("  {}   {}\n",
                        "Network Difficulty:".color(color).bold(),
                        format!(
                            "{} TiB (Proper for BHD = {} TiB)",
                            net_difficulty, bhd_net_diff
                        )
                        .color(color)
                    )
                    .as_str(),
                );
            }
        }
        (false, _, net_difficulty, _) => {
            if crate::CONF.show_human_readable_deadlines.unwrap_or_default() {
                human_readable_target_deadline =
                    format!(" ({})", format_timespan(actual_target_deadline));
            }
            new_block_message.push_str(
                format!(
                    "  {}      {}\n",
                    "Target Deadline:".color(color).bold(),
                    format!("{}{}", // (Upstream: {} | Config: {})",
                        actual_target_deadline,
                        human_readable_target_deadline,
                        /*target_deadline,
                        chain_tdl,*/
                    )
                    .color(color)
                )
                .as_str(),
            );
            if !current_chain.is_bhd.unwrap_or_default() {
                new_block_message.push_str(
                    format!("  {}   {}\n",
                        "Network Difficulty:".color(color).bold(),
                        format!("{} TiB", net_difficulty).color(color)
                    )
                    .as_str(),
                );
            } else {
                let bhd_net_diff = get_network_difficulty_for_block(base_target, 300);
                new_block_message.push_str(
                    format!("  {}   {}\n",
                        "Network Difficulty:".color(color).bold(),
                        format!(
                            "{} TiB (Proper for BHD = {} TiB)",
                            net_difficulty, bhd_net_diff
                        )
                        .color(color)
                    )
                    .as_str(),
                );
            }
        }
    };
    new_block_message.push_str(
        format!("  {} {}\n{}",
            "Generation Signature:".color(color).bold(),
            gen_sig.color(color),
            border.color(color).bold()
        )
        .as_str(),
    );
    println!("{}", new_block_message);
}

#[allow(dead_code)]
fn print_nonce_skipped(
    index: u8,
    height: u32,
    account_id: u64,
    deadline: u64,
    user_agent: &str,
    target_deadline: u64,
) {
    let current_chain = get_chain_from_index(index).unwrap();
    //let scoop_num = rand::thread_rng().gen_range(0, 4097);
    let color = get_color(&*current_chain.color);
    let mut deadline_string = deadline.to_string();
    if crate::CONF
        .show_human_readable_deadlines
        .unwrap_or_default()
    {
        deadline_string.push_str(format!(" ({})", format_timespan(deadline)).as_str());
    }
    let deadline_color = match deadline {
        0...3600 => "green",
        3601...86400 => "yellow",
        _ => "white",
    };
    println!("    {} ==> {}{} ==> {} {}{}\n        {}                           {}",
        user_agent.to_string().color(color).bold(),
        "Block #".color(color).bold(),
        height.to_string().color(color),
        "Numeric ID:".color(color).bold(),
        censor_account_id(account_id).color(color),
        format!(" [TDL: {}]", target_deadline).red(),
        "Skipped:".yellow(),
        deadline_string.color(deadline_color),
    );
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
) {
    let current_chain = get_chain_from_index(index).unwrap();

    // check if this is a submission for the actual current chain we're mining
    let actual_current_chain_index = arbiter::get_current_chain_index();
    let actual_current_chain_height = arbiter::get_latest_chain_info(actual_current_chain_index).0;

    if actual_current_chain_index == index && actual_current_chain_height == height {
        //let scoop_num = rand::thread_rng().gen_range(0, 4097);
        let color = get_color(&*current_chain.color);
        let mut deadline_string = deadline.to_string();
        if crate::CONF
            .show_human_readable_deadlines
            .unwrap_or_default()
        {
            deadline_string.push_str(format!(" ({})", format_timespan(deadline)).as_str());
        }
        let deadline_color = match deadline {
            0...3600 => "green",
            3601...86400 => "yellow",
            _ => "white",
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
        if !id_override {
            println!("    {}{} ==> {}{} ==> {} {}\n        {}                          {}",
                remote_address.color("white"),
                user_agent.to_string().color(color).bold(),
                "Block #".color(color).bold(),
                height.to_string().color(color),
                "Numeric ID:".color(color).bold(),
                censor_account_id(account_id).color(color),
                "Deadline:".color(color).bold(),
                deadline_string.color(deadline_color)
            );
        } else {
            println!("    {}{} ==> {}{} ==> {} {}{}\n        {}                          {}",
                remote_address.color("white"),
                user_agent.to_string().color(color).bold(),
                "Block #".color(color).bold(),
                height.to_string().color(color),
                "Numeric ID:".color(color).bold(),
                censor_account_id(account_id).color(color),
                format!(" [TDL: {}]", target_deadline).red(),
                "Deadline:".color(color).bold(),
                deadline_string.color(deadline_color),
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