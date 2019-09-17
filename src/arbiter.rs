use chrono::{DateTime, Local};
use ansi_term::Colour;
use reqwest;
use futures::sync::mpsc;
use futures::{Future, Stream};
use tokio_tungstenite::connect_async;
use url::Url;
use tungstenite::Message;
use std::collections::HashMap;
use std::sync::mpsc as std_mpsc;
use std::thread;
use reqwest::header;

use crate::config::PocChain;
use crate::upstream::MiningInfo;
use crate::web::{SubmitNonceResponse};
use crate::LastBlockInfo;

#[derive(Debug, Clone)]
struct MiningInfoPollingResult {
    mining_info: MiningInfo,
    chain: PocChain,
}

#[derive(Debug, Clone)]
pub struct HDPoolSubmitNonceInfo {
    pub account_id: u64,
    pub height: u32,
    pub nonce: u64,
    pub deadline_adjusted: u64,
    pub deadline_unadjusted: u64,
    pub notify_response_sender: crossbeam::channel::Sender<String>
}

fn create_chain_nonce_submission_client(chain_index: u8) {
    // get current chain
    let chain = super::get_chain_from_index(chain_index);
    let mut default_headers = header::HeaderMap::new();
    if chain.miner_alias.is_some() {
        default_headers.insert("x-mineralias", get_header_value(chain.miner_alias.unwrap()));
    }
    // if this chain is for hpool, add a default header to this client with the user's account key
    if chain.is_hpool.unwrap_or_default() {
        let app_name = format!("{}", super::uppercase_first(super::APP_NAME));
        let app_name_ver = format!("{} v{}", app_name.clone(), super::VERSION);
        // get account key from config
        let account_key_header = chain.account_key.unwrap_or(String::from(""));
        default_headers.insert("X-Account", get_header_value(account_key_header));
        let mut miner_name = match chain.miner_name.clone() {
            Some(miner_name) => format!("{} via {}", miner_name, app_name.clone()),
            _ => match gethostname::gethostname().to_str() {
                Some(hostname) => format!("{} via {}", hostname, app_name.clone()),
                None => app_name.clone(),
            },
        };
        if chain.append_version_to_miner_name.unwrap_or_default() {
            miner_name.push_str(format!(" v{}", super::VERSION).as_str());
        }
        default_headers.insert("X-MinerName", get_header_value(miner_name));
        default_headers.insert(
            "X-Capacity",
            get_header_value(format!(
                "{}",
                super::get_total_plots_size_in_tebibytes() * 1024f64
            )),
        );
        default_headers.insert("X-Miner", get_header_value(app_name_ver.clone()));
    } else {
        if chain.payout_address.is_some() && chain.account_key.is_none() {
            default_headers.insert("X-Account", get_header_value(chain.payout_address.unwrap()));
        }
    }
    let mut chain_nonce_submission_clients = crate::CHAIN_NONCE_SUBMISSION_CLIENTS.lock().unwrap();
    let timeout = match chain.timeout.unwrap_or(crate::CONF.timeout.unwrap_or(5)) as u64 {
        0..=2 => 3u64,
        other => other
    };
    chain_nonce_submission_clients.insert(
        chain_index,
        reqwest::Client::builder()
            .default_headers(default_headers)
            .timeout(std::time::Duration::from_secs(timeout))
            .build()
            .unwrap(),
    );
    drop(chain_nonce_submission_clients);
}

fn get_header_value(string_data: String) -> header::HeaderValue {
    let header_value: header::HeaderValue = match string_data.parse() {
        Ok(val) => val,
        Err(why) => {
            warn!("Couldn't parse {} into a HeaderValue: {:?}", string_data, why);
            "Invalid Header Data".parse().unwrap()
        }
    };
    header_value
}

pub fn thread_arbitrate() {
    // wait for a couple of seconds to try to let connected miners send their plot size first, to
    // try to ensure dynamic deadlines etc are correct
    std::thread::sleep(std::time::Duration::from_secs(7));
    let (mining_info_sender, mining_info_receiver) = std_mpsc::channel();
    // start polling for mining info for each chain
    for inner in &crate::CONF.poc_chains {
        for chain in inner {
            if chain.enabled.unwrap_or(true) {
                let new_sender = mining_info_sender.clone();
                let index = super::get_chain_index(&*chain.url, &*chain.name);
                create_chain_nonce_submission_client(index);
                let timeout = match chain.timeout.unwrap_or(crate::CONF.timeout.unwrap_or(5)) as u64 {
                    0..=2 => 3u64,
                    other => other
                };
                thread::spawn(move || {
                    thread_get_mining_info(
                        reqwest::Client::builder().timeout(std::time::Duration::from_secs(timeout)).build().unwrap(),
                        chain.clone(),
                        new_sender,
                    );
                });
            }
        }
    }

    loop {
        match mining_info_receiver.recv() {
            Ok(_mining_info_polling_result) => {
                info!("NEW BLOCK - {}: {:?}", &*_mining_info_polling_result.chain.name, _mining_info_polling_result.mining_info);
                update_chain_info(&_mining_info_polling_result);
                process_new_block(&_mining_info_polling_result);
            }
            Err(_) => {}
        }
    }
}

fn thread_handle_hdpool_nonce_submissions(
    chain: PocChain,
    receiver: crossbeam::channel::Receiver<HDPoolSubmitNonceInfo>,
    tx: mpsc::UnboundedSender<Message>,
    ks_rx: crossbeam::channel::Receiver<bool>,
) {
    let miner_mark = "20190910";
    let account_key = chain.account_key.clone().unwrap_or(String::from(""));
    loop {
        let ks = match ks_rx.try_recv() {
            Ok(signal) => signal,
            Err(_) => false,
        };
        if ks == true {
            break;
        }
        match receiver.try_recv() {
            Ok(submit_nonce_info) => {
                let capacity_gb = crate::get_total_plots_size_in_tebibytes() * 1024f64;
                let unix_timestamp = Local::now().timestamp();
                let mut miner_name = match chain.miner_name.clone() {
                    Some(miner_name) => {
                        format!("{} via {}", miner_name, super::uppercase_first(super::APP_NAME))
                    },
                    _ => {
                        match gethostname::gethostname().to_str() {
                            Some(hostname) => format!("{} via {}", hostname, super::uppercase_first(super::APP_NAME)),
                            None => super::uppercase_first(super::APP_NAME)
                        }
                    }
                };
                if chain.append_version_to_miner_name.unwrap_or_default() {
                    miner_name.push_str(format!(" v{}", super::VERSION).as_str());
                }
                let message = format!(r#"{{"cmd":"poolmgr.submit_nonce","para":{{"account_key":"{}","capacity":{},"miner_mark":"{}","miner_name":"{}","submit":[{{"accountId":{},"height":{},"nonce":{},"deadline":{},"ts":{}}}]}}}}"#, account_key, capacity_gb, miner_mark, miner_name, submit_nonce_info.account_id, submit_nonce_info.height, submit_nonce_info.nonce, submit_nonce_info.deadline_unadjusted, unix_timestamp);
                debug!("HDPool Websocket: SubmitNonce message: {}", message);
                match tx.unbounded_send(Message::Text(message.clone().into())) {
                    Ok(_) => {
                        info!("HDP-WS: Sent DL Successfully: {}", message.clone());
                        match submit_nonce_info.notify_response_sender.send(format!(r#"{{"result":"success","deadline":"{}"}}"#, submit_nonce_info.deadline_adjusted)) {
                            Ok(_) => debug!("HDP-WS: Sent response signal successfully."),
                            Err(_) => warn!("HDP-WS: Failed to send response signal.")
                        };
                    },
                    Err(why) => {
                        info!("HDPool Websocket SubmitNonce failure: {:?}.", why);
                        match submit_nonce_info.notify_response_sender.send(format!(r#"{{"result":"failure","reason":"{}"}}"#, why)) {
                            Ok(_) => debug!("HDP-WS: Sent response signal successfully."),
                            Err(_) => warn!("HDP-WS: Failed to send response signal.")
                        };
                    },
                }
            },
            Err(_) => {}
        };
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn thread_hdpool_websocket(
    chain: PocChain,
    mining_info_sender: crossbeam::channel::Sender<String>,
    nonce_submission_receiver: crossbeam::channel::Receiver<HDPoolSubmitNonceInfo>,
) {
    loop {
        let (tx, rx) = mpsc::unbounded();
        let (kill_switch_hb_tx, kill_switch_hb_rx) = crossbeam::channel::unbounded();
        let (kill_switch_nonce_tx, kill_switch_nonce_rx) = crossbeam::channel::unbounded();
        let rx = rx.map_err(|_| panic!());
        let txc = tx.clone();
        let txs = tx.clone();

        // set vars
        let addr;
        if chain.is_hdpool.unwrap_or_default() {
            addr = Url::parse("wss://hdminer.hdpool.com").unwrap();
        } else if chain.is_hdpool_eco.unwrap_or_default() {
            addr = Url::parse("wss://ecominer.hdpool.com").unwrap();
        } else {
            error!("Trying to set up websocket for HDPool Direct, but chain is not for HDPool CO or ECO Pool!");
            return;
        }
        let miner_mark = "20190910";
        let account_key = chain.account_key.clone().unwrap_or(String::from(""));
        let mut miner_name = match chain.miner_name.clone() {
            Some(miner_name) => {
                format!("{} via {}", miner_name, super::uppercase_first(super::APP_NAME))
            },
            _ => {
                match gethostname::gethostname().to_str() {
                    Some(hostname) => format!("{} via {}", hostname, super::uppercase_first(super::APP_NAME)),
                    None => super::uppercase_first(super::APP_NAME)
                }
            }
        };
        if chain.append_version_to_miner_name.unwrap_or_default() {
            miner_name.push_str(format!(" v{}", super::VERSION).as_str());
        }

        // Spawn thread for the heartbeat loop to run in.
        let hb_child_thread = thread::spawn(move || {
            loop {
                let ks = match kill_switch_hb_rx.try_recv() {
                    Ok(signal) => signal,
                    Err(_) => false,
                };
                if ks == true {
                    break;
                }
                let capacity_gb = crate::get_total_plots_size_in_tebibytes() * 1024f64;
                let data = format!(r#"{{"cmd":"poolmgr.heartbeat","para":{{"account_key":"{}","miner_name":"{}","miner_mark":"{}","capacity":{}}}}}"#,
                    account_key, miner_name, miner_mark, capacity_gb);
                match txc.unbounded_send(Message::Text(data.clone().into())) {
                    Ok(_) => {
                        trace!("Heartbeat Sent:\n    {}", data);
                    },
                    Err(why) => {
                        warn!("HDPool Websocket Heartbeat failure: {:?}.", why);
                    },
                };
                thread::sleep(std::time::Duration::from_secs(5));
            }
        });

        // spawn thread to handle nonce submissions
        let chain_copy = chain.clone();
        let nonce_submission_receiver_clone = nonce_submission_receiver.clone();
        let nonce_child_thread = thread::spawn(move || {
            thread_handle_hdpool_nonce_submissions(chain_copy, nonce_submission_receiver_clone, txs, kill_switch_nonce_rx);
        });

        let mining_info_sender = mining_info_sender.clone();
        let client = connect_async(addr).and_then(move |(ws_stream, _)| {
            use futures::Sink;
            let (mut sink, stream) = ws_stream.split();

            sink.start_send(Message::Text(r#"{"cmd":"mining_info"}"#.into())).unwrap();
            sink.start_send(Message::Text(r#"{"cmd":"poolmgr.mining_info"}"#.into())).unwrap();

            let ws_writer = rx.fold(sink, |mut sink, msg: Message| {
                sink.start_send(msg).unwrap();
                Ok(sink)
            });

            let ws_reader = stream.for_each(move |message: Message| {
                match message.to_text() {
                    Ok(message_str) => { 
                        match message_str.to_lowercase().as_str() {
                            r#"{"cmd":"poolmgr.heartbeat"}"# => {
                                trace!("Heartbeat acknowledged.");
                            },
                            _ => {
                                debug!("HDPool WebSocket: Received:\n    {}", message);
                                if message_str.to_lowercase().starts_with(r#"{"cmd":"mining_info""#) || message_str.to_lowercase().starts_with(r#"{"cmd":"poolmgr.mining_info"#) {
                                    let parsed_message_str: serde_json::Value = serde_json::from_str(&message_str).unwrap();
                                    let mining_info = parsed_message_str["para"].to_string().clone();
                                    debug!("HDPool WebSocket: NEW BHD BLOCK: {}", mining_info);
                                    match mining_info_sender.send(mining_info) {
                                        Ok(_) => {
                                            debug!("Sent mining info through channel successfully.");
                                        }
                                        Err(why) => {
                                            warn!("HDP-WS: Couldn't send mining info through channel: {}", why);
                                        }
                                    }
                                } else {
                                    debug!("HDPool WebSocket: Received unknown message: {}", message);
                                }
                            },
                        }
                    },
                    Err(_) => {}
                }

                Ok(())
            });

            ws_writer.map(|_| ()).map_err(|e| { warn!("HDPool WebSocket Failure: {:?}", e); () })
                .select(ws_reader.map(|_| ()).map_err(|e| { warn!("HDPool WebSocket Failure: {:?}", e); () }))
                .then(|_| Ok(()))

        }).map_err(|e| {
            use std::io;

            error!("Error during the websocket handshake occured: {}", e);
            io::Error::new(io::ErrorKind::Other, e);
        });

        tokio::runtime::run(client.map_err(|e| error!("{:?}", e)));

        let _ = kill_switch_hb_tx.send(true).unwrap();
        let _ = kill_switch_nonce_tx.send(true).unwrap();

        let _ = nonce_child_thread.join();
        let _ = hb_child_thread.join();

        info!("HDPool Websocket: Attempting to reconnect in 10 seconds.");
        thread::sleep(std::time::Duration::from_secs(10));
    }
}

fn thread_get_mining_info(
    client: reqwest::Client,
    chain: PocChain,
    sender: std_mpsc::Sender<MiningInfoPollingResult>,
) {
    let mut url = String::from(&*chain.url);
    if chain.is_boomcoin.unwrap_or(false) {
        url.push_str("/boom?requestType=getMiningInfo");
    } else {
        url.push_str("/burst?requestType=getMiningInfo");
    }
    let mut last_block_height = 0 as u32;
    let mut last_generation_signature = String::from("");
    let mut request_failure = false;
    let mut last_request_success: DateTime<Local> = Local::now();
    let mut last_outage_reminder_sent: DateTime<Local> = Local::now();
    let mut num_failures_since_success = 0u16;
    
    // setup mpsc channel to receive signal when new mining info is received from HDPool websocket
    //let (hdpool_mining_info_sender, hdpool_mining_info_receiver) = std_mpsc::channel();
    // Mpmc channels for now, for the ease of recreating the receiver if a thread dies. Will find a better solution later.
    let (hdpool_mining_info_sender, hdpool_mining_info_receiver) = crossbeam::channel::unbounded();
    
/* BEGIN ASYNC WEBSOCK STUFF. */
    if (chain.is_hdpool.unwrap_or_default() || chain.is_hdpool_eco.unwrap_or_default()) && chain.account_key.is_some() {
        // setup mpsc channel to receive signal when nonce submissions are received from connected miners
        //let (hdpool_nonce_submission_sender, hdpool_nonce_submission_receiver) = std_mpsc::channel();
        // Mpmc channels for now, for the ease of recreating the receiver if a thread dies. Will find a better solution later.

        let (hdpool_nonce_submission_sender, hdpool_nonce_submission_receiver) = crossbeam::channel::unbounded();

        // Spawn thread for the tokio reactor to run in.
        let chain_copy = chain.clone();
        thread::spawn(move || {
            // set global submit nonce sender so it can be accessed from nonce submission handler code
            *crate::HDPOOL_SUBMIT_NONCE_SENDER.lock().unwrap() = Some(hdpool_nonce_submission_sender.clone());
            thread_hdpool_websocket(chain_copy, hdpool_mining_info_sender.clone(), hdpool_nonce_submission_receiver);
        });
    }
/* END ASYNC WEBSOCK STUFF. */

    loop {
        let is_hdpool = (chain.is_hdpool.unwrap_or_default()
            || chain.is_hdpool_eco.unwrap_or_default())
            && chain.account_key.is_some();
        let mining_info_response = match is_hdpool {
            true => {
                match hdpool_mining_info_receiver.try_recv() {
                    Ok(mining_info) => mining_info,
                    Err(_) => String::from("none"),
                }
            },
            false => {
                let mut url = String::from(chain.clone().url);
                if chain.is_boomcoin.unwrap_or(false) {
                    url.push_str("/boom?requestType=getMiningInfo");
                } else {
                    url.push_str("/burst?requestType=getMiningInfo");
                }
                if chain.is_hpool.unwrap_or_default() && chain.account_key.is_some() {
                    let miner_name = match chain.miner_name.clone() {
                        Some(miner_name) => {
                            format!("{} via ", miner_name)
                        },
                        _ => {
                            match gethostname::gethostname().to_str() {
                                Some(hostname) => format!("{} via ", hostname),
                                None => String::from("")
                            }
                        }
                    };
                    match client
                        .get(url.as_str())
                        .header("User-Agent", 
                            format!("{} v{}", 
                                super::uppercase_first(super::APP_NAME), 
                                super::VERSION
                            )
                        )
                        .header("X-Account", format!("{}", chain.account_key.clone().unwrap_or(String::from(""))))
                        .header("X-MinerName", format!("{}{} v{}", miner_name, super::uppercase_first(super::APP_NAME), super::VERSION))
                        .header("X-Capacity", format!("{}", super::get_total_plots_size_in_tebibytes() * 1024f64))
                        .send() {
                        Ok(mut resp) => {
                            match &resp.text() {
                                Ok(text) => text.to_string(),
                                Err(why) => {
                                    warn!("GetMiningInfo({}): Could not get response text: {}", &*chain.name, why);
                                    String::from("none")
                                }
                            }
                        },
                        Err(why) => {
                            debug!("GetMiningInfo({}): Request failed: {}", &*chain.name, why);
                            String::from("none")
                        }
                    }
                } else {
                    match client
                        .get(url.as_str())
                        .header("User-Agent", 
                            format!("{} v{}", 
                                super::uppercase_first(super::APP_NAME), 
                                super::VERSION
                            )
                        )
                        .send() {
                        Ok(mut resp) => {
                            match &resp.text() {
                                Ok(text) => text.to_string(),
                                Err(why) => {
                                    warn!("GetMiningInfo({}): Could not get response text: {}", &*chain.name, why);
                                    String::from("none")
                                }
                            }
                        },
                        Err(why) => {
                            debug!("GetMiningInfo({}): Request failed: {}", &*chain.name, why);
                            String::from("none")
                        }
                    }
                }
            }
        };
        let parse_result;
        let _mining_info;
        if mining_info_response != "none" {
            match MiningInfo::from_json(mining_info_response.as_str()) {
                (result, mining_info) => {
                    parse_result = result;
                    _mining_info = mining_info;
                }
            };
        } else {
            parse_result = false;
            _mining_info = MiningInfo::empty();
        }
        let chain_color = super::get_color(chain.clone());
        if parse_result {
            if request_failure {
                request_failure = false;
                let outage_duration = Local::now() - last_request_success;
                let outage_duration_str = super::format_timespan(
                    outage_duration.num_seconds() as u64,
                );
                num_failures_since_success = 0;
                println!("  {} {} {}",
                    super::get_time(),
                    chain_color.paint(format!("{}", &*chain.name)),
                    Colour::Green.paint(format!("Outage over, total time unavailable: {}.", outage_duration_str))
                );
                info!("{} - Outage over, total time unavailable: {}.", &*chain.name, outage_duration_str);
            }
            last_request_success = Local::now();
            if (chain.allow_lower_block_heights.unwrap_or_default()
                && _mining_info.height != last_block_height)
                || _mining_info.height > last_block_height
                || (_mining_info.height == last_block_height && _mining_info.generation_signature != last_generation_signature) // fork detection
            {
                // fork detection
                let gensig = _mining_info.generation_signature.clone();
                if _mining_info.height == last_block_height && gensig != last_generation_signature {
                    info!("Fork Detected! (#{}, different gensig {}=>{})", last_block_height, last_generation_signature.clone(), gensig.clone());
                }
                last_block_height = _mining_info.height;
                last_generation_signature = gensig;
                let _mining_info_polling_result = MiningInfoPollingResult {
                    mining_info: _mining_info.clone(),
                    chain: chain.clone(),
                };
                match sender.send(_mining_info_polling_result) {
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
            drop(_mining_info);
        } else {
            if !is_hdpool {
                if !request_failure && num_failures_since_success > 5 {
                    request_failure = true;
                    last_outage_reminder_sent = Local::now();
                    println!("  {} {} {}",
                        super::get_time(),
                        chain_color.paint(format!("{}", &*chain.name)),
                        Colour::Red.paint("Could not retrieve mining info!")
                    );
                    info!("{} ({}) - Error getting mining info! Outage started: {}", &*chain.name, &*chain.url, mining_info_response);
                } else if num_failures_since_success > 5 {
                    let outage_duration = Local::now() - last_request_success;
                    let last_reminder = Local::now() - last_outage_reminder_sent;
                    if last_reminder.num_seconds()
                        >= crate::CONF.outage_status_update_interval.unwrap_or(300u16) as i64
                    {
                        last_outage_reminder_sent = Local::now();
                        let outage_duration_str =
                            super::format_timespan(outage_duration.num_seconds() as u64);
                        println!("  {} {} {}",
                            super::get_time(),
                            chain_color.paint(format!("{} - Last: {}", &*chain.name, last_block_height)),
                            Colour::Red.paint(format!("Outage continues, time unavailable so far: {}.", outage_duration_str))
                        );
                        info!("{} - Last: {} - Outage continues, time unavailable so far: {}", &*chain.name, last_block_height, outage_duration_str);
                    }
                } else {
                    num_failures_since_success += 1;
                }
            }
        }
        let mut interval = chain.get_mining_info_interval.unwrap_or(3) as u64;
        if interval < 1 {
            interval = 1;
        }
        thread::sleep(std::time::Duration::from_secs(interval));
    }
}

fn update_chain_info(mining_info_polling_result: &MiningInfoPollingResult) {
    // insert the new mining info into the mining infos map with the current time
    let index = super::get_chain_index(
        &*mining_info_polling_result.chain.url,
        &*mining_info_polling_result.chain.name,
    );
    let mut chain_info_map = crate::CHAIN_MINING_INFOS.lock().unwrap();
    chain_info_map.insert(
        index,
        (mining_info_polling_result.mining_info.clone(), Local::now()),
    );
}

// wrapper function to safely retrieve the current chain index from the mutex without holding a lock
pub fn get_current_chain_index() -> u8 {
    return *crate::CURRENT_CHAIN_INDEX.lock().unwrap();
}

fn requeue_forked_block(index: u8, height: u32, time: DateTime<Local>) {
    let mut chain_queue_status_map = crate::CHAIN_QUEUE_STATUS.lock().unwrap();
    chain_queue_status_map.insert(index, (height - 1, time));
    let mut block_start_printed_map = crate::BLOCK_START_PRINTED.lock().unwrap();
    block_start_printed_map.insert(index, height - 1);
    // reset the number of times this block height has been requeued
    let mut chain_requeue_times_map = crate::CHAIN_REQUEUE_TIMES.lock().unwrap();
    chain_requeue_times_map.insert(index, (height, 0));
}

fn process_new_block(mining_info_polling_result: &MiningInfoPollingResult) {
    let index = super::get_chain_index(
        &*mining_info_polling_result.chain.url,
        &*mining_info_polling_result.chain.name,
    );
    let current_chain_index = get_current_chain_index();
    let current_chain = super::get_chain_from_index(current_chain_index);
    // get currently mining block height before we change it
    let current_block_height = match super::get_current_mining_info() {
        Some(mi) => mi.height,
        _ => 0,
    };
    let last_block_time = get_time_since_block_start(current_block_height);
    // check if this is new info for a forked block
    let (fork_check_height, fork_check_gensig) = match get_current_chain_mining_info(index) {
        Some((mi, _)) => (mi.height, mi.generation_signature.clone()),
        _ => (0, String::from("")),
    };
    let is_forked = fork_check_height == mining_info_polling_result.mining_info.height 
                    && fork_check_gensig != mining_info_polling_result.mining_info.generation_signature;
    if is_forked {
        if index == current_chain_index {
            start_mining_chain(index, Some(LastBlockInfo::Forked(last_block_time, current_chain_index)));
        } else { // requeue the current block to force re-mining it with the new info
            super::print_forked_and_queued(index, fork_check_height);
            requeue_forked_block(index, fork_check_height, Local::now());
        }
        // update the mining info cache
        super::add_mining_info_to_cache(index, mining_info_polling_result.mining_info.clone());
    } else if crate::CONF.priority_mode.unwrap_or(true) {
        if mining_info_polling_result.chain.priority <= current_chain.priority {
            // higher priority is LOWER in actual value
            if index != current_chain_index {
                if !has_grace_period_elapsed() {
                    if crate::CONF.interrupt_lower_priority_blocks.unwrap_or(true) {
                        start_mining_chain(index, Some(requeue_current_block(current_chain.requeue_interrupted_blocks.unwrap_or(true), index, Some(mining_info_polling_result.clone()))));
                        return;
                    } // else queue new block
                } else {
                    // if grace period has elapsed
                    start_mining_chain(index, Some(LastBlockInfo::Completed(last_block_time, current_chain_index)));
                    return;
                }
            } else {
                match any_blocks_queued() {
                    (true, 0..=1, _) => {
                        // queue new block
                    }
                    (_, _, _) => {
                        start_mining_chain(index, Some(LastBlockInfo::Superseded(last_block_time, current_chain_index)));
                    }
                }
                return;
            }
        } else if has_grace_period_elapsed() {
            start_mining_chain(index, Some(LastBlockInfo::Completed(last_block_time, current_chain_index)));
            return;
        } // else queue new block
    } else {
        // running in FIFO mode
        if index != current_chain_index {
            if has_grace_period_elapsed() {
                match any_blocks_queued() {
                    (true, _, _) => {
                        start_mining_chain(index, Some(LastBlockInfo::Completed(last_block_time, current_chain_index)));
                        return;
                    }
                    (false, _, _) => {}
                }; // else queue new block
            } // else queue new block
        } else {
            match any_blocks_queued() {
                (false, _, _) => {
                    start_mining_chain(index, Some(LastBlockInfo::Superseded(last_block_time, current_chain_index)));
                    return;
                }
                (true, _, _) => {}
            };
        } // else queue new block
    }
    // if the code makes it to this point, the new block will be "queued".
    info!("QUEUE BLOCK - {} #{}", &*mining_info_polling_result.chain.name, mining_info_polling_result.mining_info.height);
}

fn requeue_current_block(do_requeue: bool, interrupted_by_index: u8, mining_info_polling_result: Option<MiningInfoPollingResult>) -> LastBlockInfo {
    let current_chain_index = get_current_chain_index();
    let current_chain = super::get_chain_from_index(current_chain_index);
    let (requeued_height, requeued_time) = get_queued_chain_info(current_chain_index);
    let interrupted_by_name;
    let interrupted_by_height;
    match mining_info_polling_result {
        Some(mining_info_polling_result) => {
            interrupted_by_name = mining_info_polling_result.clone().chain.clone().name;
            interrupted_by_height = mining_info_polling_result.mining_info.clone().height;
        },
        None => {
            match (super::get_chain_from_index(interrupted_by_index), get_current_chain_mining_info(interrupted_by_index)) {
                (interrupted_by_chain, Some(interrupted_by_mining_info)) => {
                    interrupted_by_name = interrupted_by_chain.clone().name;
                    interrupted_by_height = interrupted_by_mining_info.0.clone().height;
                }
                _ => {
                    interrupted_by_name = String::from("Unknown");
                    interrupted_by_height = 0;
                }
            }
        }
    }
    // get currently mining block height before we change it
    let current_block_height = match super::get_current_mining_info() {
        Some(mi) => mi.height,
        _ => 0,
    };
    let last_block_time = get_time_since_block_start(current_block_height);
    let times_requeued = get_num_times_requeued(current_chain_index, requeued_height);
    if do_requeue && times_requeued < current_chain.maximum_requeue_times.unwrap_or(u8::max_value()) {
        // set the queue status for this chain back by 1, thereby "requeuing" it
        let mut chain_queue_status_map = crate::CHAIN_QUEUE_STATUS.lock().unwrap();
        chain_queue_status_map.insert(current_chain_index, (requeued_height - 1, requeued_time));
        let mut block_start_printed_map = crate::BLOCK_START_PRINTED.lock().unwrap();
        block_start_printed_map.insert(current_chain_index, requeued_height - 1);
        // update the number of times this block height has been requeued if its run time is > 0
        if last_block_time > 0 {
            let mut chain_requeue_times_map = crate::CHAIN_REQUEUE_TIMES.lock().unwrap();
            chain_requeue_times_map.insert(current_chain_index, (requeued_height, times_requeued + 1));
        }
        if current_chain.maximum_requeue_times.is_none() {
            info!("INTERRUPT & REQUEUE BLOCK - {} #{} => {} #{}", &*current_chain.name, requeued_height, &*interrupted_by_name, interrupted_by_height);
            LastBlockInfo::Requeued(None, last_block_time, current_chain_index)
        } else {
            info!("INTERRUPT & REQUEUE BLOCK - {} #{} => {} #{} - Requeue #{} of {}", &*current_chain.name, requeued_height, &*interrupted_by_name, interrupted_by_height, times_requeued + 1, current_chain.maximum_requeue_times.unwrap());
            LastBlockInfo::Requeued(Some((times_requeued, current_chain.maximum_requeue_times.unwrap())), last_block_time, current_chain_index)
        }
    } else {
        info!("INTERRUPT BLOCK - {} #{} => {} #{}", &*current_chain.name, requeued_height, &*interrupted_by_name, interrupted_by_height);
        LastBlockInfo::Interrupted(last_block_time, current_chain_index)
    }
}

fn is_block_requeued(chain_index: u8, block_height: u32) -> bool {
    if block_height > 0 {
        let chain_queue_status_map = crate::CHAIN_QUEUE_STATUS.lock().unwrap();
        match chain_queue_status_map.get(&chain_index) {
            Some((height, _)) => {
                if *height == block_height - 1 {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn has_grace_period_elapsed() -> bool {
    let grace_period = time::Duration::seconds(crate::CONF.grace_period as i64);
    let current_chain_index = get_current_chain_index();
    let chain_queue_status_map = crate::CHAIN_QUEUE_STATUS.lock().unwrap();
    if chain_queue_status_map.len() > 0 {
        match chain_queue_status_map.get(&current_chain_index) {
            Some((_, start_time)) => {
                return (Local::now() - *start_time) >= grace_period;
            }
            None => {
                return false;
            }
        };
    } else {
        return true; // force starting a block if no blocks have been started
    }
    }

pub fn get_time_since_block_start(height: u32) -> u64 {
    let (chain_matched, current_chain_index) = get_chain_index_from_height(height);
    if chain_matched {
        let chain_queue_status_map = crate::CHAIN_QUEUE_STATUS.lock().unwrap();
        if chain_queue_status_map.len() > 0 {
            match chain_queue_status_map.get(&current_chain_index) {
                Some((_, start_time)) => {
                    return (Local::now() - *start_time).num_seconds() as u64;
                },
                _ => return 0u64,
            };
        }
    }
    return 0u64;
}

fn get_time_since_block_start_ms(height: u32) -> u64 {
    let (chain_matched, current_chain_index) = get_chain_index_from_height(height);
    if chain_matched {
        let chain_queue_status_map = crate::CHAIN_QUEUE_STATUS.lock().unwrap();
        if chain_queue_status_map.len() > 0 {
            match chain_queue_status_map.get(&current_chain_index) {
                Some((_, start_time)) => {
                    return (Local::now() - *start_time).num_milliseconds() as u64;
                    },
                _ => return 0u64,
            };
        }
    }
    return 0u64;
}

fn get_queued_chain_info(index: u8) -> (u32, DateTime<Local>) {
    let chain_queue_status_map = crate::CHAIN_QUEUE_STATUS.lock().unwrap();
    match chain_queue_status_map.get(&index) {
        Some((block_height, block_time)) => {
            return (*block_height, *block_time);
        }
        None => {
            return (0u32, Local::now());
        }
    };
}

fn get_num_times_requeued(index: u8, height: u32) -> u8 {
    let chain_requeue_times_map = crate::CHAIN_REQUEUE_TIMES.lock().unwrap();
    match chain_requeue_times_map.get(&index) {
        Some((block_height, requeues)) => {
            if *block_height == height {
                *requeues
            } else {
                0u8
            }
        },
        None => 0u8
    }
}

pub fn get_latest_chain_info(index: u8) -> (u32, DateTime<Local>) {
    let chain_mining_infos_map = crate::CHAIN_MINING_INFOS.lock().unwrap();
    match chain_mining_infos_map.get(&index) {
        Some((mining_info, block_time)) => {
            return (mining_info.height, *block_time);
        }
        None => {
            return (0u32, Local::now());
        }
    };
}

pub fn get_current_chain_mining_info(index: u8) -> Option<(MiningInfo, DateTime<Local>)> {
    let chain_mining_infos_map = crate::CHAIN_MINING_INFOS.lock().unwrap();
    match chain_mining_infos_map.get(&index) {
        Some((mining_info, block_time)) => {
            return Some((mining_info.clone(), *block_time));
        }
        None => {
            return None;
        }
    }
}

pub fn get_chain_index_from_height(height: u32) -> (bool, u8) {
    if height > 0 {
        for inner in &crate::CONF.poc_chains {
            for chain in inner {
                if chain.enabled.unwrap_or(true) {
                    let index = super::get_chain_index(&*chain.url, &*chain.name);
                    let (current_height, _) = get_latest_chain_info(index);
                    if current_height == height || (height > 0 && current_height == height - 1) {
                        trace!("Matched Block #{} to Chain #{}", height, index);
                        return (true, index);
                    }
                }
            }
        }
    }
    let current_chain_index = get_current_chain_index();
    trace!("Couldn't match Block #{} to a chain, using current chain index ({})", height, current_chain_index);
    return (false, current_chain_index);
}

// indicates state of queue
// returns highest priority block if running in priority mode, or oldest block if in FIFO mode
// (success, relative priority to current (1 = higher, 0 = same, -1 = lower), index)
fn any_blocks_queued() -> (bool, i8, u8) {
    let mut chain_indexes_with_queued_blocks: Vec<(u8, u32, u8, DateTime<Local>)> = Vec::new();
    let current_chain_index = get_current_chain_index();
    let mut current_chain_height = 0u32;
    // go through chains, check if each one has a higher blockheight queued, if so, store the index, and priority
    for inner in &crate::CONF.poc_chains {
        for chain in inner {
            if chain.enabled.unwrap_or(true) {
                let index = super::get_chain_index(&*chain.url, &*chain.name);
                let (current_height, current_time) = get_latest_chain_info(index);
                let (queued_height, _) = get_queued_chain_info(index);
                if queued_height < current_height {
                    chain_indexes_with_queued_blocks.push((
                        index,
                        queued_height,
                        chain.priority,
                        current_time,
                    ));
                }
                if index == current_chain_index {
                    current_chain_height = queued_height;
                }
            }
        }
    }
    if chain_indexes_with_queued_blocks.len() > 0 {
        let current_chain = super::get_chain_from_index(current_chain_index);
        if crate::CONF.priority_mode.unwrap_or(true) {
            let mut highest_priority_chain_index = 0u8;
            let mut highest_priority = u8::max_value();
            for (index, _, priority, _) in chain_indexes_with_queued_blocks.iter() {
                if *priority < highest_priority {
                    highest_priority = *priority;
                    highest_priority_chain_index = *index;
                }
            }
            if highest_priority < current_chain.priority {
                return (true, 1, highest_priority_chain_index);
            } else if highest_priority == current_chain.priority {
                return (true, 0, highest_priority_chain_index);
            } else {
                return (true, -1, highest_priority_chain_index);
            }
        } else {
            // FIFO mode
            let mut oldest_queued_chain_index = 0u8;
            let mut oldest_queued_chain_time = Local::now();
            for (index, height, _, time) in chain_indexes_with_queued_blocks.iter() {
                if *time < oldest_queued_chain_time
                    && (*index != current_chain_index
                        || (*index == current_chain_index && *height > current_chain_height))
                {
                    oldest_queued_chain_index = *index;
                    oldest_queued_chain_time = *time;
                }
            }
            return (true, 0, oldest_queued_chain_index);
        }
    } else {
        return (false, 0, 0);
    }
}

pub fn thread_arbitrate_queue() {
    loop {
        match any_blocks_queued() {
            (true, priority, index) => {
                // get currently mining block height before we change it
                let current_block_height = match super::get_current_mining_info() {
                    Some(mi) => mi.height,
                    _ => 0,
                };
                let current_chain_index = get_current_chain_index();
                let last_block_time = get_time_since_block_start(current_block_height);
                if crate::CONF.priority_mode.unwrap_or(true) {
                    match priority {
                        1 => {
                            // 1 = higher priority than current block
                            if has_grace_period_elapsed() {
                                start_mining_chain(index, Some(LastBlockInfo::Completed(last_block_time, current_chain_index)));
                            } else if crate::CONF.interrupt_lower_priority_blocks.unwrap_or(true) {
                                let current_chain_index = get_current_chain_index();
                                let current_chain = super::get_chain_from_index(current_chain_index);
                                start_mining_chain(index, Some(requeue_current_block(current_chain.requeue_interrupted_blocks.unwrap_or(true), index, None)));
                            } // else do nothing
                        }
                        0 => {
                            // 0 = same priority as current block
                            start_mining_chain(index, Some(LastBlockInfo::Superseded(last_block_time, current_chain_index)));
                        }
                        _ => {
                            // -1 = lower priority than current block
                            if has_grace_period_elapsed() {
                                start_mining_chain(index, Some(LastBlockInfo::Completed(last_block_time, current_chain_index)));
                            } // else do nothing
                        }
                    };
                } else {
                    // FIFO mode
                    if has_grace_period_elapsed() {
                        start_mining_chain(index, Some(LastBlockInfo::Completed(last_block_time, current_chain_index)));
                    } // else do nothing
                }
            }
            (false, _, _) => {} // nothing queued, nothing to do...
        };

        thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn start_mining_chain(index: u8, last_block_info: Option<super::LastBlockInfo>) {
    // get chain
    let chain = super::get_chain_from_index(index);
    // get access to chain mining infos
    match get_current_chain_mining_info(index) {
        Some((mining_info, _)) => {
            if mining_info.base_target > 0 {
                // print block info
                super::print_block_started(
                    index,
                    mining_info.height,
                    mining_info.base_target,
                    String::from(&*mining_info.generation_signature),
                    last_block_info,
                );
                info!("START BLOCK - Chain #{} - Block #{} - Priority {} | {} | {}", index, mining_info.height, chain.priority, &*chain.name, &*chain.url);
                // set last mining info
                *crate::LAST_MINING_INFO.lock().unwrap() = super::get_current_mining_info_json();
                // set current chain index
                *crate::CURRENT_CHAIN_INDEX.lock().unwrap() = index;
                // update the queue status for this chain
                let mut chain_queue_status_map = crate::CHAIN_QUEUE_STATUS.lock().unwrap();
                chain_queue_status_map.insert(index, (mining_info.height, Local::now()));
            }
        }
        _ => {}
    };
}

pub fn get_best_deadlines() -> HashMap<u32, Vec<(u64, u64)>> {
    return crate::BEST_DEADLINES.lock().unwrap().clone();
}

pub fn get_best_deadlines_for_block(block_height: u32) -> Option<Vec<(u64, u64)>> {
    let best_deadlines_map = crate::BEST_DEADLINES.lock().unwrap();
    match best_deadlines_map.get(&block_height) {
        Some(best_deadlines) => {
            return Some(best_deadlines.to_vec());
        }
        None => return None,
    };
}

pub fn get_best_deadline(block_height: u32, account_id: u64) -> u64 {
    match get_best_deadlines_for_block(block_height) {
        Some(best_deadlines) => {
            for best_deadline_tuple_ref in best_deadlines {
                let (id, deadline) = best_deadline_tuple_ref;
                if id == account_id {
                    return deadline;
                }
            }
        }
        None => {}
    };
    debug!("BestDL(Height={}, ID={}) = None found, using u64::max_value()", block_height, account_id);
    return u64::max_value();
}

fn update_best_deadline(block_height: u32, account_id: u64, deadline: u64) {
    debug!("NEW BestDL - Height={}, ID={}, DL={}", block_height, account_id, deadline);
    match get_best_deadlines_for_block(block_height) {
        Some(mut best_deadlines) => {
            // check if account id has a deadline in the vec
            let mut existing = (0u64, 0u64);
            let mut found = false;
            for best_deadline_for_account_id in &best_deadlines {
                let (id, _) = best_deadline_for_account_id;
                if *id == account_id {
                    existing = *best_deadline_for_account_id;
                    found = true;
                    break;
                }
            }
            if found {
                &best_deadlines.remove_item(&existing);
            }
            best_deadlines.push((account_id, deadline));
            let mut best_deadlines_map = crate::BEST_DEADLINES.lock().unwrap();
            best_deadlines_map.insert(block_height, best_deadlines);
        }
        None => {
            let mut best_deadlines_map = crate::BEST_DEADLINES.lock().unwrap();
            let mut best_deadlines: Vec<(u64, u64)> = Vec::new();
            best_deadlines.push((account_id, deadline));
            best_deadlines_map.insert(block_height, best_deadlines);
        }
    };
}

fn forward_nonce_submission(
    chain_index: u8,
    url: &str,
    user_agent_header: &str,
    mining_headers: crate::web::MiningHeaderData,
    miner_name: Option<String>,
    send_total_capacity: bool,
    is_hpool: bool,
    append_version_to_miner_name: bool,
) -> Option<String> {
    let chain_nonce_submission_clients = crate::CHAIN_NONCE_SUBMISSION_CLIENTS.lock().unwrap();
    let app_name = super::uppercase_first(super::APP_NAME);
    let app_name_ver = format!("{} v{}", app_name, super::VERSION);
    let hostname_os_str = gethostname::gethostname();
    let hostname = hostname_os_str.to_str();
    let mut submission_miner_name;
    // X-MinerName = ChainConfig.miner_name > hostname > mining software user agent > Archon vx.x.x-pre
    if is_hpool {
        if miner_name.is_some() {
            submission_miner_name = format!("{} via {}", user_agent_header, app_name.clone());
        } else if hostname.is_some() {
            submission_miner_name = format!("{} via {}", hostname.unwrap(), app_name.clone());
        } else {
            submission_miner_name = app_name.clone();
        }
        if append_version_to_miner_name {
            submission_miner_name.push_str(format!(" v{}", super::VERSION).as_str());
        }
    } else if miner_name.clone().is_some() {
        if append_version_to_miner_name {
            submission_miner_name = format!("{} via {}", miner_name.clone().unwrap(), app_name_ver.clone());
        } else {
            submission_miner_name = format!("{} via {}", miner_name.clone().unwrap(), app_name.clone());
        }
    } else if mining_headers.miner_name.len() > 0 {
        submission_miner_name = format!("{} via {}", user_agent_header, app_name_ver.clone());
    } else {
        submission_miner_name = app_name_ver.clone();
    }
    let capacity_to_send;
    if send_total_capacity {
        capacity_to_send = (super::get_total_plots_size_in_tebibytes() * 1024f64).to_string();
    } else {
        capacity_to_send = mining_headers.capacity.to_string();
    }
    trace!("DL Send URL: {}", url);
    match chain_nonce_submission_clients.get(&chain_index) {
        Some(client) => {
            match client
                .post(url)
                .header("User-Agent", format!("{} via {}", user_agent_header, app_name_ver.clone()))
                .header("X-Miner", format!("{} via {}", user_agent_header, app_name_ver.clone()))
                .header("X-Capacity", capacity_to_send)
                .header("X-MinerName", submission_miner_name)
                .send()
            {
                Ok(mut response) => match &response.text() {
                    Ok(text) => Some(text.to_string()),
                    Err(why) => {
                        warn!("Forward Nonce Submission(chInd={}, url={}, software={}) - Couldn't retrieve response data: {:?}", chain_index, url, user_agent_header, why);
                        None
                    },
                },
                Err(why) => {
                    warn!("Forward Nonce Submission(chInd={}, url={}, software={}) - Request failed: {:?}", chain_index, url, user_agent_header, why);
                    None
                },
            }
        }
        _ => {
            warn!("Forward Nonce Submission(chInd={}, url={}, software={}) - Couldn't find submission client for chain!", chain_index, url, user_agent_header);
            None
        }
    }
}

pub fn process_nonce_submission(
    block_height: u32,
    account_id: u64,
    nonce: u64,
    deadline: Option<u64>,
    user_agent_header: String,
    adjusted: bool,
    remote_addr: String,
    mining_headers: crate::web::MiningHeaderData,
) -> String {
    debug!("Received DL: Height={}, ID={}, Nonce={}, DL={:?}, Software={}, Adjusted={}, Address={}", block_height, account_id, nonce, deadline, user_agent_header, adjusted, remote_addr);
    // validate data
    // get mining info for chain
    let (chain_matched, mut chain_index) = get_chain_index_from_height(block_height); // defaults to the chain being currently mined if it cannot find a height match
    let mut height = block_height;
    if height == 0 {
        if !chain_matched {
            chain_index = get_current_chain_index();
        }
        height = match get_latest_chain_info(chain_index) {
            (height, _) => height,
        };
    }
    let current_chain = super::get_chain_from_index(chain_index);
    let chain_color = super::get_color(current_chain.clone());
    let base_target = match get_current_chain_mining_info(chain_index) {
        Some((mining_info, _)) => mining_info.base_target,
        _ => 0,
    };
    if base_target > 0 {
        let start_time = Local::now();
        let mut send_deadline = true;
        let mut print_deadline = true;
        let mut _deadline_sent = false;
        let mut deadline_accepted = false;
        let mut deadline_over_best = false;
        let mut _deadline_over_target = false;
        match deadline {
            Some(dl) => {
                let mut unadjusted_deadline = dl;
                let mut adjusted_deadline = dl / base_target as u64;
                if adjusted {
                    unadjusted_deadline = dl * base_target as u64;
                    adjusted_deadline = dl;
                }
                let mut id_override = false;
                use crate::TargetDeadlineType;
                let target_deadline = match super::get_target_deadline(Some(account_id), base_target, chain_index, Some(current_chain.clone())) {
                    TargetDeadlineType::ConfigOverriddenByID(tdl) => {
                        id_override = true;
                        tdl
                    },
                    TargetDeadlineType::ConfigChainLevel(tdl) => tdl,
                    TargetDeadlineType::Dynamic(tdl) => tdl,
                    TargetDeadlineType::PoolMaximum(tdl) => tdl,
                    TargetDeadlineType::Default => u64::max_value(),
                };
                // check that this deadline is lower than the target deadline
                if adjusted_deadline > target_deadline {
                    send_deadline = false;
                    _deadline_over_target = true;
                    print_deadline = false;
                }
                // check that this deadline is better than the best one submitted for this block and this account id
                let best_deadline = get_best_deadline(height, account_id);
                if best_deadline < adjusted_deadline {
                    send_deadline = false;
                    deadline_over_best = true;
                    print_deadline = false;
                }
                let mut failure_message: Option<String> = None;
                // find time since block was started
                let time_since_block_started = get_time_since_block_start_ms(height);
                if print_deadline {
                    super::print_nonce_submission(
                        chain_index,
                        height,
                        account_id,
                        adjusted_deadline,
                        &user_agent_header,
                        target_deadline,
                        id_override,
                        remote_addr.clone(),
                        time_since_block_started,
                    );
                }
                if !deadline_over_best {
                    update_best_deadline(height, account_id, adjusted_deadline);
                }
                let mut passphrase_str = String::from("");
                // if solo mining burst, look for a passphrase from config for this account id
                if !current_chain.is_hpool.unwrap_or_default()
                    && !current_chain.is_hdpool.unwrap_or_default()
                    && !current_chain.is_hdpool_eco.unwrap_or_default()
                    && !current_chain.is_pool.unwrap_or_default()
                    && !current_chain.is_bhd.unwrap_or_default()
                {
                    let mut passphrase_set = false;
                    match current_chain.clone().numeric_id_to_passphrase {
                        Some(map) => {
                            for id_and_passphrase in map {
                                if id_and_passphrase.0 == account_id {
                                    extern crate url;
                                    use url::form_urlencoded::{byte_serialize};
                                    let urlencoded: String = byte_serialize(&*id_and_passphrase.1.as_bytes()).collect();
                                    passphrase_str.push_str(
                                        format!("&secretPhrase={}", urlencoded).as_str(),
                                    );
                                    passphrase_set = true;
                                    break;
                                }
                            }
                        }
                        _ => {}
                    };
                    if !passphrase_set || passphrase_str.len() == 0 {
                        // send error to miner
                        let resp = SubmitNonceResponse{
                            result: String::from("failure"),
                            deadline: None,
                            reason: Some(format!("No passphrase for account ID [{}] was specified in Archon configuration for solo mining Burst/Boom! Check your spacing in the config file! There is an example at https://github.com/Bloodreaver/Archon#all-configuration-options-for-poc-chains", account_id)),
                        };
                        return resp.to_json();
                    }
                }
                if send_deadline {
                    thread::spawn(move || {
                        let mut attempt = 0;
                        let attempts = current_chain.submit_attempts.unwrap_or(crate::CONF.submit_attempts.unwrap_or(5));
                        while ((attempt == 0 && attempts == 0) || attempt < attempts) && !deadline_accepted {
                            // check if chain is HDPool Direct
                            if (current_chain.is_hdpool.unwrap_or_default() || current_chain.is_hdpool_eco.unwrap_or_default()) && current_chain.account_key.is_some() {
                                // get sender
                                let sender = crate::HDPOOL_SUBMIT_NONCE_SENDER.lock().unwrap();
                                if sender.is_some() {
                                    let sender = sender.clone().unwrap();
                                    info!("DL Send - #{} | ID={} | DL={} (Unadjusted={})", block_height, account_id, adjusted_deadline, unadjusted_deadline);
                                    trace!("HDP-WS - Send DL to MPMC:\n    ID={} Height={} Nonce={} DL={} UDL={}", account_id, height, nonce, adjusted_deadline, unadjusted_deadline);
                                    let (hdp_submit_response_sender, hdp_submit_response_receiver) = crossbeam::channel::unbounded();
                                    match sender.send(HDPoolSubmitNonceInfo { 
                                        account_id: account_id,
                                        height: height,
                                        nonce: nonce,
                                        deadline_unadjusted: unadjusted_deadline,
                                        deadline_adjusted: adjusted_deadline,
                                        notify_response_sender: hdp_submit_response_sender.clone(),
                                    }) {
                                        Ok(()) => {
                                            trace!("HDP-WS - Sent DL to MPMC Successfully:\n    ID={} Height={} Nonce={} DL={} UDL={}\n        Awaiting confirm/rejection response...", account_id, height, nonce,adjusted_deadline, unadjusted_deadline);
                                            match hdp_submit_response_receiver.recv() {
                                                Ok(response) => {
                                                    trace!("HDP-WS - Receiver responded with:\n    {}", response);
                                                    deadline_accepted = true;
                                                },
                                                Err(why) => {
                                                    debug!("HDP-WS - Receiver failed to receive submission response signal: {}", why);
                                                    attempt += 1;
                                                    std::thread::sleep(std::time::Duration::from_secs(3));
                                                    continue;
                                                }
                                            }
                                        },
                                        Err(why) => {
                                            debug!("HDP-WS - Failed to signal receiver: {:?}\n    ID={} Height={} Nonce={} DL={} UDL={}", why, account_id, height, nonce, adjusted_deadline, unadjusted_deadline);
                                        }
                                    };
                                }
                            } else { // not hdpool direct
                                let mut url = String::from(&*current_chain.url);
                                // check if NOT solo mining burst
                                if current_chain.is_hdpool.unwrap_or_default()
                                    || current_chain.is_hdpool_eco.unwrap_or_default()
                                    || current_chain.is_hpool.unwrap_or_default()
                                    || current_chain.is_bhd.unwrap_or_default()
                                    || current_chain.is_pool.unwrap_or_default() {
                                    if current_chain.is_boomcoin.unwrap_or(false) {
                                        url.push_str(format!("/boom?requestType=submitNonce&blockheight={}&accountId={}&nonce={}&deadline={}",
                                    height, account_id, nonce, unadjusted_deadline).as_str());
                                    } else {
                                        url.push_str(format!("/burst?requestType=submitNonce&blockheight={}&accountId={}&nonce={}&deadline={}",
                                    height, account_id, nonce, unadjusted_deadline).as_str());
                                    }
                                } else { // solo mining burst / boom
                                    if current_chain.is_boomcoin.unwrap_or(false) {
                                        url.push_str(format!("/boom?requestType=submitNonce&blockheight={}&accountId={}&nonce={}{}",
                                    height, account_id, nonce, passphrase_str).as_str());
                                    } else {
                                        url.push_str(format!("/burst?requestType=submitNonce&blockheight={}&accountId={}&nonce={}{}",
                                    height, account_id, nonce, passphrase_str).as_str());
                                    }
                                }
                                _deadline_sent = true;
                                info!("DL Send - #{} | ID={} | DL={} (Unadjusted={})", block_height, account_id, adjusted_deadline, unadjusted_deadline);
                                //let send_total_capacity = current_chain.is_hpool.unwrap_or_default();
                                match forward_nonce_submission(chain_index, url.as_str(), &user_agent_header, mining_headers.clone(), current_chain.miner_name.clone(), true, current_chain.is_hpool.unwrap_or_default(), current_chain.append_version_to_miner_name.unwrap_or_default())
                                {
                                    Some(text) => {
                                        debug!("DL Submit Response ({}): {}", adjusted_deadline, text);
                                        if text.contains("success")
                                            && text.contains(format!("{}", adjusted_deadline).as_str())
                                        {
                                            deadline_accepted = true;
                                        } else {
                                            if text.len() > 0 && failure_message.clone().is_none() {
                                                if text.contains("error reaching upstream") {
                                                    attempt += 1;
                                                    if attempt < attempts {
                                                        println!("          {}, {}:            {} {}",
                                                            Colour::Red.paint("Pool error"),
                                                            Colour::Yellow.paint("retrying"),
                                                            chain_color.paint(adjusted_deadline.to_string()),
                                                            Colour::Yellow.paint(format!("({}/{})", attempt + 1, attempts)),
                                                        );
                                                        std::thread::sleep(std::time::Duration::from_secs(3));
                                                        continue;
                                                    }
                                                }
                                                failure_message = Some(text);
                                                break;
                                            } else if text.len() > 0 && failure_message.clone().is_some() && text != failure_message.clone().unwrap() {
                                                // append new text to previous
                                                failure_message = Some(format!("{}{}", failure_message.clone().unwrap(), text));
                                                break;
                                            }
                                            info!("Submission failure [timeout] - DL={}", adjusted_deadline);

                                            // check if this is a submission for the actual current chain we're mining
                                            let actual_current_chain_index = get_current_chain_index();
                                            let actual_current_chain_height = get_latest_chain_info(actual_current_chain_index).0;
                                            attempt += 1;
                                            let best_deadline = get_best_deadline(height, account_id);
                                            if best_deadline < adjusted_deadline { // throw a fake confirm back to the miner since we have a better deadline than this one
                                                let resp = SubmitNonceResponse {
                                                    result: String::from("success"),
                                                    deadline: Some(adjusted_deadline),
                                                    reason: None
                                                };
                                                return resp.to_json();
                                            } else {
                                                if attempt < attempts {
                                                    if actual_current_chain_index == chain_index && actual_current_chain_height == block_height {
                                                        println!("          {}, {}:               {} {}",
                                                            Colour::Red.paint("Timeout"),
                                                            Colour::Yellow.paint("retrying"),
                                                            chain_color.paint(adjusted_deadline.to_string()),
                                                            Colour::Yellow.paint(format!("({}/{})", attempt + 1, attempts)),
                                                        );
                                                    } else if is_block_requeued(chain_index, height) {
                                                        // if the block is requeued, stop trying to resubmit the deadlines, we'll come back to them once higher priority blocks are completed
                                                        failure_message = Some(String::from("Block requeued, will retry when higher priority blocks are finished."));
                                                        break;
                                                    }
                                                } else {
                                                    if failure_message.is_none() {
                                                        let plural = match attempt {
                                                            1 => "",
                                                            _ => "s"
                                                        };
                                                        failure_message = Some(format!("No response from pool after {} attempt{}!", attempt, plural));
                                                    }
                                                    break;
                                                }
                                            }
                                            std::thread::sleep(std::time::Duration::from_secs(3));
                                            continue;
                                        }
                                    },
                                    _ => {
                                        info!("Submission failure [timeout] - DL={}", adjusted_deadline);

                                        // check if this is a submission for the actual current chain we're mining
                                        let actual_current_chain_index = get_current_chain_index();
                                        let actual_current_chain_height = get_latest_chain_info(actual_current_chain_index).0;

                                        attempt += 1;
                                        
                                        let best_deadline = get_best_deadline(height, account_id);
                                        if best_deadline < adjusted_deadline { // throw a fake confirm back to the miner since we have a better deadline than this one
                                            let resp = SubmitNonceResponse {
                                                result: String::from("success"),
                                                deadline: Some(adjusted_deadline),
                                                reason: None
                                            };
                                            return resp.to_json();
                                        } else {
                                            if attempt < attempts {
                                                if actual_current_chain_index == chain_index && actual_current_chain_height == block_height {
                                                    println!("          {}, {}:               {} {}",
                                                        Colour::Red.paint("Timeout"),
                                                        Colour::Yellow.paint("retrying"),
                                                        chain_color.paint(adjusted_deadline.to_string()),
                                                        Colour::Yellow.paint(format!("({}/{})", attempt + 1, attempts)),
                                                    );
                                                } else if is_block_requeued(chain_index, height) {
                                                    // if the block is requeued, stop trying to resubmit the deadlines, we'll come back to them once higher priority blocks are completed
                                                    failure_message = Some(String::from("Block requeued, will retry when higher priority blocks are finished."));
                                                    break;
                                                }
                                            } else {
                                                if failure_message.is_none() {
                                                    let plural = match attempt {
                                                        1 => "",
                                                        _ => "s"
                                                    };
                                                    failure_message = Some(format!("No response from pool after {} attempt{}!", attempt, plural));
                                                }
                                                break;
                                            }
                                        }
                                        std::thread::sleep(std::time::Duration::from_secs(3));
                                        continue;
                                    }
                                };
                            }
                        }
                        if deadline_accepted {
                            let confirm_time = (Local::now() - start_time).num_milliseconds();
                            info!("DL Confirmed - #{} | ID={} | DL={} (Unadjusted={}) | {}ms", block_height, account_id, adjusted_deadline, unadjusted_deadline, confirm_time);
                            // print nonce confirmation
                            super::print_nonce_accepted(chain_index, height, adjusted_deadline, confirm_time);
                            // confirm deadline to miner
                            let resp = SubmitNonceResponse {
                                result: String::from("success"),
                                deadline: Some(adjusted_deadline),
                                reason: None,
                            };
                            return resp.to_json();
                        } else { // deadline not accepted
                            let reject_time = (Local::now() - start_time).num_milliseconds();
                            info!("DL Rejected - #{} | ID={} | DL={} (Unadjusted={}) | {}ms - Response: {:?}", block_height, account_id, adjusted_deadline, unadjusted_deadline, reject_time, failure_message);
                            if failure_message.is_some() {
                                // print confirmation failure
                                super::print_nonce_rejected(chain_index, height, adjusted_deadline, reject_time, attempt, attempts, failure_message.clone(), false);
                                return failure_message.unwrap();
                            } else {    // print confirmation failure
                                super::print_nonce_rejected(chain_index, height, adjusted_deadline, reject_time, attempt, attempts, failure_message.clone(), true);
                                let resp = SubmitNonceResponse {
                                    result: String::from("failure"),
                                    deadline: Some(adjusted_deadline),
                                    reason: Some(format!("Upstream failed to respond after {}/{} attempts to submit.", attempt, attempts)),
                                };
                                return resp.to_json();
                            }
                        }
                    });
                }
                debug!("Confirmed DL => Miner @ {} - #{} | DL={} (Unadjusted={})", remote_addr, block_height, adjusted_deadline, unadjusted_deadline);
                // confirm deadline to miner
                let resp = SubmitNonceResponse {
                    result: String::from("success"),
                    deadline: Some(adjusted_deadline),
                    reason: None,
                };
                return resp.to_json();
            }
            _ => {
                if !current_chain.is_hdpool.unwrap_or_default()
                    && !current_chain.is_hdpool_eco.unwrap_or_default()
                    && !current_chain.is_hpool.unwrap_or_default()
                    && !current_chain.is_pool.unwrap_or_default()
                    && !current_chain.is_bhd.unwrap_or_default() {
                    let resp = SubmitNonceResponse{
                        result: String::from("failure"),
                        deadline: None,
                        reason: Some(String::from("Indirectly solo mining burst via Archon is not implemented at this time, please configure your miner as if pool mining, and set your passphrase in the Archon config for the chain you wish to solo mine.")),
                    };
                    return resp.to_json();
                } else {
                    let resp = SubmitNonceResponse {
                        result: String::from("failure"),
                        deadline: None,
                        reason: Some(String::from(
                            "Your miner must provide a deadline, either adjusted or unadjusted.",
                        )),
                    };
                    return resp.to_json();
                }
            }
        };
    }
    warn!("ProcessNonceSubmission({}, {}, {}, {:?}, {}, {}, {}) - Couldn't match nonce submission to a valid chain.", block_height, account_id, nonce, deadline, user_agent_header, adjusted, remote_addr);
    let resp = SubmitNonceResponse {
        result: String::from("failure"),
        deadline: None,
        reason: Some(String::from(
            "Could not match nonce submission to a valid chain.",
        )),
    };
    return resp.to_json();
}
