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
    pub revision: Option<u64>,
    pub page: Option<u64>,
    pub selection: Vec<Selection>,
}

impl FigApp {
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
