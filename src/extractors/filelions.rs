use anyhow::{anyhow, Ok};
use log::warn;
use regex::Regex;
use scraper::Selector;
use std::sync::OnceLock;

use crate::{
    models::ContentMediaItemSource,
    utils::{self, unpack::packerjs},
};

const URL: &str = "https://taylorplayer.com/v";

pub async fn extract(url: &str, prefix: &str) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let id = match url.rsplit_once("/").map(|(_, r)| r) {
        Some(id) => id,
        None => {
            warn!("[streamwish] no id found in url {url}");
            return Ok(vec![]);
        }
    };

    let host_url = format!("{URL}/{id}");

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
    let file = FILE_PROPERTY_RE
        .get_or_init(|| Regex::new(r#""hls2":\s?['"](?<file>[^"]+)['"]"#).unwrap())
        .captures(&upacked_script)
        .and_then(|m| Some(m.name("file")?.as_str()))
        .ok_or_else(|| anyhow!("[filelions] file property not found"))?;

    Ok(vec![ContentMediaItemSource::Video {
        description: prefix.into(),
        link: file.into(),
        headers: None,
    }])
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
