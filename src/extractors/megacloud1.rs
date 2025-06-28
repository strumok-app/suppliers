use std::collections::HashMap;

use anyhow::anyhow;

use crate::{
    models::ContentMediaItemSource,
    utils::{self, jwp_player::JWPConfig},
};

const MEGACLOUD_EXTRACTOR_API: &str = env!("MEGACLOUD_EXTRACTOR");
const REFERER: &str = "https://megacloud.club/";

pub async fn extract(
    url: &str,
    referer: &str,
    prefix: &str,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let (_, a) = url
        .rsplit_once("/")
        .ok_or_else(|| anyhow!("invalid megacloud url"))?;

    let xrax = a.split_once("?").map(|(xrax, _)| xrax).unwrap_or(a);

    let config: JWPConfig = utils::create_json_client()
        .get(MEGACLOUD_EXTRACTOR_API)
        .header("Referer", referer)
        .query(&[("xrax", xrax)])
        .send()
        .await?
        .json()
        .await?;

    // println!("{xrax}: {config:#?}");

    Ok(config.to_media_item_sources(
        prefix,
        Some(HashMap::from([("Referer".into(), REFERER.into())])),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_load_by_link() {
        let link = "https://megacloud.tv/embed-2/e-1/KoJagrSKr43s?k=1";
        let sources = extract(link, "https://hianime.to", "Megacloud")
            .await
            .unwrap();
        println!("{sources:#?}")
    }
}
