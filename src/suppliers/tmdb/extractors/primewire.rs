use anyhow::Result;
use futures::future::BoxFuture;
use log::error;
use reqwest::Client;
use serde::Deserialize;

use crate::{
    extractors::{dood, filelions, mixdrop, streamwish},
    models::ContentMediaItemSource,
    utils,
};

use super::SourceParams;

const URL: &str = "https://www.primewire.tf";

#[derive(Debug, Deserialize)]
struct Server {
    name: String,
    key: String,
}

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
    _langs: &'a [String],
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let tmdb = params.id;

    let link = match &params.ep {
        Some(ep) => {
            let s = ep.s;
            let e = ep.e;
            format!("{URL}/api/v1/s?tmdb={tmdb}&season={s}&episode={e}&type=tv")
        }
        None => format!("{URL}/api/v1/s?tmdb={tmdb}&type=movie"),
    };

    let client = utils::create_client();
    let servers = load_servers(client, &link).await?;

    // println!("{servers:?}");

    let sources_futures = servers
        .into_iter()
        .enumerate()
        .map(|(idx, server)| load_server_sources(client, server, idx));

    let sources = futures::future::join_all(sources_futures)
        .await
        .into_iter()
        .flatten()
        .flatten()
        .collect();

    Ok(sources)
}

async fn load_servers(client: &Client, link: &str) -> Result<Vec<Server>, anyhow::Error> {
    #[derive(Debug, Deserialize)]
    struct ApiResponse {
        servers: Vec<Server>,
    }

    let api_res_str = client.get(link).send().await?.text().await?;

    // println!("{link}");
    // println!("{api_res_str}");

    let api_res: ApiResponse = serde_json::from_str(&api_res_str)?;

    Ok(api_res.servers)
}

async fn load_server_sources(
    client: &Client,
    server: Server,
    idx: usize,
) -> Option<Vec<ContentMediaItemSource>> {
    match try_load_server_sources(client, &server, idx).await {
        Ok(sources) => Some(sources),
        Err(e) => {
            error!("[primewire] fail to extract server {server:?}: {e:?}");
            None
        }
    }
}

async fn try_load_server_sources(
    client: &Client,
    server: &Server,
    idx: usize,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let server_key = &server.key;
    let server_name = &server.name;
    let display_name = format!("[PrimeWire] {idx}. {server_name}");

    #[derive(Deserialize)]
    struct ServerSourceRes {
        link: String,
    }

    let response_str = client
        .get(format!("{URL}/api/v1/l?key={server_key}"))
        .send()
        .await?
        .text()
        .await?;

    let response: ServerSourceRes = serde_json::from_str(&response_str)?;
    let link = response.link;

    // println!("{link}");

    match server.name.as_str() {
        "Streamwish" => streamwish::extract(&link, &display_name).await,
        "Filelions" => filelions::extract(&link, &display_name).await,
        "Mixdrop" => mixdrop::extract(&link, &display_name).await,
        "Dood" => dood::extract(&link, &display_name).await,
        _ => Ok(vec![]),
    }
}

// fn decrypt_links(data: &str) -> anyhow::Result<Vec<String>> {
//     let key = &data[(data.len() - 10)..];
//     let ct = &data[..(data.len() - 10)];
//
//     let pt = crypto::decrypt_base64_blowfish_ebc(key.as_bytes(), ct.as_bytes())?;
//
//     let res = pt
//         .chunks(5)
//         .map(|chunk| String::from_utf8(chunk.to_vec()).unwrap_or_default())
//         .collect();
//
//     Ok(res)
// }

#[cfg(test)]
mod test {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    // #[test]
    // fn should_decrypt_links() {
    //     let data = "MaMWRx5GOovWdKt6VBY8YNh31oXIatX5yIjDC+2YwtKaf/zaU80DgbYWvHuLlP8SF8OWmY1OpE";
    //     let res = decrypt_links(data);
    //
    //     println!("{res:#?}")
    // }

    #[test_log::test(tokio::test)]
    async fn should_load_source() {
        let res = extract(&SourceParams {
            // id: 655,
            id: 1399,
            imdb_id: None, //Some("tt18259086".into()),
            // ep: None,
            ep: Some(Episode { s: 1, e: 1 }),
        })
        .await;
        println!("{res:#?}")
    }
}
