#![cfg(test)]

use crate::utils;

#[tokio::test]
async fn shold_load_large_dle_playlist() {
    let user_hash = super::UserHash::new("https://anitube.in.ua", "dle_login_hash");

    let _ = user_hash
        .with_retry(|hash| {
            let playlist_req = utils::create_client()
                .get("https://anitube.in.ua/engine/ajax/playlists.php")
                .query(&[
                    ("news_id", "94"),
                    ("xfield", "playlist"),
                    ("user_hash", &hash),
                ])
                .header("Referer", "https://anitube.in.ua");

            super::load_ajax_playlist(playlist_req)
        })
        .await
        .unwrap();
}
