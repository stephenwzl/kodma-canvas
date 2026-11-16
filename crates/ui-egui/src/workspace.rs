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
    pub active: Option<u64>,
    pub home: bool,
    pub busy: bool,
    pub revision: Option<u64>,
    pub page: Option<u64>,
    pub selection: Vec<Selection>,
}

/// Native UI intent. This is not a programmatic endpoint: Open may request a system picker.
#[derive(Clone, Debug)]
pub enum Action {
    New,
    Open,
    Home,
    Recent(String),
}

/// Internal presentation data for native recent-file buttons; paths are not serialized to tools.
#[derive(Clone, Debug, PartialEq)]
pub struct RecentFile {
    pub path: String,
    pub name: String,
}

impl FigApp {
    pub fn workspace_recents(&self) -> Vec<RecentFile> {
        crate::home_recents::listed(&self.session.prefs.recent_files)
            .take(figcraft_engine::cmd::file::MAX_RECENT)
            .map(|path| RecentFile {
                path: path.clone(),
                name: std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone()),
            })
            .collect()
    }

    pub fn workspace_action(&mut self, action: Action) {
        // A queued button must not navigate behind a system picker or replace its continuation.
        if self.pending_dialog.is_some() {
            return;
        }
        self.ui.last_error = None;
        match action {
            Action::New => {
                if self.run_user("file.new", serde_json::json!({})).is_some() {
                    self.ui.home = false;
                }
            }
            Action::Open => self.open_dialog(),
            Action::Home => {
                self.run("doc.home", serde_json::json!({}));
            }
            Action::Recent(path) => {
                // Capture a path, not an index: recency order can change between click and tick.
                // A button removed meanwhile cannot reopen a file the user already removed.
                if self.workspace_recents().iter().any(|file| file.path == path)
                    && self.run_user("file.openRecent", serde_json::json!({"path":path})).is_some()
                {
                    self.ui.home = false;
                }
            }
        }
    }

    /// Read on the runtime thread after queued commands. Paths, field drafts and private input
    /// are deliberately absent; callers only cache presentation values, not editable documents.
    pub fn workspace_snapshot(&self) -> Snapshot {
        let ds = self.session.ds().ok();
        let home = self.ui.home || !self.has_files();
        Snapshot {
            documents: self
                .session
                .docs
                .iter()
                .filter(|d| self.shows(d.uid))
                .map(|d| Document { id: d.uid, name: d.doc.name.clone(), dirty: d.is_dirty() })
                .collect(),
            active: ds.map(|d| d.uid),
            home,
            busy: self.pending_dialog.is_some(),
            revision: ds.map(|d| d.revision),
            page: (!home).then(|| ds.map(|d| d.page.0)).flatten(),
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
