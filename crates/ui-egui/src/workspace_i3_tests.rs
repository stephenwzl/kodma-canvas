//! Native intents exercise the original tab state, async save flow and history.
use super::Action;
use crate::{CanvasHost, layers_drag_tests::Ui, windows::CloseAnswer};
use figcraft_engine::Session;
use serde_json::json;

fn two() -> (Ui, u64, u64) {
    let mut ui = Ui::new(Session::new());
    ui.app.canvas_host = Some(CanvasHost { respect_home: true, ..Default::default() });
    let a = ui.app.session.ds().unwrap().uid;
    ui.app.run("shape.rectangle", json!({"width":150,"height":100}));
    ui.app.run("view.set", json!({"zoom":1.25,"cx":120,"cy":160}));
    ui.app.workspace_action(Action::New);
    let b = ui.app.session.ds().unwrap().uid;
    ui.app.run("shape.rectangle", json!({"width":240,"height":180}));
    ui.app.run("view.set", json!({"zoom":0.75,"cx":500,"cy":300}));
    (ui, a, b)
}

#[test]
fn uid_navigation_preserves_each_selection_view_and_undo_redo() {
    let (mut ui, a, b) = two();
    let b_selection = ui.app.session.ds().unwrap().selection.clone();
    let b_view = ui.app.run("view.get", json!({})).unwrap();
    ui.app.workspace_action(Action::Home);
    ui.app.workspace_action(Action::Switch(a));
    assert!(!ui.app.ui.home);
    assert_eq!(ui.app.workspace_snapshot().selection[0].width, 150.0);
    let a_view = ui.app.run("view.get", json!({})).unwrap();
    ui.app.run("edit.undo", json!({}));
    ui.app.workspace_action(Action::Switch(b));
    assert_eq!(ui.app.session.ds().unwrap().selection, b_selection);
    assert_eq!(ui.app.run("view.get", json!({})).unwrap(), b_view);
    assert_eq!(ui.app.workspace_snapshot().selection[0].width, 240.0);
    ui.app.workspace_action(Action::Switch(a));
    ui.app.run("edit.redo", json!({}));
    assert_eq!(ui.app.workspace_snapshot().selection[0].width, 150.0);
    assert_eq!(ui.app.run("view.get", json!({})).unwrap(), a_view);
}

#[test]
fn close_question_and_answers_are_pinned_and_fence_other_navigation() {
    let (mut ui, a, b) = two();
    ui.app.workspace_action(Action::Close(a));
    assert_eq!(ui.app.workspace_snapshot().close_question, Some(a));
    assert!(ui.app.workspace_snapshot().busy);
    ui.app.workspace_action(Action::Switch(a));
    ui.app.workspace_action(Action::New);
    assert_eq!(ui.app.session.ds().unwrap().uid, b);
    assert_eq!(ui.app.session.docs.len(), 2);
    ui.app.workspace_action(Action::AnswerClose(b, CloseAnswer::DontSave));
    assert_eq!(ui.app.session.docs.len(), 2);
    ui.app.workspace_action(Action::AnswerClose(a, CloseAnswer::Cancel));
    assert!(!ui.app.workspace_snapshot().busy);
    ui.app.workspace_action(Action::Close(a));
    ui.app.workspace_action(Action::AnswerClose(a, CloseAnswer::DontSave));
    assert_eq!(ui.app.session.ds().unwrap().uid, b);
    ui.app.workspace_action(Action::Close(a));
    ui.app.workspace_action(Action::Switch(a));
    ui.app.workspace_action(Action::AnswerClose(a, CloseAnswer::DontSave));
    assert_eq!(ui.app.session.ds().unwrap().uid, b);
    assert_eq!(ui.app.session.docs.len(), 1);
    ui.app.workspace_action(Action::Close(b));
    ui.app.workspace_action(Action::AnswerClose(b, CloseAnswer::DontSave));
    assert!(ui.app.workspace_snapshot().home);
    assert!(ui.app.workspace_snapshot().documents.is_empty());
}

#[test]
fn foreign_window_and_stale_save_intents_cannot_touch_another_document() {
    let (mut ui, a, b) = two();
    ui.app.workspace_action(Action::Save(a));
    ui.app.workspace_action(Action::SaveAs(a));
    assert!(!ui.app.workspace_snapshot().busy);
    ui.app.windows.current = 99;
    ui.app.workspace_action(Action::Switch(a));
    ui.app.workspace_action(Action::Close(b));
    assert_eq!(ui.app.session.ds().unwrap().uid, b);
    assert!(ui.app.ui.close_tab.is_none());
}

#[test]
fn saving_a_background_close_target_preserves_the_foreground_on_failure() {
    let (mut ui, a, b) = two();
    ui.app.session.docs[0].path = Some("/kodma-nonexistent-parent-for-test/a.figcraft".into());
    ui.app.workspace_action(Action::Close(a));
    ui.app.workspace_action(Action::AnswerClose(a, CloseAnswer::Save));
    ui.frame(vec![]);
    assert_eq!(ui.app.session.ds().unwrap().uid, b);
    assert_eq!(ui.app.session.docs.len(), 2);
    assert!(ui.app.session.docs[0].is_dirty());
    assert!(ui.app.ui.last_error.is_some());
}

struct SavePicker(Option<std::path::PathBuf>);
impl crate::Services for SavePicker {
    fn pick_save(&mut self, _: &str, _: &str, _: &[&str]) -> crate::DialogFuture {
        Box::pin(std::future::ready(self.0.clone()))
    }
}
#[test]
fn background_close_save_as_cancellation_and_success_do_not_retarget_foreground() {
    let (mut ui, a, b) = two();
    ui.app.services = Box::new(SavePicker(None));
    ui.app.workspace_action(Action::Close(a));
    ui.app.workspace_action(Action::AnswerClose(a, CloseAnswer::Save));
    ui.frame(vec![]);
    assert_eq!(ui.app.session.ds().unwrap().uid, b);
    assert!(ui.app.workspace_snapshot().busy);
    ui.frame(vec![]);
    assert_eq!(ui.app.session.docs.len(), 2);
    assert!(ui.app.session.docs[0].is_dirty());
    assert_eq!(ui.app.session.ds().unwrap().uid, b);
    let dir = std::env::temp_dir().join(format!("kodma-i3-save-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("background.figcraft");
    ui.app.services = Box::new(SavePicker(Some(path.clone())));
    ui.app.workspace_action(Action::Close(a));
    ui.app.workspace_action(Action::AnswerClose(a, CloseAnswer::Save));
    ui.frame(vec![]);
    ui.frame(vec![]);
    assert_eq!(ui.app.session.docs.len(), 1);
    assert_eq!(ui.app.session.ds().unwrap().uid, b);
    ui.app.run("file.open", json!({"path":path}));
    assert_eq!(ui.app.session.doc().unwrap().nodes().filter(|n| n.width == 150.0).count(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn clean_close_reindexes_and_late_uid_never_closes_the_next_tab() {
    let (mut ui, a, b) = two();
    ui.app.session.docs[0].mark_saved();
    ui.app.workspace_action(Action::Close(a));
    assert_eq!(ui.app.session.docs.len(), 1);
    assert_eq!(ui.app.session.ds().unwrap().uid, b);
    ui.app.workspace_action(Action::Close(a));
    assert!(ui.app.ui.close_tab.is_none());
    assert_eq!(ui.app.session.docs.len(), 1);
}
