use anyhow::{anyhow, Ok};
use regex::Regex;
use scraper::Selector;
use std::sync::OnceLock;

use crate::{
    models::ContentMediaItemSource,
    utils::{self, unpack::packerjs},
};

pub const PLAYER_URL: &str = "https://uqloads.xyz";

pub async fn extract(
    url: &str,
    referer: &str,
    prefix: &str,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    static SCRIPT_SELECTOR: OnceLock<Selector> = OnceLock::new();

    let html = utils::create_client()
        .get(url)
        .header("Referer", referer)
        .send()
        .await?
        .text()
        .await?;

    // println!("{html:#?}");

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

    // println!("{upacked_script}");

    static FILE_PROPERTY_RE: OnceLock<Regex> = OnceLock::new();
    let file = FILE_PROPERTY_RE
        .get_or_init(|| Regex::new(r#""hls2":\s?['"](?<file>[^"]+)['"]"#).unwrap())
        .captures(&upacked_script)
        .and_then(|m| Some(m.name("file")?.as_str()))
        .ok_or_else(|| anyhow!("[streamwish] file property not found"))?;

    Ok(vec![ContentMediaItemSource::Video {
        description: prefix.into(),
        link: file.into(),
        headers: None,
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_extract_1() {
        let result = extract(
            "https://alions.pro/v/n1kbhx78xlja",
            "https://anitaku.bz/dr-stone-episode-1",
            "streamwish",
        )
        .await
        .unwrap();
        println!("{result:#?}")
    }

    #[tokio::test()]
    async fn should_extract_2() {
        let result = extract(
            "https://awish.pro/e/d8yfe9r9up0h",
            "https://anitaku.bz/dr-stone-episode-1",
            "streamwish",
        )
        .await
        .unwrap();
        println!("{result:#?}")
    }
}
