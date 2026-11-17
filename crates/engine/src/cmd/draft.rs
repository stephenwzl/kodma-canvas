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
        let mut store = Self { root, entries: Vec::new(), problem: None };
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
            let (doc, _) = super::file::load(&path.to_string_lossy())?;
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
        let bytes = serde_json::to_vec(&next).map_err(error)?;
        if bytes.len() as u64 > INDEX_BYTES {
            return Err(error("本地稿件索引过大"));
        }
        file_access::write(self.root.join("index.json"), &bytes)?;
        self.entries = next;
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

#[cfg(test)]
mod tests;
