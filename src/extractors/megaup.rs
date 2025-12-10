use crate::{
    models::ContentMediaItemSource,
    utils::{GenericResponse, create_json_client, enc_dec_app, get_user_agent},
};

pub async fn extract(url: &str, prefix: &str) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let media_url = url.replacen("/e/", "/media/", 1);

    let client = create_json_client();

    let user_agent = get_user_agent();
    let media_url_res_str = client
        .get(media_url)
        .header("User-Agent", user_agent)
        .send()
        .await?
        .text()
        .await?;

    // println!("{media_url_res_str}");

    let media_url_res: GenericResponse = serde_json::from_str(&media_url_res_str)?;

    // println!("{media_url_res:?}");

    let jwpconfig = enc_dec_app::mega_dec(&media_url_res.result, user_agent).await?;

    // println!("{jwpconfig:?}");

    Ok(jwpconfig.to_media_item_sources(prefix, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_extract() {
        let res = extract(
            "https://megaup22.online/e/l4_2b3mqWS2JcOLzFLlM6xfpCQ",
            "test",
        )
        .await;
        println!("{res:?}")
    }
}
