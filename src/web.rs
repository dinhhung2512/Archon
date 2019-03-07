use crate::arbiter;
use crate::upstream::MiningInfo;
use rocket::http::ContentType;
use rocket::request::{self, FromRequest, LenientForm, Request};
use rocket::response::{self, Responder, Response};
use rocket::Outcome;
use std::io::Cursor;
use std::string::*;

fn parse_u64_from_str(txt: &str) -> u64 {
    match txt.parse::<u64>() {
        Ok(parsed) => parsed,
        _ => 0,
    }
}

// setup rocket responder for MiningInfo type
impl<'r> Responder<'r> for MiningInfo {
    fn respond_to(self, _: &Request) -> response::Result<'r> {
        Response::build()
            .sized_body(Cursor::new(self.to_json()))
            .raw_header(
                "User-Agent",
                format!(
                    "{} v{}",
                    super::uppercase_first(super::APP_NAME),
                    super::VERSION
                ),
            )
            .header(ContentType::new("application", "json"))
            .ok()
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
}

impl SubmitNonceResponse {
    pub fn empty() -> SubmitNonceResponse {
        SubmitNonceResponse {
            result: String::from(""),
            deadline: None,
            reason: None,
            error_code: None,
            error_description: None,
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

// setup rocket responder for SubmitNonceResponse type
impl<'r> Responder<'r> for SubmitNonceResponse {
    fn respond_to(self, _: &Request) -> response::Result<'r> {
        Response::build()
            .sized_body(Cursor::new(self.to_json()))
            .raw_header(
                "User-Agent",
                format!(
                    "{} v{}",
                    super::uppercase_first(super::APP_NAME),
                    super::VERSION
                ),
            )
            .header(ContentType::new("application", "json"))
            .ok()
    }
}

// setup rocket RequestGuard to retrieve the user agent or X-Miner header from requests
struct UserAgent(String);

#[derive(Debug)]
enum UserAgentError {
    NotPresent,
}

impl<'a, 'r> FromRequest<'a, 'r> for UserAgent {
    type Error = UserAgentError;
    fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let user_agent_header = request.headers().get_one("User-Agent");
        let x_miner_header = request.headers().get_one("X-Miner");
        let user_agent: UserAgent;
        if user_agent_header.is_some() && user_agent_header.unwrap().len() > 0 {
            user_agent = UserAgent(user_agent_header.unwrap().to_string());
        } else if x_miner_header.is_some() && x_miner_header.unwrap().len() > 0 {
            user_agent = UserAgent(x_miner_header.unwrap().to_string());
        } else {
            user_agent = UserAgent("".to_string());
        }
        match user_agent.0.len() {
            0 => Outcome::Success(UserAgent("Unknown".to_string())),
            _ => Outcome::Success(user_agent),
        }
    }
}

// set up rocket RequestGuard to retrieve the X-Deadline header from nonce submission requests
struct XDeadline(u64);

#[derive(Debug)]
enum XDeadlineError {
    NotPresent,
}

impl<'a, 'r> FromRequest<'a, 'r> for XDeadline {
    type Error = XDeadlineError;
    fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let x_deadline_header = request.headers().get_one("X-Deadline");
        match x_deadline_header {
            Some(x_deadline) => {
                let x_deadline_u64 = parse_u64_from_str(x_deadline);
                Outcome::Success(XDeadline(x_deadline_u64))
            }
            None => Outcome::Success(XDeadline(0)),
        }
    }
}

#[derive(FromForm)]
struct SubmitNonceInfo {
    #[form(field = "blockheight")]
    block_height: Option<u32>,
    #[form(field = "accountId")]
    account_id: u64,
    nonce: u64,
    deadline: u64,
    #[allow(dead_code)]
    #[form(field = "secretPhrase")]
    secret_phrase: Option<String>,
}

fn get_mining_info() -> Option<MiningInfo> {
    super::get_current_mining_info()
}

#[get("/burst?requestType=getMiningInfo", rank = 0)]
fn get_mining_info_via_get() -> Option<MiningInfo> {
    get_mining_info()
}

#[post("/burst?requestType=getMiningInfo", rank = 1)]
fn get_mining_info_via_post() -> Option<MiningInfo> {
    get_mining_info()
}

#[post("/burst?requestType=submitNonce&<submit_nonce_info..>", rank = 2)]
fn submit_nonce(
    submit_nonce_info: Option<LenientForm<SubmitNonceInfo>>,
    user_agent: UserAgent, // request guard, retrieves user-agent/x-miner header, never returns Outcome::Failure, just "Unknown" if it can't find either
    x_deadline: XDeadline, // request guard, retrieves X-Deadline header, never returns Outcome::Failure, just 0u64 if it can't find the header
) -> Option<SubmitNonceResponse> {
    match submit_nonce_info {
        Some(submit_nonce_info) => {
            let deadline;
            let is_adjusted;
            if submit_nonce_info.deadline > 0 {
                deadline = submit_nonce_info.deadline;
                is_adjusted = false;
            } else {
                deadline = x_deadline.0;
                is_adjusted = true;
            }
            arbiter::process_nonce_submission(
                submit_nonce_info.block_height.unwrap_or(0),
                submit_nonce_info.account_id,
                submit_nonce_info.nonce,
                Some(deadline),
                user_agent.0.as_str(),
                is_adjusted,
            )
        }
        _ => Some(SubmitNonceResponse {
            result: String::from("failure"),
            deadline: None,
            reason: Some(String::from(
                "Required parameters for nonce submission were not present.",
            )),
            error_code: None,
            error_description: None,
        }),
    }
}

pub fn start_server() {
    use rocket::config::{Config, Environment, LoggingLevel};
    match Config::build(Environment::Production)
        .log_level(LoggingLevel::Off)
        .address(crate::CONF.web_server_bind_address.as_str())
        .port(crate::CONF.web_server_port)
        .finalize()
    {
        Ok(rocket_config) => {
            rocket::custom(rocket_config)
                .mount(
                    "/",
                    routes![
                        get_mining_info_via_get,
                        get_mining_info_via_post,
                        submit_nonce
                    ],
                )
                .launch();
        }
        _ => {
            println!("Couldn't start web server.");
        }
    }
}
