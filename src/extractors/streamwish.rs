use anyhow::{Ok, anyhow};
use regex::Regex;
use scraper::Selector;
use std::sync::OnceLock;

use crate::{
    models::ContentMediaItemSource,
    utils::{self, unpack::packerjs},
};
const STREAMWISH_URL: &str = "https://streamwish.to";
const SUBSTITUTE_URL: &str = "https://yuguaab.com";

pub async fn extract(url: &str, prefix: &str) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    static SCRIPT_SELECTOR: OnceLock<Selector> = OnceLock::new();

    let final_url;
    let host;

    if url.starts_with(STREAMWISH_URL) {
        final_url = url.replace(STREAMWISH_URL, SUBSTITUTE_URL);
        host = SUBSTITUTE_URL
    } else {
        final_url = url.to_owned();
        host = url
            .split_once("/e/")
            .map(|(l, _)| l)
            .ok_or_else(|| anyhow!("[streamwish] invalid url: {url}"))?;
    }

    let html = utils::create_client()
        .get(final_url)
        .send()
        .await?
        .text()
        .await?;

    // println!("{html}");

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
        .ok_or_else(|| anyhow!("[streamwish] no packer script found"))?;

    let upacked_script = packerjs::unpack(packer_script).map_err(|err| anyhow!(err))?;

    static FILE_PROPERTY_RE: OnceLock<Regex> = OnceLock::new();
    let sources: Vec<_> = FILE_PROPERTY_RE
        .get_or_init(|| Regex::new(r#""hls(\d+)":\s?['"]([^"]+)['"]"#).unwrap())
        .captures_iter(&upacked_script)
        .filter_map(|m| Some((m.get(1)?.as_str(), m.get(2)?.as_str())))
        .map(|(idx, file)| {
            let link = if file.starts_with("/") {
                format!("{}{}", host, file)
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
    // Ok(vec![ContentMediaItemSource::Video {
    //     description: prefix.into(),
    //     link: file.into(),
    //     headers: None,
    // }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_extract_1() {
        let result = extract("https://streamwish.to/e/1mranuy7w6r2", "streamwish")
            .await
            .unwrap();
        println!("{result:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_extract_2() {
        let result = extract("https://yesmovies.baby/e/ahu6x76icl5g", "streamwish")
            .await
            .unwrap();
        println!("{result:#?}")
    }
}
