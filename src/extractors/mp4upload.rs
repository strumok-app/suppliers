use std::{collections::HashMap, sync::OnceLock};

use regex::Regex;

use crate::{models::ContentMediaItemSource, utils};

pub async fn extract(
    url: &str,
    referer: &str,
    prefix: &str,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let html = utils::create_client()
        .get(url)
        .header("Referer", referer)
        .send()
        .await?
        .text()
        .await?;

    

    static SRC_REGEXP: OnceLock<Regex> = OnceLock::new();
    let file = SRC_REGEXP
        .get_or_init(|| Regex::new(r#"src:?\s+"(?<src>.*?(mp4|m3u8))""#).unwrap())
        .captures(&html)
        .and_then(|m| Some(m.name("src")?.as_str()))
        .ok_or_else(|| anyhow::anyhow!("[mp4upload] no src found in page"))?;

    Ok(vec![ContentMediaItemSource::Video {
        link: file.into(),
        description: prefix.into(),
        headers: Some(HashMap::from([("Referer".into(), url.into())])),
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_extract() {
        let res = extract(
            "https://www.mp4upload.com/embed-h5x14yaphmdk.html",
            "https://anitaku.bz/fairy-tail-100-years-quest-episode-20",
            "mp4upload",
        )
        .await
        .unwrap();
        println!("{res:#?}")
    }
}
