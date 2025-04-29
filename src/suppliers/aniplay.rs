use std::{
    collections::{BTreeMap, HashMap},
    str, vec,
};

use anyhow::anyhow;
use log::warn;
use serde::Deserialize;
use serde_json::json;

use crate::{
    models::{ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType},
    utils::{anilist, jwp_player::Source, nextjs},
};

use super::ContentSupplier;

const URL: &str = "https://aniplaynow.live";
const SERVERS_NEXT_ACTION_ID: &str = "7f965ff19ca58fbad3efbf74125a419a92abe784ab";
const SOURCES_NEXT_ACTION_ID: &str = "7f50a8ca3b4c8348d34c6410f89ca2d4edc30da540";

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

    async fn search(&self, query: String, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        anilist::search_anime(&query, page).await
    }

    async fn load_channel(&self, _channel: String, _page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        Err(anyhow!("unimplemented"))
    }

    async fn get_content_details(
        &self,
        id: String,
        _langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        anilist::get_anime(&id).await
    }

    async fn load_media_items(
        &self,
        id: String,
        _langs: Vec<String>,
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

        let servers: Vec<AniplayServer> =
            nextjs::server_action(url.as_str(), SERVERS_NEXT_ACTION_ID, 1, &json!([id, true,]))
                .await?;

        let mut sorted_media_items: BTreeMap<u32, ContentMediaItem> = BTreeMap::new();

        for server in servers {
            let provider = server.provider_id;
            for episode in server.episodes {
                let media_item = sorted_media_items.entry(episode.number).or_insert_with(|| {
                    let num = episode.number;
                    let title = episode.title;
                    ContentMediaItem {
                        title: format!("{num}. {title}"),
                        section: None,
                        image: None,
                        sources: None,
                        params: vec![episode.number.to_string()],
                    }
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
        langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.len() < 4 {
            return Err(anyhow!("Incorrect params"));
        }

        let ep_number = &params[0];
        let server_params: Vec<_> = params.iter().skip(1).collect();
        if server_params.len() % 3 != 0 {
            return Err(anyhow!("Incorrect params"));
        }

        let mut results = vec![];

        for params in server_params.chunks(3) {
            if langs.contains(&"en".to_string()) && params[2] == "1" {
                let mut sources =
                    load_server_media_item_sources(&id, params[0], params[1], ep_number, "dub")
                        .await;

                results.append(&mut sources);
            }
            if langs.contains(&"ja".to_string()) {
                let mut sources =
                    load_server_media_item_sources(&id, params[0], params[1], ep_number, "sub")
                        .await;

                results.append(&mut sources);
            }
        }

        Ok(results)
    }
}

async fn load_server_media_item_sources(
    id: &str,
    provider: &str,
    ep_id: &str,
    ep_number: &str,
    r#type: &str,
) -> Vec<ContentMediaItemSource> {
    let res = load_server_by_type(id, provider, ep_id, ep_number, r#type).await;

    match res {
        Ok(sources) => sources,
        Err(err) => {
            warn!("[aniplay] fail to load server source(id: {id}, provider: {provider}, ep_id: {ep_id}, ep_numeber: {ep_number}, {type}): {err}");
            vec![]
        }
    }
}

async fn load_server_by_type(
    id: &str,
    provider: &str,
    ep_id: &str,
    ep_number: &str,
    r#type: &str,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let url = format!("{URL}/anime/watch/{id}?host={provider}&ep={ep_number}&type={type}");

    let res: ServerRes = nextjs::server_action(
        &url,
        SOURCES_NEXT_ACTION_ID,
        1,
        &json!([id, provider, ep_id, ep_number, r#type,]),
    )
    .await?;

    let prefix = format!("[{type}] {provider}");

    let sources: Vec<_> = res
        .sources
        .iter()
        .enumerate()
        .map(|(idx, source)| {
            let num = idx + 1;
            let mut description = format!("{prefix} {num}.");

            if let Some(label) = &source.label {
                description.push(' ');
                description.push_str(label);
            }

            ContentMediaItemSource::Video {
                link: String::from(&source.file),
                headers: res.headers.clone(),
                description,
            }
        })
        .collect();

    Ok(sources)
}

#[derive(Deserialize, Debug)]
struct ServerRes {
    headers: Option<HashMap<String, String>>,
    #[serde(default)]
    sources: Vec<Source>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn should_load_media_items() {
        let res = AniplayContentSupplier
            .load_media_items("151807".into(), vec![], vec![])
            // .load_media_items("170942".into(), vec![])
            .await;

        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items_sources() {
        let res = AniplayContentSupplier
            .load_media_item_sources(
                "151807".into(),
                vec!["ja".to_owned(), "en".to_owned()],
                vec![
                    "12".to_owned(),
                    "maze".to_owned(),
                    "solo-leveling-310/epi-12-80446".to_owned(),
                    "1".to_owned(),
                    "yuki".to_owned(),
                    "solo-leveling-18718?ep=123078".to_owned(),
                    "1".to_owned(),
                    "pahe".to_owned(),
                    "62289-5421".to_owned(),
                    "1".to_owned(),
                ],
            )
            .await;

        println!("{res:#?}");
    }
}
