use freehand_v2_search_plugin::{SearchError, SearchPlugin, SearchRecord};

fn record(id: &str, keywords: &[&str]) -> SearchRecord {
    SearchRecord::new(
        id,
        "session",
        "session-log",
        keywords.iter().map(|s| s.to_string()).collect(),
        None,
    )
    .expect("record")
}

#[test]
fn index_query_and_cache_are_deterministic() {
    let mut plugin = SearchPlugin::new();
    plugin
        .index(record("s1", &["alpha", "beta"]))
        .expect("index");
    plugin.index(record("s2", &["beta"])).expect("index");
    let first = plugin.query("beta").expect("query");
    let second = plugin.query("beta").expect("cached query");
    assert_eq!(first, second);
    assert_eq!(first.len(), 2);
}

#[test]
fn invalidate_clears_cache() {
    let mut plugin = SearchPlugin::new();
    plugin.index(record("s1", &["alpha"])).expect("index");
    plugin.query("alpha").expect("query");
    plugin.invalidate();
    plugin.index(record("s2", &["alpha"])).expect("index");
    assert_eq!(plugin.query("alpha").expect("query").len(), 2);
}

#[test]
fn empty_keyword_is_rejected() {
    let mut plugin = SearchPlugin::new();
    plugin.index(record("s1", &["alpha"])).expect("index");
    let err = plugin.query("").expect_err("empty keyword");
    assert_eq!(err, SearchError::EmptyKeyword);
}

#[test]
fn duplicate_index_is_rejected() {
    let mut plugin = SearchPlugin::new();
    plugin.index(record("s1", &["alpha"])).expect("index");
    let err = plugin
        .index(record("s1", &["beta"]))
        .expect_err("duplicate");
    assert_eq!(err, SearchError::Duplicate("s1".to_owned()));
}
