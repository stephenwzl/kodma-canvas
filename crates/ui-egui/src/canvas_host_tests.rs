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
fn native_focus_cancels_the_canvas_preedit_without_ending_its_text_edit() {
    let mut session = Session::new();
    let node = id(&session.execute("text.create", &json!({"text": "Hi", "x": 40, "y": 40})).unwrap());
    let mut ui = canvas(session);
    figcraft_engine::textedit::begin(&mut ui.app.session, node, Some(2), false).unwrap();
    ui.frame(vec![Event::Ime(ImeEvent::Preedit { text: "ni".into(), active_range_chars: Some(2..2) })]);
    assert!(ui.app.preedit_view().is_some());
    ui.app.canvas_host.as_mut().unwrap().keyboard = false;
    let output = ui.frame_output(vec![Event::Text("native".into()), Event::Ime(ImeEvent::Commit("你".into()))]);
    let edit = ui.app.session.ds().unwrap().text_edit.as_ref().unwrap();
    assert!(edit.preedit.is_none(), "the inactive canvas must not retain a composition owned by its old native context");
    assert!(ui.app.preedit_view().is_none());
    assert!(output.platform_output.ime.is_none());
    let text = |ui: &Ui| match &ui.app.session.doc().unwrap().node(node).unwrap().kind {
        NodeKind::Text(text) => text.characters.clone(),
        _ => panic!("not text"),
    };
    assert_eq!(text(&ui), "Hi");
    ui.app.canvas_host.as_mut().unwrap().keyboard = true;
    ui.frame(vec![Event::Ime(ImeEvent::Preedit { text: "hao".into(), active_range_chars: Some(3..3) })]);
    ui.frame_raw_with_discard(RawInput { events: vec![Event::Ime(ImeEvent::Commit("好".into()))], ..Default::default() });
    assert_eq!(text(&ui), "Hi好", "returning to the canvas starts a fresh composition and commits once");
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

#[test]
fn hosted_marquee_multi_drag_resize_and_rotate_keep_the_original_gesture_transactions() {
    for inset in [0.0, 320.0, 500.0] {
        let mut session = Session::new();
        let a = id(&session.execute("shape.rectangle", &json!({"x": 100, "y": 100, "width": 100, "height": 80})).unwrap());
        let b = id(&session.execute("shape.rectangle", &json!({"x": 260, "y": 100, "width": 100, "height": 80})).unwrap());
        session.execute("select.set", &json!({"ids": []})).unwrap();
        let mut ui = canvas(session);
        ui.app.canvas_host.as_mut().unwrap().right_inset = inset;
        ui.frame(vec![]);
        let from = ui.app.to_screen(70.0, 70.0);
        let to = ui.app.to_screen(390.0, 210.0);
        ui.drag_to(from, to);
        ui.button(to, false);
        let mut selected = ui.app.session.selection().to_vec();
        selected.sort();
        assert_eq!(selected, vec![a, b], "marquee with inset {inset}");
        // Avoid the smart-selection reorder dot at the layer centre.
        let from = ui.app.to_screen(125.0, 120.0);
        let to = from + vec2(40.0, 30.0);
        ui.drag_to(from, to);
        ui.button(to, false);
        assert_eq!(
            ui.app.session.doc().unwrap().node(a).unwrap().transform.translation().x,
            140.0,
            "inset {inset}, selection {:?}, tool {:?}",
            ui.app.session.selection(),
            ui.app.session.tool.tool
        );
        assert_eq!(ui.app.session.doc().unwrap().node(b).unwrap().transform.translation().x, 300.0);
        // A command after a gesture is its own undo step, even while chrome is absent.
        ui.app.run("node.set", json!({"ids": [b], "props": {"name": "external"}}));
        ui.app.run("edit.undo", json!({}));
        assert_ne!(ui.app.session.doc().unwrap().node(b).unwrap().name, "external");
        assert_eq!(ui.app.session.doc().unwrap().node(a).unwrap().transform.translation().x, 140.0);
        ui.app.run("edit.undo", json!({}));
        assert_eq!(ui.app.session.doc().unwrap().node(a).unwrap().transform.translation().x, 100.0);
        ui.app.run("select.set", json!({"ids": [a]}));
        ui.frame(vec![]);
        let from = ui.app.to_screen(200.0, 180.0);
        let to = ui.app.to_screen(230.0, 200.0);
        ui.drag_to(from, to);
        ui.button(to, false);
        assert_eq!(ui.app.session.doc().unwrap().node(a).unwrap().width, 130.0);
        ui.app.run("edit.undo", json!({}));
        ui.frame(vec![]);
        let from = ui.app.to_screen(90.0, 90.0);
        let to = ui.app.to_screen(220.0, 90.0);
        ui.drag_to(from, to);
        ui.button(to, false);
        let rotation = figcraft_geom::rotation_degrees(ui.app.session.doc().unwrap().node(a).unwrap().transform);
        assert!(rotation.abs() > 10.0, "rotation handle with inset {inset}: {rotation}");
        ui.app.run("edit.undo", json!({}));
        assert!(figcraft_geom::rotation_degrees(ui.app.session.doc().unwrap().node(a).unwrap().transform).abs() < 0.001);
    }
}

#[test]
fn a_hosted_gradient_handle_keeps_its_picker_and_commits_one_undo_step() {
    let mut session = Session::new();
    let a = id(&session.execute("shape.rectangle", &json!({"x": 100, "y": 100, "width": 200, "height": 200})).unwrap());
    session.execute("fill.set", &json!({"id": a, "paint": {"type": "linearGradient", "stops": ["#ff0000", "#0000ff"]}})).unwrap();
    session.execute("select.set", &json!({"ids": [a]})).unwrap();
    let mut ui = canvas(session);
    ui.app.canvas_host.as_mut().unwrap().right_inset = 320.0;
    let target = crate::colorpicker::PickerTarget::Paint { fills: true, index: 0, ids: vec![a] };
    ui.app.open_picker(target.clone(), None);
    ui.frame(vec![]);
    let depth = ui.app.session.undo_depth().0;
    let from = ui.app.to_screen(200.0, 100.0);
    let to = ui.app.to_screen(100.0, 200.0);
    ui.drag_to(from, to);
    ui.button(to, false);
    assert_eq!(ui.app.ui.picker, Some(target));
    let figcraft_color::PaintKind::Gradient(gradient) = &ui.app.session.doc().unwrap().node(a).unwrap().fills[0].kind else { panic!("not gradient") };
    assert!(gradient.from.0.abs() < 0.02 && (gradient.from.1 - 0.5).abs() < 0.02);
    assert_eq!(ui.app.session.undo_depth().0, depth + 1);
    ui.app.run("edit.undo", json!({}));
    let figcraft_color::PaintKind::Gradient(gradient) = &ui.app.session.doc().unwrap().node(a).unwrap().fills[0].kind else { panic!("not gradient") };
    assert_eq!(gradient.from, (0.5, 0.0));
}

#[test]
fn a_hosted_drop_finishes_in_its_receiving_document_after_a_switch() {
    #[derive(Debug)]
    struct Svg;
    impl egui::DroppedFile for Svg {
        fn path(&self) -> &std::path::Path {
            std::path::Path::new("s001-virtual-drop.svg")
        }
        fn bytes(&self) -> Result<Vec<u8>, String> {
            Ok(br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10"/></svg>"#.to_vec())
        }
    }
    let mut ui = canvas(Session::new());
    ui.app.canvas_host.as_mut().unwrap().right_inset = 320.0;
    let receiving = ui.app.session.ds().unwrap().uid;
    ui.app.run("file.new", json!({"name": "Other"}));
    let other = ui.app.session.ds().unwrap().uid;
    ui.app.run("doc.switch", json!({"index": 0}));
    ui.frame(vec![]);
    let at = ui.app.canvas.rect.center();
    ui.frame_raw(RawInput { events: vec![Event::PointerMoved(at)], dropped_files: vec![std::sync::Arc::new(Svg)], ..Default::default() });
    assert_eq!(ui.app.ui.pending_opens[0].target.as_ref().unwrap().uid, receiving);
    ui.frame(vec![]);
    ui.app.run("doc.switch", json!({"index": 1}));
    ui.frame(vec![]);
    ui.frame(vec![]);
    assert_eq!(ui.app.session.ds().unwrap().uid, other);
    let layers = |uid| {
        let doc = &ui.app.session.docs.iter().find(|doc| doc.uid == uid).unwrap().doc;
        doc.node(doc.pages[0]).unwrap().children.len()
    };
    assert_eq!((layers(receiving), layers(other)), (1, 0));
}

#[test]
fn a_hosted_prototype_connection_uses_the_original_canvas_handles() {
    let mut session = Session::new();
    let a = id(&session.execute("frame.create", &json!({"x": 100, "y": 100, "width": 100, "height": 80})).unwrap());
    let b = id(&session.execute("frame.create", &json!({"x": 350, "y": 100, "width": 100, "height": 80})).unwrap());
    session.execute("select.set", &json!({"ids": [a]})).unwrap();
    let mut ui = canvas(session);
    ui.app.canvas_host.as_mut().unwrap().right_inset = 320.0;
    ui.app.ui.right_tab = crate::RightTab::Prototype;
    ui.frame(vec![]);
    let doc = ui.app.session.doc().unwrap().clone();
    let from = crate::canvas::proto_canvas::handles(&ui.app, &doc, &[a])[0].1;
    let to = ui.app.to_screen(400.0, 140.0);
    ui.drag_to(from, to);
    ui.button(to, false);
    ui.frame(vec![]);
    let interactions = &ui.app.session.doc().unwrap().node(a).unwrap().interactions;
    assert_eq!(interactions.len(), 1);
    assert!(matches!(interactions[0].actions.as_slice(), [figcraft_doc::proto::Action::Navigate { dest, .. }] if *dest == b));
    ui.app.run("edit.undo", json!({}));
    assert!(ui.app.session.doc().unwrap().node(a).unwrap().interactions.is_empty());
}

#[test]
fn vector_edit_survives_host_region_changes_and_native_focus() {
    let mut session = Session::new();
    let a = id(&session.execute("shape.rectangle", &json!({"x": 100, "y": 100, "width": 100, "height": 80})).unwrap());
    let mut ui = canvas(session);
    ui.app.run("vector.edit.begin", json!({"id": a}));
    for inset in [320.0, 500.0, 0.0] {
        let host = ui.app.canvas_host.as_mut().unwrap();
        host.right_inset = inset;
        host.keyboard = false;
        ui.frame(vec![Event::Text("native".into())]);
        assert_eq!(ui.app.session.ds().unwrap().vector_edit.as_ref().unwrap().id, a);
        ui.app.canvas_host.as_mut().unwrap().keyboard = true;
        ui.frame(vec![]);
        assert_eq!(ui.app.session.ds().unwrap().vector_edit.as_ref().unwrap().id, a);
    }
    ui.app.run("vector.edit.end", json!({}));
    assert!(ui.app.session.ds().unwrap().vector_edit.is_none());
}

#[test]
fn workspace_reserves_all_three_regions_and_keeps_hits_inside_the_canvas() {
    let mut ui = canvas(Session::new());
    for size in [vec2(1440.0, 960.0), vec2(1200.0, 760.0)] {
        ui.screen = size;
        let host = ui.app.canvas_host.as_mut().unwrap();
        host.left_inset = 242.0;
        host.top_inset = 84.0;
        host.right_inset = 408.0;
        ui.frame(vec![]);
        assert_eq!(ui.app.canvas.rect, Rect::from_min_max(Pos2::new(242.0, 84.0), Pos2::new(size.x - 408.0, size.y)));
        let page = ui.app.to_page(ui.app.to_screen(123.0, 77.0));
        assert!((page.x - 123.0).abs() < 0.001 && (page.y - 77.0).abs() < 0.001);
    }
}

#[test]
fn native_home_fences_input_but_still_drains_control_commands() {
    let mut session = Session::new();
    let node = id(&session.execute("shape.rectangle", &json!({"x":40,"y":40,"width":80,"height":60})).unwrap());
    let mut ui = canvas(session);
    ui.app.canvas_host.as_mut().unwrap().respect_home = true;
    ui.app.run("doc.home", json!({}));
    let (tx, rx) = std::sync::mpsc::channel();
    ui.app.control_queue.push(ControlRequest {
        id: json!(1),
        state: Default::default(),
        method: "engine.execute".into(),
        params: json!({"command":"node.set", "params":{"ids":[node],"width":130}}),
        reply: tx,
    });
    ui.frame(vec![Event::Key { key: egui::Key::Delete, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }]);
    assert!(rx.recv().unwrap().result.is_ok());
    assert_eq!(ui.app.session.doc().unwrap().node(node).unwrap().width, 130.0);
    assert!(!ui.app.canvas.rect.is_positive(), "native home cannot retain an editable canvas hit region");
}
