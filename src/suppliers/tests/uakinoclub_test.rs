use suppliers::{get_supplier, AllContentSuppliers};

use crate::suppliers::{self, ContentSupplier};

const NAME: &str = "UAKinoClub";

#[tokio::test]
async fn should_load_channel() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_channel(&sup, "Новинки".into(), 2)
        .await
        .unwrap();
    println!("{res:#?}");
    assert_eq!(true, res.len() > 0)
}

#[tokio::test]
async fn should_search() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::search(&sup, "Термінатор".into(), vec![])
        .await
        .unwrap();
    println!("{res:#?}");
    assert_eq!(true, res.len() > 0)
}

#[tokio::test]
async fn should_load_content_details() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::get_content_details(
        &sup,
        "filmy/genre_comedy/24898-zhyv-sobi-policeiskyi".into(),
    )
    .await
    .unwrap();
    println!("{res:#?}");
}

#[tokio::test]
async fn should_load_media_items() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_media_items(
        &sup,
        "filmy/genre_comedy/24898-zhyv-sobi-policeiskyi".into(),
        vec!["https://ashdi.vip/vod/151972".into()],
    )
    .await
    .unwrap();
    println!("{res:#?}");
}

#[tokio::test]
async fn should_load_media_items_for_dle_playlist() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_media_items(
        &sup,
        "seriesss/drama_series/7312-zoryaniy-kreyser-galaktika-1-sezon".into(),
        vec![],
    )
    .await
    .unwrap();
    println!("{res:#?}");
}

#[tokio::test]
async fn should_load_media_items_source() {
    let sup = get_supplier(NAME).unwrap();
    let res = AllContentSuppliers::load_media_item_sources(
        &sup,
        "seriesss/drama_series/7312-zoryaniy-kreyser-galaktika-1-sezon".into(),
        vec![
            "ТакТребаПродакшн (1-2)".into(),
            "https://ashdi.vip/vod/150511".into(),
        ],
    )
    .await
    .unwrap();
    println!("{res:#?}");
}
