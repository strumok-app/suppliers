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

pub struct AniTubeContentSupplier {
    channels_map: IndexMap<&'static str, String>,
    processor_content_info_items: html::ItemsProcessor<ContentInfo>,
    processor_content_details: html::ScopeProcessor<ContentDetails>,
    regexp_dle_hash: Regex,
}

impl Default for AniTubeContentSupplier {
    fn default() -> Self {
        Self {
            channels_map: IndexMap::from([("Новинки", format!("{URL}/anime/page/"))]),
            processor_content_info_items: html::ItemsProcessor::new(
                "article.story",
                content_info_processor(),
            ),
            processor_content_details: html::ScopeProcessor::new(
                "div.content",
                html::ContentDetailsProcessor {
                    media_type: MediaType::Video,
                    title: html::TextValue::new()
                        .map(|s| utils::text::sanitize_text(&s))
                        .in_scope(".story_c > .rcol > h2")
                        .unwrap_or_default()
                        .boxed(),
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
                            .map(utils::text::sanitize_text)
                            .filter(|s| !s.is_empty() && !s.starts_with("."))
                            .collect();

                        if res.len() <= 5 {
                            return vec![];
                        }

                        res.drain(2..(res.len() - 3)).collect()
                    })
                    .in_scope(".story_c > .rcol")
                    .unwrap_or_default()
                    .boxed(),
                    similar: html::items_processor(
                        "ul.portfolio_items > li",
                        html::ContentInfoProcessor {
                            id: html::AttrValue::new("href")
                                .map_optional(|s| datalife::extract_id_from_url(URL, s))
                                .in_scope_flatten(".sl_poster > a")
                                .unwrap_or_default()
                                .boxed(),
                            title: html::text_value(".text_content > a"),
                            secondary_title: html::default_value(),
                            image: html::ExtractValue::new(|el| {
                                el.attr("src")
                                    .or(el.attr("data-src"))
                                    .unwrap_or_default()
                                    .to_owned()
                            })
                            .in_scope(".sl_poster img")
                            .map_optional(extract_image)
                            .unwrap_or_default()
                            .boxed(),
                        }
                        .boxed(),
                    ),
                    params: html::default_value(),
                }
                .boxed(),
            ),
            regexp_dle_hash: Regex::new(r#"dle_login_hash\s+=\s+'(?<hash>[a-z0-9]+)'"#).unwrap(),
        }
    }
}

impl ContentSupplier for AniTubeContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        self.channels_map.keys().map(|&s| s.into()).collect()
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

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        utils::scrap_page(
            datalife::search_request(URL, query).query(&[("search_start", page.to_string())]),
            &self.processor_content_info_items,
        )
        .await
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let url = datalife::get_channel_url(&self.channels_map, channel, page)?;

        utils::scrap_page(
            utils::create_client().get(&url),
            &self.processor_content_info_items,
        )
        .await
    }

    async fn get_content_details(
        &self,
        id: &str,
        _langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        let url = datalife::format_id_from_url(URL, id);

        let html = utils::create_client()
            .get(&url)
            .send()
            .await?
            .text()
            .await?;

        let document = scraper::Html::parse_document(&html);
        let root = document.root_element();

        let mut maybe_details = self.processor_content_details.process(&root);

        if let Some(&mut ref mut details) = maybe_details.as_mut() {
            details.params = self.extract_params(&html).unwrap_or_default()
        }

        Ok(maybe_details)
    }

    async fn load_media_items(
        &self,
        id: &str,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        if params.len() != 1 {
            return Err(anyhow!("user hash expected"));
        }

        let news_id = id
            .split_once("-")
            .map(|(l, _)| l)
            .ok_or_else(|| anyhow!("unable to extract news_id"))?;

        let playlist_req = utils::create_client()
            .get(format!("{URL}/engine/ajax/playlists.php"))
            .query(&[
                ("xfield", "playlist"),
                ("news_id", news_id),
                ("user_hash", &params[0]),
            ])
            .header("Referer", URL);

        datalife::load_ajax_playlist(playlist_req).await
    }

    async fn load_media_item_sources(
        &self,
        _id: &str,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if !params.len().is_multiple_of(2) {
            return Err(anyhow!("Wrong params size"));
        }

        let mut results = vec![];
        for chunk in params.chunks(2) {
            let description = &chunk[0];
            let url = &chunk[1];

            let mut sources = playerjs::load_and_parse_playerjs_sources(
                utils::create_client().get(url),
                description,
            )
            .await
            .unwrap_or_default();
            results.append(&mut sources);
        }

        Ok(results)
    }
}

impl AniTubeContentSupplier {
    fn extract_params(&self, html: &str) -> Option<Vec<String>> {
        let hash = self
            .regexp_dle_hash
            .captures(html)
            .and_then(|c| c.name("hash"))
            .map(|m| m.as_str())?;

        Some(vec![hash.into()])
    }
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .map_optional(|s| datalife::extract_id_from_url(URL, s))
            .in_scope_flatten(".story_c > h2 > a")
            .unwrap_or_default()
            .boxed(),
        title: html::text_value(".story_c > h2 > a"),
        secondary_title: html::default_value(),
        image: html::self_hosted_image(URL, ".story_c_l img", "data-src"),
    }
    .into()
}

fn extract_image(src: String) -> String {
    if src.starts_with("http") {
        return src.to_string();
    }
    format!("{URL}{src}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn should_load_channel() {
        let res = AniTubeContentSupplier::default()
            .load_channel("Новинки", 2)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_search() {
        let res = AniTubeContentSupplier::default()
            .search("ball", 2)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_content_details() {
        let res = AniTubeContentSupplier::default()
            .get_content_details("31-zapisnik-smert", vec![])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items() {
        let res = AniTubeContentSupplier::default()
            .load_media_items(
                "5513-vanpan-3-sezon",
                vec![],
                vec!["867ca5be02de10b799c164d7b7c31e6eece1bb10".into()],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items_source() {
        let res = AniTubeContentSupplier::default()
            .load_media_item_sources(
                "5513-vanpan-3-sezon",
                vec![],
                vec![
                    // "ОЗВУЧЕННЯ FANVOXUA ПЛЕЄР ASHDI",
                    // "https://ashdi.vip/vod/213716",
                    // "ОЗВУЧЕННЯ FANVOXUA ПЛЕЄР MOON",
                    // "https://moonanime.art/iframe/ziroreraihcwpzdrtcu/",
                    "ОЗВУЧЕННЯ FANVOXUA ПЛЕЄР TRG",
                    "https://tortuga.tw/vod/119859",
                    // "ОЗВУЧЕННЯ Робота Голосом ПЛЕЄР MOON",
                    // "https://moonanime.art/iframe/nilgcqbfjpjswecgrhqzpfhqji/",
                    // "ОЗВУЧЕННЯ Робота Голосом ПЛЕЄР ASHDI",
                    // "https://ashdi.vip/vod/213732",
                    // "ОЗВУЧЕННЯ AMANOGAWA ПЛЕЄР MOON",
                    // "https://moonanime.art/iframe/peaqxhdbcojeofbw/",
                    "ОЗВУЧЕННЯ AMANOGAWA ПЛЕЄР TRG",
                    "https://tortuga.tw/vod/119858",
                    // "СУБТИТРИ Робота Голосом ПЛЕЄР MOON",
                    // "https://moonanime.art/iframe/ponbhrkhpxmsikczdgcyvodgimctlm/",
                ]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }
}
