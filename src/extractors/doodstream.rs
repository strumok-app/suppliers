use std::{collections::HashMap, sync::OnceLock};

use anyhow::anyhow;
use regex::Regex;

use crate::{models::ContentMediaItemSource, utils};

const DOOD_URL: &str = "https://dood.re";
const RND_STRING: &str = "d96ZdcNq9N";

pub async fn extract(url: &str, prefix: &str) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    static MP5_PASS_RE: OnceLock<Regex> = OnceLock::new();

    let (_, id) = url
        .rsplit_once("/")
        .ok_or_else(|| anyhow!("No id found in url"))?;

    let iframe_url = format!("{DOOD_URL}/e/{id}");

    let iframe_res = utils::create_client()
        .get(&iframe_url)
        .send()
        .await?
        .text()
        .await?;

    println!("{iframe_res}");

    let maybe_md5_pass = MP5_PASS_RE
        .get_or_init(|| Regex::new(r"/pass_md5/(?<pass>[^']*)").unwrap())
        .captures(&iframe_res)
        .map(|caps| caps.name("pass").unwrap().as_str());

    if maybe_md5_pass.is_none() {
        return Err(anyhow!("[doodstream] mp5pass not found"));
    }

    let md5_pass = maybe_md5_pass.unwrap();
    let media_link_part = utils::create_client()
        .get(format!("{DOOD_URL}/pass_md5/{md5_pass}"))
        .header("Referer", &iframe_url)
        .send()
        .await?
        .text()
        .await?;

    let media_link = format!("{media_link_part}{RND_STRING}?token={md5_pass}");

    Ok(vec![ContentMediaItemSource::Video {
        link: media_link,
        description: prefix.into(),
        headers: Some(HashMap::from([("Referer".into(), iframe_url)])),
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test()]
    async fn should_extract() {
        let sources = extract("https://dood.wf/e/c37gfflwk73i", "dood")
            .await
            .unwrap();
        print!("{sources:#?}")
    }
}
