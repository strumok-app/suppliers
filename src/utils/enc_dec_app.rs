use std::collections::{BTreeMap, HashMap};

use anyhow::Ok;
use serde::{Deserialize, Serialize};

use crate::{
    models::ContentMediaItem,
    utils::{GenericResponse, create_json_client, jwp_player::JWPConfig},
};

pub const ENC_DEC_APP_URL: &str = "https://enc-dec.app";

// Common response for both enc and dec

#[derive(Debug, Serialize)]
struct GenericRequest {
    text: String,
}

#[derive(Debug, Deserialize)]
struct DecResponse {
    result: DecResult,
}

#[derive(Debug, Deserialize)]
struct DecResult {
    url: String,
}

// Animekai DB section

#[derive(Debug, Deserialize)]
struct KaiDBFind {
    episodes: HashMap<u32, HashMap<u32, KaiDBEpisode>>,
}

#[derive(Debug, Deserialize)]
struct KaiDBEpisode {
    title: Option<String>,
    token: String,
}

pub enum KaiBDId {
    KaiId,
    AniId,
}

pub async fn kai_enc(text: &str) -> anyhow::Result<String> {
    let url = format!("{ENC_DEC_APP_URL}/api/enc-kai?text={text}");

    let res_str = create_json_client().get(url).send().await?.text().await?;

    let res: GenericResponse = serde_json::from_str(&res_str)?;

    Ok(res.result)
}

pub async fn kai_dec(text: &str) -> anyhow::Result<String> {
    let url = format!("{ENC_DEC_APP_URL}/api/dec-kai");

    let res_str = create_json_client()
        .post(url)
        .json(&GenericRequest {
            text: text.to_string(),
        })
        .send()
        .await?
        .text()
        .await?;

    // println!("{res_str}");

    let res: DecResponse = serde_json::from_str(&res_str)?;

    Ok(res.result.url)
}

pub async fn kai_db_find(id: KaiBDId, id_val: &str) -> anyhow::Result<Vec<ContentMediaItem>> {
    let id_name = match id {
        KaiBDId::AniId => "ani_id",
        KaiBDId::KaiId => "kai_id",
    };

    let url = format!("{ENC_DEC_APP_URL}/db/kai/find?{id_name}={id_val}");

    let kai_db_res_str = create_json_client().get(url).send().await?.text().await?;

    let kai_db_res: Vec<KaiDBFind> = serde_json::from_str(&kai_db_res_str)?;

    let mut sorted_media_items: BTreeMap<u32, ContentMediaItem> = BTreeMap::new();
    if let Some(kai_db_item) = kai_db_res.into_iter().next() {
        let seasons = kai_db_item.episodes;
        let has_multiple_seasons = seasons.len() > 1;

        for (season, episodes) in seasons.into_iter() {
            for (ep_num, episode) in episodes.into_iter() {
                let title = match episode.title {
                    Some(title) => format!("{}. {}", ep_num, title),
                    None => format!("{ep_num}."),
                };

                let section = if has_multiple_seasons {
                    Some(season.to_string())
                } else {
                    None
                };

                let key = season * 1000 + ep_num;

                sorted_media_items.insert(
                    key,
                    ContentMediaItem {
                        title,
                        section,
                        image: None,
                        sources: None,
                        params: vec![episode.token],
                    },
                );
            }
        }
    }

    Ok(sorted_media_items.into_values().collect())
}

// MegaDec section

#[derive(Debug, Serialize)]
struct PlayerDecRequest {
    text: String,
    agent: String,
}

#[derive(Debug, Deserialize)]
struct PlayerDecResponse {
    result: JWPConfig,
}

pub async fn mega_dec(text: &str, user_agent: &str) -> anyhow::Result<JWPConfig> {
    player_dec("mega", text, user_agent).await
}

pub async fn rapid_dec(text: &str, user_agent: &str) -> anyhow::Result<JWPConfig> {
    player_dec("rapid", text, user_agent).await
}

async fn player_dec(palyer: &str, text: &str, user_agent: &str) -> anyhow::Result<JWPConfig> {
    let url = format!("{ENC_DEC_APP_URL}/api/dec-{palyer}");

    let client = create_json_client();
    let mega_dec_res_str = client
        .post(&url)
        .json(&PlayerDecRequest {
            text: text.to_string(),
            agent: user_agent.to_string(),
        })
        .send()
        .await?
        .text()
        .await?;

    let mega_dec_res: PlayerDecResponse = serde_json::from_str(&mega_dec_res_str)?;

    Ok(mega_dec_res.result)
}

// Flix section

#[derive(Debug, Deserialize)]
struct FlixFindTV {
    episodes: HashMap<u32, HashMap<u32, FlixEpisode>>,
}

#[derive(Debug, Deserialize)]
struct FlixEpisode {
    title: Option<String>,
    eid: String,
}

pub async fn flix_db_find_tv(tmdb_id: u32, s: u32, e: u32) -> anyhow::Result<Option<String>> {
    flix_db_find(tmdb_id, s, e, true).await
}

pub async fn flix_db_find_movie(tmdb_id: u32) -> anyhow::Result<Option<String>> {
    flix_db_find(tmdb_id, 1, 1, false).await
}

async fn flix_db_find(tmdb_id: u32, s: u32, e: u32, tv: bool) -> anyhow::Result<Option<String>> {
    let t = if tv { "tv" } else { "movie" };
    let url = format!("{ENC_DEC_APP_URL}/db/flix/find?tmdb_id={tmdb_id}&type={t}");

    let flix_db_res_str = create_json_client().get(url).send().await?.text().await?;

    // println!("{flix_db_res_str}");

    let flix_db_res: Vec<FlixFindTV> = serde_json::from_str(&flix_db_res_str)?;

    let maybe_eid = flix_db_res.into_iter().next().and_then(|flix_find| {
        flix_find
            .episodes
            .get(&s)
            .and_then(|season| season.get(&e))
            .map(|episode| episode.eid.clone())
    });

    Ok(maybe_eid)
}

pub async fn flix_enc(text: &str) -> anyhow::Result<String> {
    let url = format!("{ENC_DEC_APP_URL}/api/enc-movies-flix?text={text}");

    let res_str = create_json_client().get(url).send().await?.text().await?;

    let res: GenericResponse = serde_json::from_str(&res_str)?;

    Ok(res.result)
}

pub async fn flix_dec(text: &str) -> anyhow::Result<String> {
    let url = format!("{ENC_DEC_APP_URL}/api/dec-movies-flix");

    let res_str = create_json_client()
        .post(url)
        .json(&GenericRequest {
            text: text.to_string(),
        })
        .send()
        .await?
        .text()
        .await?;

    // println!("{res_str}");

    let res: DecResponse = serde_json::from_str(&res_str)?;

    Ok(res.result.url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_get_episodes_tokens() {
        let res = kai_db_find(KaiBDId::KaiId, "d4W59g").await;
        println!("{res:?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_encode_kai() {
        let res = kai_enc("Jte4p_jlugjhm3QQ0MuI").await;
        println!("{res:?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_decode_kai() {
        let res = kai_dec("KKr_fsLSeN3qUntIt18i1b9SB7cZjMkUBwyEEISmuMJwbflEUa_vXwDsFw2KkwJe8XOqoOLG_aG_hMhCKJITK6lmqJDRHGi4zwMgeNY0JBhGqZs_VUJG4USp2qfZ1GxzWMHlNXF3bpHoqr9cpeFZLADzzmTxp7dL9IS6j")
            .await;
        println!("{res:?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_find_flix_tv() {
        let res = flix_db_find_tv(1399, 2, 2).await;
        println!("{res:?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_find_flix_movie() {
        let res = flix_db_find_movie(176).await;
        println!("{res:?}");
    }
}
