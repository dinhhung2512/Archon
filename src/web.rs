use crate::arbiter;
use crate::upstream::MiningInfo;
use rocket::http::{Status, ContentType};
use rocket::request::{self, Request, FromRequest, LenientForm};
use rocket::response::{self, Responder, Response};
use rocket::Outcome;
use std::io::Cursor;
use std::string::*;

// 404 handler
/*fn p404(req: &HttpRequest) -> Result<fs::NamedFile> {
    Ok(fs::NamedFile::open("webresources/404.html")?.set_status_code(StatusCode::NOT_FOUND))
}*/

fn parse_u64_from_str(txt: &str) -> u64 {
    match txt.parse::<u64>() {
        Ok(parsed) => parsed,
        _ => 0,
    }
}
/*
fn try_get_query_string_value(req: &HttpRequest, name: &str) -> (bool, String) {
    match req.query().get(name) {
        Some(val) => {
            return (true, val.clone());
        }
        _ => {
            return (false, String::from(""));
        }
    }
}

fn try_get_submit_nonce_data(req: &HttpRequest) -> (bool, u64, u64, u64, u64) {
    let block_height = match try_get_query_string_value(req, "blockheight") {
        (true, val) => parse_u64_from_str(val.as_str()),
        (false, _) => 0u64,
    };
    let account_id = match try_get_query_string_value(req, "accountId") {
        (true, val) => parse_u64_from_str(val.as_str()),
        (false, _) => 0u64,
    };
    let nonce = match try_get_query_string_value(req, "nonce") {
        (true, val) => parse_u64_from_str(val.as_str()),
        (false, _) => 0u64,
    };
    let deadline = match try_get_query_string_value(req, "deadline") {
        (true, val) => parse_u64_from_str(val.as_str()),
        (false, _) => 0u64,
    };
    if account_id > 0 && nonce > 0 && deadline > 0 {
        return (true, block_height, account_id, nonce, deadline);
    }
    return (false, 0u64, 0u64, 0u64, 0u64);
}

fn api_handler(req: &HttpRequest) -> FutureResult<HttpResponse, Error> {
    match try_get_query_string_value(&req, "requestType") {
        (true, request_type) => match request_type.to_lowercase().as_str() {
            "getbestdeadlines" => match try_get_query_string_value(&req, "height") {
                (true, height_string) => match str::parse::<u32>(height_string.as_str()) {
                    Ok(height) => {
                        let best_block_deadlines = arbiter::get_best_deadlines_for_block(height);
                        let json = serde_json::to_string(&best_block_deadlines).unwrap_or(String::from("{\"result\":\"failure\",\"reason\":\"Couldn't serialize best deadlines for block.\"}"));
                        result(Ok(HttpResponse::build(StatusCode::OK)
                            .header(
                                actix_web::http::header::USER_AGENT,
                                format!(
                                    "{} v{}",
                                    super::uppercase_first(super::APP_NAME),
                                    super::VERSION
                                ),
                            )
                            .header(actix_web::http::header::CONNECTION, "close")
                            .content_type("application/json")
                            .body(json)))
                    }
                    Err(_) => result(Ok(HttpResponse::build(StatusCode::OK)
                        .header(
                            actix_web::http::header::USER_AGENT,
                            format!(
                                "{} v{}",
                                super::uppercase_first(super::APP_NAME),
                                super::VERSION
                            ),
                        )
                        .header(actix_web::http::header::CONNECTION, "close")
                        .content_type("application/json")
                        .body("{\"result\":\"failure\",\"reason\":\"Couldn't parse height.\"}"))),
                },
                (false, _) => {
                    let best_deadlines = arbiter::get_best_deadlines();
                    let json = serde_json::to_string(&best_deadlines).unwrap_or(String::from("{\"result\":\"failure\",\"reason\":\"Couldn't serialize best deadlines.\"}"));
                    result(Ok(HttpResponse::build(StatusCode::OK)
                        .header(
                            actix_web::http::header::USER_AGENT,
                            format!(
                                "{} v{}",
                                super::uppercase_first(super::APP_NAME),
                                super::VERSION
                            ),
                        )
                        .header(actix_web::http::header::CONNECTION, "close")
                        .content_type("application/json")
                        .body(json)))
                }
            },
            "getconfig" => {
                let json = serde_json::to_string(&crate::CONF).unwrap_or(String::from(
                    "{\"result\":\"failure\",\"reason\":\"Couldn't serialize config.\"}",
                ));
                result(Ok(HttpResponse::build(StatusCode::OK)
                    .header(
                        actix_web::http::header::USER_AGENT,
                        format!(
                            "{} v{}",
                            super::uppercase_first(super::APP_NAME),
                            super::VERSION
                        ),
                    )
                    .header(actix_web::http::header::CONNECTION, "close")
                    .content_type("application/json")
                    .body(json)))
            }
            _ => result(Ok(HttpResponse::build(StatusCode::BAD_REQUEST)
                .header(
                    actix_web::http::header::USER_AGENT,
                    format!(
                        "{} v{}",
                        super::uppercase_first(super::APP_NAME),
                        super::VERSION
                    ),
                )
                .header(actix_web::http::header::CONNECTION, "close")
                .content_type("application/json")
                .body(
                    "{\"result\":\"failure\",\"reason\":\"Invalid request type.\"}",
                ))),
        },
        (false, _) => result(Ok(HttpResponse::build(StatusCode::BAD_REQUEST)
            .header(
                actix_web::http::header::USER_AGENT,
                format!(
                    "{} v{}",
                    super::uppercase_first(super::APP_NAME),
                    super::VERSION
                ),
            )
            .header(actix_web::http::header::CONNECTION, "close")
            .content_type("application/json")
            .body(
                "{\"result\":\"failure\",\"reason\":\"Request Type parameter was not found.\"}",
            ))),
    }
}

// burst api hander
fn burst_handler(req: &HttpRequest) -> FutureResult<HttpResponse, Error> {
    //println!("{:?}", req);
    match try_get_query_string_value(&req, "requestType") {
        (true, request_type) => {
            match request_type.to_lowercase().as_str() {
                "getmininginfo" => match super::get_current_mining_info() {
                    Some(current_mining_info) => result(Ok(HttpResponse::Ok()
                        .header(
                            actix_web::http::header::USER_AGENT,
                            format!(
                                "{} v{}",
                                super::uppercase_first(super::APP_NAME),
                                super::VERSION
                            ),
                        )
                        .header(actix_web::http::header::CONNECTION, "close")
                        .content_type("application/json")
                        .body(&current_mining_info.to_json()))),
                    _ => result(Ok(HttpResponse::Ok()
                        .header(
                            actix_web::http::header::USER_AGENT,
                            format!(
                                "{} v{}",
                                super::uppercase_first(super::APP_NAME),
                                super::VERSION
                            ),
                        )
                        .header(actix_web::http::header::CONNECTION, "close")
                        .content_type("application/json")
                        .body("{\"result\":\"failure\",\"reason\":\"No mining info!\"}"))),
                },
                "submitnonce" => {
                    if req.method() == Method::POST {
                        //println!("{:?}", req);
                        match try_get_submit_nonce_data(req) {
                            (true, block_height, account_id, nonce, deadline) => {
                                let mut user_agent_header = match req.headers().get(header::USER_AGENT) {
                                    Some(value) => {
                                        match value.to_str() {
                                            Ok(value) => value,
                                            _ => "Unknown",
                                        }
                                    },
                                    _ => "Unknown",
                                };
                                // if can't find user agent header, try X-Miner
                                if user_agent_header == "Unknown" {
                                    user_agent_header = match req.headers().get("X-Miner") {
                                        Some(value) => {
                                            match value.to_str() {
                                                Ok(value) => value,
                                                _ => "Unknown",
                                            }
                                        },
                                        _ => "Unknown",
                                    };
                                }
                                let x_deadline_present;
                                let x_deadline;
                                match req.headers().get("X-Deadline") {
                                    Some(x_deadline_header) => {
                                        match x_deadline_header.to_str() {
                                            Ok(value) => {
                                                x_deadline_present = true;
                                                x_deadline = str::parse::<u64>(value).unwrap_or(u64::max_value());
                                            },
                                            _ => {
                                                x_deadline_present = false;
                                        x_deadline = u64::max_value();
                                            }
                                        };
                                    },
                                    _ => {
                                        x_deadline_present = false;
                                        x_deadline = u64::max_value();
                                    }
                                };
                                let deadline_to_use;
                                if deadline == u64::max_value() && x_deadline_present && x_deadline < u64::max_value() {
                                    deadline_to_use = Some(x_deadline);
                                } else if deadline < u64::max_value() {
                                    deadline_to_use = Some(deadline);
                                } else {
                                    deadline_to_use = None;
                                }
                                crate::arbiter::process_nonce_submission(block_height as u32, account_id, nonce, deadline_to_use, user_agent_header, x_deadline_present)
                            },
                            _ => {
                                result(Ok(HttpResponse::build(StatusCode::BAD_REQUEST)
                                    .header(
                                        actix_web::http::header::USER_AGENT,
                                        format!(
                                            "{} v{}",
                                            super::uppercase_first(super::APP_NAME),
                                            super::VERSION
                                        ),
                                    )
                                    .header(actix_web::http::header::CONNECTION, "close")
                                    .content_type("application/json")
                                    .body(
                                        "{\"result\":\"failure\",\"reason\":\"Required parameters were not present!\"}"
                                    )))
                            },
                        }
                    } else {
                        result(Ok(HttpResponse::build(StatusCode::METHOD_NOT_ALLOWED)
                            .header(
                                actix_web::http::header::USER_AGENT,
                                format!(
                                    "{} v{}",
                                    super::uppercase_first(super::APP_NAME),
                                    super::VERSION
                                ),
                            )
                            .header(actix_web::http::header::CONNECTION, "close")
                            .content_type("application/json")
                            .body(
                                "{\"result\":\"failure\",\"reason\":\"Submission of nonces must be done via POST!\"}"
                            )))
                    }
                }
                _ => result(Ok(HttpResponse::build(StatusCode::BAD_REQUEST)
                    .header(
                        actix_web::http::header::USER_AGENT,
                        format!(
                            "{} v{}",
                            super::uppercase_first(super::APP_NAME),
                            super::VERSION
                        ),
                    )
                    .header(actix_web::http::header::CONNECTION, "close")
                    .content_type("application/json")
                    .body(
                        "{\"result\":\"failure\",\"reason\":\"Invalid request type.\"}",
                    ))),
            }
        }
        (false, _) => result(Ok(HttpResponse::build(StatusCode::BAD_REQUEST)
            .header(
                actix_web::http::header::USER_AGENT,
                format!(
                    "{} v{}",
                    super::uppercase_first(super::APP_NAME),
                    super::VERSION
                ),
            )
            .header(actix_web::http::header::CONNECTION, "close")
            .content_type("application/json")
            .body(
                "{\"result\":\"failure\",\"reason\":\"Request Type parameter was not found.\"}",
            ))),
    }
}*/

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
            0 => Outcome::Failure((Status::BadRequest, UserAgentError::NotPresent)),
            _ => Outcome::Success(user_agent)
        }
    }
}

struct Adjusted(bool);

#[derive(Debug)]
enum AdjustedError {
    NotPresent,
}

impl<'a, 'r> FromRequest<'a, 'r> for Adjusted {
    type Error = AdjustedError;
    fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let x_deadline_header = request.headers().get_one("X-Deadline");
        Outcome::Success(Adjusted(x_deadline_header.is_some()))
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
    user_agent: UserAgent,
    adjusted: Adjusted,
) -> Option<SubmitNonceResponse> {
    match submit_nonce_info {
        Some(submit_nonce_info) => {
            arbiter::process_nonce_submission(
                submit_nonce_info.block_height.unwrap_or(0),
                submit_nonce_info.account_id,
                submit_nonce_info.nonce,
                Some(submit_nonce_info.deadline),
                user_agent.0.as_str(),
                adjusted.0
            )
        },
        _ => Some(SubmitNonceResponse {
            result: String::from("failure"),
            deadline: None,
            reason: Some(String::from("Required parameters for nonce submission were not present.")),
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
