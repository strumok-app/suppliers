use suppliers::{get_supplier, AllContentSuppliers};

use crate::suppliers::{self, ContentSupplier};

const NAME: &str = "UFDub";

#[tokio::test]
async fn should_load_channel() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_channel(&sup, "Аніме".into(), 2)
        .await
        .unwrap();
    println!("{res:#?}");
}

#[tokio::test]
async fn should_search() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::search(&sup, "Засновник темного шляху".into(), vec![])
        .await
        .unwrap();
    println!("{res:#?}");
}

#[tokio::test]
async fn should_load_content_details() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::get_content_details(
        &sup,
        "anime/302-the-oni-girl-moia-divchyna-oni".into(),
    )
    .await
    .unwrap();
    println!("{res:#?}");
}

#[tokio::test]
async fn should_load_media_items_serial() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_media_items(
        &sup,
        "anime/301-zasnovnyk-temnogo-shliakhu-mo-dao-zu-shi".into(),
        vec![String::from("https://video.ufdub.com/AT/VP.php?ID=301")],
    )
    .await
    .unwrap();
    println!("{res:#?}");
}

#[tokio::test]
async fn should_load_media_items_movie() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_media_items(
        &sup,
        "anime/302-the-oni-girl-moia-divchyna-oni".into(),
        vec![String::from("https://video.ufdub.com/AT/VP.php?ID=302")],
    )
    .await
    .unwrap();
    println!("{res:#?}");
}
