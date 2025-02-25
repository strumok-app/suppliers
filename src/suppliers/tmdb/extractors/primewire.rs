use std::sync::OnceLock;

use anyhow::{anyhow, Result};
use futures::future::BoxFuture;
use log::{error, warn};
use reqwest::{header, redirect, Client};
use scraper::{Html, Selector};

use crate::{
    extractors::{doodstream, mixdrop, streamwish},
    models::ContentMediaItemSource,
    utils::{self, crypto},
};

use super::SourceParams;

const URL: &str = "https://www.primewire.tf";
const DS_KEY: &str = "JyjId97F9PVqUPuMO0";

pub fn extract_boxed(
    params: &SourceParams,
) -> BoxFuture<anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    if params.imdb_id.is_none() {
        return Ok(vec![]);
    }

    let client = utils::create_client();
    let link = lookup_page(&client, params).await?;
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

async fn lookup_page(client: &Client, params: &SourceParams) -> Result<String, anyhow::Error> {
    let imdb_id = params.imdb_id.as_ref().unwrap();

    let hash = crypto::sha1_hex(format!("{imdb_id}{DS_KEY}").as_str());
    let (ds, _) = hash.split_at(10);

    let html = client
        .get(format!("{URL}/filter"))
        .query(&[("s", imdb_id.as_str()), ("ds", ds)])
        .send()
        .await?
        .text()
        .await?;

    let doc = Html::parse_document(&html);

    static ITEM_SELECTOR: OnceLock<Selector> = OnceLock::new();
    let original_link = doc
        .select(ITEM_SELECTOR.get_or_init(|| {
            Selector::parse(".index_container .index_item.index_item_ie a").unwrap()
        }))
        .filter_map(|a| a.attr("href"))
        .map(utils::html::sanitize_text)
        .next()
        .ok_or_else(|| anyhow!("[primewire] No search results found for imdb_id: {imdb_id}"))?;

    let link = match &params.ep {
        Some(ep) => {
            let s = ep.s;
            let e = ep.e;
            let t = original_link.replacen("-", "/", 1);
            format!("{URL}{t}-season-{s}-episode-{e}")
        }
        _ => format!("{URL}/{original_link}"),
    };

    // println!("{link:#?}");

    Ok(link)
}

async fn load_servers(client: &Client, link: &str) -> Result<Vec<Server>, anyhow::Error> {
    let html = client.get(link).send().await?.text().await?;

    let doc = Html::parse_document(&html);

    static SERVER_SEL: OnceLock<Selector> = OnceLock::new();
    static LINK_SEL: OnceLock<Selector> = OnceLock::new();
    static NAME_SEL: OnceLock<Selector> = OnceLock::new();
    static KEY_SEL: OnceLock<Selector> = OnceLock::new();

    let data = doc
        .select(KEY_SEL.get_or_init(|| Selector::parse("#user-data").unwrap()))
        .filter_map(|e| e.attr("v"))
        .next()
        .ok_or_else(|| anyhow!("[primewire] No link encryption data found"))?;

    let links = decrypt_links(data)?;

    let result = doc
        .select(SERVER_SEL.get_or_init(|| Selector::parse(".movie_version").unwrap()))
        .filter_map(|e| {
            let link_version = e
                .select(LINK_SEL.get_or_init(|| Selector::parse(".go-link").unwrap()))
                .next()?
                .attr("link_version")?
                .parse::<usize>()
                .ok()?;

            let name = e
                .select(NAME_SEL.get_or_init(|| Selector::parse(".version-host").unwrap()))
                .next()?
                .text()
                .collect::<String>();

            let link_sufix = links.get(link_version)?;

            Some(Server {
                name: name.trim().to_string(),
                link: format!("{URL}/links/go/{link_sufix}"),
            })
        })
        .collect::<Vec<_>>();

    Ok(result)
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
            id: 0,
            imdb_id: Some("tt18259086".into()),
            ep: None,
            // ep: Some(Episode { s: 1, e: 3 }),
        })
        .await;
        println!("{res:#?}")
    }
}
