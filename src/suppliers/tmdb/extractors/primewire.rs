use std::sync::OnceLock;

use anyhow::{anyhow, Result};
use futures::future::BoxFuture;
use log::error;
use regex::Regex;
use reqwest::{redirect, Client};
use serde::Deserialize;

use crate::{
    extractors::{filelions, mixdrop, streamwish},
    models::ContentMediaItemSource,
    utils::{self, crypto},
};

use super::SourceParams;

const URL: &str = "https://www.primewire.tf";

// TODO: extract token from js?
const TOKEN: &str = "0.xUvp3K0Cgiy-2-DV25L753hboTEWcTYMNvucA-v724qpKhgGl8AX-yZkRwonRvP7eTqWWX2p69JJuTmrm8lypEGrIMMFOibXakj8n-NfDMVX7hXE-o6jGkgtnDyEmk6eSqoTQS7Shv7iULknMI4nIdPKs1ZuuD9ncfsUrRFeQaUBL6a48WIDt75NDkUjuo_AaxxP7DHJKVXD3BOyVbzSoB5nmASe0IO75UuY99B3KAYyCqVyn0aa326OAeXv3XDa1Mapxg50WRkvRCxMwgnu56G7l7tHEqBRadF9Uy_xWNvy8pobkvO2qLd2x01fkxhsPcrDIF9e02gBaN0Efl_J1lMAIJqdC41oUsDdEhekrZj0X17GYt01DszcXmRZ0WyI9yDyswNRdICyol4HiGKqqvsvWFMW8i4weJu5-RrqlgH9wlCi08WpVF83Adbk74dRH2wWiw_-s4elP_F_qkrE-4nEkqhkjHVSt0lBB2nf_jhG_jAwwiU1RVOtrnp9hR_HoRTihjp5r09QIWvFxaRYFNWCquxcB5bNmFshiCjB2XKNxi87xC0ToRnhhrAxYyM-UOgplqlv0uc76f-w9D41C3udBetN4F-ER757JZuQ325mlprLOFQ_xSa06cP2pObc-NNs9SnRIoqRjfheq2JJOTrjKWbA3GxmQ_yyulbVfBJcAOQjBPVcMbdj8s90a64PjcYaTNtdXfnWbv20ggW-Lmovy8FPhBjNvMR1J7hS1088K5uVew7-YoNVpqpS73uE9S0f5GO-Em1B3Ai7gZlrDttCN07yrAxHrdbe2AK8xQs_igEIGYyIxoBgNfakiDsLxHn_McrHme6D3T_QS4Wy-ik5QEaH_QJE_jELUDdWjKKA4o3B-Q6n_cSlWvWvkEYh.XZyFppBH6S-d6fFp12VguQ.bb1d22d7d0411edb066282cead4f437a3eba4bc7edeb62d435ea433014fb0d63";

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
    _langs: &'a [String],
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
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

    let client = utils::create_client();
    let servers = load_servers(client, &link).await?;

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

    static KEY_RE: OnceLock<Regex> = OnceLock::new();
    static SERVERS_RE: OnceLock<Regex> = OnceLock::new();

    let key = KEY_RE
        .get_or_init(|| Regex::new(r#"v="([-A-Za-z0-9+/=_]+)""#).unwrap())
        .captures(&html)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str())
        .ok_or_else(|| anyhow!("[primewire] cant extract key"))?;

    let links_hashes = decrypt_links(key)?;

    let servers = SERVERS_RE
        .get_or_init(|| Regex::new(r#""authority":"([a-zA-Z0-9\.]+)""#).unwrap())
        .captures_iter(&html)
        .enumerate()
        .take_while(|&(n, _)| n < links_hashes.len())
        .filter_map(|(n, cap)| {
            let server_name = cap.get(1)?.as_str();
            let link_hash = links_hashes.get(n)?;

            Some(Server {
                name: server_name.trim().to_string(),
                link: format!("{URL}/links/go/{link_hash}?token={TOKEN}&embed=true"),
            })
        })
        .collect::<Vec<_>>();

    Ok(servers)
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
    let server_link = &server.link;
    let server_name = &server.name;
    let display_name = format!("{idx}. {server_name}");

    #[derive(Deserialize)]
    struct ServerSourceRes {
        link: String,
    }

    let response_str = client.get(server_link).send().await?.text().await?;
    let response: ServerSourceRes = serde_json::from_str(&response_str)?;
    let link = response.link;

    match server.name.as_str() {
        "streamwish.to" => streamwish::extract(&link, &display_name).await,
        "filelions.to" => filelions::extract(&link, &display_name).await,
        "mixdrop.ag" => mixdrop::extract(&link, &display_name).await,
        _ => Ok(vec![]),
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
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    #[test]
    fn should_decrypt_links() {
        let data = "MaMWRx5GOovWdKt6VBY8YNh31oXIatX5yIjDC+2YwtKaf/zaU80DgbYWvHuLlP8SF8OWmY1OpE";
        let res = decrypt_links(data);

        println!("{res:#?}")
    }

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
