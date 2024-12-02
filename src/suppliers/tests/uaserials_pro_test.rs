use suppliers::{get_supplier, AllContentSuppliers};

use crate::suppliers::{self, ContentSupplier};

const NAME: &str = "UASerialsPro";

#[tokio::test]
async fn should_load_channel() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_channel(&sup, "Серіали".into(), 2)
        .await
        .unwrap();
    println!("{res:#?}");
}

#[tokio::test]
async fn should_search() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::search(&sup, "Термінатор".into(), vec![])
        .await
        .unwrap();
    println!("{res:#?}");
}

#[tokio::test]
async fn should_load_content_details() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::get_content_details(&sup, "8831-gotel-kokayin".into())
        .await
        .unwrap();
    println!("{res:#?}");
}

#[tokio::test]
async fn should_load_media_items() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_media_items(
        &sup,
        "8831-gotel-kokayin".into(),
        vec!["https://hdvbua.pro/embed/8831".into()],
    )
    .await
    .unwrap();
    println!("{res:#?}");
}
