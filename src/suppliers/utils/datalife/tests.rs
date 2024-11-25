#![cfg(test)]

use crate::suppliers::utils;

#[tokio::test]
async fn shold_load_large_dle_playlist() {
    let playlist_req = utils::create_client()
        .get("https://anitube.in.ua/engine/ajax/playlists.php?news_id=94&xfield=playlist&user_hash=867ca5be02de10b799c164d7b7c31e6eece1bb10");

    let _ = super::load_ajax_playlist(playlist_req).await.unwrap();

    // println!("{res:?}")
}
