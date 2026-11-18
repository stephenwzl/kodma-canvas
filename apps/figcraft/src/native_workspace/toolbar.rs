//! Floating native controls dispatch the reference runtime's existing commands.
use super::{LEFT, label, panel, rect, style};
use figcraft_ui_egui::workspace::{Action, Snapshot};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{NSBox, NSTextField, NSView};
const WIDE_WIDTH: f64 = 508.;
const NARROW_WIDTH: f64 = 336.;
const NARROW_CONTROLS: [(f64, f64); 6] = [(8., 40.), (52., 40.), (96., 30.), (180., 40.), (224., 40.), (288., 40.)];
const WIDE_CONTROLS: [(f64, f64); 6] = [(8., 38.), (52., 38.), (118., 30.), (230., 36.), (274., 36.), (342., 154.)];

fn compact(canvas_width: f64) -> bool {
    // The two reference layouts have canvas widths of 622 and 790 points.
    canvas_width < 706.
}

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
        for (icon, title, tooltip, command) in [
            ("pointer", "", "选取", "tool.move"),
            ("hand", "", "平移", "tool.hand"),
            ("search", "", "缩放至适合", "view.zoomFit"),
            ("undo", "", "撤销", "edit.undo"),
            ("redo", "", "重做", "edit.redo"),
            ("play", "预览", "预览", "proto.preview"),
        ] {
            let button = style::Button::new(mtm, &content, title, 0xffffff, None, 10.);
            button.icon(icon, 0x736e69);
            button.tooltip(tooltip);
            buttons.push((button, command));
        }
        let zoom = label(mtm, "", 14., 0x121212);
        content.addSubview(&zoom);
        Ok(Self { view, buttons, zoom })
    }
    pub fn sync(&self, snapshot: &Snapshot, canvas_width: f64, height: f64, flipped: bool) {
        let compact = compact(canvas_width);
        let (width, controls, zoom_x, zoom_width) =
            if compact { (NARROW_WIDTH, NARROW_CONTROLS, 128., 50.) } else { (WIDE_WIDTH, WIDE_CONTROLS, 156., 75.) };
        let x = LEFT + ((canvas_width - width) / 2.).max(0.);
        let y = if flipped { 111. } else { height - 163. };
        self.view.setFrame(rect(x, y, width, 52.));
        self.view.setHidden(snapshot.home);
        self.zoom.setFrame(rect(zoom_x, 14., zoom_width, 26.));
        self.zoom.setStringValue(&objc2_foundation::NSString::from_str(&format!("{}%", (snapshot.zoom * 100.).round())));
        for ((button, id), (x, width)) in self.buttons.iter().zip(controls) {
            if *id == "proto.preview" && button.control.title().is_empty() != compact {
                button.title(if compact { "" } else { "预览" });
                button.icon("play", 0x736e69);
            }
            button.frame(x, 7., width, 38.);
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

#[cfg(test)]
mod tests {
    use super::{NARROW_CONTROLS, NARROW_WIDTH, WIDE_CONTROLS, WIDE_WIDTH, compact};

    #[test]
    fn the_reference_window_sizes_use_distinct_toolbar_layouts() {
        assert!(compact(622.));
        assert!(!compact(790.));
        for (width, controls) in [(NARROW_WIDTH, NARROW_CONTROLS), (WIDE_WIDTH, WIDE_CONTROLS)] {
            assert!(controls.windows(2).all(|pair| pair[0].0 + pair[0].1 <= pair[1].0));
            let (x, button_width) = controls[5];
            assert!(x + button_width <= width);
        }
    }
}
