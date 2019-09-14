use crate::arbiter;
use actix_web::http::{header, Method, StatusCode};
use actix_web::{server, App, Error, HttpRequest, HttpResponse};
use futures::future::{result, FutureResult};
use std::string::*;

use crate::config::{Config, PocChain};

fn parse_u32_from_str(txt: &str) -> u32 {
    match txt.parse::<u32>() {
        Ok(parsed) => parsed,
        _ => 0,
    }
}
fn parse_u64_from_str(txt: &str) -> u64 {
    match txt.parse::<u64>() {
        Ok(parsed) => parsed,
        _ => 0,
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubmitNonceResponse {
    pub result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl SubmitNonceResponse {
    pub fn empty() -> SubmitNonceResponse {
        SubmitNonceResponse {
            result: String::from(""),
            deadline: None,
            reason: None,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn from_json(json: &str) -> (bool, SubmitNonceResponse) {
        match serde_json::from_str(json) {
            Ok(snr) => {
                return (true, snr);
            }
            Err(_) => {
                return (false, SubmitNonceResponse::empty());
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubmitNonceErrorResponse {
    pub error_code: String,
    pub error_description: String,
}

impl SubmitNonceErrorResponse {
    pub fn empty() -> SubmitNonceErrorResponse {
        return SubmitNonceErrorResponse {
            error_code: "".to_string(),
            error_description: "".to_string(),
        };
    }

    pub fn from_json(json: &str) -> (bool, SubmitNonceErrorResponse) {
        match serde_json::from_str(json) {
            Ok(sner) => {
                return (true, sner);
            },
            _ => {
                return (false, SubmitNonceErrorResponse::empty());
            }
        }
    }
}

fn try_get_query_string_value(req: &HttpRequest, name: &str) -> (bool, String) {
    match req.query().get(name) {
        Some(val) => {
            return (true, val.clone());
        }
        _ => {
            return (false, "".to_string());
        }
    }
}

#[derive(Debug, Clone)]
pub struct MiningHeaderData {
    pub capacity: f64,
    pub miner: String,
    pub miner_name: String,
    pub plot_file: String,
}

pub struct SubmitNonceInfo {
    pub block_height: Option<u32>,
    pub account_id: u64,
    pub nonce: u64,
    pub deadline: u64,
    #[allow(dead_code)]
    pub secret_phrase: Option<String>,
}

impl SubmitNonceInfo {
    pub fn from(height: Option<u32>, id: u64, nonce: u64, deadline: u64, passphrase: Option<String>) -> SubmitNonceInfo {
        return SubmitNonceInfo {
            block_height: height,
            account_id: id,
            nonce: nonce,
            deadline: deadline,
            secret_phrase: passphrase
        };
    }
}

fn try_get_submit_nonce_data(req: &HttpRequest) -> Option<SubmitNonceInfo> {
    let block_height = match try_get_query_string_value(req, "blockheight") {
        (true, val) => parse_u32_from_str(val.as_str()),
        _ => 0u32,
    };
    let account_id = match try_get_query_string_value(req, "accountId") {
        (true, val) => parse_u64_from_str(val.as_str()),
        _ => 0u64,
    };
    let nonce = match try_get_query_string_value(req, "nonce") {
        (true, val) => parse_u64_from_str(val.as_str()),
        _ => 0u64,
    };
    let deadline = match try_get_query_string_value(req, "deadline") {
        (true, val) => parse_u64_from_str(val.as_str()),
        _ => 0u64,
    };
    let secret_phrase = match try_get_query_string_value(req, "secretPhrase") {
        (true, val) => Some(val),
        _ => None
    };
    if account_id > 0 && nonce > 0 && deadline > 0 {
        return Some(SubmitNonceInfo::from(Some(block_height), account_id, nonce, deadline, secret_phrase));
    }
    return None;
}

fn create_response(status_code: StatusCode, body: String) -> FutureResult<HttpResponse, Error> {
    result(Ok(HttpResponse::build(status_code)
        .header(header::USER_AGENT, get_user_agent_str())
        .content_type("application/json")
        .body(body)))
}

fn get_x_deadline_value(req: &HttpRequest) -> (bool, u64) {
    match req.headers().get("x-deadline") {
        Some(container) => {
            match container.to_str() {
                Ok(value) => (true, parse_u64_from_str(value)),
                Err(_) => (false, u64::max_value())
            }
        },
        _ => (false, u64::max_value())
    }
}

fn get_miner_software(req: &HttpRequest) -> String {
    match req.headers().get(header::USER_AGENT) {
        Some(container) => {
            match container.to_str() {
                Ok(value) => value.clone().to_string(),
                _ => get_header_value(req, "x-miner", "Unknown")
            }
        },
        _ => get_header_value(req, "x-miner", "Unknown")
    }
}

fn get_all_mining_headers(req: &HttpRequest) -> MiningHeaderData {
    MiningHeaderData { 
        miner: get_header_value(req, "x-miner", ""),
        miner_name: get_header_value(req, "x-minername", ""),
        capacity: str::parse::<f64>(get_header_value(req, "x-capacity", "").as_str()).unwrap_or(0f64),
        plot_file: get_header_value(req, "x-plotfile", "")
    }
}

fn get_header_value(req: &HttpRequest, name: &str, default_value: &str) -> String {
    match req.headers().get(name) {
        Some(container) => {
            match container.to_str() {
                Ok(value) => value.clone().to_string(),
                _ => default_value.to_string(),
            }
        },
        _ => default_value.to_string(),
    }
}

fn handle_get_mining_info(req: &HttpRequest) -> FutureResult<HttpResponse, Error> {
    trace!("GetMiningInfo Request from [{}] (Method: {})", req.connection_info().remote().unwrap_or("Unknown"), req.method().to_string());
    let mining_header_data = get_all_mining_headers(req);
    super::update_connected_miners(req.connection_info().remote().unwrap_or(""), mining_header_data);
    create_response(StatusCode::OK, super::get_current_mining_info_json())
}

fn handle_submit_nonce(req: &HttpRequest) -> FutureResult<HttpResponse, Error> {
    trace!("SubmitNonce Request from [{}] (Method: {})", req.connection_info().remote().unwrap_or("Unknown"), req.method().to_string());
    match *req.method() {
        Method::POST => {
            let mining_header_data = get_all_mining_headers(req);
            super::update_connected_miners(req.connection_info().remote().unwrap_or(""), mining_header_data.clone());
            match try_get_submit_nonce_data(req) {
                Some(submit_nonce_data) => {
                    let miner_software = get_miner_software(&req);
                    let (is_adjusted, x_deadline) = get_x_deadline_value(&req);
                    let deadline;
                    if is_adjusted && x_deadline < u64::max_value() {
                        deadline = Some(x_deadline);
                    } else if submit_nonce_data.deadline < u64::max_value() {
                        deadline = Some(submit_nonce_data.deadline);
                    } else {
                        deadline = None;
                    }
                    create_response(
                        StatusCode::OK,
                        arbiter::process_nonce_submission(
                            submit_nonce_data.block_height.unwrap_or(0),
                            submit_nonce_data.account_id,
                            submit_nonce_data.nonce,
                            deadline,
                            miner_software,
                            is_adjusted,
                            req.connection_info().remote().unwrap_or("").to_string(),
                            mining_header_data,
                        )
                    )
                },
                _ => {
                    create_response(StatusCode::OK, r#"{"result":"failure","reason":"Required parameters for nonce submission were not present. Must include ID/Nonce/Deadline. Tip: If you are using Scavenger, make sure you DO NOT HAVE ANY PASSPHRASES defined in your Scavenger configuration!"}"#.to_string())
                }
            }
        },
        _ => {
            create_response(StatusCode::OK, r#"{"result":"failure","reason":"Nonce submission must be done via POST request."}"#.to_string())
        }
    }
}

fn burst_handler(req: &HttpRequest) -> FutureResult<HttpResponse, Error> {
    match try_get_query_string_value(&req, "requestType") {
        (true, request_type) => {
            match request_type.to_lowercase().as_str() {
                "getmininginfo" => handle_get_mining_info(&req),
                "submitnonce" => handle_submit_nonce(&req),
                _ => handle_invalid_request_type()
            }
        },
        (false, _) => {
            create_response(StatusCode::BAD_REQUEST, r#"{"result":"failure","reason":"requestType parameter was not found."#.to_string())
        }
    }
}

fn handle_invalid_request_type() -> FutureResult<HttpResponse, Error> {
    create_response(StatusCode::BAD_REQUEST, r#"{"result":"failure","reason":"requestType parameter was not found."#.to_string())
}

fn handle_api_get_best_deadlines(req: &HttpRequest) -> FutureResult<HttpResponse, Error> {
    trace!("GetBestDeadlines Request from [{}] (Method: {})", req.connection_info().remote().unwrap_or("Unknown"), req.method().to_string());
    match try_get_query_string_value(&req, "height") {
        (true, height_str) => 
            match str::parse::<u32>(height_str.as_str()) {
                Ok(height) => {
                    let best_block_deadlines = arbiter::get_best_deadlines_for_block(height);
                    let json;
                    if best_block_deadlines.is_some() {
                        json = serde_json::to_string(&best_block_deadlines.unwrap()).unwrap_or(r#"{"result":"failure","reason":"Couldn't serialize best deadlines."}"#.to_string());
                    } else {
                        json = r#"{"result":"failure","reason":"There are no records for that block height!"}"#.to_string();
                    }
                    create_response(StatusCode::OK, json)
                },
                Err(_) => create_response(StatusCode::OK, r#"{"result":"failure","reason":"Couldn't parse block height."}"#.to_string())
            },
        (false, _) => {
            let best_deadlines = arbiter::get_best_deadlines();
            let json = serde_json::to_string(&best_deadlines).unwrap_or(r#"{"result":"failure","reason":"Couldn't serialize best deadlines."}"#.to_string());
            create_response(StatusCode::OK, json)
        }
    }
}

fn handle_api_get_config(req: &HttpRequest) -> FutureResult<HttpResponse, Error> {
    trace!("GetConfig Request from [{}] (Method: {})", req.connection_info().remote().unwrap_or("Unknown"), req.method().to_string());
    // don't allow access to other machines for this request - used for modifying the config from the WebUI only
    let mut remote_address = req.connection_info().remote().unwrap_or("").to_string();
    if remote_address.len() >= 9 {
        remote_address.truncate(9);
        if remote_address == "127.0.0.1" || remote_address.to_lowercase() == "localhost" {
            // copy config into new obj that we can serialize
            let mut conf = Config {
                grace_period: crate::CONF.grace_period,
                priority_mode: crate::CONF.priority_mode,
                interrupt_lower_priority_blocks: crate::CONF.interrupt_lower_priority_blocks,
                web_server_bind_address: crate::CONF.web_server_bind_address.clone(),
                web_server_port: crate::CONF.web_server_port,
                use_poc_chain_colors: crate::CONF.use_poc_chain_colors,
                poc_chains: Some(Vec::new()),
                outage_status_update_interval: crate::CONF.outage_status_update_interval,
                show_human_readable_deadlines: crate::CONF.show_human_readable_deadlines,
                mask_account_ids_in_console: crate::CONF.mask_account_ids_in_console,
                use_24_hour_time: crate::CONF.use_24_hour_time,
                num_old_log_files_to_keep: crate::CONF.num_old_log_files_to_keep,
                logging_level: crate::CONF.logging_level.clone(),
                show_miner_addresses: crate::CONF.show_miner_addresses,
                dependency_logging_level: crate::CONF.dependency_logging_level.clone(),
                miner_update_timeout: crate::CONF.miner_update_timeout,
                miner_offline_warnings: crate::CONF.miner_offline_warnings,
                timeout: crate::CONF.timeout.clone(),
                submit_attempts: crate::CONF.submit_attempts.clone(),
            };
            let mut chains: Vec<PocChain> = Vec::new();
            for inner in &crate::CONF.poc_chains {
                for chain in inner {
                    chains.push(chain.clone());
                }
            }
            conf.poc_chains = Some(chains);
            let json = serde_json::to_string(&conf).unwrap_or(r#"{"result":"failure","reason":"Couldn't serialize Config object."#.to_string());
            create_response(StatusCode::OK, json)
        } else {
            create_response(StatusCode::FORBIDDEN, r#"{"result":"failure","reason":"This request can only be made from the host machine."}"#.to_string())
        }
    } else {
        create_response(StatusCode::FORBIDDEN, r#"{"result":"failure","reason":"This request can only be made from the host machine."}"#.to_string())
    }
}

fn api_handler(req: &HttpRequest) -> FutureResult<HttpResponse, Error> {
    match try_get_query_string_value(&req, "requestType") {
        (true, request_type) => {
            match request_type.to_lowercase().as_str() {
                "getbestdeadlines" => handle_api_get_best_deadlines(&req),
                "getconfig" => handle_api_get_config(&req),
                _ => handle_invalid_request_type()
            }
        },
        (false, _) => {
            create_response(StatusCode::BAD_REQUEST, r#"{"result":"failure","reason":"requestType parameter was not found."#.to_string())
        }
    }
}

fn webui_handler(req: &HttpRequest) -> FutureResult<HttpResponse, Error> {
    trace!("WEB UI Request from [{}] (Method: {})", req.connection_info().remote().unwrap_or("Unknown"), req.method().to_string());
    // just serve a static landing page for now
    result(Ok(HttpResponse::build(StatusCode::NOT_FOUND)
        .header(header::USER_AGENT, get_user_agent_str())
        .content_type("text/html")
        .body(r#"<!DOCTYPE html>

<head>
    <title>Archon | Coming Soon</title>
	
    <link href="data:image/x-icon;base64,AAABAAEAEBAAAAEAIABoBAAAFgAAACgAAAAQAAAAIAAAAAEAIAAAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAANywXHS8oFhAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAaFokE3tVHJNsSRR5fVgFLXNPBQ5bPwYGX0oIAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHNaJAaJYxAuwZoJeaeMFK6zlyb1n4AV+Kd8Bt+GYQi4fmEKm4BeBj1ZPQYHAAAAAAAAAAAAAAAAAAAAAAAAAAC9jQszsYoPzte5FPzGrCH/w6wl/7WaF//DlQr/nXcM/7GNB/6VbATfdk0EgloxAwoAAAAAAAAAAIg+AASrdgYgzKIGmOrOCf3hyyL/7OJQ/8yzNP/Cnh7/y6oX/9q1Ef/myQj/0rkO/6d4COaIUAROAAAAAK18HQfOrh5l3cQQz+7eF/n79Cj/+PM8/+Pfdv+Ohkf/noso/+TUMf/45R3/9+kU//fydP/ey1T/mmcKvmUvAiPVwkgl7uh6zvv3dP/9/Db//f5g//r6h/+3tYD/RUg9/1ZQJv/b0kn//PQp/+zaJP/x65X/3cpQ/6p7B+t5RQVK2Mo/GfHsgb3695b/9u8y//v6fP///+H/zcy5/09TTv+Bfmn/9fO3//n3eP/KrzL/r4cc/6+AD/+dawTZgFAGLdezAATq2jSX38lE/9a6Pf/597T//v73/8fHvf9LTkf/j4+A//Py3P/u7Z7/08pa/6+NGP+newf/mG4FuHNKBREAAAAA5c8bhtOyOP/UvUv/5eKO/8C9nP9bWlD/HB8e/zw4Kv+Lh2X/wbxz/+LWP//hzRb/sIgG/5VsBopPOwcCAAAAAOPCEHnkwhn+vKEw/4+HQP9yakf/Wk04/xobGf83Jhn/Sz4j/1xUJP+Gch7/w6IO/55wA/B9UgRMAAAAAAAAAADlzRJB6dQa5bifMf94Zjv/p5FI/5eHTv87PTT/Szwm/5Z6LP+Yfiv/jW0r/8ejFf+jcQPDdkkDGQAAAAAAAAAA28sPB+3nIIXq3Tb2x7BQ/+TbUv/MvVz/bGpW/2FQMP/LtjH/4M4v/76UJP+9mQ74oXgKeQAAAAAAAAAAAAAAAAAAAADp4SAS7OUpc8+6Q6nazUrt3M8//4FyS/+Dajj/3cou/8mqIPymdhLCo3kJaohgCxwAAAAAAAAAAAAAAAAAAAAAAAAAAPv9DQKykjEO3skkauLXJqaQei3NqZI01uLNI+vQqRWkrH0SLQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA7tcPAd+3EQbb0xgKiHEbKLagHjXbvR1ZzaMSFQAAAAAAAAAAAAAAAAAAAAAAAAAA//8AAP7/AAD8DwAA8AMAAOADAADAAQAAgAEAAIABAACAAQAAgAEAAMADAADAAwAAwAcAAPAPAAD8HwAA//8AAA==" rel="icon" type="image/x-icon" />

	<link href="https://fonts.googleapis.com/css?family=Teko" rel="stylesheet" />
	
    <style type="text/css">
        html, body {
			background: #36393f;
            font-family: "Teko", "Open Sans", sans-serif;
			color: #DCDCDE;
        }
		div.container {
			margin: 0 auto;
			height: 325px;
			width: 500px;
			margin-top: 200px;
			background: #606c88; /* Old browsers */
			background: -moz-linear-gradient(top, #606c88 0%, #3f4c6b 100%); /* FF3.6-15 */
			background: -webkit-linear-gradient(top, #606c88 0%,#3f4c6b 100%); /* Chrome10-25,Safari5.1-6 */
			background: linear-gradient(to bottom, #606c88 0%,#3f4c6b 100%); /* W3C, IE10+, FF16+, Chrome26+, Opera12+, Safari7+ */
			filter: progid:DXImageTransform.Microsoft.gradient( startColorstr='#606c88', endColorstr='#3f4c6b',GradientType=0 ); /* IE6-9 */
			-webkit-border-radius: 20px;
			-moz-border-radius: 20px;
			border-radius: 20px;
			padding: 0px 20px 10px 20px;
			border: 1px solid #000000;    
			-moz-box-shadow: inset 2px 2px 2px rgba(255, 255, 255, .4), inset -2px -2px 2px rgba(0, 0, 0, .4);
			-webkit-box-shadow: inset 2px 2px 2px rgba(255, 255, 255, .4), inset -2px -2px 2px rgba(0, 0, 0, .4);
			box-shadow: inset 2px 2px 2px rgba(255, 255, 255, .4), inset -2px -2px 2px rgba(0, 0, 0, .4);
		}
		div.container > div.img {
			margin: 0 auto;
			text-align: center;
		}
		div.container > div.img > img {
			position: relative;
			top: -150px;
		}
		div.container > div.text {
			margin-top: -175px;
		}
		div.merging {
			text-align: left;
			font-size: 6em;
		}
		div.merging2 {
			margin-top: -40px;
			text-align: right;
			font-size: 2em;
		}
    </style>
</head>

<body>
	<div class="container">
		<div class="img"><img src="https://www.dropbox.com/s/yvkbeb6vn6ttjm1/Archon-Icon.png?dl=1" height="344" /></div>
		<div class="text">
			<div class="merging">"MERGING...</div>
			<div class="merging2">...is not yet complete..."</div>
		</div>
	</div>
</body>

</html>"#)))
}

pub fn get_user_agent_str() -> String {
    format!("{} v{}", super::uppercase_first(super::APP_NAME), super::VERSION)
}

pub fn start_server() {
    use colored::Colorize;
    use std::process::exit;
    let archon_web_server_sys = actix::System::new("archon");
    server::new(|| {
        App::new()
            .resource("/", |r| r.route().a(webui_handler))
            .resource("/burst", |r| r.route().a(burst_handler))
            .resource("/api", |r| r.route().a(api_handler))
            .default_resource(|r| {
                r.route().f(|_| HttpResponse::MethodNotAllowed());
            })
    })
    .bind(format!("{}:{}", &crate::CONF.web_server_bind_address, &crate::CONF.web_server_port))
    .map_err(|why| {
        println!("\n\n  ERROR: Couldn't bind to {}:{}! Please ensure it isn't in use! - {:?}",  &crate::CONF.web_server_bind_address, &crate::CONF.web_server_port, why);
        error!("Couldn't bind to {}:{}! Please ensure it isn't in use! - {:?}",  &crate::CONF.web_server_bind_address, &crate::CONF.web_server_port, why);
        println!(
            "\n  {}",
            "Execution completed. Press enter to exit."
                .red()
                .underline()
        );

        let mut blah = String::new();
        std::io::stdin().read_line(&mut blah).expect("FAIL");
        exit(0);
    })
    .unwrap()
    .shutdown_timeout(0)
    .server_hostname(get_user_agent_str().to_string())
    .start();
    if archon_web_server_sys.run().is_ok() {}
}
