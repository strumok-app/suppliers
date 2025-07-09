use reqwest::header;
use serde::{de::DeserializeOwned, Serialize};

use crate::utils;

pub async fn server_action<P: Serialize, R: DeserializeOwned>(
    url: &str,
    action: &str,
    res_id: u8,
    params: &P,
) -> anyhow::Result<R> {
    let res = utils::create_json_client()
        .post(url)
        .header("Referer", url)
        .header(header::CONTENT_TYPE, "text/x-component")
        .header("Next-Action", action)
        .json(params)
        .send()
        .await?
        .text()
        .await?;

    

    let split_pattern = format!("{res_id}:");
    let action_result = res
        .split("\n")
        .filter_map(|line| {
            line.split_once(split_pattern.as_str())
                .map(|(_, json)| json)
        })
        .last()
        .unwrap_or_default();

    

    let result: R = serde_json::from_str(action_result)?;

    Ok(result)
}
