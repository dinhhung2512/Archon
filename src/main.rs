#![feature(vec_remove_item, proc_macro_hygiene, decl_macro)]
#[cfg(target_os = "windows")]
use ansi_term;
use chrono::{DateTime, Local};
use colored;
use colored::Colorize;
use std::collections::HashMap;
use std::fs::File;
use std::process::exit;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use fern;

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

const APP_NAME: &'static str = env!("CARGO_PKG_NAME");
const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn main() -> Result<(), ()> {
    app::new().map(|application| {
        app.start();
        app
    }).map_err(|why| {
        println!("  {} {}", "ERROR".red().underline(), "Archon has failed to start.".red());
    })
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