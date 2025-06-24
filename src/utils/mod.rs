#![allow(dead_code)]

pub mod crypto;
// pub mod crypto_js;
pub mod anilist;
pub mod datalife;
mod doh;
pub mod html;
pub mod jwp_player;
pub mod lang;
pub mod nextjs;
pub mod playerjs;
pub mod unpack;

use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use doh::DoHResolver;
use regex::Regex;
use reqwest::{
    header::{self, HeaderMap},
    ClientBuilder,
};

pub fn get_user_agent<'a>() -> &'a str {
    // todo: rotate user agent
    "Mozilla/5.0 (X11; Linux x86_64; rv:133.0) Gecko/20100101 Firefox/133.0"
}

pub fn create_client() -> reqwest::Client {
    let builder = create_client_builder();

    let mut headers = get_default_headers();
    headers.insert(
        header::ACCEPT,
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
            .parse()
            .unwrap(),
    );

    builder.default_headers(headers).build().unwrap()
}

pub fn create_json_client() -> reqwest::Client {
    let builder = create_client_builder();

    let mut headers = get_default_headers();
    headers.insert(header::ACCEPT, "application/json".parse().unwrap());
    headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
    headers.insert("X-Requested-With", "XMLHttpRequest".parse().unwrap());

    builder.default_headers(headers).build().unwrap()
}

pub fn create_client_builder() -> reqwest::ClientBuilder {
    ClientBuilder::new()
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(30))
        .user_agent(get_user_agent())
        .danger_accept_invalid_certs(true)
        .cookie_store(true)
        .dns_resolver(Arc::new(DoHResolver::default()))
}

pub fn get_default_headers() -> HeaderMap {
    let mut headers = HeaderMap::default();

    headers.insert(
        header::ACCEPT_ENCODING,
        "gzip, deflate, br".parse().unwrap(),
    );
    headers.insert(header::ACCEPT_LANGUAGE, "en-US,en;q=0.5".parse().unwrap());
    headers.insert(header::CACHE_CONTROL, "no-cache".parse().unwrap());
    headers.insert(header::PRAGMA, "no-cache".parse().unwrap());
    headers.insert(header::CONNECTION, "keep-alive".parse().unwrap());
    headers.insert(header::DNT, "1".parse().unwrap());
    headers.insert(header::UPGRADE_INSECURE_REQUESTS, "1".parse().unwrap());
    headers.insert(header::TE, "trailers".parse().unwrap());
    headers
}

pub async fn scrap_page<T>(
    request_builder: reqwest::RequestBuilder,
    processor: &dyn html::DOMProcessor<T>,
) -> Result<T, anyhow::Error> {
    let html = request_builder.send().await?.text().await?;

    // println!("{html:#?}");

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

pub fn to_full_url(url: &str) -> String {
    if url.starts_with("//") {
        format!("https:{url}")
    } else {
        url.into()
    }
}
