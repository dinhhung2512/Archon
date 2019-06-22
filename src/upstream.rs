use serde::de;
use serde_derive::Deserialize;
use std::convert::TryFrom;
use std::fmt;

use std::string::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MiningInfo {
    #[serde(deserialize_with = "string_or_number_to_u32")]
    pub height: u32,
    #[serde(deserialize_with = "string_or_number_to_u64", default = "u64::max_value")]
    pub base_target: u64,
    pub generation_signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_processing_time: Option<u8>,
    #[serde(deserialize_with = "string_or_number_to_u64", default = "u64::max_value")]
    pub target_deadline: u64,
}

impl MiningInfo {
    pub fn empty() -> MiningInfo {
        return MiningInfo {
            height: 0,
            base_target: 0,
            generation_signature: String::from(""),
            request_processing_time: Some(0),
            target_deadline: 0,
        };
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn from_json(json: &str) -> (bool, MiningInfo) {
        match serde_json::from_str(json) {
            Ok(mi) => {
                return (true, mi);
            }
            Err(why) => {
                warn!("MiningInfo::from_json({}): Failed parse: {:?}", json, why);
                return (false, MiningInfo::empty());
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubmitNonceInfo {
    #[serde(deserialize_with = "string_or_number_to_u64")]
    pub account_id: u64,
    #[serde(deserialize_with = "string_or_number_to_u64")]
    pub nonce: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_phrase: Option<String>,
    #[serde(deserialize_with = "string_or_number_to_u32")]
    pub blockheight: u32,
    #[serde(deserialize_with = "string_or_number_to_u32")]
    pub deadline: u32,
}

impl SubmitNonceInfo {
    pub fn empty() -> SubmitNonceInfo {
        return SubmitNonceInfo {
            account_id: 0u64,
            nonce: 0u64,
            secret_phrase: None,
            blockheight: 0u32,
            deadline: 0u32,
        };
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn from_json(json: &str) -> (bool, SubmitNonceInfo) {
        match serde_json::from_str(json) {
            Ok(sni) => {
                return (true, sni);
            }
            Err(_) => {
                return (false, SubmitNonceInfo::empty());
            }
        }
    }
}

fn string_or_number_to_u32<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct StringOrNumberVisitor;

    impl<'de> de::Visitor<'de> for StringOrNumberVisitor {
        type Value = u32;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an u32 as a number or a string")
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
            u32::try_from(value)
                .map_err(|_| E::custom(format!("number does not fit in u32: {}", value)))
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
            value
                .parse()
                .map_err(|e| E::custom(format!("Could not parse u32: {}", e)))
        }
    }

    deserializer.deserialize_any(StringOrNumberVisitor)
}

#[allow(dead_code)]
fn string_or_number_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct StringOrNumberVisitor;

    impl<'de> de::Visitor<'de> for StringOrNumberVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an u64 as a number or a string")
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
            u64::try_from(value)
                .map_err(|_| E::custom(format!("number does not fit in u64: {}", value)))
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
            value
                .parse()
                .map_err(|e| E::custom(format!("Could not parse u64: {}", e)))
        }
    }

    deserializer.deserialize_any(StringOrNumberVisitor)
}

#[allow(dead_code)]
fn string_or_number_to_i32<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct StringOrNumberVisitor;

    impl<'de> de::Visitor<'de> for StringOrNumberVisitor {
        type Value = i32;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an i32 as a number or a string")
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
            i32::try_from(value)
                .map_err(|_| E::custom(format!("number does not fit in i32: {}", value)))
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
            value
                .parse()
                .map_err(|e| E::custom(format!("Could not parse i32: {}", e)))
        }
    }

    deserializer.deserialize_any(StringOrNumberVisitor)
}

#[allow(dead_code)]
fn string_or_number_to_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct StringOrNumberVisitor;

    impl<'de> de::Visitor<'de> for StringOrNumberVisitor {
        type Value = i64;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an i64 as a number or a string")
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
            i64::try_from(value)
                .map_err(|_| E::custom(format!("number does not fit in i64: {}", value)))
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
            value
                .parse()
                .map_err(|e| E::custom(format!("Could not parse i64: {}", e)))
        }
    }

    deserializer.deserialize_any(StringOrNumberVisitor)
}
