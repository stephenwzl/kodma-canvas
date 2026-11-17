//! Read-only native-host observations of the existing Session, never another document model.

use serde::Serialize;

use crate::FigApp;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Document {
    pub id: u64,
    pub name: String,
    pub dirty: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Selection {
    pub id: u64,
    pub name: String,
    pub kind: String,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Snapshot {
    pub documents: Vec<Document>,
    pub navigation: crate::workspace_navigation::Navigation,
    pub active: Option<u64>,
    pub home: bool,
    pub busy: bool,
    pub close_question: Option<u64>,
    pub revision: Option<u64>,
    pub page: Option<u64>,
    pub selection: Vec<Selection>,
    pub page_name: Option<String>,
    pub zoom: f64,
    pub tool: String,
}

/// Native UI intent. This is not a programmatic endpoint: Open may request a system picker.
#[derive(Clone, Debug)]
pub enum Action {
    New,
    Open,
    Home,
    Recent(String),
    RenameRecent(String, String),
    SaveAsRecent(String),
    RemoveRecent(String),
    TrashRecent(String),
    ForgetMissingRecent(String),
    Switch(u64),
    Close(u64),
    Save(u64),
    SaveAs(u64),
    AnswerClose(u64, crate::windows::CloseAnswer),
    Property(crate::workspace_properties::Commit),
    Command(&'static str),
    Page(u64, u64),
    Layer(u64, u64, u64),
    OwnedCommand(u64, &'static str),
}

/// Internal presentation data for native recent-file buttons; paths are not serialized to tools.
#[derive(Clone, Debug, PartialEq)]
pub struct RecentFile {
    pub path: String,
    pub name: String,
    pub managed: bool,
}

impl FigApp {
    pub fn workspace_recents(&self) -> Vec<RecentFile> {
        let managed = self.session.drafts.as_ref().into_iter().flat_map(|store| {
            store.entries.iter().map(|entry| RecentFile {
                path: store.path(&entry.id).to_string_lossy().into_owned(),
                name: entry.name.clone(),
                managed: true,
            })
        });
        let external = crate::home_recents::listed(&self.session.prefs.recent_files)
            .take(figcraft_engine::cmd::file::MAX_RECENT)
            .filter(|path| !self.session.is_managed_path(path))
            .map(|path| RecentFile {
                path: path.clone(),
                name: std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone()),
                managed: false,
            });
        let mut seen = std::collections::HashSet::new();
        let mut files: Vec<_> = managed
            .chain(external)
            .filter(|file| {
                figcraft_engine::file_access::allowed(&file.path, figcraft_engine::file_access::Access::Read) && seen.insert(file.path.clone())
            })
            .map(|mut file| {
                if let Some(ds) = self.session.docs.iter().find(|d| d.path.as_deref() == Some(file.path.as_str())) {
                    file.name = ds.doc.name.clone();
                }
                file
            })
            .collect();
        // Recent operations order both ownership kinds together. The rest of the managed
        // index follows in saved order, without applying the reference host's 20-item cap.
        files.sort_by_key(|f| self.session.prefs.recent_files.iter().position(|p| p == &f.path).unwrap_or(usize::MAX));
        files
    }

    /// Finish managed writes before navigation/lifecycle actions. A failed save leaves the
    /// document open and dirty; existing native error presentation offers retry through Save.
    pub(crate) fn workspace_flush_managed(&mut self) -> bool {
        let owner = self.session.ds().ok().map(|d| d.uid);
        let targets: Vec<_> = self
            .session
            .docs
            .iter()
            .filter(|d| d.is_dirty() && d.path.as_deref().is_some_and(|p| self.session.is_managed_path(p)))
            .map(|d| d.uid)
            .collect();
        let mut success = true;
        for uid in targets {
            if let Some(index) = self.session.docs.iter().position(|d| d.uid == uid) {
                self.session.active = index;
                if self.run("draft.save", serde_json::json!({})).is_none() {
                    success = false;
                    break;
                }
            }
        }
        if let Some(index) = owner.and_then(|uid| self.session.docs.iter().position(|d| d.uid == uid)) {
            self.session.active = index;
        }
        success
    }

    pub fn workspace_action(&mut self, action: Action) {
        // A queued button must not navigate behind a system picker or replace its continuation.
        if let Action::AnswerClose(uid, answer) = action {
            if self.ui.close_tab == Some(crate::windows::TabClose::Ask(uid)) && self.shows(uid) {
                let ctx = self.ui_context.clone().unwrap_or_default();
                crate::windows::answer_close_tab(self, &ctx, answer);
            }
            return;
        }
        if self.pending_dialog.is_some() || self.ui.close_tab.is_some() {
            return;
        }
        self.ui.last_error = None;
        match action {
            Action::Page(owner, page) => {
                if !self.ui.home
                    && self.shows(owner)
                    && self.session.ds().is_ok_and(|d| d.uid == owner && d.page.0 != page)
                    && self.run_user("page.select", serde_json::json!({"id":page})).is_some()
                {
                    // Native rows can take the pointer without an egui PointerGone event.
                    // The old page's hover must not draw over the newly selected page.
                    self.clear_navigation_hover();
                }
            }
            Action::Layer(owner, page, node) => {
                if !self.ui.home
                    && self.shows(owner)
                    && self.session.ds().is_ok_and(|d| {
                        d.uid == owner
                            && d.page.0 == page
                            && d.doc.node(figcraft_doc::NodeId(node)).is_some_and(|n| !n.is_page() && d.doc.ancestors(n.id).contains(&d.page))
                    })
                {
                    self.run_user("select.set", serde_json::json!({"ids":[node]}));
                }
            }
            Action::OwnedCommand(owner, id) => {
                if !self.ui.home && self.session.ds().is_ok_and(|d| d.uid == owner) && self.shows(owner) {
                    let page = self.session.ds().ok().map(|d| d.page);
                    self.run_user(id, serde_json::json!({}));
                    if self.session.ds().ok().map(|d| d.page) != page {
                        self.clear_navigation_hover();
                    }
                }
            }
            Action::Command(id) => {
                self.run_user(id, serde_json::json!({}));
            }
            Action::New => {
                if self.session.drafts.is_some() {
                    // Retry an unpublished initial draft rather than accumulating ghost tabs.
                    let pending = self.session.docs.iter().position(|d| {
                        d.path.as_deref().is_some_and(|p| {
                            self.session.drafts.as_ref().is_some_and(|s| s.id_at(p).is_some_and(|id| !s.entries.iter().any(|e| e.id == id)))
                        })
                    });
                    self.ui.home = false;
                    if let Some(index) = pending {
                        self.session.active = index;
                        self.run_user("draft.save", serde_json::json!({}));
                    } else {
                        match self.services.new_draft_id() {
                            Ok(id) => {
                                self.run_user("draft.create", serde_json::json!({"id":id}));
                            }
                            Err(error) => self.show_error(error),
                        }
                    }
                } else if self.run_user("file.new", serde_json::json!({})).is_some() {
                    self.ui.home = false;
                }
            }
            Action::Switch(uid) => {
                if let Some(index) = self.workspace_index(uid)
                    && self.run("doc.switch", serde_json::json!({"index":index})).is_some()
                {
                    self.ui.home = false;
                }
            }
            Action::Close(uid) => {
                if let Some(index) = self.workspace_index(uid) {
                    if self.session.docs.get(index).and_then(|d| d.path.as_deref()).is_some_and(|p| self.session.is_managed_path(p))
                        && !self.workspace_flush_managed()
                    {
                        return;
                    }
                    crate::windows::ask_close_tab(self, index);
                }
            }
            Action::Save(uid) | Action::SaveAs(uid) => {
                if self.workspace_index(uid).is_some() && self.session.ds().is_ok_and(|d| d.uid == uid) && !self.ui.home {
                    if matches!(action, Action::SaveAs(_)) {
                        self.save_as_dialog();
                    } else {
                        self.save();
                    }
                }
            }
            Action::Property(commit) => {
                self.workspace_commit_property(commit);
            }
            Action::AnswerClose(..) => {}
            Action::Open => self.open_dialog(),
            Action::Home => {
                if self.workspace_flush_managed() {
                    self.run("doc.home", serde_json::json!({}));
                }
            }
            Action::RemoveRecent(path) => {
                if self.workspace_recents().iter().any(|f| f.path == path && !f.managed) {
                    self.run("file.openRecent", serde_json::json!({"remove":path}));
                }
            }
            Action::TrashRecent(path) => {
                if let Some(id) = self.session.drafts.as_ref().and_then(|s| s.id_at(&path)) {
                    self.run_user("draft.trash", serde_json::json!({"id":id,"confirmed":true}));
                }
            }
            Action::ForgetMissingRecent(path) => {
                if let Some(id) = self.session.drafts.as_ref().and_then(|s| s.id_at(&path)) {
                    self.run_user("draft.forgetMissing", serde_json::json!({"id":id}));
                }
            }
            Action::RenameRecent(path, name) => {
                let name = name.trim();
                if !name.is_empty()
                    && self.workspace_recents().iter().any(|f| f.path == path)
                    && self.run_user("file.open", serde_json::json!({"path":path})).is_some()
                    && self.run_user("file.rename", serde_json::json!({"name":name})).is_some()
                {
                    self.run_user("file.save", serde_json::json!({}));
                }
            }
            Action::SaveAsRecent(path) => {
                if self.workspace_recents().iter().any(|f| f.path == path) && self.run_user("file.open", serde_json::json!({"path":path})).is_some() {
                    self.ui.home = false;
                    self.save_as_dialog();
                }
            }
            Action::Recent(path) => {
                // Capture a path, not an index: recency order can change between click and tick.
                // A button removed meanwhile cannot reopen a file the user already removed.
                if self.workspace_recents().iter().any(|file| file.path == path)
                    && self.run_user("file.open", serde_json::json!({"path":path})).is_some()
                {
                    self.ui.home = false;
                }
            }
        }
    }

    fn clear_navigation_hover(&mut self) {
        if let Ok(d) = self.session.ds_mut() {
            d.hover = None;
        }
        self.session.pointer_left();
    }

    fn workspace_index(&self, uid: u64) -> Option<usize> {
        self.session.docs.iter().position(|d| d.uid == uid && self.shows(uid))
    }

    /// Read on the runtime thread after queued commands. Paths, field drafts and private input
    /// are deliberately absent; callers only cache presentation values, not editable documents.
    pub fn workspace_snapshot(&self) -> Snapshot {
        let ds = self.session.ds().ok();
        let home = self.ui.home || !self.has_files();
        Snapshot {
            navigation: self.workspace_navigation(),
            documents: self
                .session
                .docs
                .iter()
                .filter(|d| self.shows(d.uid))
                .map(|d| Document { id: d.uid, name: d.doc.name.clone(), dirty: d.is_dirty() })
                .collect(),
            active: ds.map(|d| d.uid),
            home,
            busy: self.pending_dialog.is_some() || self.ui.close_tab.is_some(),
            close_question: match self.ui.close_tab {
                Some(crate::windows::TabClose::Ask(uid)) if self.workspace_index(uid).is_some() => Some(uid),
                _ => None,
            },
            revision: ds.map(|d| d.revision),
            page: (!home).then(|| ds.map(|d| d.page.0)).flatten(),
            page_name: ds.and_then(|d| d.doc.node(d.page)).map(|p| p.name.clone()),
            zoom: ds.map_or(1.0, |d| d.view.zoom),
            tool: self.session.tool.tool.id().into(),
            selection: ds
                .filter(|_| !home)
                .map(|d| {
                    d.selection
                        .iter()
                        .filter_map(|id| d.doc.node(*id))
                        .map(|node| Selection {
                            id: node.id.0,
                            name: node.name.clone(),
                            kind: node.kind.type_name().into(),
                            width: node.width,
                            height: node.height,
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

/// Keep all hit testing in the original frame layout; native controls reserve real space.
pub(crate) fn reserve(ui: &mut egui::Ui, host: Option<crate::CanvasHost>, panel: egui::Color32) {
    let Some(host) = host else { return };
    let finite_inset = |value: f32, available: f32| if value.is_finite() { value.clamp(0.0, (available - 1.0).max(0.0)) } else { 0.0 };
    let left = finite_inset(host.left_inset, ui.available_width());
    let top = finite_inset(host.top_inset, ui.available_height());
    if left > 0.0 {
        egui::Panel::left("canvas_host_native_left").exact_size(left).resizable(false).frame(egui::Frame::NONE).show(ui, |_| {});
    }
    if top > 0.0 {
        egui::Panel::top("canvas_host_native_top").exact_size(top).resizable(false).frame(egui::Frame::NONE).show(ui, |_| {});
    }
    let right = finite_inset(host.right_inset, ui.available_width());
    if right > 0.0 {
        egui::Panel::right("canvas_host_native_space").exact_size(right).resizable(false).frame(egui::Frame::NONE.fill(panel)).show(ui, |_| {});
    }
}

#[cfg(test)]
mod tests {
    use crate::layers_drag_tests::Ui;
    use figcraft_engine::Session;
    use serde_json::json;

    fn fixture(tag: &str) -> (std::path::PathBuf, String) {
        let dir = std::env::temp_dir().join(format!("kodma-i2-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("actual.figcraft");
        std::fs::write(&path, figcraft_format::save_bytes(&figcraft_doc::Document::new("Actual I2"), None).unwrap()).unwrap();
        (dir, path.to_string_lossy().into_owned())
    }

    #[test]
    fn home_actions_create_a_real_document_and_return_without_losing_the_old_one() {
        let mut ui = Ui::new(Session::new());
        ui.app.run("shape.rectangle", json!({"width":150,"height":100}));
        let original = ui.app.workspace_snapshot();
        ui.app.workspace_action(super::Action::Home);
        assert!(ui.app.ui.home);
        ui.app.workspace_action(super::Action::New);
        assert!(!ui.app.ui.home);
        assert_eq!(ui.app.session.docs.len(), 2);
        assert_ne!(ui.app.workspace_snapshot().active, original.active);
        assert!(ui.app.session.selection().is_empty());
        assert_eq!(ui.app.session.docs[0].uid, original.active.unwrap());
        assert_eq!(ui.app.session.docs[0].selection[0].0, original.selection[0].id);
    }

    #[test]
    fn recent_actions_capture_a_path_across_reordering_and_reuse_an_open_file() {
        let (dir, path) = fixture("reorder");
        let mut ui = Ui::new(Session::new());
        ui.app.session.prefs.recent_files = vec![path.clone()];
        let action = super::Action::Recent(ui.app.workspace_recents()[0].path.clone());
        ui.app.session.prefs.recent_files.insert(0, dir.join("other.figcraft").to_string_lossy().into_owned());
        ui.app.ui.home = true;
        ui.app.workspace_action(action);
        assert!(!ui.app.ui.home);
        assert_eq!(ui.app.session.doc().unwrap().name, "Actual I2");
        let uid = ui.app.session.ds().unwrap().uid;
        ui.app.workspace_action(super::Action::Home);
        ui.app.workspace_action(super::Action::Recent(path));
        assert_eq!(ui.app.session.docs.len(), 2);
        assert_eq!(ui.app.session.ds().unwrap().uid, uid);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_and_invalid_recent_files_leave_the_document_intact_and_report_the_real_error() {
        let (dir, path) = fixture("errors");
        let mut ui = Ui::new(Session::new());
        ui.app.run("shape.rectangle", json!({"width":150,"height":100}));
        let original = ui.app.workspace_snapshot();
        std::fs::remove_file(&path).unwrap();
        ui.app.session.prefs.recent_files = vec![path.clone()];
        ui.app.ui.home = true;
        ui.app.workspace_action(super::Action::Recent(path.clone()));
        assert!(ui.app.ui.home && ui.app.ui.last_error.is_some());
        assert_eq!(ui.app.session.docs.len(), 1);
        assert_eq!(ui.app.session.ds().unwrap().uid, original.active.unwrap());
        assert_eq!(ui.app.workspace_recents().len(), 1, "failed open keeps an explicit removal path");
        ui.app.workspace_action(super::Action::RemoveRecent(path.clone()));
        assert!(ui.app.workspace_recents().is_empty());
        std::fs::write(&path, b"not a figcraft file").unwrap();
        ui.app.session.prefs.recent_files = vec![path.clone()];
        ui.app.workspace_action(super::Action::Recent(path));
        assert!(ui.app.ui.last_error.is_some() && ui.app.ui.home);
        assert_eq!(ui.app.session.docs.len(), 1);
        assert_eq!(ui.app.session.ds().unwrap().selection[0].0, original.selection[0].id);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn native_recents_are_bounded_and_a_removed_button_target_cannot_reopen() {
        let mut ui = Ui::new(Session::new());
        ui.app.session.prefs.recent_files = (0..40).map(|i| format!("/tmp/kodma-i2-unused-{i}.figcraft")).collect();
        let recents = ui.app.workspace_recents();
        assert_eq!(recents.len(), 20);
        let path = recents[0].path.clone();
        ui.app.session.prefs.recent_files.clear();
        ui.app.ui.home = true;
        let before = ui.app.workspace_snapshot();
        ui.app.workspace_action(super::Action::Recent(path));
        assert_eq!(ui.app.workspace_snapshot(), before);
        assert!(ui.app.ui.last_error.is_none());
    }

    #[test]
    fn recent_menu_renames_its_path_and_removal_keeps_open_document_and_disk() {
        let (dir, path) = fixture("menu-target");
        let mut ui = Ui::new(Session::new());
        ui.app.run("file.open", json!({"path":path}));
        let original = ui.app.session.ds().unwrap().uid;
        ui.app.run("file.new", json!({"name":"Other"}));
        let other = ui.app.session.ds().unwrap().uid;
        ui.app.workspace_action(super::Action::RenameRecent(path.clone(), "新名称".into()));
        assert_eq!(ui.app.session.ds().unwrap().uid, original);
        assert_eq!(ui.app.workspace_recents()[0].name, "新名称");
        assert_eq!(ui.app.session.docs.iter().find(|d| d.uid == other).unwrap().doc.name, "Other");
        ui.app.workspace_action(super::Action::RemoveRecent(path.clone()));
        assert!(std::path::Path::new(&path).exists());
        assert!(ui.app.workspace_recents().is_empty());
        let before = ui.app.workspace_snapshot();
        ui.app.workspace_action(super::Action::RenameRecent(path, "Stale".into()));
        assert_eq!(ui.app.workspace_snapshot(), before);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn full_managed_list_merges_recent_operations_and_path_owned_actions() {
        let (dir, external) = fixture("full-managed-list");
        let mut session = Session::empty();
        session.drafts = Some(figcraft_engine::cmd::draft::Store::load(dir.join("Drafts")));
        for n in 1..=26 {
            session.execute("draft.create", &json!({"id":format!("{n:032x}"),"name":format!("稿件 {n:02}")})).unwrap();
            session.execute("file.close", &json!({})).unwrap();
        }
        let mut ui = Ui::new(session);
        assert_eq!(ui.app.workspace_recents().len(), 26);
        let oldest = ui.app.workspace_recents().last().unwrap().path.clone();
        let pending = super::Action::RenameRecent(oldest.clone(), "同名 App".into());
        ui.app.run("file.open", json!({"path":external}));
        assert_eq!(ui.app.workspace_recents().len(), 27);
        assert_eq!(ui.app.workspace_recents()[0].path, external);
        ui.app.workspace_action(pending);
        assert_eq!(ui.app.workspace_recents()[0].path, oldest);
        assert_eq!(ui.app.workspace_recents()[0].name, "同名 App");
        ui.app.workspace_action(super::Action::Recent(external.clone()));
        ui.app.run("shape.rectangle", json!({"width":33,"height":44}));
        let uid = ui.app.session.ds().unwrap().uid;
        ui.app.workspace_action(super::Action::RemoveRecent(external.clone()));
        assert!(ui.app.session.ds().unwrap().is_dirty());
        assert_eq!(ui.app.session.ds().unwrap().uid, uid);
        assert_eq!(ui.app.workspace_recents().len(), 26);
        assert!(std::path::Path::new(&external).is_file());
        ui.app.run("file.open", json!({"path":external}));
        assert_eq!(ui.app.workspace_recents().len(), 27);
        assert_eq!(ui.app.session.ds().unwrap().uid, uid);
        std::fs::remove_dir_all(dir).unwrap();
    }

    struct DraftIds(u64);
    impl crate::Services for DraftIds {
        fn new_draft_id(&mut self) -> Result<String, String> {
            self.0 += 1;
            Ok(format!("{:032x}", self.0))
        }
    }
    #[test]
    fn managed_new_home_close_and_reopen_keep_names_contents_and_identity() {
        let (dir, _) = fixture("managed");
        let root = dir.join("Drafts");
        let mut session = Session::empty();
        session.drafts = Some(figcraft_engine::cmd::draft::Store::load(root.clone()));
        let mut ui = Ui::new(session);
        ui.app.services = Box::new(DraftIds(0));
        ui.app.workspace_action(super::Action::New);
        let uid = ui.app.session.ds().unwrap().uid;
        assert!(!ui.app.session.ds().unwrap().is_dirty());
        assert_eq!(ui.app.workspace_recents().len(), 1);
        let path = ui.app.workspace_recents()[0].path.clone();
        assert!(std::path::Path::new(&path).is_file());
        ui.app.run("shape.rectangle", json!({"width":60,"height":70}));
        ui.app.workspace_action(super::Action::Home);
        assert!(ui.app.ui.home && !ui.app.session.ds().unwrap().is_dirty());
        ui.app.workspace_action(super::Action::RenameRecent(path.clone(), "中文稿名".into()));
        assert_eq!(ui.app.workspace_recents()[0].name, "中文稿名");
        ui.app.workspace_action(super::Action::Close(uid));
        assert!(ui.app.session.docs.is_empty());
        ui.app.workspace_action(super::Action::Recent(path.clone()));
        assert_eq!(ui.app.session.doc().unwrap().name, "中文稿名");
        assert_ne!(ui.app.session.ds().unwrap().uid, uid);
        let count = ui.app.session.docs.len();
        ui.app.workspace_action(super::Action::RemoveRecent(path));
        assert_eq!(ui.app.workspace_recents().len(), 1, "managed content cannot be removed as a recent reference");
        assert_eq!(ui.app.session.docs.len(), count);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn managed_save_failure_cancels_home_close_and_quit_and_retry_creates_no_duplicate() {
        let (dir, _) = fixture("managed-error");
        let root = dir.join("Drafts");
        let mut session = Session::empty();
        session.drafts = Some(figcraft_engine::cmd::draft::Store::load(root.clone()));
        std::fs::write(&root, b"blocked").unwrap();
        let mut ui = Ui::new(session);
        ui.app.services = Box::new(DraftIds(0));
        ui.app.workspace_action(super::Action::New);
        let uid = ui.app.session.ds().unwrap().uid;
        assert!(ui.app.session.ds().unwrap().is_dirty() && ui.app.workspace_recents().is_empty());
        ui.app.workspace_action(super::Action::Home);
        assert!(!ui.app.ui.home);
        ui.app.workspace_action(super::Action::Close(uid));
        assert_eq!(ui.app.session.docs.len(), 1);
        let out = ui.ctx.run_ui(egui::RawInput::default(), |frame| crate::windows::request_close(&mut ui.app, frame.ctx()));
        assert!(out.viewport_output.values().any(|v| v.commands.iter().any(|c| matches!(c, egui::ViewportCommand::CancelClose))));
        assert!(!ui.app.ui.quit);
        assert!(crate::control::handle(&mut ui.app, &ui.ctx, "app.quit", &json!({})).is_err());
        assert!(!ui.app.ui.quit && ui.app.session.ds().unwrap().is_dirty());
        std::fs::remove_file(&root).unwrap();
        ui.app.workspace_action(super::Action::New);
        assert_eq!(ui.app.session.docs.len(), 1);
        assert_eq!(ui.app.session.ds().unwrap().uid, uid);
        assert_eq!(ui.app.workspace_recents().len(), 1);
        assert!(!ui.app.session.ds().unwrap().is_dirty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    struct Picker(Option<std::path::PathBuf>);
    impl crate::Services for Picker {
        fn pick_open(&mut self, _title: &str, _exts: &[&str]) -> crate::DialogFuture {
            Box::pin(std::future::ready(self.0.clone()))
        }
    }

    #[test]
    fn native_open_routes_to_the_existing_async_picker_and_cancel_preserves_home_and_history() {
        let (dir, path) = fixture("picker");
        let mut app = crate::FigApp::new(Session::new(), Box::new(Picker(None)));
        app.ui.home = true;
        let before = app.workspace_snapshot();
        app.workspace_action(super::Action::Open);
        assert!(app.workspace_snapshot().busy);
        app.workspace_action(super::Action::New);
        assert_eq!(app.session.docs.len(), 1, "a pending picker fences other native navigation");
        app.poll_dialog(&egui::Context::default());
        assert_eq!(app.workspace_snapshot(), before);
        app.services = Box::new(Picker(Some(path.into())));
        app.workspace_action(super::Action::Open);
        app.poll_dialog(&egui::Context::default());
        assert!(!app.ui.home && !app.workspace_snapshot().busy);
        assert_eq!(app.session.doc().unwrap().name, "Actual I2");
        assert_eq!(app.session.docs.len(), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn workspace_observation_is_read_only_and_rejects_a_background_target() {
        let mut ui = Ui::new(Session::new());
        ui.app.run("shape.rectangle", json!({"width":150,"height":100}));
        let before = ui.app.workspace_snapshot();
        let history = ui.app.session.ds().unwrap().history.undo.len();
        let observed = crate::control::handle(&mut ui.app, &ui.ctx, "ui.workspace", &json!({})).unwrap();
        assert_eq!(observed["snapshot"], serde_json::to_value(&before).unwrap());
        assert!(observed["snapshot"]["documents"][0].get("path").is_none());
        assert_eq!(ui.app.workspace_snapshot(), before);
        assert_eq!(ui.app.session.ds().unwrap().history.undo.len(), history);
        ui.app.run("file.new", json!({"name":"B"}));
        assert!(crate::control::handle(&mut ui.app, &ui.ctx, "ui.workspace", &json!({"document":before.active})).is_err());
    }

    #[test]
    fn snapshots_follow_real_identity_selection_undo_and_home_without_changing_documents() {
        let mut ui = Ui::new(Session::new());
        ui.app.run("shape.rectangle", json!({"name":"I1 rectangle","width":150,"height":100}));
        let a = ui.app.workspace_snapshot();
        let node = a.selection[0].id;
        ui.app.run("file.new", json!({"name":"B"}));
        let b = ui.app.workspace_snapshot();
        assert_ne!(a.active, b.active);
        ui.app.run("doc.switch", json!({"index":0}));
        ui.app.run("node.set", json!({"ids":[node],"width":180}));
        assert_eq!(ui.app.workspace_snapshot().selection[0].width, 180.0);
        ui.app.run("edit.undo", json!({}));
        assert_eq!(ui.app.workspace_snapshot().selection[0].width, 150.0);
        ui.app.run("doc.home", json!({}));
        let home = ui.app.workspace_snapshot();
        assert!(home.home && home.selection.is_empty() && home.page.is_none());
        assert_eq!(home.documents.len(), 2);
        assert_eq!(home.active, a.active);
        assert!(ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(node)).is_some());
    }
}

#[cfg(test)]
#[path = "workspace_i3_tests.rs"]
mod i3_tests;
