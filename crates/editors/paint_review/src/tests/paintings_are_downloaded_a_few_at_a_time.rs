use super::*;

use block_editor_plugin::FetchResult;

use crate::download::{start, Source};

const AT_ONCE: usize = 8;
const COUNT: usize = 20;

fn answer(host: &EditorHost, body: impl Fn(&str) -> Vec<u8>) -> usize {
    let asked = host.take_fetches();
    for (request, url) in &asked {
        host.set_fetched(*request, FetchResult::Body(body(url)));
    }
    asked.len()
}

fn tree(paths: &[String]) -> Vec<u8> {
    let entries: Vec<_> = paths
        .iter()
        .map(|path| serde_json::json!({"type": "blob", "path": path}))
        .collect();
    serde_json::to_vec(&serde_json::json!({"truncated": false, "tree": entries})).unwrap()
}

#[test]
fn paintings_are_downloaded_a_few_at_a_time() {
    let paths: Vec<String> = (0..COUNT)
        .map(|index| format!("painting.{index:02}.paint"))
        .collect();
    let host = EditorHost::default();
    let mut download = start(&Source::Branch, &host);

    assert_eq!(answer(&host, |_| tree(&paths)), 1);
    assert!(download.poll(&host).is_none());

    let mut requested = 0;
    let mut rounds = 0;
    loop {
        let asked = answer(&host, |url| url.as_bytes().to_vec());
        assert!(asked <= AT_ONCE);
        requested += asked;
        rounds += 1;
        match download.poll(&host) {
            None => assert!(rounds < COUNT),
            Some(found) => {
                let found = found.unwrap();
                assert_eq!(requested, COUNT);
                assert_eq!(rounds, COUNT.div_ceil(AT_ONCE));
                assert_eq!(
                    found.iter().map(|it| it.path.clone()).collect::<Vec<_>>(),
                    paths
                );
                assert!(found[0].data.ends_with(b"snapshots/painting.00.paint"));
                break;
            }
        }
    }

    assert!(download.poll(&host).is_none());
}
