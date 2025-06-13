use std::collections::HashMap;

use futures::future::BoxFuture;

use super::embed_su::URL;
use crate::{models::ContentMediaItemSource, utils};

use super::SourceParams;

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
    let client = utils::create_json_client();

    let id = params.id;
    let mut result: Vec<ContentMediaItemSource> = vec![];

    for lang in langs {
        let lang_code = match lang.as_str() {
            "en" => "eng",
            "uk" => "ukr",
            _ => continue,
        };

        let url_id = match &params.ep {
            Some(ep) => {
                let e = ep.e;
                let s = ep.s;
                format!("t:{id}:{s}:{e}/{lang_code}?ss={s}&ep={e}")
            }
            None => format!("m:{id}/{lang_code}"),
        };

        let links_res = client
            .get(format!("{URL}/api/get-subs/{url_id}"))
            .header("Referer", URL)
            .send()
            .await?
            .text()
            .await?;

        println!("{links_res:#?}");

        let links: Vec<String> = serde_json::from_str(&links_res)?;

        links.into_iter().enumerate().for_each(|(idx, link)| {
            let num = idx + 1;
            result.push(ContentMediaItemSource::Subtitle {
                link: format!("{URL}{link}"),
                description: format!("[embed_su] {lang_code} {num}"),
                headers: Some(HashMap::from([("Referer".into(), URL.into())])),
            });
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_extract_subtitles() {
        let langs = ["en".to_string()];

        let res = extract(
            &SourceParams {
                id: 609681,
                imdb_id: None,
                ep: None,
            },
            &langs,
        )
        .await;

        println!("{res:#?}")
    }
}
