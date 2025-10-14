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

const URL: &str = "https://primesrc.me";

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

    // println!("{link}");

    let client = utils::create_client();
    let servers = load_servers(client, &link).await?;

    // println!("{servers:?}");

    let mut sources: Vec<ContentMediaItemSource> = vec![];
    for (idx, server) in servers.into_iter().enumerate() {
        let mut maybe_server_sources = load_server_sources(client, server, idx).await;
        if let Some(server_sources) = maybe_server_sources.as_mut() {
            sources.append(server_sources);
        }
    }

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

    // println!("{link}");

    match server.name.as_str() {
        "Streamwish" => {
            let link = load_server_link(client, server_key).await?;
            streamwish::extract(&link, &display_name).await
        }
        "Filelions" => {
            let link = load_server_link(client, server_key).await?;
            filelions::extract(&link, &display_name).await
        }
        "Mixdrop" => {
            let link = load_server_link(client, server_key).await?;
            mixdrop::extract(&link, &display_name).await
        }
        "Dood" => {
            let link = load_server_link(client, server_key).await?;
            dood::extract(&link, &display_name).await
        }
        _ => Ok(vec![]),
    }
}

async fn load_server_link(client: &Client, server_key: &String) -> Result<String, anyhow::Error> {
    #[derive(Deserialize)]
    struct ServerSourceRes {
        link: String,
    }
    // tokio::time::sleep(Duration::from_millis(1000)).await;
    let url = format!("{URL}/api/v1/l?key={server_key}");
    let response_str = client.get(url).send().await?.text().await?;
    let response: ServerSourceRes = serde_json::from_str(&response_str)?;
    let link = response.link;
    Ok(link)
}

#[cfg(test)]
mod test {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

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
