#![allow(dead_code)]

pub mod anilist;
pub mod crypto;
pub mod crypto_js;
pub mod datalife;
mod doh;
pub mod html;
pub mod jwp_player;
pub mod lang;
pub mod nextjs;
pub mod playerjs;
pub mod text;
pub mod unpack;

use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use doh::DoHResolver;
use reqwest::{
    ClientBuilder,
    header::{self, HeaderMap},
};

pub fn get_user_agent<'a>() -> &'a str {
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36 Edg/138.0.0.0"
}

pub fn create_client() -> &'static reqwest::Client {
    static LAZZY_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    LAZZY_CLIENT.get_or_init(|| {
        let builder = create_client_builder();

        let mut headers = get_default_headers();
        headers.insert(
            header::ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
                .parse()
                .unwrap(),
        );

        builder.default_headers(headers).build().unwrap()
    })
}

pub fn create_json_client() -> &'static reqwest::Client {
    static LAZZY_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    LAZZY_CLIENT.get_or_init(|| {
        let builder = create_client_builder();

        let mut headers = get_default_headers();
        headers.insert(header::ACCEPT, "application/json".parse().unwrap());
        headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());

        builder.default_headers(headers).build().unwrap()
    })
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

    let document = scraper::Html::parse_document(&html);
    let root = document.root_element();

    Ok(processor.process(&root))
}
