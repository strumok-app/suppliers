use anyhow::{Ok, anyhow};
use log::warn;
use regex::Regex;
use scraper::Selector;
use std::sync::OnceLock;

use crate::{
    models::ContentMediaItemSource,
    utils::{self, unpack::packerjs},
};

const URL: &str = "https://dinisglows.com";

pub async fn extract(url: &str, prefix: &str) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let id = match url.rsplit_once("/").map(|(_, r)| r) {
        Some(id) => id,
        None => {
            warn!("[streamwish] no id found in url {url}");
            return Ok(vec![]);
        }
    };

    let host_url = format!("{URL}/v/{id}");

    static SCRIPT_SELECTOR: OnceLock<Selector> = OnceLock::new();

    let html = utils::create_client()
        .get(host_url)
        .send()
        .await?
        .text()
        .await?;

    let document = scraper::Html::parse_document(&html);
    let packer_script = document
        .select(SCRIPT_SELECTOR.get_or_init(|| Selector::parse("script").unwrap()))
        .filter_map(|el| {
            let script = el.text().next()?;
            if !packerjs::detect(script) {
                return None;
            }

            Some(script)
        })
        .next()
        .ok_or_else(|| anyhow!("[filelions] no packer script found"))?;

    let upacked_script = packerjs::unpack(packer_script).map_err(|err| anyhow!(err))?;

    static FILE_PROPERTY_RE: OnceLock<Regex> = OnceLock::new();
    let sources: Vec<_> = FILE_PROPERTY_RE
        .get_or_init(|| Regex::new(r#""hls(\d+)":\s?['"]([^"]+)['"]"#).unwrap())
        .captures_iter(&upacked_script)
        .filter_map(|m| Some((m.get(1)?.as_str(), m.get(2)?.as_str())))
        .map(|(idx, file)| {
            let link = if file.starts_with("/") {
                format!("{}{}", URL, file)
            } else {
                file.to_owned()
            };

            ContentMediaItemSource::Video {
                link,
                description: format!("{prefix} hls{idx}."),
                headers: None,
            }
        })
        .collect();

    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_extract_1() {
        let result = extract("https://filelions.to/v/p7n08t2i7jee", "filelions")
            .await
            .unwrap();
        println!("{result:#?}")
    }
}
