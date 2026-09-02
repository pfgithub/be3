use super::*;

#[test]
fn an_unreadable_games_index_is_reported() {
    let host = EditorHost::default();
    let mut catalog = Catalog::default();

    catalog.poll(&host);
    answer(&host, AssetResult::Failed("no such asset".to_owned()));
    catalog.poll(&host);

    assert!(catalog.installed());
    assert!(catalog.games().is_empty());
    assert_eq!(catalog.errors(), ["games.json: no such asset"]);
}
