use serde::{Deserialize, Serialize};

pub const ENC_DEC_APP_URL: &str = "https://enc-dec.app";

// Common types

#[derive(Debug, Serialize)]
struct GenericRequest {
    text: String,
}

#[derive(Debug, Deserialize)]
struct DecResponse {
    result: DecResult,
}

#[derive(Debug, Deserialize)]
struct DecResult {
    url: String,
}
