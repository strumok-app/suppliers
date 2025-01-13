use std::{collections::BTreeMap, vec};

use anyhow::anyhow;
use log::warn;
use serde::Deserialize;
use serde_json::json;

use crate::{
    models::{ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType},
    utils::{anilist, jwp_player::JWPConfig, nextjs},
};

use super::ContentSupplier;

const URL: &str = "https://aniplaynow.live";

#[derive(Default)]
pub struct AniplayContentSupplier;

impl ContentSupplier for AniplayContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![ContentType::Anime]
    }

    fn get_supported_languages(&self) -> Vec<String> {
        vec!["en".into()]
    }

    async fn search(&self, query: String, _types: Vec<String>) -> anyhow::Result<Vec<ContentInfo>> {
        anilist::search_anime(&query).await
    }

    async fn load_channel(&self, _channel: String, _page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        Err(anyhow!("unimplemented"))
    }

    async fn get_content_details(&self, id: String) -> anyhow::Result<Option<ContentDetails>> {
        anilist::get_anime(&id).await
    }

    async fn load_media_items(
        &self,
        id: String,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        let url = format!("{URL}/anime/info/{id}");

        #[derive(Deserialize, Debug)]
        struct AniplayEpisode {
            id: String,
            number: u32,
            #[serde(alias = "hasDub")]
            has_dub: bool,
            #[serde(default)]
            title: String,
            #[serde(default)]
            img: String,
        }

        #[derive(Deserialize, Debug)]
        struct AniplayServer {
            #[serde(default)]
            episodes: Vec<AniplayEpisode>,
            #[serde(alias = "providerId")]
            provider_id: String,
        }

        let servers: Vec<AniplayServer> = nextjs::server_action(
            url.as_str(),
            "f3422af67c84852f5e63d50e1f51718f1c0225c4",
            1,
            &json!([id, true,]),
        )
        .await?;

        let mut sorted_media_items: BTreeMap<u32, ContentMediaItem> = BTreeMap::new();

        for server in servers {
            let provider = server.provider_id;
            for episode in server.episodes {
                let media_item =
                    sorted_media_items
                        .entry(episode.number)
                        .or_insert_with(|| ContentMediaItem {
                            number: episode.number - 1,
                            title: episode.title,
                            section: None,
                            image: None,
                            sources: None,
                            params: vec![episode.number.to_string()],
                        });

                if media_item.image.is_none() && episode.img.starts_with("http") {
                    media_item.image = Some(episode.img);
                }

                media_item.params.push(provider.clone());
                media_item.params.push(episode.id);
                media_item.params.push(if episode.has_dub {
                    "1".into()
                } else {
                    "0".into()
                });
            }
        }

        Ok(sorted_media_items.into_values().collect())
    }

    async fn load_media_item_sources(
        &self,
        id: String,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.len() < 4 {
            return Err(anyhow!("Incorrect params"));
        }

        let ep_numeber = &params[0];
        let server_params: Vec<_> = params.iter().skip(1).collect();
        if server_params.len() % 3 != 0 {
            return Err(anyhow!("Incorrect params"));
        }

        let futures = server_params.chunks(3).map(|params| {
            load_server_media_item_sources(&id, params[0], params[1], ep_numeber, params[2] == "1")
        });

        let sources: Vec<_> = futures::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect();

        Ok(sources)
    }
}

async fn load_server_media_item_sources(
    id: &str,
    provider: &str,
    ep_id: &str,
    ep_number: &str,
    has_dub: bool,
) -> Vec<ContentMediaItemSource> {
    let mut result: Vec<ContentMediaItemSource> = vec![];

    if has_dub {
        let mut res = load_server_by_type(id, provider, ep_id, ep_number, "dub").await;

        match &mut res {
            Ok(sources) => result.append(sources),
            Err(err) => {
                warn!("[aniplay] fail to load server source(id: {id}, provider: {provider}, ep_id: {ep_id}, ep_numeber: {ep_number}, dub): {err}")
            }
        }
    }

    let mut res = load_server_by_type(id, provider, ep_id, ep_number, "sub").await;

    match &mut res {
        Ok(sources) => result.append(sources),
        Err(err) => {
            warn!("[aniplay] fail to load server source(id: {id}, provider: {provider}, ep_id: {ep_id}, ep_numeber: {ep_number}, sub): {err}")
        }
    }
    result
}

async fn load_server_by_type(
    id: &str,
    provider: &str,
    ep_id: &str,
    ep_number: &str,
    r#type: &str,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let url = format!("{URL}/anime/watch/{id}?host={provider}&ep={ep_number}&type={type}");

    let config: JWPConfig = nextjs::server_action(
        &url,
        "5dbcd21c7c276c4d15f8de29d9ef27aef5ea4a5e",
        1,
        &json!([id, provider, ep_id, ep_number, r#type,]),
    )
    .await?;

    let sources = config.to_media_item_sources(&format!("[{type}] {provider}"), None);

    Ok(sources)
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn should_media_items() {
        let res = AniplayContentSupplier
            .load_media_items("21".into(), vec![])
            // .load_media_items("170942".into(), vec![])
            .await;

        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items() {
        let res = AniplayContentSupplier
            .load_media_item_sources(
                "176508".into(),
                vec![
                    "1".into(),
                    "yuki".into(),
                    "shangri-la-frontier-season-2-19324?ep=128608".into(),
                    "1".into(),
                    "anya".into(),
                    "shangri-la-frontier-kusoge-hunter-kamige-ni-idoman-to-su-2nd-season-episode-1"
                        .into(),
                    "1".into(),
                ],
            )
            .await;

        println!("{res:#?}");
    }
}
