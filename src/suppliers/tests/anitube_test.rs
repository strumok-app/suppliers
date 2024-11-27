use suppliers::{get_supplier, AllContentSuppliers};

use crate::suppliers::{self, ContentSupplier};

const NAME: &str = "AniTube";

#[tokio::test]
async fn should_load_channel() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_channel(&sup, "Новинки".into(), 2).await.unwrap();
    println!("{res:#?}");
    assert_eq!(true, res.len() > 0)
}

#[tokio::test]
async fn should_search() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::search(&sup, "Доктор Стоун".into(), vec![]).await.unwrap();
    println!("{res:#?}");
    assert_eq!(true, res.len() > 0)
}

#[tokio::test]
async fn should_load_content_details() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::get_content_details(
        &sup, 
        "3419-dokor-kamin".into()
    ).await.unwrap();
    println!("{res:#?}");
}

#[tokio::test]
async fn should_load_media_items() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_media_items(
        &sup, 
        "7633-dr-stone-4".into(),
        vec!["3419".into(), "fa06e9031e506c6f56099b6500b0613e50a60656".into()]
    ).await.unwrap();
    println!("{res:#?}");
}

#[tokio::test]
async fn should_load_media_items_source() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_media_item_sources(
        &sup,
        "7633-dr-stone-4".into(),
        vec![
            "ОЗВУЧУВАННЯ DZUSKI ПЛЕЄР ASHDI".into(),
            "https://ashdi.vip/vod/43190".into(),
            "ОЗВУЧУВАННЯ DZUSKI ПЛЕЄР TRG".into(),
            "https://tortuga.tw/vod/10654".into(),
            // "ОЗВУЧУВАННЯ Togarashi ПЛЕЄР MOON".into(),
            // "https://moonanime.art/iframe/qcsyutjdkhtucmzxdmmw".into(),
            // "ОЗВУЧУВАННЯ Togarashi ПЛЕЄР МОНСТР ".into(),
            // "https://mmonstro.site/embed/649292".into(),
            "СУБТИТРИ СУБТИТРИ ПЛЕЄР МОНСТР ".into(),
            "https://mmonstro.site/embed/704444/".into(),
        ],
    )
    .await
    .unwrap();
    println!("{res:#?}");
}