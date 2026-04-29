use std::{collections::HashMap, sync::OnceLock};
use anyhow::anyhow;
use regex::Regex;
use scraper::Selector;

use crate::{models::ContentMediaItemSource, utils::{self, unpack::packerjs}};

pub async fn extract(
    url: &str,
    referer: &str,
    prefix: String,
    hls_proxy: bool,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let host = url
        .split("/")
        .nth(2)
        .ok_or_else(|| anyhow!("[packer_hls] invalid url"))?;

    // println!("host: {host}");

    let iframe_res: String = utils::create_client()
        .get(url)
        .header("Referer", referer)
        .send()
        .await?
        .text()
        .await?;

    // println!("{iframe_res}");

    static SCRIPT_SELECTOR: OnceLock<Selector> = OnceLock::new();
    let script_selector = SCRIPT_SELECTOR.get_or_init(|| Selector::parse("script").unwrap());

    let document = scraper::Html::parse_document(&iframe_res);
    let packer_script = document
        .select(script_selector)
        .filter_map(|el| {
            let script = el.text().next()?;
            if !packerjs::detect(script) {
                return None;
            }

            Some(script)
        })
        .next()
        .ok_or_else(|| anyhow!("[packer_hls] no packer script found"))?;

    let upacked_script = packerjs::unpack(packer_script).map_err(|err| anyhow!(err))?;

    // println!("{upacked_script}");

    static HLS_PROPERTY_RE: OnceLock<Regex> = OnceLock::new();
    let hls_property_re = HLS_PROPERTY_RE
        .get_or_init(|| Regex::new(r#""hls(\d+)":\s?['"]([^"]+)['"]"#).unwrap());

    let sources: Vec<_> = 
        hls_property_re.captures_iter(&upacked_script)
        .filter_map(|m| Some((m.get(1)?.as_str(), m.get(2)?.as_str())))
        .map(|(idx, file)| {
            let link = if file.starts_with("/") {
                format!("https://{}{}", host, file)
            } else {
                file.to_owned()
            };

            ContentMediaItemSource::Video {
                link,
                description: format!("{prefix} {idx}."),
                headers: Some(HashMap::from([("Referer".to_string(), url.to_string())])),
                hls_proxy: hls_proxy,
            }
        })
        .collect();

    Ok(sources)
}

mod tests {

    #[tokio::test]
    async fn test_extract() {
        let url = "https://otakuhg.site/e/nc84twztnk37";
        let referer = "https://anitaku.to";

        let res = super::extract(url, referer, "Test Title".to_string(), true).await.unwrap();

        println!("{res:#?}");
    }
}