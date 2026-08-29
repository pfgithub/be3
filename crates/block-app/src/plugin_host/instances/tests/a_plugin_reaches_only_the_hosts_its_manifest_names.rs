use super::*;

#[test]
fn a_plugin_reaches_only_the_hosts_its_manifest_names() {
    let hosts = vec!["api.github.com".to_owned()];

    assert!(allowed(
        "https://api.github.com/repos/be3/git/trees",
        &hosts
    ));
    assert!(allowed("https://api.github.com", &hosts));

    assert!(!allowed("http://api.github.com/repos", &hosts));
    assert!(!allowed("https://raw.githubusercontent.com/be3", &hosts));
    assert!(!allowed("https://api.github.com.evil.test/repos", &hosts));
    assert!(!allowed("https://api.github.com@evil.test/repos", &hosts));
    assert!(!allowed("https://api.github.com:8443/repos", &hosts));
    assert!(!allowed("https://evil.test/?q=api.github.com", &hosts));
    assert!(!allowed("https://api.github.com/repos", &[]));
}
