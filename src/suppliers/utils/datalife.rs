
use std::collections::HashMap;
use anyhow::anyhow;

use reqwest::{self, RequestBuilder};

pub fn search_request(url: &str, query: &String) -> RequestBuilder {
    let client =  super::create_client();

    client.post(format!("{url}/index.php"))
        .form(&[
            ("do", "search"),
            ("subaction", "search"),
            ("story", &query),
            ("sortby", "date"),
            ("resorder", "desc"),
        ])
}

pub fn get_channel_url(channels_map: &HashMap<String, String>, channel: &str, page: u16) -> anyhow::Result<String> {
    match channels_map.get(channel) {
        Some(url) => {
            if url.ends_with("/page/") {
                Ok(format!("{url}{page}"))
            } else {
                Ok(url.into())
            }
        },
        _ => Err(anyhow!("unknown channel")),
    }
}

pub fn extract_id_from_url(url: &str, mut id: String) -> String {
    // remove site name
    id.replace_range(0..(url.len() + 1), "");
    // remove .html
    id.replace_range((id.len() - 5)..id.len(), "");
    id
}

pub fn format_id_from_url(url: &str,  id: &String) -> String {
    format!("{url}/{id}.html")
}