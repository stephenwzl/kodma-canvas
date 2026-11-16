//! Floating native controls dispatch the reference runtime's existing commands.
use super::{label, panel, rect, style};
use figcraft_ui_egui::workspace::{Action, Snapshot};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{NSBox, NSTextField, NSView};
pub(super) struct Toolbar {
    pub view: Retained<NSBox>,
    buttons: Vec<(style::Button, &'static str)>,
    zoom: Retained<NSTextField>,
}
impl Toolbar {
    pub fn install(mtm: MainThreadMarker, parent: &NSView) -> Result<Self, String> {
        let view = panel(mtm, 0xffffff);
        view.setCornerRadius(16.);
        view.setBorderWidth(1.);
        view.setBorderColor(&super::color(0xd4d0cc));
        parent.addSubview(&view);
        let content = view.contentView().ok_or("missing toolbar content")?;
        let mut buttons = Vec::new();
        for (icon, title, command, x, w) in [
            ("pointer", "", "tool.move", 8., 38.),
            ("hand", "", "tool.hand", 52., 38.),
            ("search", "", "view.zoomFit", 118., 30.),
            ("undo", "", "edit.undo", 230., 36.),
            ("redo", "", "edit.redo", 274., 36.),
            ("play", "预览", "proto.preview", 342., 154.),
        ] {
            let button = style::Button::new(mtm, &content, title, 0xffffff, None, 10.);
            button.icon(icon, 0x736e69);
            button.frame(x, 7., w, 38.);
            button.control.setToolTip(Some(&objc2_foundation::NSString::from_str(command)));
            buttons.push((button, command));
        }
        let zoom = label(mtm, "", 14., 0x121212);
        zoom.setFrame(rect(156., 14., 75., 26.));
        content.addSubview(&zoom);
        Ok(Self { view, buttons, zoom })
    }
    pub fn sync(&self, snapshot: &Snapshot, width: f64, height: f64, flipped: bool) {
        let x = 242. + ((width - 242. - 408. - 508.) / 2.).max(24.);
        let y = if flipped { 111. } else { height - 163. };
        self.view.setFrame(rect(x, y, 508., 52.));
        self.view.setHidden(snapshot.home);
        self.zoom.setStringValue(&objc2_foundation::NSString::from_str(&format!("{}%", (snapshot.zoom * 100.).round())));
        for (button, id) in &self.buttons {
            button.enabled(!snapshot.busy);
            button.paint(if (*id == "tool.move" && snapshot.tool == "move") || (*id == "tool.hand" && snapshot.tool == "hand") {
                0xaadaff
            } else if *id == "proto.preview" {
                0xf7f5f2
            } else {
                0xffffff
            });
        }
    }
    pub fn actions(&self) -> Vec<Action> {
        self.buttons.iter().filter(|(b, _)| b.pressed()).map(|(_, id)| Action::Command(id)).collect()
    }
}
