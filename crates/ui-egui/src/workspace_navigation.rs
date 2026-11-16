//! Bounded read-only navigation projection of the original document. Native disclosure state
//! is presentation only; selection and page changes remain original engine commands.
use serde::Serialize;
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Page {
    pub id: u64,
    pub name: String,
    pub current: bool,
    pub divider: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Layer {
    pub id: u64,
    pub parent: u64,
    pub name: String,
    pub kind: String,
    pub depth: usize,
    pub children: bool,
    pub selected: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Navigation {
    pub pages: Vec<Page>,
    pub layers: Vec<Layer>,
}
impl crate::FigApp {
    pub(super) fn workspace_navigation(&self) -> Navigation {
        let Some(d) = self.session.ds().ok().filter(|d| !self.ui.home && self.shows(d.uid)) else { return Navigation::default() };
        let pages = d
            .doc
            .pages
            .iter()
            .take(1024)
            .filter_map(|id| d.doc.node(*id))
            .map(|n| Page { id: n.id.0, name: n.name.chars().take(256).collect(), current: n.id == d.page, divider: d.doc.is_divider(n.id) })
            .collect();
        let mut layers = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut pending = vec![(d.page, 0usize)];
        // Hostile imported trees cannot recurse forever or allocate unbounded native controls.
        while let Some((id, depth)) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            let Some(n) = d.doc.node(id) else { continue };
            if id != d.page {
                layers.push(Layer {
                    id: id.0,
                    parent: n.parent.map_or(d.page.0, |p| p.0),
                    name: n.name.chars().take(256).collect(),
                    kind: n.kind.type_name().into(),
                    depth: depth.saturating_sub(1),
                    children: !n.children.is_empty(),
                    selected: d.selection.contains(&id),
                });
            }
            if visited.len() >= 10000 {
                break;
            }
            if depth < 128 {
                for child in n.children.iter().take(10000 - pending.len()).copied() {
                    pending.push((child, depth + 1));
                }
            }
        }
        Navigation { pages, layers }
    }
}

#[cfg(test)]
mod tests {
    use crate::{layers_drag_tests::Ui, workspace::Action};
    use figcraft_engine::Session;
    use serde_json::json;
    #[test]
    fn navigation_follows_the_current_page_and_real_parent_order() {
        let mut ui = Ui::new(Session::new());
        let frame = ui.app.run("node.create", json!({"type":"frame","name":"Parent"})).unwrap()["id"].as_u64().unwrap();
        let child = ui.app.run("node.create", json!({"type":"rectangle","name":"Child","parent":frame})).unwrap()["id"].as_u64().unwrap();
        let before = ui.app.workspace_snapshot();
        assert_eq!(
            before.navigation.layers.iter().map(|n| (n.id, n.depth, n.parent)).collect::<Vec<_>>(),
            vec![(frame, 0, before.page.unwrap()), (child, 1, frame)]
        );
        assert!(before.navigation.layers[1].selected);
        let page = before.page.unwrap();
        let owner = before.active.unwrap();
        ui.app.run("page.add", json!({"name":"Empty"}));
        let empty = ui.app.workspace_snapshot();
        assert_eq!(empty.navigation.pages.len(), 2);
        assert!(empty.navigation.layers.is_empty());
        ui.app.workspace_action(Action::Page(owner, page));
        assert_eq!(ui.app.workspace_snapshot().navigation.layers.len(), 2);
        ui.app.workspace_action(Action::Layer(owner, page, child));
        assert_eq!(ui.app.session.selection(), &[figcraft_doc::NodeId(child)]);
        ui.app.run("node.set", json!({"id":child,"props":{"name":"Renamed"}}));
        assert_eq!(ui.app.workspace_snapshot().navigation.layers[1].name, "Renamed");
        ui.app.workspace_action(Action::Home);
        assert!(ui.app.workspace_snapshot().navigation.layers.is_empty());
        assert!(ui.app.workspace_snapshot().navigation.pages.is_empty());
    }
    #[test]
    fn native_tree_intents_refuse_stale_file_page_deleted_and_home_targets() {
        let mut ui = Ui::new(Session::new());
        let id = ui.app.run("shape.rectangle", json!({})).unwrap()["id"].as_u64().unwrap();
        let original = ui.app.workspace_snapshot();
        let owner = original.active.unwrap();
        let page = original.page.unwrap();
        ui.app.run("page.add", json!({}));
        ui.app.workspace_action(Action::Layer(owner, page, id));
        assert!(ui.app.session.selection().is_empty());
        ui.app.workspace_action(Action::Page(owner, page));
        ui.app.run("select.none", json!({})).unwrap();
        ui.app.run("edit.delete", json!({"ids":[id]})).unwrap();
        ui.app.workspace_action(Action::Layer(owner, page, id));
        assert!(ui.app.session.selection().is_empty());
        ui.app.run("file.new", json!({}));
        let b = ui.app.workspace_snapshot();
        ui.app.workspace_action(Action::Page(owner, page));
        ui.app.workspace_action(Action::Layer(owner, page, id));
        assert_eq!(ui.app.workspace_snapshot().active, b.active);
        assert_eq!(ui.app.workspace_snapshot().page, b.page);
        ui.app.workspace_action(Action::Home);
        ui.app.workspace_action(Action::Page(b.active.unwrap(), b.page.unwrap()));
        assert!(ui.app.ui.home);
    }
    #[test]
    fn tree_selection_is_presentation_only_and_keeps_original_history_and_stroke() {
        let mut ui = Ui::new(Session::new());
        let id = ui.app.run("shape.rectangle", json!({"props":{"strokes":["#D8D2DE"],"strokeWeight":3}})).unwrap()["id"].as_u64().unwrap();
        let before = ui.app.session.ds().unwrap().doc.clone();
        let history = ui.app.session.ds().unwrap().history.undo.len();
        let s = ui.app.workspace_snapshot();
        ui.app.workspace_action(Action::Layer(s.active.unwrap(), s.page.unwrap(), id));
        assert_eq!(ui.app.session.ds().unwrap().history.undo.len(), history);
        assert!(std::sync::Arc::ptr_eq(&before, &ui.app.session.ds().unwrap().doc));
        ui.app.run("edit.undo", json!({}));
        assert!(ui.app.session.ds().unwrap().doc.node(figcraft_doc::NodeId(id)).is_none());
    }
}

#[cfg(test)]
mod page_focus_tests {
    use crate::{layers_drag_tests::Ui, workspace::Action};
    use figcraft_engine::Session;
    use serde_json::json;
    #[test]
    fn native_page_changes_clear_the_old_hover_and_the_current_page_is_a_noop() {
        let mut ui = Ui::new(Session::new());
        let id = ui.app.run("shape.rectangle", json!({})).unwrap()["id"].as_u64().unwrap();
        let first = ui.app.workspace_snapshot();
        let owner = first.active.unwrap();
        let page = first.page.unwrap();
        let empty = ui.app.run("page.add", json!({})).unwrap()["id"].as_u64().unwrap();
        ui.app.run("page.select", json!({"id":page})).unwrap();
        ui.app.run("select.set", json!({"ids":[id]})).unwrap();
        ui.app.run("view.set", json!({"zoom":0.75,"cx":42,"cy":56})).unwrap();
        let before = ui.app.session.ds().unwrap().view.clone();
        ui.app.workspace_action(Action::Page(owner, page));
        assert_eq!(ui.app.session.selection(), &[figcraft_doc::NodeId(id)], "clicking the current page clears its selection");
        assert_eq!(ui.app.session.ds().unwrap().view, before);
        ui.app.session.ds_mut().unwrap().hover = Some(figcraft_doc::NodeId(id));
        ui.app.workspace_action(Action::Page(owner, empty));
        assert_eq!(ui.app.session.ds().unwrap().hover, None, "the old page's hover leaks onto an empty native page");
        ui.app.session.ds_mut().unwrap().hover = Some(figcraft_doc::NodeId(id));
        ui.app.workspace_action(Action::OwnedCommand(owner, "page.add"));
        assert_eq!(ui.app.session.ds().unwrap().hover, None, "the add-page button retains a stale hover");
    }
    #[test]
    fn native_hover_does_not_survive_page_selection_and_creation() {
        let mut ui = Ui::new(Session::new());
        let id = ui.app.run("shape.rectangle", json!({})).unwrap()["id"].as_u64().unwrap();
        let first = ui.app.workspace_snapshot();
        let owner = first.active.unwrap();
        let page = first.page.unwrap();
        let empty = ui.app.run("page.add", json!({})).unwrap()["id"].as_u64().unwrap();
        ui.app.run("page.select", json!({"id":page})).unwrap();
        ui.app.session.ds_mut().unwrap().hover = Some(figcraft_doc::NodeId(id));
        ui.app.workspace_action(Action::Page(owner, empty));
        assert_eq!(ui.app.session.ds().unwrap().hover, None);
        ui.app.session.ds_mut().unwrap().hover = Some(figcraft_doc::NodeId(id));
        ui.app.workspace_action(Action::OwnedCommand(owner, "page.add"));
        assert_eq!(ui.app.session.ds().unwrap().hover, None);
    }
}
