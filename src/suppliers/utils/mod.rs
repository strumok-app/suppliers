pub mod datalife;
pub mod html;
pub mod playerjs;

use std::time::Duration;

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

pub fn extract_digits(text: &String) -> u32 {
    let mut acc: u32 = 0;

    for ch in text.chars() {
        match ch.to_digit(10) {
            Some(digit) => {
                acc = acc * 10 + digit;
            }
            None => {}
        }
    }

    acc
}