pub mod crypto;
pub mod crypto_js;
pub mod datalife;
pub mod html;
pub mod jwp_player;
pub mod playerjs;
pub mod unpack;

use std::{sync::OnceLock, time::Duration};

use regex::Regex;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    ClientBuilder,
};

pub fn get_user_agent<'a>() -> &'a str {
    // todo: rotate user agent
    "Mozilla/5.0 (X11; Linux x86_64; rv:132.0) Gecko/20100101 Firefox/132.0"
}

pub fn create_client() -> reqwest::Client {
    let mut headers = HeaderMap::new();
    headers.insert(
        "User-Agent",
        HeaderValue::from_str(get_user_agent()).unwrap(),
    );

    ClientBuilder::new()
        .connect_timeout(Duration::from_secs(30))
        .default_headers(headers)
        .build()
        .unwrap()
}

pub async fn scrap_page<T>(
    request_builder: reqwest::RequestBuilder,
    processor: &dyn html::DOMProcessor<T>,
) -> Result<T, anyhow::Error> {
    let html = request_builder.send().await?.text().await?;

    let document = scraper::Html::parse_document(&html);
    let root = document.root_element();

    Ok(processor.process(&root))
}

pub fn extract_digits(text: &str) -> u32 {
    let mut acc: u32 = 0;

    for ch in text.chars() {
        if let Some(digit) = ch.to_digit(10) {
            acc = acc * 10 + digit;
        }
    }

    acc
}

pub fn extract_file_property(script: &str) -> Option<&str> {
    static FILE_PROPERTY_RE: OnceLock<Regex> = OnceLock::new();
    FILE_PROPERTY_RE
        .get_or_init(|| Regex::new(r#"file:\s?['"](?<file>[^"]+)['"]"#).unwrap())
        .captures(script)
        .and_then(|m| Some(m.name("file")?.as_str()))
}
