use crate::{
    layers_drag_tests::Ui,
    workspace::Action,
    workspace_properties::{Commit, Field},
};
use figcraft_engine::Session;
use serde_json::json;
fn setup() -> (Ui, u64) {
    let mut ui = Ui::new(Session::new());
    let id = ui.app.run("shape.rectangle", json!({"name":"A","x":120,"y":200,"width":150,"height":100,"props":{"fills":["#61B8F6"]}})).unwrap()["id"]
        .as_u64()
        .unwrap();
    (ui, id)
}
fn draft(ui: &mut Ui, field: Field, value: &str) -> Commit {
    Commit { target: ui.app.workspace_inspector().target.unwrap(), field, value: value.into() }
}
#[test]
fn one_commit_is_one_original_undo_and_only_changes_its_field() {
    let (mut ui, id) = setup();
    let before = ui.app.session.ds().unwrap().history.undo.len();
    let edit = draft(&mut ui, Field::Width, "180");
    assert!(ui.app.workspace_commit_property(edit));
    assert_eq!(ui.app.session.ds().unwrap().history.undo.len(), before + 1);
    let node = ui.app.run("node.get", json!({"id":id})).unwrap();
    assert_eq!(node["width"], 180.0);
    assert_eq!(node["height"], 100.0);
    ui.app.run("edit.undo", json!({}));
    assert_eq!(ui.app.run("node.get", json!({"id":id})).unwrap()["width"], 150.0);
    ui.app.run("edit.redo", json!({}));
    assert_eq!(ui.app.workspace_inspector().fields.iter().find(|p| p.field == Field::Width).unwrap().value.as_deref(), Some("180"));
}

#[test]
fn read_commands_and_view_changes_do_not_cancel_an_unchanged_document_draft() {
    let (mut ui, id) = setup();
    let edit = draft(&mut ui, Field::Width, "180");
    ui.app.run("node.get", json!({"id":id}));
    ui.app.run("view.set", json!({"cx":300,"cy":200}));
    assert!(ui.app.workspace_commit_property(edit));
    assert_eq!(ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(id)).unwrap().width, 180.);
    ui.app.run("edit.undo", json!({}));
    assert_eq!(ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(id)).unwrap().width, 150.);
}
#[test]
fn same_node_ids_in_different_files_and_round_trips_never_reuse_a_draft() {
    let (mut ui, a) = setup();
    let uid = ui.app.session.ds().unwrap().uid;
    let edit = draft(&mut ui, Field::Name, "late");
    ui.app.workspace_action(Action::New);
    let b = ui.app.run("shape.rectangle", json!({"width":220,"height":120})).unwrap()["id"].as_u64().unwrap();
    assert_eq!(a, b);
    assert!(!ui.app.workspace_commit_property(edit.clone()));
    ui.app.workspace_action(Action::Switch(uid));
    assert!(!ui.app.workspace_commit_property(edit));
    assert_eq!(ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(a)).unwrap().name, "A");
    let edit = draft(&mut ui, Field::Width, "999");
    ui.app.workspace_action(Action::Home);
    ui.app.workspace_action(Action::Switch(uid));
    assert!(!ui.app.workspace_commit_property(edit));
}

#[test]
fn a_native_menu_command_keeps_its_file_owner_and_home_does_not_edit_hidden_files() {
    let (mut ui, a) = setup();
    let owner = ui.app.session.ds().unwrap().uid;
    ui.app.workspace_action(Action::New);
    let b = ui.app.run("shape.rectangle", json!({"width":220,"height":120})).unwrap()["id"].as_u64().unwrap();
    assert_eq!(a, b);
    ui.app.workspace_action(Action::OwnedCommand(owner, "edit.delete"));
    assert!(ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(b)).is_some());
    ui.app.workspace_action(Action::Switch(owner));
    ui.app.workspace_action(Action::Home);
    ui.app.workspace_action(Action::OwnedCommand(owner, "edit.delete"));
    assert!(ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(a)).is_some());
    ui.app.workspace_action(Action::Switch(owner));
    ui.app.workspace_action(Action::OwnedCommand(owner, "edit.delete"));
    assert!(ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(a)).is_none());
}
#[test]
fn selection_round_trip_external_edit_deletion_and_close_invalidate() {
    let (mut ui, a) = setup();
    let original = draft(&mut ui, Field::Width, "999");
    let b = ui.app.run("shape.rectangle", json!({"width":70,"height":50})).unwrap()["id"].as_u64().unwrap();
    ui.app.run("select.set", json!({"ids":[a]}));
    assert!(!ui.app.workspace_commit_property(original));
    let edit = draft(&mut ui, Field::Width, "999");
    ui.app.run("node.set", json!({"ids":[a],"props":{"width":180}}));
    assert!(!ui.app.workspace_commit_property(edit));
    let edit = draft(&mut ui, Field::Name, "late");
    ui.app.run("edit.delete", json!({"ids":[a]}));
    assert!(!ui.app.workspace_commit_property(edit));
    ui.app.run("select.set", json!({"ids":[b]}));
    let edit = draft(&mut ui, Field::Name, "late");
    ui.app.run("file.close", json!({"force":true}));
    assert!(!ui.app.workspace_commit_property(edit));
}
#[test]
fn multi_selection_exposes_only_supported_common_fields_and_commits_one_transaction() {
    let (mut ui, a) = setup();
    let b =
        ui.app.run("shape.rectangle", json!({"width":80,"height":50,"props":{"opacity":0.5,"fills":["#99D9BD"]}})).unwrap()["id"].as_u64().unwrap();
    ui.app.run("select.set", json!({"ids":[a,b]}));
    let view = ui.app.workspace_inspector();
    assert!(view.fields.iter().find(|p| p.field == Field::Opacity).unwrap().mixed);
    assert!(view.fields.iter().find(|p| p.field == Field::Fill).unwrap().mixed);
    assert!(!view.fields.iter().find(|p| p.field == Field::Width).unwrap().enabled);
    let before = ui.app.session.ds().unwrap().history.undo.len();
    let edit = draft(&mut ui, Field::Opacity, "75%");
    assert!(ui.app.workspace_commit_property(edit));
    assert_eq!(ui.app.session.ds().unwrap().history.undo.len(), before + 1);
    for id in [a, b] {
        assert_eq!(ui.app.run("node.get", json!({"id":id})).unwrap()["opacity"], 0.75);
    }
    ui.app.run("edit.undo", json!({}));
    assert!(ui.app.workspace_inspector().fields.iter().find(|p| p.field == Field::Opacity).unwrap().mixed);
}
#[test]
fn invalid_numbers_locked_and_disabled_fields_cannot_mutate_or_add_history() {
    let (mut ui, id) = setup();
    let before = ui.app.session.ds().unwrap().history.undo.len();
    for (field, value) in [(Field::Width, "NaN"), (Field::Width, "-1"), (Field::Opacity, "120%"), (Field::Fill, "broken"), (Field::Fill, "#11223380")]
    {
        let edit = draft(&mut ui, field, value);
        assert!(!ui.app.workspace_commit_property(edit));
    }
    assert_eq!(ui.app.session.ds().unwrap().history.undo.len(), before);
    let edit = draft(&mut ui, Field::Name, "late");
    ui.app.run("node.set", json!({"ids":[id],"props":{"locked":true}}));
    assert!(!ui.app.workspace_commit_property(edit));
    assert!(ui.app.workspace_inspector().fields.iter().all(|p| !p.enabled));
    let edit = draft(&mut ui, Field::Name, "locked");
    assert!(!ui.app.workspace_commit_property(edit));
}
#[test]
fn color_edits_preserve_paint_metadata_and_complex_fills_stay_disabled() {
    let (mut ui, id) = setup();
    ui.app.run("fill.set", json!({"ids":[id],"index":0,"opacity":0.4,"visible":false,"blend":"multiply"}));
    let before = ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(id)).unwrap().fills[0].clone();
    let edit = draft(&mut ui, Field::Fill, "#112233");
    assert!(ui.app.workspace_commit_property(edit));
    let after = &ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(id)).unwrap().fills[0];
    assert_eq!(after.opacity, before.opacity);
    assert_eq!(after.visible, before.visible);
    assert_eq!(after.blend, before.blend);
    ui.app.run("fill.add", json!({"ids":[id],"color":"#fff"}));
    let edit = draft(&mut ui, Field::Fill, "#000");
    assert!(!ui.app.workspace_commit_property(edit));
    assert_eq!(ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(id)).unwrap().fills.len(), 2);
}

#[test]
fn an_rgb_only_native_field_cannot_flatten_a_translucent_paint_color() {
    let (mut ui, id) = setup();
    // A native file may contain an unnormalised solid colour. Normal command-created RGBA
    // paints keep alpha in Paint::opacity instead (covered separately below).
    let doc = std::sync::Arc::make_mut(&mut ui.app.session.ds_mut().unwrap().doc);
    doc.get_mut(figcraft_doc::NodeId(id)).unwrap().fills[0].kind =
        figcraft_color::PaintKind::Solid { color: figcraft_color::Color::new(0.1, 0.2, 0.3, 0.5) };
    let original = ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(id)).unwrap().fills.clone();
    let before = ui.app.session.ds().unwrap().history.undo.len();
    let edit = draft(&mut ui, Field::Fill, "#445566");
    assert!(!ui.app.workspace_commit_property(edit));
    assert!(!ui.app.workspace_inspector().fields.iter().find(|p| p.field == Field::Fill).unwrap().enabled);
    assert_eq!(ui.app.session.ds().unwrap().history.undo.len(), before);
    assert_eq!(ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(id)).unwrap().fills, original);
}

#[test]
fn normal_rgba_fill_keeps_its_opacity_and_color_alpha_when_rgb_changes() {
    let (mut ui, id) = setup();
    ui.app.run("node.set", json!({"ids":[id],"props":{"fills":["#11223380"]}}));
    let before = ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(id)).unwrap().fills[0].clone();
    let edit = draft(&mut ui, Field::Fill, "#445566");
    assert!(ui.app.workspace_commit_property(edit));
    let after = &ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(id)).unwrap().fills[0];
    assert_eq!(after.opacity, before.opacity);
    assert_eq!(after.color_alpha, before.color_alpha);
    assert_eq!(after.visible, before.visible);
    assert_eq!(after.blend, before.blend);
    ui.app.run("edit.undo", json!({}));
    assert_eq!(ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(id)).unwrap().fills[0], before);
}
#[test]
fn plain_text_edit_and_size_are_independent_and_rich_text_is_not_flattened() {
    let (mut ui, _) = setup();
    let id = ui.app.run("text.create", json!({"text":"你好 A","props":{"fontSize":32}})).unwrap()["id"].as_u64().unwrap();
    let edit = draft(&mut ui, Field::Text, "你好，Kodma 中文");
    assert!(ui.app.workspace_commit_property(edit));
    let node = ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(id)).unwrap();
    assert_eq!(node.kind.text().unwrap().style.font_size, 32.0);
    let edit = draft(&mut ui, Field::FontSize, "24");
    assert!(ui.app.workspace_commit_property(edit));
    let doc = std::sync::Arc::make_mut(&mut ui.app.session.ds_mut().unwrap().doc);
    let text = doc.get_mut(figcraft_doc::NodeId(id)).unwrap().kind.text_mut().unwrap();
    text.runs.push(figcraft_doc::StyleRun { start: 0, end: 1, style: figcraft_doc::TextStyle { font_size: 18.0, ..Default::default() } });
    let original = text.clone();
    let view = ui.app.workspace_inspector();
    assert!(view.fields.iter().filter(|p| matches!(p.field, Field::Text | Field::FontSize)).all(|p| !p.enabled));
    let edit = draft(&mut ui, Field::Text, "flatten");
    assert!(!ui.app.workspace_commit_property(edit));
    assert_eq!(ui.app.session.doc().unwrap().node(figcraft_doc::NodeId(id)).unwrap().kind.text().unwrap(), &original);
}

#[test]
fn version_preview_and_empty_selection_refuse_commits_without_history() {
    let (mut ui, _) = setup();
    let edit = draft(&mut ui, Field::Width, "999");
    ui.app.run("file.saveVersion", json!({"name":"Readonly test"})).unwrap();
    ui.app.run("version.preview", json!({"index":0})).unwrap();
    assert!(!ui.app.workspace_commit_property(edit));
    assert!(ui.app.workspace_inspector().fields.iter().all(|p| !p.enabled));
    ui.app.run("version.exitPreview", json!({}));
    ui.app.run("select.set", json!({"ids":[]}));
    assert!(ui.app.workspace_inspector().target.is_none());
}
