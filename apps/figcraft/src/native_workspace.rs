//! S002 native frame. All values are observations of the one existing FigApp/Session.
//! NSWindow and WinitView remain owned by eframe; no reparenting or replacement GPU layer.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSBox, NSBoxType, NSColor, NSFont, NSResponder, NSTextField, NSTitlePosition, NSView, NSWindow};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use figcraft_ui_egui::CanvasHost;
use figcraft_ui_egui::workspace::Snapshot;

const LEFT: f64 = 242.0;
const TOP: f64 = 84.0;
const RIGHT: f64 = 408.0;

pub struct Host {
    window: Retained<NSWindow>,
    canvas: Retained<NSView>,
    left: Retained<NSBox>,
    top: Retained<NSBox>,
    right: Retained<NSBox>,
    home: Retained<NSBox>,
    brand: Retained<NSTextField>,
    drafts: Retained<NSTextField>,
    list: Retained<NSTextField>,
    tab: Retained<NSTextField>,
    design: Retained<NSTextField>,
    selection: Retained<NSTextField>,
    dimensions: Retained<NSTextField>,
    greeting: Retained<NSTextField>,
    headline: Retained<NSTextField>,
    previous: Option<Snapshot>,
    geometry: Option<(f64, f64, bool)>,
}

impl Host {
    pub fn install(title: &str) -> Result<Self, String> {
        let mtm = MainThreadMarker::new().ok_or("native workspace requires the main thread")?;
        let windows = NSApplication::sharedApplication(mtm).windows();
        let mut candidates = windows.iter().filter(|window| window.title().to_string() == title);
        let window = candidates.next().ok_or("could not find the workspace window")?;
        if candidates.next().is_some() {
            return Err("ambiguous workspace window".into());
        }
        let canvas = window.contentView().ok_or("workspace has no content view")?;
        let left = panel(mtm, 0xf7f5f2);
        let top = panel(mtm, 0xffffff);
        let right = panel(mtm, 0xffffff);
        let home = panel(mtm, 0xffffff);
        let brand = label(mtm, "kodma", 30.0, 0x121212);
        let drafts = label(mtm, "稿件", 14.0, 0x736e69);
        let list = label(mtm, "", 16.0, 0x121212);
        let tab = label(mtm, "首页", 16.0, 0x121212);
        let design = label(mtm, "设计", 16.0, 0x121212);
        let selection = label(mtm, "", 20.0, 0x121212);
        let dimensions = label(mtm, "", 16.0, 0x736e69);
        let greeting = label(mtm, "今天，", 64.0, 0x736e69);
        let headline = label(mtm, "想把什么画出来？", 56.0, 0x121212);
        add_labels(&left, &[&brand, &drafts, &list])?;
        add_labels(&top, &[&tab])?;
        add_labels(&right, &[&design, &selection, &dimensions])?;
        add_labels(&home, &[&greeting, &headline])?;
        // Add as siblings of the original rendering view's contents, never its replacement.
        for view in [&*left, &*top, &*right, &*home] {
            canvas.addSubview(view);
        }
        window.makeFirstResponder(Some(&canvas));
        log::info!("native_workspace installed in NSWindow {}, canvas_flipped={}", window.windowNumber(), canvas.isFlipped());
        Ok(Self {
            window,
            canvas,
            left,
            top,
            right,
            home,
            brand,
            drafts,
            list,
            tab,
            design,
            selection,
            dimensions,
            greeting,
            headline,
            previous: None,
            geometry: None,
        })
    }

    /// Main-thread presentation only. I1 fields are read-only; editable callbacks arrive in I4.
    pub fn sync(&mut self, snapshot: Snapshot) -> CanvasHost {
        let bounds = self.canvas.bounds();
        let w = bounds.size.width;
        let h = bounds.size.height;
        if self.geometry != Some((w, h, snapshot.home)) {
            self.geometry = Some((w, h, snapshot.home));
            self.left.setFrame(rect(0.0, 0.0, LEFT, h));
            // WinitView is flipped on this backend; native panel content views are not.
            // Convert only the panel origins, preserving AppKit's native text coordinates.
            let (top_y, body_y) = panel_origins(self.canvas.isFlipped(), h);
            self.top.setFrame(rect(LEFT, top_y, (w - LEFT).max(0.0), TOP));
            self.right.setFrame(rect(w - RIGHT, body_y, RIGHT, (h - TOP).max(0.0)));
            self.home.setFrame(rect(LEFT, body_y, (w - LEFT).max(0.0), (h - TOP).max(0.0)));
            self.right.setHidden(snapshot.home);
            self.home.setHidden(!snapshot.home);
            self.brand.setFrame(rect(36.0, h - 82.0, 180.0, 44.0));
            self.drafts.setFrame(rect(36.0, h - 164.0, 180.0, 24.0));
            self.list.setFrame(rect(36.0, h - 440.0, 180.0, 250.0));
            self.tab.setFrame(rect(42.0, 9.0, (w - LEFT - 84.0).max(0.0), 42.0));
            self.design.setFrame(rect(24.0, h - TOP - 60.0, 360.0, 30.0));
            self.selection.setFrame(rect(24.0, h - TOP - 143.0, 360.0, 36.0));
            self.dimensions.setFrame(rect(24.0, h - TOP - 265.0, 360.0, 85.0));
            self.greeting.setFrame(rect(78.0, h - TOP - 235.0, (w - LEFT - 100.0).max(0.0), 92.0));
            self.headline.setFrame(rect(78.0, h - TOP - 320.0, (w - LEFT - 100.0).max(0.0), 82.0));
        }
        if self.previous.as_ref() != Some(&snapshot) {
            let names = snapshot.documents.iter().take(8).map(|d| format!("{}{}", d.name, if d.dirty { " •" } else { "" })).collect::<Vec<_>>();
            self.list.setStringValue(&NSString::from_str(&names.join("\n\n")));
            let active = snapshot.documents.iter().find(|d| Some(d.id) == snapshot.active);
            let title = if snapshot.home {
                "首页".into()
            } else {
                active.map(|d| format!("首页     {}{}", d.name, if d.dirty { " •" } else { "" })).unwrap_or_default()
            };
            self.tab.setStringValue(&NSString::from_str(&title));
            let (name, dimensions) = match snapshot.selection.as_slice() {
                [node] => (node.name.clone(), format!("{}\nW  {}     H  {}", node.kind, node.width, node.height)),
                [] => (String::new(), String::new()),
                nodes => (format!("{} 个图层", nodes.len()), String::new()),
            };
            self.selection.setStringValue(&NSString::from_str(&name));
            self.dimensions.setStringValue(&NSString::from_str(&dimensions));
            self.previous = Some(snapshot);
        }
        CanvasHost {
            left_inset: LEFT as f32,
            top_inset: TOP as f32,
            right_inset: RIGHT as f32,
            respect_home: true,
            keyboard: self.window.firstResponder().is_some_and(|r| std::ptr::eq::<NSResponder>(&*r, &**self.canvas)),
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.window.makeFirstResponder(Some(&self.canvas));
        for panel in [&self.left, &self.top, &self.right, &self.home] {
            panel.removeFromSuperview();
        }
        log::info!("native_workspace detached from NSWindow {}", self.window.windowNumber());
    }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

fn color(rgb: u32) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(
        f64::from((rgb >> 16) & 255) / 255.0,
        f64::from((rgb >> 8) & 255) / 255.0,
        f64::from(rgb & 255) / 255.0,
        1.0,
    )
}

fn panel(mtm: MainThreadMarker, rgb: u32) -> Retained<NSBox> {
    let panel = NSBox::initWithFrame(NSBox::alloc(mtm), rect(0.0, 0.0, 1.0, 1.0));
    panel.setBoxType(NSBoxType::Custom);
    panel.setTitlePosition(NSTitlePosition::NoTitle);
    panel.setBorderWidth(0.0);
    panel.setContentViewMargins(NSSize::new(0.0, 0.0));
    panel.setFillColor(&color(rgb));
    panel
}

fn label(mtm: MainThreadMarker, text: &str, size: f64, rgb: u32) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    label.setFont(Some(&NSFont::systemFontOfSize(size)));
    label.setTextColor(Some(&color(rgb)));
    label.setSelectable(false);
    label.setMaximumNumberOfLines(0);
    label
}

fn add_labels(panel: &NSBox, labels: &[&NSTextField]) -> Result<(), String> {
    // NSBox owns a content view. Adding labels directly to the box can replace that view,
    // making the last label's frame the container frame and moving text outside its region.
    let content = panel.contentView().ok_or("native panel has no content view")?;
    for label in labels {
        content.addSubview(label);
    }
    Ok(())
}

fn panel_origins(flipped: bool, height: f64) -> (f64, f64) {
    if flipped { (0.0, TOP) } else { (height - TOP, 0.0) }
}

#[cfg(test)]
mod tests {
    use super::panel_origins;
    #[test]
    fn native_panels_use_the_actual_parent_coordinate_direction() {
        assert_eq!(panel_origins(true, 960.0), (0.0, 84.0));
        assert_eq!(panel_origins(false, 960.0), (876.0, 0.0));
        assert_eq!(panel_origins(true, 600.0), (0.0, 84.0));
    }
}
