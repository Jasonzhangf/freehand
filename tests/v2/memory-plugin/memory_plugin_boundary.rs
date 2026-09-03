use freehand_v2_contracts::SessionId;
use freehand_v2_memory_plugin::{MemoryError, MemoryPlugin, MemoryRecord};

fn session(id: &str) -> SessionId {
    SessionId::try_new(id).expect("session id")
}

fn record(id: &str, session_id: &str, summary: &str) -> MemoryRecord {
    MemoryRecord::new(id, session(session_id), summary, "session-log", None).expect("record")
}

#[test]
fn attach_save_load_search_export_detach_round_trip() {
    let mut plugin = MemoryPlugin::new();
    plugin.attach(session("s1"));
    plugin
        .summarize(record("m1", "s1", "summary alpha"))
        .expect("summarize");
    assert_eq!(plugin.load(&session("s1")).len(), 1);
    assert_eq!(plugin.search("alpha").expect("search").len(), 1);
    assert_eq!(plugin.export(&session("s1")).expect("export").len(), 1);
    plugin.detach(&session("s1")).expect("detach");
}

#[test]
fn save_before_attach_is_rejected() {
    let mut plugin = MemoryPlugin::new();
    let err = plugin
        .summarize(record("m1", "s1", "summary"))
        .expect_err("save");
    assert_eq!(err, MemoryError::NotAttached("s1".to_owned()));
}

#[test]
fn duplicate_summary_is_rejected() {
    let mut plugin = MemoryPlugin::new();
    plugin.attach(session("s1"));
    plugin
        .summarize(record("m1", "s1", "summary"))
        .expect("save");
    let err = plugin
        .summarize(record("m1", "s1", "summary"))
        .expect_err("duplicate");
    assert_eq!(err, MemoryError::Duplicate("m1".to_owned()));
}

#[test]
fn export_without_records_is_rejected() {
    let mut plugin = MemoryPlugin::new();
    plugin.attach(session("s1"));
    let err = plugin.export(&session("s1")).expect_err("export");
    assert_eq!(err, MemoryError::NoRecords("s1".to_owned()));
}
