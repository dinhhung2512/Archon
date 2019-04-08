use chrono::{DateTime, Local};
use colored::Colorize;
use reqwest;
use futures::sync::mpsc;
use futures::{Future, Stream};
use tokio_tungstenite::connect_async;
use url::Url;
use tungstenite::Message;
use std::collections::HashMap;
use std::sync::mpsc as std_mpsc;
use std::thread;

use crate::config::PocChain;
use crate::upstream::MiningInfo;
use crate::web::{SubmitNonceResponse, SubmitNonceErrorResponse, SubmitNonceInfo};

#[derive(Debug, Clone)]
struct MiningInfoPollingResult {
    mining_info: MiningInfo,
    chain: PocChain,
}

fn create_chain_nonce_submission_client(chain_index: u8) {
    // get current chain
    let chain = super::get_chain_from_index(chain_index).unwrap();
    use reqwest::header;
    let mut default_headers = header::HeaderMap::new();
    // if this chain is for hpool, add a default header to this client with the user's account key
    if chain.is_hpool.unwrap_or_default() {
        // get account key from config
        let account_key_header = chain.account_key.unwrap_or(String::from(""));
        // attempt to parse account key into a HeaderValue
        let account_key_header_value: header::HeaderValue = match account_key_header.parse() {
            Ok(val) => {
                info!("{} (HPOOL) - Set default headers to include AccountKey=>{}", &*chain.name, account_key_header);
                val
            },
            Err(why) => {
                warn!("Couldn't parse account key into a HeaderValue for chain #{}: {:?}", chain_index, why);
                "Invalid Header Data".parse().unwrap()
            },
        };
        default_headers.insert("X-Account", account_key_header_value);
    }
    let mut chain_nonce_submission_clients = crate::CHAIN_NONCE_SUBMISSION_CLIENTS.lock().unwrap();
    chain_nonce_submission_clients.insert(
        chain_index, 
        reqwest::Client::builder()
            .default_headers(default_headers)
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap()
        );
    drop(chain_nonce_submission_clients);
}

pub fn thread_arbitrate() {
    let (mining_info_sender, mining_info_receiver) = std_mpsc::channel();
    // start polling for mining info for each chain
    for inner in &crate::CONF.poc_chains {
        for chain in inner {
            if chain.enabled.unwrap_or(true) {
                let new_sender = mining_info_sender.clone();
                let index = super::get_chain_index(&*chain.url, &*chain.name);
                create_chain_nonce_submission_client(index);
                thread::spawn(move || {
                    thread_get_mining_info(
                        reqwest::Client::builder().timeout(std::time::Duration::from_secs(5)).build().unwrap(),
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
    receiver: crossbeam::channel::Receiver<SubmitNonceInfo>,
    tx: mpsc::UnboundedSender<Message>,
) {
    let miner_mark = "20190327";
    let account_key = chain.account_key.clone().unwrap_or(String::from(""));
    loop {
        match receiver.try_recv() {
            Ok(submit_nonce_info) => {
                let capacity_gb = crate::get_total_plots_size_in_tebibytes() * 1024f64;
                let unix_timestamp = Local::now().timestamp();
                let message = format!(r#"{{"cmd":"poolmgr.submit_nonce","para":{{"account_key":"{}","capacity":{},"miner_mark":"{}","miner_name":"{} v{}","submit":[{},{},{},{},{}]}}}}"#, account_key, capacity_gb, miner_mark, super::uppercase_first(super::APP_NAME), super::VERSION, submit_nonce_info.account_id, submit_nonce_info.block_height.unwrap_or_default(), submit_nonce_info.nonce, submit_nonce_info.deadline, unix_timestamp);
                //if tx.unbounded_send(Message::Text(message.clone().into())).is_ok() {}
                debug!("HDPool Websocket: SubmitNonce message: {}", message);
                match tx.unbounded_send(Message::Text(message.clone().into())) {
                    Ok(_) => {},
                    Err(why) => {
                        warn!("HDPool Websocket SubmitNonce failure: {:?}.", why);
                        break;
                    },
                }
            },
            Err(_) => {}
        };
    }
}

fn thread_hdpool_websocket(
    chain: PocChain,
    mining_info_sender: std_mpsc::Sender<String>,
    nonce_submission_receiver: crossbeam::channel::Receiver<SubmitNonceInfo>,
) {
    loop {
        let (tx, rx) = mpsc::unbounded();
        let rx = rx.map_err(|_| panic!());
        let txc = tx.clone();
        let txs = tx.clone();

        // set vars
        let addr = Url::parse("wss://hdminer.hdpool.com").unwrap();
        let miner_mark = "20190327";
        let account_key = chain.account_key.clone().unwrap_or(String::from(""));

        // Spawn thread for the heartbeat loop to run in.
        let hb_child_thread = thread::spawn(move || {
            loop {
                let capacity_gb = crate::get_total_plots_size_in_tebibytes() * 1024f64;
                let data = format!(r#"{{"cmd":"poolmgr.heartbeat","para":{{"account_key":"{}","miner_name":"{} v{}","miner_mark":"{}","capacity":{}}}}}"#,
                    account_key,
                    crate::uppercase_first(crate::APP_NAME),
                    crate::VERSION,
                    miner_mark,
                    capacity_gb);
                match txc.unbounded_send(Message::Text(data.into())) {
                    Ok(_) => {},
                    Err(why) => {
                        warn!("HDPool Websocket Heartbeat failure: {:?}.", why);
                        break;
                    },
                };
                thread::sleep(std::time::Duration::from_secs(5));
            }
        });

        // spawn thread to handle nonce submissions
        let chain_copy = chain.clone();
        let nonce_submission_receiver_clone = nonce_submission_receiver.clone();
        let nonce_child_thread = thread::spawn(move || {
            thread_handle_hdpool_nonce_submissions(chain_copy, nonce_submission_receiver_clone, txs);
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
                                if message_str.to_lowercase().starts_with(r#"{"cmd":"mining_info""#) || message_str.to_lowercase().starts_with(r#"{"cmd":"poolmgr.mining_info"#) {
                                    let parsed_message_str: serde_json::Value = serde_json::from_str(&message_str).unwrap();
                                    let mining_info = parsed_message_str["para"].to_string().clone();
                                    trace!("HDPool WebSocket: NEW BHD BLOCK: {}", mining_info);
                                    match mining_info_sender.send(mining_info) {
                                        Ok(_) => {}
                                        Err(_) => {}
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

            ws_writer.map(|_| ()).map_err(|e| { debug!("HDPool WebSocket Failure: {:?}", e); () })
                .select(ws_reader.map(|_| ()).map_err(|e| { debug!("HDPool WebSocket Failure: {:?}", e); () }))
                .then(|_| Ok(()))

        }).map_err(|e| {
            use std::io;

            error!("Error during the websocket handshake occured: {}", e);
            io::Error::new(io::ErrorKind::Other, e);
        });

        tokio::runtime::run(client.map_err(|e| error!("{:?}", e)));

        let _ = nonce_child_thread.join();
        let _ = hb_child_thread.join();

        warn!("HDPool Websocket: Attempting to reconnect in 10 seconds.");
        thread::sleep(std::time::Duration::from_secs(10));
    }
}

fn thread_get_mining_info(
    client: reqwest::Client,
    chain: PocChain,
    sender: std_mpsc::Sender<MiningInfoPollingResult>,
) {
    let mut url = String::from(&*chain.url);
    url.push_str("/burst?requestType=getMiningInfo");
    let mut last_block_height = 0 as u32;
    let mut request_failure = false;
    let mut last_request_success: DateTime<Local> = Local::now();
    let mut last_outage_reminder_sent: DateTime<Local> = Local::now();

    // setup mpsc channel to receive signal when new mining info is received from HDPool websocket
    let (hdpool_mining_info_sender, hdpool_mining_info_receiver) = std_mpsc::channel();
    // setup mpsc channel to receive signal when nonce submissions are received from connected miners
    //let (hdpool_nonce_submission_sender, hdpool_nonce_submission_receiver) = std_mpsc::channel();

    // Mpmc channels for now, for the ease of recreating the receiver if a thread dies. Will find a better solution later.
    let (hdpool_nonce_submission_sender, hdpool_nonce_submission_receiver) = crossbeam::channel::unbounded();
    
    /* BEGIN ASYNC WEBSOCK STUFF. */
    if chain.is_hdpool.unwrap_or_default() && chain.account_key.is_some() {
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
        let is_hdpool = chain.is_hdpool.unwrap_or_default() && chain.account_key.is_some();
        let mining_info_response = match is_hdpool {
            true => {
                match hdpool_mining_info_receiver.try_recv() {
                    Ok(mining_info) => mining_info,
                    Err(_) => String::from(""),
                }
            },
            false => {
                let mut url = String::from(chain.clone().url);
                url.push_str("/burst?requestType=getMiningInfo");
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
                                String::from("")
                            }
                        }
                    },
                    Err(why) => {
                        warn!("GetMiningInfo({}): Request failed: {}", &*chain.name, why);
                        String::from("")
                    }
                }
            }
        };
        if mining_info_response.len() > 0 {
            match MiningInfo::from_json(mining_info_response.as_str()) {
                (true, _mining_info) => {
                    if request_failure {
                        request_failure = false;
                        let outage_duration = Local::now() - last_request_success;
                        let outage_duration_str = super::format_timespan(
                            outage_duration.num_seconds() as u64,
                        );
                        println!("  {} {} {}",
                            super::get_time().white(),
                            format!("{}", &*chain.name).color(&*chain.color),
                            format!("Outage over, total time unavailable: {}.", outage_duration_str).green()
                        );
                        info!("{} - Outage over, total time unavailable: {}.", &*chain.name, outage_duration_str);
                    }
                    last_request_success = Local::now();
                    if (chain.allow_lower_block_heights.unwrap_or_default()
                        && _mining_info.height != last_block_height)
                        || _mining_info.height > last_block_height
                    {
                        last_block_height = _mining_info.height;
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
                }
                (false, _mining_info) => {
                    if !request_failure {
                        request_failure = true;
                        last_outage_reminder_sent = Local::now();
                        println!("  {} {} {}",
                            super::get_time().white(),
                            format!("{}", &*chain.name).color(&*chain.color),
                            "Could not retrieve mining info!".red()
                        );
                        info!("{} ({}) - Error getting mining info! Outage started: {}", &*chain.name, &*chain.url, mining_info_response);
                    } else {
                        let outage_duration = Local::now() - last_request_success;
                        let last_reminder = Local::now() - last_outage_reminder_sent;
                        if last_reminder.num_seconds()
                            >= crate::CONF.outage_status_update_interval.unwrap_or(300u16) as i64
                        {
                            last_outage_reminder_sent = Local::now();
                            let outage_duration_str =
                                super::format_timespan(outage_duration.num_seconds() as u64);
                            println!("  {} {} {}",
                                super::get_time().white(),
                                format!("{} - Last: {}", &*chain.name, last_block_height).color(&*chain.color),
                                format!("Outage continues, time unavailable so far: {}.", outage_duration_str).red()
                            );
                            info!("{} - Last: {} - Outage continues, time unavailable so far: {}", &*chain.name, last_block_height, outage_duration_str);
                        }
                    }
                    drop(_mining_info);
                }
            };
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

fn process_new_block(mining_info_polling_result: &MiningInfoPollingResult) {
    let index = super::get_chain_index(
        &*mining_info_polling_result.chain.url,
        &*mining_info_polling_result.chain.name,
    );
    let current_chain_index = get_current_chain_index();
    let current_chain = super::get_chain_from_index(current_chain_index).unwrap();
    if crate::CONF.priority_mode.unwrap_or(true) {
        if mining_info_polling_result.chain.priority <= current_chain.priority {
            // higher priority is LOWER in actual value
            if index != current_chain_index {
                if !has_grace_period_elapsed() {
                    if crate::CONF.interrupt_lower_priority_blocks.unwrap_or(true) {
                        requeue_current_block(
                            current_chain.requeue_interrupted_blocks.unwrap_or(true),
                            index,
                            Some(mining_info_polling_result.clone())
                        );
                        start_mining_chain(index);
                        return;
                    } // else queue new block
                } else {
                    // if grace period has elapsed
                    start_mining_chain(index);
                    return;
                }
            } else {
                match any_blocks_queued() {
                    (true, 0...1, _) => {
                        // queue new block
                    }
                    (_, _, _) => {
                        start_mining_chain(index);
                    }
                }
                return;
            }
        } else if has_grace_period_elapsed() {
            start_mining_chain(index);
            return;
        } // else queue new block
    } else {
        // running in FIFO mode
        if index != current_chain_index {
            if has_grace_period_elapsed() {
                match any_blocks_queued() {
                    (true, _, _) => {
                        start_mining_chain(index);
                        return;
                    }
                    (false, _, _) => {}
                }; // else queue new block
            } // else queue new block
        } else {
            match any_blocks_queued() {
                (false, _, _) => {
                    start_mining_chain(index);
                    return;
                }
                (true, _, _) => {}
            };
        } // else queue new block
    }
    // if the code makes it to this point, the new block will be "queued".
    info!("QUEUE BLOCK - {} #{}", &*mining_info_polling_result.chain.name, mining_info_polling_result.mining_info.height);
    /*super::print_block_queued(
        &*mining_info_polling_result.chain.name,
        &*mining_info_polling_result.chain.color,
        mining_info_polling_result.mining_info.height,
    );*/
}

fn requeue_current_block(do_requeue: bool, interrupted_by_index: u8, mining_info_polling_result: Option<MiningInfoPollingResult>) {
    let current_chain_index = get_current_chain_index();
    let current_chain = super::get_chain_from_index(current_chain_index).unwrap();
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
                (Some(interrupted_by_chain), Some(interrupted_by_mining_info)) => {
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
    if do_requeue {
        info!("INTERRUPT & REQUEUE BLOCK - {} #{} => {} #{}", &*current_chain.name, requeued_height, &*interrupted_by_name, interrupted_by_height);
        // set the queue status for this chain back by 1, thereby "requeuing" it
        let mut chain_queue_status_map = crate::CHAIN_QUEUE_STATUS.lock().unwrap();
        chain_queue_status_map.insert(current_chain_index, (requeued_height - 1, requeued_time));
        trace!("SET START - Chain #{} Block #{} ==> #{}", current_chain_index, requeued_height, requeued_height - 1);
        let mut block_start_printed_map = crate::BLOCK_START_PRINTED.lock().unwrap();
        block_start_printed_map.insert(current_chain_index, requeued_height - 1);
    } else {
        info!("INTERRUPT BLOCK - {} #{} => {} #{}", &*current_chain.name, requeued_height, &*interrupted_by_name, interrupted_by_height);
    }
    // print
    super::print_block_requeued_or_interrupted(
        &*current_chain.name,
        &*current_chain.color,
        requeued_height,
        do_requeue,
    );
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

pub fn get_time_since_block_start(height: u32) -> Option<u64> {
    let current_chain_index = get_chain_index_from_height(height);
    let chain_queue_status_map = crate::CHAIN_QUEUE_STATUS.lock().unwrap();
    if chain_queue_status_map.len() > 0 {
        match chain_queue_status_map.get(&current_chain_index) {
            Some((_, start_time)) => {
                return Some((Local::now() - *start_time).num_seconds() as u64);
            }
            None => {}
        };
    }
    return None;
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

fn get_current_chain_mining_info(index: u8) -> Option<(MiningInfo, DateTime<Local>)> {
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

pub fn get_chain_index_from_height(height: u32) -> u8 {
    for inner in &crate::CONF.poc_chains {
        for chain in inner {
            if chain.enabled.unwrap_or(true) {
                let index = super::get_chain_index(&*chain.url, &*chain.name);
                let (current_height, _) = get_latest_chain_info(index);
                if current_height == height || current_height == height - 1 {
                    return index;
                }
            }
        }
    }
    return get_current_chain_index();
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
        let current_chain = super::get_chain_from_index(current_chain_index).unwrap();
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
                if crate::CONF.priority_mode.unwrap_or(true) {
                    match priority {
                        1 => {
                            // 1 = higher priority than current block
                            if has_grace_period_elapsed() {
                                start_mining_chain(index);
                            } else if crate::CONF.interrupt_lower_priority_blocks.unwrap_or(true) {
                                let current_chain_index = get_current_chain_index();
                                let current_chain =
                                    super::get_chain_from_index(current_chain_index).unwrap();
                                requeue_current_block(
                                    current_chain.requeue_interrupted_blocks.unwrap_or(true),
                                    index,
                                    None
                                );
                                start_mining_chain(index);
                            } // else do nothing
                        }
                        0 => {
                            // 0 = same priority as current block
                            start_mining_chain(index);
                        }
                        _ => {
                            // -1 = lower priority than current block
                            if has_grace_period_elapsed() {
                                start_mining_chain(index);
                            } // else do nothing
                        }
                    };
                } else {
                    // FIFO mode
                    if has_grace_period_elapsed() {
                        start_mining_chain(index);
                    } // else do nothing
                }
            }
            (false, _, _) => {} // nothing queued, nothing to do...
        };

        thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn start_mining_chain(index: u8) {
    // get chain
    match super::get_chain_from_index(index) {
        Some(chain) => {
            // get access to chain mining infos
            match get_current_chain_mining_info(index) {
                Some((mining_info, _)) => {
                    if mining_info.base_target > 0 {
                        // get currently mining block height before we change it
                        let current_block_height = match super::get_current_mining_info() {
                            Some(mi) => mi.height,
                            _ => 0,
                        };
                        let last_block_time = get_time_since_block_start(current_block_height);
                        // print block info
                        super::print_block_started(
                            index,
                            mining_info.height,
                            mining_info.base_target,
                            String::from(&*mining_info.generation_signature),
                            mining_info.target_deadline,
                            last_block_time,
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
        None => {}
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
                    trace!("GetBestDL(Height={}, ID={}) = {}", block_height, account_id, deadline);
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

fn get_target_deadline(
    account_id: u64,
    base_target: u32,
    chain_index: u8,
    chain_global_tdl: Option<u64>,
    chain_num_id_to_tdls: Option<HashMap<u64, u64>>,
) -> (u64, bool) {
    // get max deadline from upstream if present
    let upstream_target_deadline;
    let mut target_deadline = match get_current_chain_mining_info(chain_index) {
        Some((mining_info, _)) => mining_info.target_deadline,
        _ => u64::max_value(),
    };
    if target_deadline == 0 {
        target_deadline = u64::max_value();
    }
    upstream_target_deadline = target_deadline;

    // get chain's global target deadline, if set
    target_deadline = match chain_global_tdl {
        Some(global_tdl) => {
            if global_tdl < target_deadline {
                global_tdl
            } else {
                target_deadline
            }
        }
        _ => target_deadline,
    };

    // calculate the dynamic deadline
    target_deadline = match super::get_dynamic_deadline_for_block(base_target) {
        (true, _, _, dynamic_target_deadline) => {
            if dynamic_target_deadline < target_deadline {
                dynamic_target_deadline
            } else {
                target_deadline
            }
        }
        _ => target_deadline,
    };

    // check if there is a target deadline specified for this account id in this chain's config, if so, override all other deadlines with it
    let mut id_override = false;
    match chain_num_id_to_tdls {
        Some(num_id_to_tdls_map) => {
            for id_to_tdl in num_id_to_tdls_map {
                if id_to_tdl.0 == account_id && id_to_tdl.1 < upstream_target_deadline {
                    target_deadline = id_to_tdl.1;
                    id_override = true;
                }
            }
        }
        _ => {}
    };
    debug!("GetTDL(ID={}, BTgt={}, chInd={}, ChTDL{:?}) = {} (Override: {})", account_id, base_target, chain_index, chain_global_tdl, target_deadline, id_override);
    return (target_deadline, id_override);
}

fn forward_nonce_submission(chain_index: u8, url: &str, user_agent_header: &str) -> Option<String> {
    let chain_nonce_submission_clients = crate::CHAIN_NONCE_SUBMISSION_CLIENTS.lock().unwrap();
    match chain_nonce_submission_clients.get(&chain_index) {
        Some(client) => {
            match client
                .post(url)
                .header(
                    "User-Agent",
                    format!(
                        "{} via {} v{}",
                        user_agent_header,
                        super::uppercase_first(super::APP_NAME),
                        super::VERSION
                    ),
                )
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
    user_agent_header: &str,
    adjusted: bool,
    remote_addr: String,
) -> String {
    debug!("Received DL: Height={}, ID={}, Nonce={}, DL={:?}, Software={}, Adjusted={}, Address={}", block_height, account_id, nonce, deadline, user_agent_header, adjusted, remote_addr);
    // validate data
    // get mining info for chain
    let chain_index = get_chain_index_from_height(block_height); // defaults to the chain being currently mined if it cannot find a height match
    let current_chain = super::get_chain_from_index(chain_index).unwrap();
    let base_target = match get_current_chain_mining_info(chain_index) {
        Some((mining_info, _)) => mining_info.base_target,
        _ => 0,
    };
    if base_target > 0 {
        let mut height = block_height;
        if height == 0 {
            height = match get_latest_chain_info(chain_index) {
                (height, _) => height,
            };
        }
        let start_time = Local::now();
        let mut send_deadline = true;
        let mut print_deadline = true;
        let mut _deadline_sent = false;
        let mut deadline_accepted = false;
        let mut deadline_rejected = false;
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
                let (target_deadline, id_override) = get_target_deadline(
                    account_id,
                    base_target,
                    chain_index,
                    current_chain.target_deadline,
                    current_chain.numeric_id_to_target_deadline,
                );
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
                let mut failure_message = String::from("");
                if print_deadline {
                    super::print_nonce_submission(
                        chain_index,
                        height,
                        account_id,
                        adjusted_deadline,
                        user_agent_header,
                        target_deadline,
                        id_override,
                        remote_addr,
                    );
                }
                if !deadline_over_best {
                    update_best_deadline(height, account_id, adjusted_deadline);
                }
                let mut passphrase_str = String::from("");
                // if solo mining burst, look for a passphrase from config for this account id
                if !current_chain.is_hpool.unwrap_or_default()
                    && !current_chain.is_hdpool.unwrap_or_default()
                    && !current_chain.is_pool.unwrap_or_default()
                    && !current_chain.is_bhd.unwrap_or_default()
                {
                    let mut passphrase_set = false;
                    match current_chain.numeric_id_to_passphrase {
                        Some(map) => {
                            for id_and_passphrase in map {
                                if id_and_passphrase.0 == account_id {
                                    passphrase_str.push_str(
                                        format!("&secretPhrase={}", id_and_passphrase.1).as_str(),
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
                            reason: Some(format!("No passphrase for account ID [{}] was specified in Archon configuration for solo mining burst.", account_id)),
                        };
                        return resp.to_json();
                    }
                }
                if send_deadline {
                    if current_chain.is_hdpool.unwrap_or_default() && current_chain.account_key.is_some() {
                        // get sender
                        let sender = crate::HDPOOL_SUBMIT_NONCE_SENDER.lock().unwrap();
                        if sender.is_some() {
                            let sender = sender.clone().unwrap();
                            let mut attempts = 0;
                            while attempts < 5 && !deadline_accepted && !deadline_rejected {
                                info!("DL Send - #{} | ID={} | DL={} (Unadjusted={}) - Attempt #{}/5", block_height, account_id, adjusted_deadline, unadjusted_deadline, attempts + 1);
                                if sender.send(SubmitNonceInfo { 
                                    account_id: account_id,
                                    block_height: Some(height),
                                    nonce: nonce,
                                    deadline: unadjusted_deadline,
                                    secret_phrase: None,
                                }).is_ok() {
                                    deadline_accepted = true;
                                }
                                attempts += 1;
                            }
                            if !deadline_accepted {
                                deadline_rejected = true;
                            }
                        }
                    } else {
                        let mut url = String::from(&*current_chain.url);
                        // check if NOT solo mining burst
                        if current_chain.is_hdpool.unwrap_or_default()
                            || current_chain.is_hpool.unwrap_or_default()
                            || current_chain.is_bhd.unwrap_or_default()
                            || current_chain.is_pool.unwrap_or_default() {
                            url.push_str(format!("/burst?requestType=submitNonce&blockheight={}&accountId={}&nonce={}&deadline={}",
                            height, account_id, nonce, unadjusted_deadline).as_str());
                        } else { // solo mining burst
                            url.push_str(format!("/burst?requestType=submitNonce&blockheight={}&accountId={}&nonce={}{}",
                            height, account_id, nonce, passphrase_str).as_str());
                        }
                        //let client = reqwest::Client::new();
                        let mut attempts = 0;
                        while attempts < 5 && !deadline_accepted && !deadline_rejected {
                            _deadline_sent = true;
                            info!("DL Send - #{} | ID={} | DL={} (Unadjusted={}) - Attempt #{}/5", block_height, account_id, adjusted_deadline, unadjusted_deadline, attempts + 1);
                            match forward_nonce_submission(chain_index, url.as_str(), user_agent_header)
                            {
                                Some(text) => {
                                    debug!("DL Submit Response: {}", text);
                                    if text.contains("success")
                                        && text.contains(format!("{}", adjusted_deadline).as_str())
                                    {
                                        deadline_accepted = true;
                                    } else {
                                        deadline_rejected = true;
                                        failure_message.push_str(text.as_str());
                                    }
                                    break;
                                }
                                _ => {}
                            };
                            attempts += 1;
                            thread::sleep(std::time::Duration::from_secs(1));
                        }
                    }
                }
                if deadline_accepted {
                    let confirm_time = (Local::now() - start_time).num_milliseconds();
                    info!("DL Confirmed - #{} | ID={} | DL={} (Unadjusted={}) | {}ms", block_height, account_id, adjusted_deadline, unadjusted_deadline, confirm_time);
                    // print nonce confirmation
                    super::print_nonce_accepted(
                        chain_index,
                        height,
                        adjusted_deadline,
                        confirm_time,
                    );
                    // confirm deadline to miner
                    let resp = SubmitNonceResponse {
                        result: String::from("success"),
                        deadline: Some(adjusted_deadline),
                        reason: None,
                    };
                    return resp.to_json();
                } else if deadline_rejected {
                    let reject_time = (Local::now() - start_time).num_milliseconds();
                    info!("DL Rejected - #{} | ID={} | DL={} (Unadjusted={}) | {}ms - Response: {}", block_height, account_id, adjusted_deadline, unadjusted_deadline, reject_time, failure_message);
                    // print confirmation failure
                    super::print_nonce_rejected(chain_index, height, adjusted_deadline, reject_time);
                    let (ds_success, response) = SubmitNonceResponse::from_json(failure_message.as_str());
                    if ds_success {
                        return response.to_json();
                    } else {
                        let (ds_error_success, _) = SubmitNonceErrorResponse::from_json(failure_message.as_str());
                        if ds_error_success {
                            return failure_message;
                        } else {
                            let resp = SubmitNonceResponse {
                                result: String::from("failure"),
                                deadline: None,
                                reason: Some(format!(
                                    "Unknown - Upstream returned: {}",
                                    failure_message
                                )),
                            };
                            return resp.to_json();
                        }
                    }
                } else {
                    debug!("FAKE Confirm - #{} | DL={} (Unadjusted={})", block_height, adjusted_deadline, unadjusted_deadline);
                    // confirm deadline to miner
                    let resp = SubmitNonceResponse {
                        result: String::from("success"),
                        deadline: Some(adjusted_deadline),
                        reason: None,
                    };
                    return resp.to_json();
                }
            }
            _ => {
                if !current_chain.is_hdpool.unwrap_or_default()
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
