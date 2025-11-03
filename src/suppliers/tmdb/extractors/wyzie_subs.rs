use futures::future::BoxFuture;
use log::warn;
use reqwest::Client;
use serde::Deserialize;

use crate::{models::ContentMediaItemSource, suppliers::tmdb::extractors::SourceParams, utils};

const URL: &str = "https://sub.wyzie.ru";

#[derive(Debug, Deserialize)]
struct SubRes {
    url: String,
    display: String,
}

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
    langs: &'a [String],
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params, langs))
}

pub async fn extract(
    params: &SourceParams,
    langs: &[String],
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    if langs.is_empty() {
        return Ok(vec![]);
    }

    let id = params.id;

    let url = match &params.ep {
        Some(ep) => format!("{}/search?id={id}&season={}&episode={}", URL, ep.s, ep.e),
        None => format!("{URL}/search?id={id}"),
    };

    let client = utils::create_json_client();
    let mut result: Vec<ContentMediaItemSource> = vec![];

    let mut index = 1;
    for lang in langs {
        let lang_url = format!("{url}&language={lang}");
        let subs = match load_subs(&lang_url, client).await {
            Ok(s) => s,
            Err(e) => {
                warn!("[wyzie] subs load failed for langus {lang}: {e}");
                continue;
            }
        };

        for sub in subs {
            result.push(ContentMediaItemSource::Subtitle {
                link: sub.url,
                description: format!("[wyzie] {}. {}", index, sub.display),
                headers: None,
            });
            index += 1;
        }
    }

    Ok(result)
}

async fn load_subs(url: &str, client: &Client) -> anyhow::Result<Vec<SubRes>> {
    let res_str = client.get(url).send().await?.text().await?;
    if res_str.starts_with('{') {
        warn!("[wyzie] {url} failed: {res_str}");
        return Ok(vec![]);
    }

    let res: Vec<SubRes> = serde_json::from_str(&res_str)?;

    Ok(res)
}

#[cfg(test)]
mod tests {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_load_subtitle() {
        let res = extract(
            &SourceParams {
                id: 655,
                imdb_id: Some("tt0092455".to_owned()),
                ep: Some(Episode { e: 1, s: 1 }),
            },
            &["en".to_owned(), "uk".to_owned()],
        )
        .await;

        println!("{res:#?}")
    }
}
