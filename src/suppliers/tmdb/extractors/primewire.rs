use std::sync::OnceLock;

use anyhow::{anyhow, Result};
use futures::future::BoxFuture;
use log::{error, warn};
use regex::Regex;
use reqwest::{header, redirect, Client};

use crate::{
    extractors::{doodstream, mixdrop, streamwish},
    models::ContentMediaItemSource,
    utils::{self, crypto},
};

use super::SourceParams;

const URL: &str = "https://www.primewire.tf";

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
    _langs: &'a [String],
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    // let id = match &params.imdb_id {
    //     Some(imdb_id) => format!("imdb={imdb_id}"),
    //     None => {
    //         let tmdb = params.id;
    //         format!("tmdb={tmdb}")
    //     }
    // };

    let tmdb = params.id;
    let id = format!("tmdb={tmdb}");

    let link = match &params.ep {
        Some(ep) => {
            let s = ep.s;
            let e = ep.e;
            format!("{URL}/embed/tv?{id}&season={s}&episode={e}")
        }
        None => format!("{URL}/embed/movie?{id}"),
    };

    // println!("{link}");

    let client = utils::create_client();
    let servers = load_servers(&client, &link).await?;

    // println!("{servers:#?}");

    let no_redirect_client = utils::create_client_builder()
        .default_headers(utils::get_default_headers())
        .redirect(redirect::Policy::none())
        .build()
        .unwrap();

    let sources_futures = servers
        .into_iter()
        .enumerate()
        .map(|(idx, server)| load_server_sources(&no_redirect_client, server, idx));

    let sources = futures::future::join_all(sources_futures)
        .await
        .into_iter()
        .flatten()
        .flatten()
        .collect();

    Ok(sources)
}

async fn load_servers(client: &Client, link: &str) -> Result<Vec<Server>, anyhow::Error> {
    let html = client.get(link).send().await?.text().await?;

    // println!("{html}");

    static KEY_RE: OnceLock<Regex> = OnceLock::new();
    static SERVERS_RE: OnceLock<Regex> = OnceLock::new();

    let key = KEY_RE
        .get_or_init(|| Regex::new(r#"v="([-A-Za-z0-9+/=]+)""#).unwrap())
        .captures(&html)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str())
        .ok_or_else(|| anyhow!("[primewire] cant extract key"))?;

    let links_hashes = decrypt_links(key)?;

    // println!("{links_hashes:?}");

    let servers = SERVERS_RE
        .get_or_init(|| Regex::new(r##""#([0-9]+)\s+\-\s+([a-zA-Z0-9\.]+)"##).unwrap())
        .captures_iter(&html)
        .enumerate()
        .take_while(|&(n, _)| n < links_hashes.len())
        .filter_map(|(n, cap)| {
            let server_name = cap.get(2)?.as_str();
            let link_hash = links_hashes.get(n)?;

            Some(Server {
                name: server_name.trim().to_string(),
                link: format!("{URL}/links/go/{link_hash}"),
            })
        })
        .collect::<Vec<_>>();

    // println!("{servers:?}");

    Ok(servers)
}

async fn load_server_sources(
    client: &Client,
    server: Server,
    idx: usize,
) -> Option<Vec<ContentMediaItemSource>> {
    let server_link = &server.link;
    let server_name = &server.name;
    let display_name = format!("{idx}. {server_name}");

    let maybe_response = client.get(server_link).send().await;
    let maybe_location = match &maybe_response {
        Ok(resp) => resp.headers().get(header::LOCATION),
        _ => None,
    };

    // println!("{maybe_location:#?}");

    let location = match maybe_location {
        Some(l) => l.to_str().unwrap(),
        _ => {
            warn!("[primewire] No location header for link: {server_link}");
            return None;
        }
    };

    let res = match server.name.as_str() {
        "dood.watch" => doodstream::extract(location, &display_name).await,
        "streamwish.to" | "filelions.to" => {
            streamwish::extract(location, server_link, &display_name).await
        }
        "mixdrop.ag" => mixdrop::extract(location, &display_name).await,
        _ => return None,
    };

    match res {
        Ok(sources) => Some(sources),
        Err(err) => {
            error!(
                "[primewire] {server_name} fail to load source link (server: {server_link}): {err}"
            );
            None
        }
    }
}

fn decrypt_links(data: &str) -> anyhow::Result<Vec<String>> {
    let key = &data[(data.len() - 10)..];
    let ct = &data[..(data.len() - 10)];

    let pt = crypto::decrypt_base64_blowfish_ebc(key.as_bytes(), ct.as_bytes())?;

    let res = pt
        .chunks(5)
        .map(|chunk| String::from_utf8(chunk.to_vec()).unwrap_or_default())
        .collect();

    Ok(res)
}

#[derive(Debug)]
struct Server {
    name: String,
    link: String,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn should_decrypt_links() {
        let data = "MaMWRx5GOovWdKt6VBY8YNh31oXIatX5yIjDC+2YwtKaf/zaU80DgbYWvHuLlP8SF8OWmY1OpE";
        let res = decrypt_links(data);

        println!("{res:#?}")
    }

    #[tokio::test]
    async fn should_load_source() {
        let res = extract(&SourceParams {
            id: 549509,
            imdb_id: None, //Some("tt18259086".into()),
            ep: None,
            // ep: Some(Episode { s: 1, e: 3 }),
        })
        .await;
        println!("{res:#?}")
    }
}
