use std::{collections::HashMap, sync::OnceLock};

use regex::Regex;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::utils::html::ContentInfoProcessor,
};

use super::{
    utils::{
        self, datalife,
        html::{self, DOMProcessor},
        playerjs,
    },
    ContentSupplier,
};

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

    async fn search(
        &self,
        query: String,
        _types: Vec<String>,
    ) -> Result<Vec<ContentInfo>, anyhow::Error> {
        utils::scrap_page(
            datalife::search_request(URL, &query),
            content_info_items_processor(),
        )
        .await
    }

    async fn load_channel(
        &self,
        channel: String,
        page: u16,
    ) -> Result<Vec<ContentInfo>, anyhow::Error> {
        let url = datalife::get_channel_url(get_channels_map(), &channel, page)?;

        utils::scrap_page(
            utils::create_client().get(&url),
            content_info_items_processor(),
        )
        .await
    }

    async fn get_content_details(
        &self,
        id: String,
    ) -> Result<Option<ContentDetails>, anyhow::Error> {
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
    ) -> Result<Vec<ContentMediaItem>, anyhow::Error> {
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
    ) -> Result<Vec<ContentMediaItemSource>, anyhow::Error> {
        if params.len() % 2 != 0 {
            return Err(anyhow!("Wrong params size"));
        }

        let mut results = vec![];
        while !params.is_empty() {
            let description = params.remove(0);
            let url = params.remove(0);

            let mut sources = playerjs::load_and_parse_playerjs_sources(&description, &url).await?;
            results.append(&mut sources);
        }

        Ok(results)
    }
}

fn extract_params(id: &String, html: &String) -> Option<Vec<String>> {
    static DLE_HASH_REGEXP: OnceLock<regex::Regex> = OnceLock::new();
    let dle_hash_re = DLE_HASH_REGEXP
        .get_or_init(|| Regex::new(r#"dle_login_hash\s+=\s+'(?<hash>[a-z0-9]+)'"#).unwrap());

    let (news_id, _) = id.split_once("-")?;
    let hash = dle_hash_re
        .captures(&html)
        .map(|c| c.name("hash"))
        .flatten()
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
        secondary_title: html::default_value::<Option<String>>(),
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
                    .map(|s| html::sanitize_text(s))
                    .in_scope(".story_c > .rcol > h2")
                    .unwrap_or_default()
                    .into(),
                original_title: html::default_value::<Option<String>>(),
                image: html::self_hosted_image(URL, ".story_c .story_post img", "src"),
                description: html::text_value(
                    ".story_c > .rcol > .story_c_r > .story_c_text > .my-text",
                ),
                additional_info: html::ExtractValue::new(|el| {
                    let mut res: Vec<_> = el
                        .text()
                        .collect::<String>()
                        .split("\n")
                        .map(|s| html::sanitize_text(s.to_owned()))
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
                    ContentInfoProcessor {
                        id: html::AttrValue::new("href")
                            .map(|s| datalife::extract_id_from_url(URL, s))
                            .in_scope(".sl_poster > a")
                            .unwrap_or_default()
                            .into(),
                        title: html::text_value(".text_content > a"),
                        secondary_title: html::default_value::<Option<String>>(),
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
                params: html::default_value::<Vec<String>>(),
            }
            .into(),
        )
    })
}

fn get_channels_map() -> &'static HashMap<String, String> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<HashMap<String, String>> = OnceLock::new();
    CONTENT_DETAILS_PROCESSOR
        .get_or_init(|| HashMap::from([("Новинки".into(), format!("{URL}/anime/page/"))]))
}
