//! Kodma's managed local files. The index contains names/locations only; Session owns all edits.
use crate::{CommandSpec, EngineError, Result, Session, bad, file_access, spec};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
const LIMIT: usize = 10_000;
const INDEX_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    stamp: Option<crate::library::Stamp>,
}
#[derive(Debug)]
pub struct Store {
    pub root: PathBuf,
    pub entries: Vec<Entry>,
    pub problem: Option<String>,
    /// Host-provided recoverable system trash; absent in CLI/Web and reference sessions.
    pub trash: Option<fn(&Path) -> Result<PathBuf>>,
}
fn error(message: impl ToString) -> EngineError {
    EngineError::Other(message.to_string())
}
fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|c| c.is_ascii_hexdigit())
}
pub(super) fn name(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 256 {
        return Err(error("稿件名称须为 1–256 个字符"));
    }
    Ok(value.to_string())
}
impl Store {
    pub fn load(root: PathBuf) -> Self {
        let mut store = Self { root, entries: Vec::new(), problem: None, trash: None };
        if let Err(e) = store.read() {
            store.problem = Some(e.to_string());
        }
        store
    }
    fn read(&mut self) -> Result<()> {
        if !file_access::exists(&self.root) {
            return Ok(());
        }
        let index = self.root.join("index.json");
        if file_access::exists(&index) {
            self.entries = serde_json::from_slice(&file_access::read_capped(&index, INDEX_BYTES)?).map_err(error)?;
            if self.entries.len() > LIMIT || self.entries.iter().any(|e| !valid_id(&e.id) || name(&e.name).is_err()) {
                return Err(error("本地稿件索引无效"));
            }
            let mut ids = std::collections::HashSet::new();
            if self.entries.iter().any(|e| !ids.insert(e.id.clone())) {
                return Err(error("本地稿件索引有重复标识"));
            }
        }
        // A content write may have succeeded before its index update failed. Recover only missing or changed
        // entries at startup; never decode every file on a UI frame, nor silently truncate a list.
        let paths = file_access::read_dir_capped(&self.root, LIMIT + 1)?;
        let mut recovered = 0;
        for path in paths {
            let Some(id) = path.file_stem().and_then(|s| s.to_str()).filter(|id| valid_id(id)) else { continue };
            if path.extension().and_then(|s| s.to_str()) != Some("figcraft") {
                continue;
            }
            let previous = self.entries.iter().position(|e| e.id == id);
            let stamp = crate::library::stat(&path.to_string_lossy()).map_err(error)?;
            if previous.is_some_and(|i| self.entries.get(i).is_some_and(|e| e.stamp.as_ref() == Some(&stamp))) {
                continue;
            }
            recovered += 1;
            if recovered > 32 || (previous.is_none() && self.entries.len() >= LIMIT) {
                return Err(error("本地稿件恢复数量超过本轮安全上限"));
            }
            let (doc, _) = match super::file::load(&path.to_string_lossy()) {
                Ok(value) => value,
                // A damaged file must not disable every other draft. Keep its known identity
                // so it can be trashed explicitly; opening still reports the original error.
                Err(_) => {
                    if previous.is_none() {
                        self.entries.push(Entry { id: id.to_string(), name: "无法读取的稿件".into(), stamp: None });
                    }
                    continue;
                }
            };
            let entry = Entry { id: id.to_string(), name: name(&doc.name)?, stamp: Some(stamp) };
            if let Some(slot) = previous.and_then(|i| self.entries.get_mut(i)) {
                *slot = entry;
            } else {
                self.entries.push(entry);
            }
        }
        Ok(())
    }
    pub fn path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.figcraft"))
    }
    pub fn id_at(&self, path: &str) -> Option<String> {
        let path = Path::new(path);
        let id = path.file_stem()?.to_str()?;
        (valid_id(id) && path == self.path(id)).then(|| id.to_string())
    }
    fn write_entry(&mut self, entry: Entry) -> Result<()> {
        if let Some(problem) = &self.problem {
            return Err(error(problem));
        }
        if self.entries.len() >= LIMIT && !self.entries.iter().any(|e| e.id == entry.id) {
            return Err(error("本地稿件数量超过安全上限"));
        }
        let mut next = self.entries.clone();
        next.retain(|e| e.id != entry.id);
        next.insert(0, entry);
        self.write_index(&next)?;
        self.entries = next;
        Ok(())
    }
    fn write_index(&self, entries: &[Entry]) -> Result<()> {
        let bytes = serde_json::to_vec(entries).map_err(error)?;
        if bytes.len() as u64 > INDEX_BYTES {
            return Err(error("本地稿件索引过大"));
        }
        file_access::write(self.root.join("index.json"), &bytes)?;
        Ok(())
    }
}

impl Session {
    pub fn is_managed_path(&self, path: &str) -> bool {
        self.drafts.as_ref().and_then(|s| s.id_at(path)).is_some()
    }

    /// Called after a real content write, including an asynchronous FileSnapshot writeback.
    pub fn record_draft_save(&mut self, uid: u64) -> Result<()> {
        let Some(ds) = self.docs.iter().find(|d| d.uid == uid) else { return Ok(()) };
        let Some(store) = &mut self.drafts else { return Ok(()) };
        let Some(id) = ds.path.as_deref().and_then(|p| store.id_at(p)) else { return Ok(()) };
        let result = name(&ds.saved_doc.name).and_then(|name| {
            let stamp = crate::library::stat(&store.path(&id).to_string_lossy()).map_err(error)?;
            store.write_entry(Entry { id, name, stamp: Some(stamp) })
        });
        if result.is_err()
            && let Some(ds) = self.docs.iter_mut().find(|d| d.uid == uid)
        {
            // Index failure must not turn into a clean, undiscoverable "saved" document.
            ds.versions_changed = true;
        }
        result
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "draft.trash",
            "Move local draft to system trash",
            "{id, confirmed: true}; saves edits first; requires native trash service",
            false,
            trash
        ),
        spec!("draft.forgetMissing", "Remove missing local draft record", "{id}; refuses existing files or open documents", false, forget_missing),
        spec!("draft.create", "New local draft", "{id (32 hex digits, supplied by the host), name?}", false, create),
        spec!("draft.list", "Local drafts", "{} → {files: [{id, name, path}]}; configured native host only", false, |s, _| {
            let store = s.drafts.as_ref().ok_or_else(|| error("本地稿件存储尚未配置"))?;
            if let Some(problem) = &store.problem {
                return Err(error(problem));
            }
            let files: Vec<_> = store
                .entries
                .iter()
                .filter(|e| file_access::allowed(store.path(&e.id), file_access::Access::Read))
                .map(|e| json!({"id":e.id,"name":e.name,"path":store.path(&e.id)}))
                .collect();
            Ok(json!({"files":files}))
        }),
        spec!("draft.save", "Save local draft", "{}; retry a failed initial save or save the current managed draft", false, |s, _| {
            let path = s.ds()?.path.as_deref().ok_or(EngineError::NoDocument)?;
            if !s.is_managed_path(path) {
                return Err(error("当前稿件不属于应用管理目录"));
            }
            let root = s.drafts.as_ref().ok_or_else(|| error("本地稿件存储尚未配置"))?.root.clone();
            file_access::create_dir_all(root)?;
            s.execute("file.save", &json!({}))
        }),
    ]
}
fn create(s: &mut Session, p: &Value) -> Result<Value> {
    let id = crate::params::req_string("draft.create", p, "id")?;
    if !valid_id(&id) {
        return Err(bad("draft.create", "id: expected 32 hex digits"));
    }
    let display = name(p.get("name").and_then(Value::as_str).unwrap_or("未命名稿件"))?;
    let store = s.drafts.as_ref().ok_or_else(|| error("本地稿件存储尚未配置"))?;
    if let Some(problem) = &store.problem {
        return Err(error(problem));
    }
    if store.entries.len() >= LIMIT {
        return Err(error("本地稿件数量超过安全上限"));
    }
    let path = store.path(&id);
    if file_access::exists(&path) || s.docs.iter().any(|d| d.path.as_deref() == Some(path.to_string_lossy().as_ref())) {
        return Err(error("稿件标识已存在，未覆盖原稿"));
    }
    // Retain the new document on any storage error so retry never loses what the person made.
    let root = store.root.clone();
    s.execute("file.new", &json!({"name":display}))?;
    let ds = s.ds_mut()?;
    ds.path = Some(path.to_string_lossy().into_owned());
    ds.versions_changed = true;
    let uid = ds.uid;
    file_access::create_dir_all(root)?;
    s.execute("file.save", &json!({}))?;
    Ok(json!({"id":id,"document":uid,"path":path}))
}

fn target(s: &Session, p: &Value) -> Result<(String, PathBuf)> {
    let id = crate::params::req_string("draft", p, "id")?;
    let store = s.drafts.as_ref().ok_or_else(|| error("本地稿件存储尚未配置"))?;
    if let Some(problem) = &store.problem {
        return Err(error(problem));
    }
    if !store.entries.iter().any(|entry| entry.id == id) {
        return Err(error("稿件已不在列表中"));
    }
    let path = store.path(&id);
    file_access::allow(&path, file_access::Access::Write)?;
    Ok((id, path))
}
fn forget_missing(s: &mut Session, p: &Value) -> Result<Value> {
    let (id, path) = target(s, p)?;
    if !file_access::missing(&path)? {
        return Err(error("文件仍存在，请使用移到废纸篓"));
    }
    if s.docs.iter().any(|d| d.path.as_deref() == path.to_str()) {
        return Err(error("稿件仍在编辑，请先另存或关闭"));
    }
    let store = s.drafts.as_mut().ok_or(EngineError::NoDocument)?;
    let next: Vec<_> = store.entries.iter().filter(|e| e.id != id).cloned().collect();
    store.write_index(&next)?;
    store.entries = next;
    s.prefs.recent_files.retain(|p| Path::new(p) != path);
    Ok(json!({"removed":id}))
}
fn trash(s: &mut Session, p: &Value) -> Result<Value> {
    if p.get("confirmed").and_then(Value::as_bool) != Some(true) {
        return Ok(json!({"cancelled":true}));
    }
    let (id, path) = target(s, p)?;
    let platform = s.drafts.as_ref().and_then(|d| d.trash).ok_or_else(|| error("系统废纸篓不可用，未删除文件"))?;
    let index = s.docs.iter().position(|d| d.path.as_deref() == path.to_str() && d.branch_of.is_none());
    if index.and_then(|i| s.docs.get(i)).is_some_and(|d| s.live().doc_uid == Some(d.uid)) {
        return Err(error("请先离开稿件的协作会话"));
    }
    let owner = s.ds().ok().map(|d| d.uid);
    if let Some(i) = index {
        s.active = i;
        let saved = if s.ds()?.is_dirty() { s.execute("draft.save", &json!({})).map(|_| ()) } else { Ok(()) };
        if let Some(i) = owner.and_then(|uid| s.docs.iter().position(|d| d.uid == uid)) {
            s.active = i;
        }
        saved?;
    }
    let store = s.drafts.as_mut().ok_or(EngineError::NoDocument)?;
    let next: Vec<_> = store.entries.iter().filter(|e| e.id != id).cloned().collect();
    // Preflight the index before touching content. A crash before the OS move is recovered by
    // startup's orphan scan. Keep the in-memory entry and every tab until the OS confirms success.
    store.write_index(&next)?;
    let trashed = match platform(&path) {
        Ok(path) => path,
        Err(e) => {
            let rollback = store.write_index(&store.entries);
            return Err(error(match rollback {
                Ok(()) => format!("未能移到废纸篓：{e}"),
                Err(restore) => format!("未能移到废纸篓：{e}；索引恢复失败：{restore}。原文件与编辑仍保留"),
            }));
        }
    };
    store.entries = next;
    s.prefs.recent_files.retain(|p| Path::new(p) != path);
    if let Some(i) = index {
        // The target was saved above. Removing its UID also invalidates all pending snapshots;
        // they encode only and cannot recreate a file after its tab has gone.
        s.execute("file.close", &json!({"index":i,"force":true}))?;
    }
    Ok(json!({"id":id,"trashed":trashed}))
}

#[cfg(test)]
mod tests;
