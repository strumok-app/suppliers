use anyhow::anyhow;
use core::str;
use regex::Regex;
use scraper::Selector;
use std::{collections::HashMap, sync::OnceLock};

use crate::{
    models::ContentMediaItemSource,
    utils::{self, unpack::packerjs},
};

pub async fn extract(url: &str, prefix: &str) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let ifame_url = url.replace("/f/", "/e/");

    let user_agent = utils::get_user_agent();
    let iframe = utils::create_client_builder()
        .default_headers(utils::get_default_headers())
        .user_agent(user_agent)
        .build()
        .unwrap()
        .get(&ifame_url)
        .send()
        .await?
        .text()
        .await?;

    static SCRIPT_SELECTOR: OnceLock<Selector> = OnceLock::new();
    let document = scraper::Html::parse_document(&iframe);
    let packer_script = document
        .select(SCRIPT_SELECTOR.get_or_init(|| Selector::parse("script").unwrap()))
        .filter_map(|el| {
            let script = el.text().next()?;
            script.split("\n").find(|line| packerjs::detect(line))
        })
        .next()
        .ok_or_else(|| anyhow!("[mixdrop] no packer script found"))?;

    let upacked_script = packerjs::unpack(packer_script).map_err(|err| anyhow!(err))?;

    static FILE_PROPERTY_RE: OnceLock<Regex> = OnceLock::new();
    let mut file = FILE_PROPERTY_RE
        .get_or_init(|| Regex::new(r#"MDCore.wurl=["](?<file>[^"]+)["]"#).unwrap())
        .captures(&upacked_script)
        .and_then(|m| Some(m.name("file")?.as_str().to_owned()))
        .ok_or_else(|| anyhow!("[mixdrop] file property not found"))?;

    if file.starts_with("//") {
        file = format!("https:{file}")
    }

    Ok(vec![ContentMediaItemSource::Video {
        link: file,
        description: prefix.into(),
        headers: Some(HashMap::from([(
            "User-Agent".into(),
            user_agent.to_string(),
        )])),
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_extract() {
        let res = extract("https://mixdrop.ps/f/364x6xlmtdp1p7", "mixdrop").await;
        println!("{res:?}");
    }
}
