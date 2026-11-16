//! The host policy crosses the original frame, not an alternative editing implementation.

use egui::{Event, ImeEvent, Pos2, RawInput, Rect, vec2};
use figcraft_doc::{NodeId, NodeKind};
use figcraft_engine::Session;
use serde_json::{Value, json};

use crate::layers_drag_tests::Ui;
use crate::{CanvasHost, ControlRequest};

fn canvas(session: Session) -> Ui {
    let mut ui = Ui::new(session);
    ui.app.canvas_host = Some(CanvasHost::default());
    ui.frame(vec![]);
    ui
}

fn id(value: &Value) -> NodeId {
    NodeId(value["id"].as_u64().unwrap())
}

#[test]
fn canvas_host_keeps_chrome_hidden_across_home_tabs_and_view_toggles_without_saving_policy() {
    let mut ui = canvas(Session::new());
    let before = ui.app.session.ds().unwrap().view.ui_hidden;
    ui.app.ui.home = true;
    ui.app.run("view.toggleUi", json!({"on": false}));
    ui.frame(vec![]);
    assert_eq!(ui.app.canvas.rect, Rect::from_min_size(Pos2::ZERO, ui.screen));
    assert!(!ui.app.session.ds().unwrap().view.ui_hidden);
    ui.app.run("file.new", json!({"name": "Second"}));
    ui.app.run("doc.switch", json!({"index": 0}));
    ui.frame(vec![]);
    assert_eq!(ui.app.canvas.rect, Rect::from_min_size(Pos2::ZERO, ui.screen));
    assert_eq!(ui.app.session.ds().unwrap().view.ui_hidden, before);
    ui.app.canvas_host = None;
    ui.app.ui.home = false;
    ui.frame(vec![]);
    assert!(ui.app.canvas.rect.width() < ui.screen.x, "the reference editor still draws its panels");
    assert!(ui.app.canvas.rect.top() > 0.0, "the reference tab strip returns");
}

#[test]
fn native_reserved_space_and_window_resize_keep_page_coordinates_invertible() {
    let mut ui = canvas(Session::new());
    for (size, inset) in [(vec2(1440.0, 900.0), 320.0), (vec2(900.0, 600.0), 240.0), (vec2(1200.0, 800.0), 500.0)] {
        ui.screen = size;
        ui.app.canvas_host.as_mut().unwrap().right_inset = inset;
        ui.frame(vec![]);
        assert!((ui.app.canvas.rect.width() - (size.x - inset)).abs() < 1.0);
        assert_eq!(ui.app.canvas.rect.height(), size.y);
        let p = ui.app.to_page(ui.app.to_screen(123.0, 77.0));
        assert!((p.x - 123.0).abs() < 0.001 && (p.y - 77.0).abs() < 0.001);
    }
    ui.app.canvas_host.as_mut().unwrap().right_inset = f32::NAN;
    ui.frame(vec![]);
    assert!(ui.app.canvas.rect.is_finite());
    assert_eq!(ui.app.canvas.rect.width(), ui.screen.x);
}

#[test]
fn the_same_pointer_gesture_and_undo_work_with_hidden_chrome() {
    let mut session = Session::new();
    let node = id(&session.execute("shape.rectangle", &json!({"x": 40, "y": 40, "width": 80, "height": 60})).unwrap());
    session.execute("select.set", &json!({"ids": [node]})).unwrap();
    let mut ui = canvas(session);
    let before = ui.app.session.doc().unwrap().node(node).unwrap().transform.translation().x;
    let start = ui.app.to_screen(80.0, 70.0);
    let end = start + vec2(40.0, 20.0);
    ui.drag_to(start, end);
    ui.button(end, false);
    let moved = ui.app.session.doc().unwrap().node(node).unwrap().transform.translation().x;
    assert!(moved > before);
    ui.app.run("edit.undo", json!({}));
    assert_eq!(ui.app.session.doc().unwrap().node(node).unwrap().transform.translation().x, before);
    ui.app.run("edit.redo", json!({}));
    assert_eq!(ui.app.session.doc().unwrap().node(node).unwrap().transform.translation().x, moved);
}

#[test]
fn native_keyboard_owner_blocks_text_without_ending_the_canvas_editor() {
    let mut session = Session::new();
    let node = id(&session.execute("text.create", &json!({"text": "Hi", "x": 40, "y": 40})).unwrap());
    let mut ui = canvas(session);
    figcraft_engine::textedit::begin(&mut ui.app.session, node, Some(2), false).unwrap();
    ui.app.canvas_host.as_mut().unwrap().keyboard = false;
    ui.frame(vec![Event::Text("native".into())]);
    let text = |ui: &Ui| match &ui.app.session.doc().unwrap().node(node).unwrap().kind {
        NodeKind::Text(text) => text.characters.clone(),
        _ => panic!("not text"),
    };
    assert_eq!(text(&ui), "Hi");
    assert!(ui.app.session.ds().unwrap().text_edit.is_some());
    ui.app.canvas_host.as_mut().unwrap().keyboard = true;
    ui.frame(vec![Event::Ime(ImeEvent::Preedit { text: "nihao".into(), active_range_chars: Some(5..5) })]);
    assert_eq!(text(&ui), "Hi", "composition is still only a preview");
    ui.frame_raw_with_discard(RawInput { events: vec![Event::Ime(ImeEvent::Commit("你好".into()))], ..Default::default() });
    assert_eq!(text(&ui), "Hi你好", "a discarded pass must not repeat the commit");
}

#[test]
fn hidden_panel_does_not_destroy_an_explicit_crop_target() {
    use base64::Engine as _;
    let mut session = Session::new();
    let image = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
    let mut png = std::io::Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png).unwrap();
    let data = base64::engine::general_purpose::STANDARD.encode(png.into_inner());
    let node = id(&session.execute("image.place", &json!({"data": data, "width": 100, "height": 100})).unwrap());
    session.execute("select.set", &json!({"ids": [node]})).unwrap();
    let mut ui = canvas(session);
    ui.app.run("image.crop", json!({}));
    ui.frame(vec![]);
    assert_eq!(ui.app.ui.crop, Some((node, 0)));
}

#[test]
fn a_canvas_host_answers_commands_on_logic_only_ticks() {
    let mut ui = canvas(Session::new());
    let (reply, replies) = std::sync::mpsc::channel();
    ui.app.control_queue.push(ControlRequest {
        id: json!(1),
        method: "engine.execute".into(),
        params: json!({"command": "shape.rectangle", "params": {"name": "Background", "width": 90, "height": 70}}),
        reply,
        state: Default::default(),
    });
    let mut raw = RawInput::default();
    raw.viewports.insert(egui::ViewportId::ROOT, egui::ViewportInfo { minimized: Some(true), ..Default::default() });
    let _ = ui.ctx.run_logic(&raw, |ctx| ui.app.background_tick(ctx));
    let result = replies.try_recv().expect("answered without drawing any panel or canvas").result.unwrap();
    let node = id(&result);
    assert_eq!(ui.app.session.doc().unwrap().node(node).unwrap().name, "Background");
    ui.frame(vec![]);
    assert_eq!(ui.app.session.doc().unwrap().node(node).unwrap().name, "Background");
}

#[test]
fn an_empty_canvas_host_stays_blank_and_can_open_a_document_again() {
    let mut ui = canvas(Session::empty());
    assert!(ui.app.session.docs.is_empty());
    assert_eq!(ui.app.canvas.rect, Rect::from_min_size(Pos2::ZERO, ui.screen));
    ui.app.run("file.new", json!({"name": "Opened again"}));
    ui.frame(vec![]);
    assert_eq!(ui.app.session.doc().unwrap().name, "Opened again");
    assert_eq!(ui.app.canvas.rect, Rect::from_min_size(Pos2::ZERO, ui.screen));
}
