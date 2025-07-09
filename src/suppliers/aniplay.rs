use std::collections::{BTreeMap, HashMap};

use anyhow::anyhow;
use base64::{prelude::BASE64_STANDARD, Engine};
use log::warn;
use reqwest::Client;
use serde::Deserialize;

use crate::{
    models::{ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType},
    utils::{anilist, create_json_client, crypto_js::decrypt_aes_no_salt},
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
        #[derive(Deserialize, Debug)]
        struct AniplayEpisode {
            id: String,
            number: f32,
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
            #[serde(rename = "providerId")]
            provider_id: String,
        }

        #[derive(Deserialize, Debug)]
        struct ApiplayEpisodesResponse {
            #[serde(rename = "episodes")]
            servers: Vec<AniplayServer>,
        }

        let res_str = create_json_client()
            .get(format!("{URL}/api/anime/episodes"))
            .query(&[
                ("id", id.as_str()),
                ("releasing", "false"),
                ("refresh", "false"),
            ])
            .header("Referer", format!("{URL}/anime/info/{id}"))
            .send()
            .await?
            .text()
            .await?;

        

        let res: ApiplayEpisodesResponse = serde_json::from_str(&res_str)?;

        let mut sorted_media_items: BTreeMap<i32, ContentMediaItem> = BTreeMap::new();

        for server in res.servers {
            let provider = server.provider_id;
            for episode in server.episodes {
                let key = (episode.number * 100.0) as i32; // fucking rust f32 is not orderer!
                                                           // Its insame stupidity!
                let media_item = sorted_media_items.entry(key).or_insert_with(|| {
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

        let client = create_json_client();

        for params in server_params.chunks(3) {
            if langs.contains(&"en".to_string()) && params[2] == "1" {
                let mut sources = load_server_media_item_sources(
                    client, &id, params[0], params[1], ep_number, "dub",
                )
                .await;

                results.append(&mut sources);
            }
            if langs.contains(&"ja".to_string()) {
                let mut sources = load_server_media_item_sources(
                    client, &id, params[0], params[1], ep_number, "sub",
                )
                .await;

                results.append(&mut sources);
            }
        }

        Ok(results)
    }
}

async fn load_server_media_item_sources(
    client: &Client,
    id: &str,
    provider: &str,
    ep_id: &str,
    ep_number: &str,
    r#type: &str,
) -> Vec<ContentMediaItemSource> {
    let res = load_server_by_type(client, id, provider, ep_id, ep_number, r#type).await;

    match res {
        Ok(sources) => sources,
        Err(err) => {
            warn!("[aniplay] fail to load server source(id: {id}, provider: {provider}, ep_id: {ep_id}, ep_numeber: {ep_number}, {type}): {err}");
            vec![]
        }
    }
}

async fn load_server_by_type(
    client: &Client,
    id: &str,
    provider: &str,
    ep_id: &str,
    ep_number: &str,
    r#type: &str,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    #[derive(Deserialize, Debug)]
    struct ServerSubtitle {
        url: String,
        lang: String,
    }

    #[derive(Deserialize, Debug)]
    struct ServerSource {
        url: String,
    }

    #[derive(Deserialize, Debug)]
    struct ServerRes {
        headers: Option<HashMap<String, String>>,
        #[serde(default)]
        sources: Vec<ServerSource>,
        #[serde(default)]
        subtitles: Vec<ServerSubtitle>,
    }

    let res_str = client
        .get(format!("{URL}/api/anime/sources"))
        .query(&[
            ("id", id),
            ("provider", provider),
            ("epId", ep_id),
            ("epNum", ep_number),
            ("subType", r#type),
            ("cache", "true"),
        ])
        .header("Referer", format!("{URL}/anime/watch/{id}"))
        .send()
        .await?
        .text()
        .await?;

    

    let res_b64 = BASE64_STANDARD.decode(&res_str)?;
    let res_dec_str = decrypt_aes_no_salt(&[], &res_b64)?;

    

    let res: ServerRes = serde_json::from_str(&res_dec_str)?;

    let prefix = format!("[{type}] {provider}");

    let mut sources: Vec<ContentMediaItemSource> = vec![];

    res.sources.iter().enumerate().for_each(|(idx, source)| {
        let num = idx + 1;
        let description = format!("{prefix} {num}.");

        sources.push(ContentMediaItemSource::Video {
            link: String::from(&source.url),
            headers: res.headers.clone(),
            description,
        });
    });

    res.subtitles.iter().enumerate().for_each(|(idx, sub)| {
        let num = idx + 1;
        let language = &sub.lang;
        let description = format!("{prefix} {num} {language}.");

        sources.push(ContentMediaItemSource::Subtitle {
            link: String::from(&sub.url),
            headers: None,
            description,
        });
    });

    Ok(sources)
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn should_load_media_items() {
        let res = AniplayContentSupplier
            .load_media_items("177709".into(), vec![], vec![])
            // .load_media_items("170942".into(), vec![])
            .await;

        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_load_media_items_sources() {
        let res = AniplayContentSupplier
            .load_media_item_sources(
                "151807".into(),
                vec!["ja".to_owned(), "en".to_owned()],
                vec![
                    "1",
                    "maze",
                    "/sakamoto-days-171/epi-1-1810/",
                    "1",
                    "pahe",
                    "67736-5857",
                    "1",
                    "yuki",
                    "sakamoto-days-19431?ep=131796",
                    "1",
                    "akane",
                    "sakamoto-days-19431?ep=131796",
                    "1",
                    "owl",
                    "sakamoto-days-2$episode-227706&episode-227713",
                    "1",
                    "koto",
                    "131796",
                    "1",
                ]
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
            )
            .await;

        println!("{res:#?}");
    }
}
