use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn session(label: &str) -> (Session, PathBuf) {
    let root = std::env::temp_dir().join(format!("kodma-i3-{label}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let mut s = Session::empty();
    s.drafts = Some(Store::load(root.clone()));
    (s, root)
}
const A: &str = "11111111111111111111111111111111";
const B: &str = "22222222222222222222222222222222";
#[test]
fn creation_names_identity_and_restart_use_real_files_not_recents() {
    let (mut s, root) = session("restart");
    s.execute("draft.create", &json!({"id":A,"name":"阅读记录"})).unwrap();
    let uid = s.ds().unwrap().uid;
    s.execute("shape.rectangle", &json!({"x":10,"width":40,"height":50})).unwrap();
    s.execute("file.rename", &json!({"name":"重命名"})).unwrap();
    s.execute("draft.save", &json!({})).unwrap();
    let original = s.ds().unwrap().path.clone().unwrap();
    s.execute("draft.create", &json!({"id":B,"name":"重命名"})).unwrap();
    assert_ne!(original, s.ds().unwrap().path.clone().unwrap());
    let mut other = Session::empty();
    other.drafts = Some(Store::load(root.clone()));
    let list = other.execute("draft.list", &json!({})).unwrap();
    assert_eq!(list["files"].as_array().unwrap().len(), 2);
    other.execute("file.open", &json!({"path":original})).unwrap();
    assert_ne!(uid, other.ds().unwrap().uid);
    assert_eq!(other.doc().unwrap().name, "重命名");
    assert_eq!(other.doc().unwrap().nodes().filter(|n| !n.is_page()).count(), 1);
    assert!(s.execute("draft.create", &json!({"id":A})).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn failed_initial_write_keeps_editable_draft_and_retry_publishes_one_entry() {
    let (mut s, root) = session("initial-failure");
    std::fs::write(&root, b"blocked directory").unwrap();
    assert!(s.execute("draft.create", &json!({"id":A})).is_err());
    let uid = s.ds().unwrap().uid;
    assert!(s.ds().unwrap().is_dirty());
    assert_eq!(s.execute("draft.list", &json!({})).unwrap()["files"], json!([]));
    s.execute("shape.rectangle", &json!({"width":40,"height":50})).unwrap();
    std::fs::remove_file(&root).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    s.execute("draft.save", &json!({})).unwrap();
    assert_eq!(uid, s.ds().unwrap().uid);
    assert!(!s.ds().unwrap().is_dirty());
    assert_eq!(s.execute("draft.list", &json!({})).unwrap()["files"].as_array().unwrap().len(), 1);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn index_failure_is_dirty_and_orphan_content_is_recovered_on_restart() {
    let (mut s, root) = session("index-failure");
    std::fs::create_dir_all(root.join("index.json")).unwrap();
    assert!(s.execute("draft.create", &json!({"id":A,"name":"保留内容"})).is_err());
    assert!(s.ds().unwrap().is_dirty());
    assert!(root.join(format!("{A}.figcraft")).is_file());
    std::fs::remove_dir(root.join("index.json")).unwrap();
    let recovered = Store::load(root.clone());
    assert!(recovered.problem.is_none());
    assert_eq!(recovered.entries[0].name, "保留内容");
    s.execute("draft.save", &json!({})).unwrap();
    assert!(!s.ds().unwrap().is_dirty());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn invalid_and_unconfigured_calls_never_create_files() {
    let (mut s, root) = session("invalid");
    for id in ["../bad", "", &"a".repeat(100)] {
        assert!(s.execute("draft.create", &json!({"id":id})).is_err());
    }
    assert!(s.execute("draft.create", &json!({"id":A,"name":"   "})).is_err());
    assert!(s.docs.is_empty() && !root.exists());
    assert!(Session::empty().execute("draft.create", &json!({"id":A})).is_err());
}

#[test]
fn save_as_preserves_dirty_original_and_rejects_late_snapshot_after_destination_changes() {
    let (mut s, root) = session("save-as");
    s.execute("draft.create", &json!({"id":A,"name":"原稿"})).unwrap();
    let original = s.ds().unwrap().path.clone().unwrap();
    s.execute("shape.rectangle", &json!({"width":40,"height":50})).unwrap();
    let late = super::super::file::FileSnapshot::of(s.ds().unwrap());
    let bytes = late.encode().unwrap();
    let outside = root.with_extension("external.figcraft");
    s.execute("file.saveAs", &json!({"path":outside})).unwrap();
    assert!(!late.save(&mut s, &bytes).unwrap());
    s.execute("file.rename", &json!({"name":"外部副本"})).unwrap();
    s.execute("file.save", &json!({})).unwrap();
    s.execute("file.open", &json!({"path":original})).unwrap();
    assert_eq!(s.doc().unwrap().name, "原稿");
    assert_eq!(s.doc().unwrap().nodes().filter(|n| !n.is_page()).count(), 1);
    assert_eq!(s.execute("draft.list", &json!({})).unwrap()["files"].as_array().unwrap().len(), 1);
    std::fs::remove_file(outside).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn failed_index_rename_reconciles_the_existing_entry_after_restart() {
    let (mut s, root) = session("stale-name");
    s.execute("draft.create", &json!({"id":A,"name":"旧名"})).unwrap();
    let old_index = std::fs::read(root.join("index.json")).unwrap();
    s.execute("file.rename", &json!({"name":"新的更长名字"})).unwrap();
    s.execute("draft.save", &json!({})).unwrap();
    // Reproduce a crash after content commits but before the new index commits.
    std::fs::write(root.join("index.json"), old_index).unwrap();
    let recovered = Store::load(root.clone());
    assert!(recovered.problem.is_none());
    assert_eq!(recovered.entries[0].name, "新的更长名字");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn snapshot_index_name_matches_saved_bytes_while_later_rename_stays_dirty() {
    let (mut s, root) = session("snapshot-name");
    s.execute("draft.create", &json!({"id":A,"name":"第一版"})).unwrap();
    s.execute("shape.rectangle", &json!({"width":40,"height":50})).unwrap();
    let snapshot = super::super::file::FileSnapshot::of(s.ds().unwrap());
    let bytes = snapshot.encode().unwrap();
    s.execute("file.rename", &json!({"name":"尚未保存的名称"})).unwrap();
    assert!(snapshot.save(&mut s, &bytes).unwrap());
    assert!(s.ds().unwrap().is_dirty());
    assert_eq!(s.execute("draft.list", &json!({})).unwrap()["files"][0]["name"], "第一版");
    assert!(s.execute("file.rename", &json!({"name":"  "})).is_err());
    s.execute("draft.save", &json!({})).unwrap();
    assert_eq!(s.execute("draft.list", &json!({})).unwrap()["files"][0]["name"], "尚未保存的名称");
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn outside_write_is_not_overwritten_by_managed_save() {
    let (mut s, root) = session("conflict");
    s.execute("draft.create", &json!({"id":A})).unwrap();
    let path = s.ds().unwrap().path.clone().unwrap();
    s.execute("shape.rectangle", &json!({"width":40,"height":50})).unwrap();
    let external = figcraft_format::save_bytes(&figcraft_doc::Document::new("外部修改"), None).unwrap();
    figcraft_format::write_file_atomic(std::path::Path::new(&path), &external).unwrap();
    assert!(s.execute("draft.save", &json!({})).is_err());
    assert!(s.ds().unwrap().is_dirty());
    assert_eq!(std::fs::read(path).unwrap(), external);
    std::fs::remove_dir_all(root).unwrap();
}
