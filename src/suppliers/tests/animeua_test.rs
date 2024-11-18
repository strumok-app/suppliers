use suppliers::{get_supplier, AllContentSuppliers};

use crate::suppliers::{self, ContentSupplier};

const NAME: &str = "AnimeUA";

#[tokio::test]
async fn should_load_channel() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_channel(&sup, "ТОП 100".into(), 2).await.unwrap();
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
        "7633-dr-stone-4".into()
    ).await.unwrap();
    println!("{res:#?}");
}

#[tokio::test]
async fn should_load_media_items() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_media_items(
        &sup, 
        "7633-dr-stone-4".into(),
        vec![String::from("https://ashdi.vip/serial/971?season=4")]
    ).await.unwrap();
    println!("{res:#?}");
}