use anyhow::{Ok, anyhow};
use indexmap::IndexMap;
use scraper::Selector;
use serde::Deserialize;

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

pub struct MangaInUaContentSupplier {
    channels_map: IndexMap<&'static str, String>,
    processor_content_info_items: html::ItemsProcessor<ContentInfo>,
    processor_content_details: html::ScopeProcessor<ContentDetails>,
}

impl Default for MangaInUaContentSupplier {
    fn default() -> Self {
        Self {
            channels_map: IndexMap::from([
                ("Новинки", format!("{URL}/page/")),
                ("Манґа", format!("{URL}/xfsearch/type/manga/page/")),
                ("Манхва", format!("{URL}/xfsearch/type/manhwa/page/")),
                ("Романтика", format!("{URL}/mangas/romantika/page/")),
                ("Драма", format!("{URL}/mangas/drama/page/")),
                ("Комедія", format!("{URL}/mangas/komedia/page/")),
                ("Буденність", format!("{URL}/mangas/budenst/page/")),
                ("Фентезі", format!("{URL}/mangas/fentez/page/")),
                ("Школа", format!("{URL}/mangas/shkola/page/")),
                ("Надприродне", format!("{URL}/mangas/nadprirodne/page/")),
                ("Пригоди", format!("{URL}/mangas/prigodi/page/")),
                ("Бойовик", format!("{URL}/mangas/boyovik/page/")),
                ("Психологія", format!("{URL}/mangas/psihologia/page/")),
            ]),
            processor_content_info_items: html::ItemsProcessor::new(
                ".feed article.card",
                html::ContentInfoProcessor {
                    // channel pages use absolute links, search results use relative ones
                    id: html::attr_value_map(".card__title > a", "href", extract_id),
                    title: html::text_value(".card__title > a"),
                    secondary_title: html::optional_text_value(".card__cover > .card__type"),
                    image: html::self_hosted_image(URL, ".card__cover > img", "src"),
                }
                .boxed(),
            ),
            processor_content_details: html::ScopeProcessor::new(
                "article.mt",
                html::ContentDetailsProcessor {
                    media_type: MediaType::Manga,
                    title: html::text_value(".mt-title"),
                    original_title: html::default_value(),
                    image: html::self_hosted_image(URL, ".mt-cover > img", "src"),
                    description: html::text_value_map(".mt-desc", |s| {
                        utils::text::sanitize_text(&s)
                    }),
                    additional_info: html::merge(vec![
                        html::items_processor(
                            ".mt-tags-row > li",
                            html::TextValue::new()
                                .all_nodes()
                                .map(|s| utils::text::sanitize_text(&s))
                                .boxed(),
                        ),
                        html::items_processor(".mt-genres > a", html::TextValue::new().boxed())
                            .map(|genres| vec![genres.join(", ")])
                            .boxed(),
                        html::items_processor(
                            ".mt-facts > div",
                            html::join_processors(vec![
                                html::text_value("dt"),
                                html::Any::new(
                                    |s: &String| !s.is_empty(),
                                    vec![
                                        html::items_processor(
                                            "dd .tm-chip",
                                            html::TextValue::new().all_nodes().boxed(),
                                        )
                                        .map(|teams| teams.join(", "))
                                        .boxed(),
                                        html::text_value("dd"),
                                    ],
                                )
                                .boxed(),
                            ])
                            .map(|strs| {
                                strs.into_iter()
                                    .map(|s| utils::text::sanitize_text(&s))
                                    .filter(|s| !s.is_empty())
                                    .collect::<Vec<_>>()
                                    .join(": ")
                            })
                            .boxed(),
                        ),
                    ])
                    .map(|items| items.into_iter().filter(|s| !s.is_empty()).collect())
                    .boxed(),
                    similar: html::default_value(),
                    params: html::default_value(),
                }
                .boxed(),
            ),
        }
    }
}

impl ContentSupplier for MangaInUaContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        self.channels_map.keys().map(|&s| s.to_string()).collect()
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

    async fn get_content_details(&self, id: &str) -> anyhow::Result<Option<ContentDetails>> {
        // ids saved before the `mangas/` prefix was dropped still carry it
        let id = id.strip_prefix("mangas/").unwrap_or(id);
        let url = format!("{URL}/mangas/{id}.html");

        utils::scrap_page(
            utils::create_client().get(&url),
            &self.processor_content_details,
        )
        .await
    }

    async fn load_media_items(
        &self,
        id: &str,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        let news_id = id
            .rsplit_once("/")
            .and_then(|(_, r)| r.split_once("-"))
            .map(|(l, _)| l)
            .ok_or_else(|| anyhow!("cant extract news_id for id: {}", id))?;

        let res: ChaptersResponse = utils::create_client()
            .post(format!("{URL}/engine/ajax/controller.php"))
            .query(&[("mod", "mgchapters")])
            .form(&[("news_id", news_id)])
            .send()
            .await?
            .json()
            .await?;

        // Chapters come ordered from first to last; alternative translations
        // of the same chapter share its number, the primary one has `alt: false`.
        let mut chapters: IndexMap<(String, String), Vec<Chapter>> = IndexMap::new();
        for chapter in res.chapters {
            chapters
                .entry((chapter.tom.clone(), chapter.num.clone()))
                .or_default()
                .push(chapter);
        }

        let result = chapters
            .into_iter()
            .map(|((tom, num), mut translations)| {
                translations.sort_by_key(|c| c.alt);

                let name = translations
                    .iter()
                    .map(|c| c.name.trim())
                    .find(|n| !n.is_empty());

                let title = match name {
                    Some(name) => format!("Розділ {num} · {name}"),
                    None => format!("Розділ {num}"),
                };

                let section = (!tom.is_empty() && tom != "0").then(|| format!("Том {tom}"));

                let sources = translations
                    .into_iter()
                    .map(|c| ContentMediaItemSource::Manga {
                        description: if c.team.trim().is_empty() {
                            "Переклад".to_string()
                        } else {
                            c.team.trim().to_string()
                        },
                        headers: None,
                        pages: None,
                        params: vec![c.id.to_string()],
                        language: None,
                    })
                    .collect();

                ContentMediaItem {
                    title,
                    section,
                    image: None,
                    params: vec![],
                    sources: Some(sources),
                }
            })
            .collect();

        Ok(result)
    }

    async fn load_media_item_sources(
        &self,
        _id: &str,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        Err(anyhow!("unimplemented"))
    }
}

impl MangaPagesLoader for MangaInUaContentSupplier {
    async fn load_pages(&self, _id: &str, params: Vec<String>) -> anyhow::Result<Vec<String>> {
        if params.is_empty() {
            return Err(anyhow!("invalid params number"));
        }

        let chapter_id = params[0].as_str();

        // redirects to the chapter reader page, which renders all page images
        let html = utils::create_client()
            .get(format!("{URL}/index.php"))
            .query(&[("newsid", chapter_id)])
            .send()
            .await?
            .text()
            .await?;

        let document = scraper::Html::parse_document(&html);

        let img_sel = Selector::parse(".rd-pages figure.rp img").unwrap();
        let pages: Vec<_> = document
            .select(&img_sel)
            .filter_map(|el| el.attr("data-src").or_else(|| el.attr("src")))
            .filter(|src| !src.is_empty())
            .map(|src| html::self_hosted_url(URL, src))
            .collect();

        Ok(pages)
    }
}

fn extract_id(href: String) -> String {
    let path = href.strip_prefix(URL).unwrap_or(&href);
    let path = path.trim_start_matches('/');
    let path = path.strip_prefix("mangas/").unwrap_or(path);
    path.strip_suffix(".html").unwrap_or(path).to_string()
}

#[derive(Debug, Deserialize)]
struct ChaptersResponse {
    #[serde(default)]
    chapters: Vec<Chapter>,
}

#[derive(Debug, Deserialize)]
struct Chapter {
    id: u64,
    #[serde(default)]
    tom: String,
    num: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    team: String,
    #[serde(default)]
    alt: bool,
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn mangainua_should_extract_id() {
        assert_eq!(
            extract_id("https://manga.in.ua/mangas/boyovik/14196-hunter-x-hunter.html".into()),
            "boyovik/14196-hunter-x-hunter"
        );
        assert_eq!(
            extract_id("/mangas/boyovik/16478-solo-leveling.html".into()),
            "boyovik/16478-solo-leveling"
        );
    }

    #[tokio::test]
    async fn mangainua_should_search() {
        let result = MangaInUaContentSupplier::default()
            .search("solo leveling", 0)
            .await
            .unwrap();
        println!("{result:#?}");
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn mangainua_should_load_channel() {
        let result = MangaInUaContentSupplier::default()
            .load_channel("Новинки", 2)
            .await
            .unwrap();
        println!("{result:#?}");
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn mangainua_should_get_content_details() {
        let result = MangaInUaContentSupplier::default()
            .get_content_details("boyovik/14196-hunter-x-hunter")
            .await
            .unwrap();
        println!("{result:#?}");
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn mangainua_should_get_content_details_for_legacy_id() {
        let result = MangaInUaContentSupplier::default()
            .get_content_details("mangas/boyovik/14196-hunter-x-hunter")
            .await
            .unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn mangainua_should_load_media_items() {
        let result = MangaInUaContentSupplier::default()
            .load_media_items("boyovik/14196-hunter-x-hunter", vec![])
            .await
            .unwrap();
        println!("{result:#?}");
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn mangainua_should_load_pages() {
        let result = MangaInUaContentSupplier::default()
            .load_pages("boyovik/14196-hunter-x-hunter", vec!["14813".to_string()])
            .await
            .unwrap();
        println!("{result:#?}");
        assert!(!result.is_empty());
    }
}
