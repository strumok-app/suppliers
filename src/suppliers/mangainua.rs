use std::{collections::HashMap, sync::OnceLock};

use anyhow::{anyhow, Ok};
use indexmap::IndexMap;
use regex::Regex;
use scraper::{selectable::Selectable, Selector};

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    utils::{
        self, datalife,
        html::{self, DOMProcessor},
    },
};

use super::{ContentSupplier, MangaPagesLoader};

const URL: &str = "https://manga.in.ua";

#[derive(Default)]
pub struct MangaInUaContentSupplier;

impl ContentSupplier for MangaInUaContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|&s| s.to_string()).collect()
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![ContentType::Manga]
    }

    fn get_supported_languages(&self) -> Vec<String> {
        vec!["uk".to_string()]
    }

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        if page > 1 {
            return Ok(vec![]);
        }

        utils::scrap_page(
            datalife::search_request(URL, query),
            content_info_items_processor(),
        )
        .await
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let url = datalife::get_channel_url(get_channels_map(), channel, page)?;

        utils::scrap_page(
            utils::create_client().get(&url),
            content_info_items_processor(),
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

        let mut maybe_details = content_details_processor().process(&root);

        if let Some(&mut ref mut details) = maybe_details.as_mut() {
            details.params = extract_params(id, &html)?;
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
            .rsplit_once("/")
            .and_then(|(_, r)| r.split_once("-"))
            .map(|(l, _)| l)
            .ok_or_else(|| anyhow!("cant extract news_id for id: {}", id))?;

        let user_hash = &params[0];

        let client = utils::create_json_client();

        let mut form_params = HashMap::new();
        form_params.insert("action", "show");
        form_params.insert("news_id", news_id);
        form_params.insert("user_hash", user_hash);
        form_params.insert("news_category", "1");
        form_params.insert("this_link", "");

        let chaptes_list_html = client
            .post(format!("{URL}/engine/ajax/controller.php"))
            .query(&[("mod", "load_chapters")])
            .form(&form_params)
            .header(
                "Content-Type",
                "application/x-www-form-urlencoded; charset=UTF-8",
            )
            .send()
            .await?
            .text()
            .await?;

        let fragment = scraper::Html::parse_fragment(&chaptes_list_html);

        let title_sel = Selector::parse("a").unwrap();

        let result = fragment
            .root_element()
            .child_elements()
            .filter_map(|el| {
                let maybe_volume = el.attr("manga-tom");
                let chapter = el.attr("manga-chappter")?;

                let title_el = el.select(&title_sel).next()?;
                let title = title_el.text().next()?;

                Some(ContentMediaItem {
                    section: maybe_volume.map(|s| s.to_string()),
                    title: title.to_string(),
                    image: None,
                    params: vec![user_hash.clone(), chapter.to_string()],
                    sources: None,
                })
            })
            .collect();

        Ok(result)
    }

    async fn load_media_item_sources(
        &self,
        id: &str,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.len() < 2 {
            return Err(anyhow!("invalid params number"));
        }

        let last_id_part = id.rsplit_once("/").map(|(_, l)| l).unwrap_or_default();
        let this_url = format!("{URL}/chapters/{last_id_part}.html");

        let news_id = last_id_part
            .split_once("-")
            .map(|(l, _)| l)
            .ok_or_else(|| anyhow!("cant extract news_id for id: {}", id))?;

        let user_hash = &params[0];
        let chapter = &params[1];

        let client = utils::create_json_client();

        let mut form_params = HashMap::new();
        form_params.insert("action", "show");
        form_params.insert("news_id", news_id);
        form_params.insert("user_hash", user_hash);
        form_params.insert("news_category", "54");
        form_params.insert("this_link", &this_url);

        let translators_list = client
            .post(format!("{URL}/engine/ajax/controller.php"))
            .query(&[("mod", "load_chapters")])
            .form(&form_params)
            .header(
                "Content-Type",
                "application/x-www-form-urlencoded; charset=UTF-8",
            )
            .send()
            .await?
            .text()
            .await?;

        let fragment = scraper::Html::parse_fragment(&translators_list);

        let result: Vec<_> = fragment
            .root_element()
            .child_elements()
            .filter_map(|el| {
                let translator = el.attr("data-translator").unwrap_or_default();
                let translator_num = el.attr("data-chapter").unwrap_or_default();

                let translator_news_id = el
                    .attr("value")
                    .and_then(|link| link.rsplit_once("/"))
                    .and_then(|(_, r)| r.split_once("-"))
                    .map(|(l, _)| l)?;

                Some(ContentMediaItemSource::Manga {
                    description: translator.to_string(),
                    headers: None,
                    pages: None,
                    params: vec![
                        translator_news_id.to_string(),
                        user_hash.to_string(),
                        chapter.to_string(),
                        translator_num.to_string(),
                    ],
                })
            })
            .collect();

        Ok(result)
    }
}

impl MangaPagesLoader for MangaInUaContentSupplier {
    async fn load_pages(&self, _id: &str, params: Vec<String>) -> anyhow::Result<Vec<String>> {
        if params.len() < 4 {
            return Err(anyhow!("invalid params number"));
        }

        let news_id = &params[0];
        let user_hash = &params[1];
        let chapter = &params[2];
        let translator = &params[3];
        let cookie_header = format!("lastchapp={news_id}|{chapter}|{translator}");

        let client = utils::create_json_client();

        let pages_list = client
            .get(format!("{URL}/engine/ajax/controller.php"))
            .query(&[
                ("mod", "load_chapters_image"),
                ("news_id", news_id),
                ("user_hash", user_hash),
                ("action", "show"),
            ])
            .header("Cookie", cookie_header)
            .send()
            .await?
            .text()
            .await?;

        // println!("{pages_list:#?}");

        let fragment = scraper::Html::parse_fragment(&pages_list);

        let img_sel = Selector::parse("img").unwrap();
        let pages: Vec<_> = fragment
            .root_element()
            .select(&img_sel)
            .filter_map(|el| el.attr("data-src"))
            .map(|link| link.to_string())
            .collect();

        Ok(pages)
    }
}

fn get_channels_map() -> &'static IndexMap<&'static str, String> {
    static CHANNELS_MAP: OnceLock<IndexMap<&'static str, String>> = OnceLock::new();
    CHANNELS_MAP.get_or_init(|| {
        IndexMap::from([
            ("Новинки", URL.to_string()),
            ("Манґа", format!("{URL}/xfsearch/type/manga/")),
            ("Манхва", format!("{URL}/xfsearch/type/manhwa/")),
            ("Романтика", format!("{URL}/mangas/romantika/")),
            ("Драма", format!("{URL}/mangas/drama/")),
            ("Комедія", format!("{URL}/mangas/komedia/")),
            ("Буденність", format!("{URL}/mangas/budenst/")),
            ("Фентезі", format!("{URL}/mangas/fentez/")),
            ("Школа", format!("{URL}/mangas/shkola/")),
            ("Надприродне", format!("{URL}/mangas/nadprirodne/")),
            ("Пригоди", format!("{URL}/mangas/prigodi/")),
            ("Бойовик", format!("{URL}/mangas/boyovik/")),
            ("Психологія", format!("{URL}/mangas/psihologia/")),
        ])
    })
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .map_optional(|s| datalife::extract_id_from_url(URL, s))
            .in_scope(".card__content > h3 > a")
            .flatten()
            .unwrap_or_default()
            .boxed(),
        title: html::text_value(".card__content > h3 > a"),
        secondary_title: html::items_processor(".card__category a", html::TextValue::new().boxed())
            .map(|str| Some(str.join(", ")))
            .boxed(),
        image: html::join_processors(vec![
            html::attr_value(".card__cover > figure > img", "data-src"),
            html::attr_value(".card__cover > figure > img", "src"),
        ])
        .map(|items| {
            items
                .into_iter()
                .filter(|src| !src.is_empty())
                .map(|src| format!("{URL}{src}"))
                .next()
        })
        .unwrap_or_default()
        .boxed(),
    }
    .into()
}

fn content_info_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR.get_or_init(|| {
        html::ItemsProcessor::new(".movie > article.item", content_info_processor())
    })
}

fn content_details_processor() -> &'static html::ScopeProcessor<ContentDetails> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopeProcessor<ContentDetails>> =
        OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        html::ScopeProcessor::new(
            "#site-content",
            html::ContentDetailsProcessor {
                media_type: MediaType::Manga,
                title: html::text_value(".item__full-title span"),
                original_title: html::default_value(),
                image: html::self_hosted_image(URL, ".item__full-sidebar--poster img", "src"),
                description: html::text_value(".item__full-description"),
                additional_info: html::ItemsProcessor::new(
                    ".item__full-sidebar > .item__full-sidebar--section > .item__full-sideba--header",
                    html::join_processors(vec![
                        html::text_value(".item__full-sidebar--sub"),
                        html::text_value(".item__full-sidebar--description"),
                    ])
                    .map(|strs| {
                        strs.into_iter()
                            .map(|s| html::sanitize_text(&s))
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .boxed(),
                )
                .boxed(),
                similar: html::default_value(),
                params: html::default_value(),
            }
            .boxed(),
        )
    })
}

fn extract_params(id: &str, html: &str) -> anyhow::Result<Vec<String>> {
    static RE_USER_HASH: OnceLock<Regex> = OnceLock::new();
    let user_hash = RE_USER_HASH
        .get_or_init(|| Regex::new(r"site_login_hash\s+=\s+'([a-z0-9]+)'").unwrap())
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|g| g.as_str())
        .ok_or_else(|| anyhow!("user hash not found for id: {}", id))?;

    Ok(vec![user_hash.to_string()])
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn should_search() {
        let result = MangaInUaContentSupplier.search("solo leveling", 0).await;
        println!("{result:#?}")
    }

    #[tokio::test]
    async fn should_load_channel() {
        let result = MangaInUaContentSupplier.load_channel("Новинки", 1).await;
        println!("{result:#?}")
    }

    #[tokio::test]
    async fn should_get_content_details() {
        let result = MangaInUaContentSupplier
            .get_content_details("mangas/boyovik/14196-hunter-x-hunter", vec![])
            .await;
        println!("{result:#?}")
    }

    #[tokio::test]
    async fn should_load_media_items() {
        let result = MangaInUaContentSupplier
            .load_media_items(
                "mangas/boyovik/14196-hunter-x-hunter",
                vec![],
                vec!["772f84a2554710856146eb1863c483d705b01412".to_string()],
            )
            .await;
        println!("{result:#?}")
    }

    #[tokio::test]
    async fn should_load_media_item_sources() {
        let result = MangaInUaContentSupplier
            .load_media_item_sources(
                "mangas/boyovik/14196-hunter-x-hunter",
                vec![],
                vec![
                    "772f84a2554710856146eb1863c483d705b01412".to_string(),
                    "1".to_string(),
                ],
            )
            .await;
        println!("{result:#?}")
    }

    #[tokio::test]
    async fn should_load_pages() {
        let result = MangaInUaContentSupplier
            .load_pages(
                "mangas/boyovik/14196-hunter-x-hunter",
                vec![
                    "14275".to_string(),
                    "772f84a2554710856146eb1863c483d705b01412".to_string(),
                    "1".to_string(),
                    "1".to_string(),
                ],
            )
            .await;
        println!("{result:#?}")
    }
}
