use std::sync::OnceLock;

use indexmap::IndexMap;
use regex::Regex;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    utils::{
        self, datalife,
        html::{self, DOMProcessor},
        playerjs,
    },
};

use super::ContentSupplier;

use anyhow::anyhow;

const URL: &str = "https://anitube.in.ua";

#[derive(Default)]
pub struct AniTubeContentSupplier;

impl ContentSupplier for AniTubeContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|s| s.into()).collect()
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![ContentType::Anime]
    }

    fn get_supported_languages(&self) -> Vec<String> {
        vec!["uk".into()]
    }

    async fn search(&self, query: String) -> anyhow::Result<Vec<ContentInfo>> {
        utils::scrap_page(
            datalife::search_request(URL, &query),
            content_info_items_processor(),
        )
        .await
    }

    async fn load_channel(&self, channel: String, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let url = datalife::get_channel_url(get_channels_map(), &channel, page)?;

        utils::scrap_page(
            utils::create_client().get(&url),
            content_info_items_processor(),
        )
        .await
    }

    async fn get_content_details(&self, id: String) -> anyhow::Result<Option<ContentDetails>> {
        let url = datalife::format_id_from_url(URL, &id);

        let html = utils::create_client()
            .get(&url)
            .send()
            .await?
            .text()
            .await?;

        let document = scraper::Html::parse_document(&html);
        let root = document.root_element();

        let mut maybe_details = content_details_processor().process(&root);

        if let Some(&mut ref mut details) = maybe_details.as_mut() {
            details.params = extract_params(&id, &html).unwrap_or_default()
        }

        Ok(maybe_details)
    }

    async fn load_media_items(
        &self,
        _id: String,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        if params.len() != 2 {
            return Err(anyhow!("news_id and user hash expected"));
        }

        let playlist_req = utils::create_client()
            .get(format!("{URL}/engine/ajax/playlists.php"))
            .query(&[("xfield", "playlist")])
            .query(&[("news_id", &params[0]), ("user_hash", &params[1])])
            .header("Referer", URL);

        datalife::load_ajax_playlist(playlist_req).await
    }

    async fn load_media_item_sources(
        &self,
        _id: String,
        mut params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.len() % 2 != 0 {
            return Err(anyhow!("Wrong params size"));
        }

        let mut results = vec![];
        while !params.is_empty() {
            let description = params.remove(0);
            let url = params.remove(0);

            let mut sources = playerjs::load_and_parse_playerjs_sources(&description, &url)
                .await
                .unwrap_or_default();
            results.append(&mut sources);
        }

        Ok(results)
    }
}

fn extract_params(id: &str, html: &str) -> Option<Vec<String>> {
    static DLE_HASH_REGEXP: OnceLock<regex::Regex> = OnceLock::new();
    let dle_hash_re = DLE_HASH_REGEXP
        .get_or_init(|| Regex::new(r#"dle_login_hash\s+=\s+'(?<hash>[a-z0-9]+)'"#).unwrap());

    let (news_id, _) = id.split_once("-")?;
    let hash = dle_hash_re
        .captures(html)
        .and_then(|c| c.name("hash"))
        .map(|m| m.as_str())?;

    Some(vec![news_id.into(), hash.into()])
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .map(|s| datalife::extract_id_from_url(URL, s))
            .in_scope(".story_c > h2 > a")
            .unwrap_or_default()
            .into(),
        title: html::text_value(".story_c > h2 > a"),
        secondary_title: html::default_value(),
        image: html::self_hosted_image(URL, ".story_c_l img", "src"),
    }
    .into()
}

fn content_info_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR
        .get_or_init(|| html::ItemsProcessor::new("article.story", content_info_processor()))
}

fn content_details_processor() -> &'static html::ScopeProcessor<ContentDetails> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopeProcessor<ContentDetails>> =
        OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        html::ScopeProcessor::new(
            "div.content",
            html::ContentDetailsProcessor {
                media_type: MediaType::Video,
                title: html::TextValue::new()
                    .map(|s| html::sanitize_text(&s))
                    .in_scope(".story_c > .rcol > h2")
                    .unwrap_or_default()
                    .into(),
                original_title: html::default_value(),
                image: html::self_hosted_image(URL, ".story_c .story_post img", "src"),
                description: html::text_value(
                    ".story_c > .rcol > .story_c_r > .story_c_text > .my-text",
                ),
                additional_info: html::ExtractValue::new(|el| {
                    let mut res: Vec<_> = el
                        .text()
                        .collect::<String>()
                        .split("\n")
                        .map(html::sanitize_text)
                        .filter(|s| !s.is_empty() && !s.starts_with("."))
                        .collect();

                    if res.len() <= 5 {
                        return vec![];
                    }

                    res.drain(2..(res.len() - 3)).collect()
                })
                .in_scope(".story_c > .rcol")
                .unwrap_or_default()
                .into(),
                similar: html::items_processor(
                    "ul.portfolio_items > li",
                    html::ContentInfoProcessor {
                        id: html::AttrValue::new("href")
                            .map(|s| datalife::extract_id_from_url(URL, s))
                            .in_scope(".sl_poster > a")
                            .unwrap_or_default()
                            .into(),
                        title: html::text_value(".text_content > a"),
                        secondary_title: html::default_value(),
                        image: html::ExtractValue::new(|el| {
                            el.attr("src")
                                .or(el.attr("data-src"))
                                .unwrap_or_default()
                                .to_owned()
                        })
                        .in_scope(".sl_poster img")
                        .map_optional(move |src| format!("{URL}{src}"))
                        .flatten()
                        .into(),
                    }
                    .into(),
                ),
                params: html::default_value(),
            }
            .into(),
        )
    })
}

fn get_channels_map() -> &'static IndexMap<String, String> {
    static CHANNELS_MAP: OnceLock<IndexMap<String, String>> = OnceLock::new();
    CHANNELS_MAP.get_or_init(|| IndexMap::from([("Новинки".into(), format!("{URL}/anime/page/"))]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn should_load_channel() {
        let res = AniTubeContentSupplier
            .load_channel("Новинки".into(), 2)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_search() {
        let res = AniTubeContentSupplier
            .search("Доктор Стоун".into())
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_content_details() {
        let res = AniTubeContentSupplier
            .get_content_details("3419-dokor-kamin".into())
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items() {
        let res = AniTubeContentSupplier
            .load_media_items(
                "7633-dr-stone-4".into(),
                vec![
                    "3419".into(),
                    "fa06e9031e506c6f56099b6500b0613e50a60656".into(),
                ],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items_source() {
        let res = AniTubeContentSupplier
            .load_media_item_sources(
                "7633-dr-stone-4".into(),
                vec![
                    "ОЗВУЧУВАННЯ DZUSKI ПЛЕЄР ASHDI".into(),
                    "https://ashdi.vip/vod/43190".into(),
                    "ОЗВУЧУВАННЯ DZUSKI ПЛЕЄР TRG".into(),
                    "https://tortuga.tw/vod/10654".into(),
                    // "ОЗВУЧУВАННЯ Togarashi ПЛЕЄР MOON".into(),
                    // "https://moonanime.art/iframe/qcsyutjdkhtucmzxdmmw".into(),
                    // "ОЗВУЧУВАННЯ Togarashi ПЛЕЄР МОНСТР ".into(),
                    // "https://mmonstro.site/embed/649292".into(),
                    "СУБТИТРИ СУБТИТРИ ПЛЕЄР МОНСТР ".into(),
                    "https://mmonstro.site/embed/704444/".into(),
                ],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }
}
