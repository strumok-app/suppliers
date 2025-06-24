use anyhow::Ok;
use futures::future::BoxFuture;
use serde::Deserialize;

use crate::{
    models::ContentMediaItemSource,
    utils::{create_json_client, lang},
};

use super::SourceParams;

const BACKEND_URL: &str = "https://api2.vidsrc.vip";
const URL: &str = "	https://vidsrc.vip";

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
    let id = params.id;

    // const C = tmdb
    // .toString()
    // .split("")
    // .map((digit) => {
    //     const encoding = "abcdefghij";
    //     return encoding[parseInt(digit)];
    // })
    // .join("");
    // const B = C.split("").reverse().join("");
    // const A = btoa(B);
    // const D = btoa(A);
    // const urlovo = `https://api2.vidsrc.vip/movie/${D}`;
    //
    // const formattedString = `${tmdb}-${season}-${episode}`;
    // const reversedString = formattedString.split('').reverse().join('');
    // const firstBase64 = btoa(reversedString);
    // const secondBase64 = btoa(firstBase64);
    // const url = `https://api2.vidsrc.vip/tv/${secondBase64}`;
    //
    let link = match &params.ep {
        Some(ep) => {
            let s = ep.s;
            let e = ep.e;
            format!("{BACKEND_URL}/tv&id={id}/{s}/{e}")
        }
        None => format!("{BACKEND_URL}/api/getVideoSource?type=movie&id={id}"),
    };

    Ok(vec![])
}

#[cfg(test)]
mod tests {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    #[tokio::test]
    async fn should_extract_tv() {
        let res = extract(
            &SourceParams {
                id: 655,
                imdb_id: None,
                ep: Some(Episode { e: 1, s: 1 }),
            },
            &["en".to_owned()],
        )
        .await;

        println!("{res:#?}")
    }
}
