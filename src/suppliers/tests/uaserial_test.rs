use suppliers::{get_supplier, AllContentSuppliers};

use crate::suppliers::{self, ContentSupplier};

const NAME: &str = "UASerial";

#[tokio::test]
async fn should_load_channel() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_channel(&sup, "Серіали".into(), 2).await.unwrap();
    println!("{res:#?}");
    assert_eq!(true, res.len() > 0)
}

#[tokio::test]
async fn should_search() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::search(&sup, "Термінатор".into(), vec![]).await.unwrap();
    println!("{res:#?}");
    assert_eq!(true, res.len() > 0)
}

#[tokio::test]
async fn should_load_content_details_for_movie() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::get_content_details(
        &sup, 
        "movie-the-terminator".into()
    ).await.unwrap();
    println!("{res:#?}");
}

#[tokio::test]
async fn should_load_content_details_for_tv_show() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::get_content_details(
        &sup, 
        "universal-basic-guys/season-1".into()
    ).await.unwrap();
    println!("{res:#?}");
}

#[tokio::test]
async fn should_load_media_item_sources() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_media_item_sources(
        &sup, 
        "blue-exorcist/season-1".into(),
        vec!["/embed/blue-exorcist/season-1/episode-1".into()]
    ).await.unwrap();
    println!("{res:#?}");
}