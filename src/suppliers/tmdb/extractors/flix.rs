use std::sync::OnceLock;

use super::SourceParams;
use crate::{
    extractors::rapid,
    models::ContentMediaItemSource,
    utils::{self, GenericResponse, enc_dec_app},
};
use futures::future::BoxFuture;
use regex::Regex;

// const URL: &str = "https://yflix.to/";
const URL_AJAX: &str = "https://yflix.to/ajax";

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
    langs: &'a [String],
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params, langs))
}

pub async fn extract(
    params: &SourceParams,
    _langs: &[String],
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let maybe_eid = match &params.ep {
        Some(ep) => enc_dec_app::flix_db_find_tv(params.id, ep.s, ep.e).await?,
        None => enc_dec_app::flix_db_find_movie(params.id).await?,
    };

    let eid = match maybe_eid {
        Some(eid) => eid,
        None => return Ok(vec![]),
    };

    let enc_eid = enc_dec_app::flix_enc(&eid).await?;

    let links_res_str = utils::create_json_client()
        .get(format!("{URL_AJAX}/links/list?eid={eid}&_={enc_eid}"))
        .send()
        .await?
        .text()
        .await?;

    let links_res: GenericResponse = serde_json::from_str(&links_res_str)?;

    // println!("{links_res:#?}");

    let lids = get_lids(&links_res.result);

    // println!("{lids:#?}");

    let mut result: Vec<ContentMediaItemSource> = vec![];
    let mut srv_num = 1u8;
    for lid in lids {
        let maybe_sources = load_server_link(&utils::create_json_client(), lid, srv_num).await;
        if let Ok(sources) = maybe_sources {
            result.extend(sources);
        }
        srv_num += 1;
    }

    Ok(result)
}

fn get_lids(html: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"data-lid="([^"]+)""#).unwrap());

    re.captures_iter(html)
        .map(|cap| cap[1].to_string())
        .collect()
}

async fn load_server_link(
    client: &reqwest::Client,
    lid: String,
    srv_num: u8,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let enc_lid = enc_dec_app::flix_enc(&lid).await?;
    let link_res_str = client
        .get(format!("{URL_AJAX}/links/view?id={lid}&_={enc_lid}"))
        .send()
        .await?
        .text()
        .await?;

    // println!("{link_res_str:#?}");

    let link_res: GenericResponse = serde_json::from_str(&link_res_str)?;

    let link = enc_dec_app::flix_dec(&link_res.result).await?;

    let sources = rapid::extract(&link, &format!("[Flix] Server {srv_num} -")).await?;

    Ok(sources)
}

#[cfg(test)]
mod tests {

    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    #[test_log::test(tokio::test)]
    async fn shoul_load_sources_tv() {
        let res = extract(
            &SourceParams {
                id: 1799,
                ep: Some(Episode { s: 1, e: 1 }),
                imdb_id: None,
            },
            &[],
        )
        .await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn shoul_load_sources_movie() {
        let res = extract(
            &SourceParams {
                id: 176,
                ep: None,
                imdb_id: None,
            },
            &[],
        )
        .await;
        println!("{res:#?}")
    }
}
