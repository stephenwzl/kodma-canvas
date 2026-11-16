//! Native home controls. Button state is consumed on the established main-thread host tick;
//! actions carry paths, never a changing recent-list index, and dispatch through FigApp.
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSBezelStyle, NSBox, NSButton, NSButtonType, NSFont, NSScrollView, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSString};

use super::{color, rect};
use figcraft_ui_egui::workspace::{Action, RecentFile};

struct Row {
    file: RecentFile,
    button: Retained<NSButton>,
}

pub(super) struct Controls {
    new: Retained<NSButton>,
    open: Retained<NSButton>,
    home: Retained<NSButton>,
    error: Retained<NSTextField>,
    scroll: Retained<NSScrollView>,
    rows_view: Retained<NSView>,
    rows: Vec<Row>,
    previous_error: Option<String>,
    geometry: Option<(f64, f64, bool)>,
}

impl Controls {
    pub(super) fn install(mtm: MainThreadMarker, left: &NSBox, top: &NSBox, home_panel: &NSBox) -> Result<Self, String> {
        let new = button(mtm, "新建稿件");
        let open = button(mtm, "打开稿件…");
        let home = button(mtm, "首页");
        let error = NSTextField::wrappingLabelWithString(&NSString::new(), mtm);
        error.setFont(Some(&NSFont::systemFontOfSize(13.0)));
        error.setTextColor(Some(&color(0xa83030)));
        error.setHidden(true);
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), rect(0.0, 0.0, 190.0, 300.0));
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDrawsBackground(false);
        let rows_view = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, 190.0, 300.0));
        scroll.setDocumentView(Some(&rows_view));
        let left = left.contentView().ok_or("native navigation has no content view")?;
        left.addSubview(&new);
        left.addSubview(&scroll);
        left.addSubview(&error);
        top.contentView().ok_or("native tabs have no content view")?.addSubview(&home);
        home_panel.contentView().ok_or("native home has no content view")?.addSubview(&open);
        Ok(Self { new, open, home, error, scroll, rows_view, rows: Vec::new(), previous_error: None, geometry: None })
    }

    pub(super) fn take_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        if pressed(&self.new) {
            actions.push(Action::New);
        }
        if pressed(&self.open) {
            actions.push(Action::Open);
        }
        if pressed(&self.home) {
            actions.push(Action::Home);
        }
        for row in &self.rows {
            if pressed(&row.button) {
                actions.push(Action::Recent(row.file.path.clone()));
            }
        }
        actions
    }

    pub(super) fn sync(&mut self, recents: Vec<RecentFile>, error: Option<String>, busy: bool, width: f64, height: f64) {
        let changed = self.rows.iter().map(|r| &r.file).ne(recents.iter());
        if changed {
            for row in &self.rows {
                row.button.removeFromSuperview();
            }
            self.rows.clear();
            if let Some(mtm) = MainThreadMarker::new() {
                for file in recents {
                    let title: String = file.name.chars().take(80).collect();
                    let button = button(mtm, &title);
                    button.setToolTip(Some(&NSString::from_str(&file.path)));
                    self.rows_view.addSubview(&button);
                    self.rows.push(Row { file, button });
                }
            }
        }
        if self.previous_error != error {
            self.error.setStringValue(&NSString::from_str(error.as_deref().unwrap_or_default()));
            self.error.setHidden(error.is_none());
            self.error.setToolTip(error.as_deref().map(NSString::from_str).as_deref());
            self.previous_error = error;
        }
        let key = (width, height, self.previous_error.is_some());
        if changed || self.geometry != Some(key) {
            self.geometry = Some(key);
            self.new.setFrame(rect(26.0, height - 144.0, 190.0, 40.0));
            self.home.setFrame(rect(42.0, 9.0, 84.0, 42.0));
            self.open.setFrame(rect(78.0, height - 84.0 - 464.0, 180.0, 44.0));
            self.error.setFrame(rect(26.0, 16.0, 190.0, 180.0));
            let bottom = if self.previous_error.is_some() { 208.0 } else { 16.0 };
            let viewport = (height - 200.0 - bottom).max(1.0);
            self.scroll.setFrame(rect(26.0, bottom, 190.0, viewport));
            let rows_height = (self.rows.len() as f64 * 52.0).max(viewport);
            self.rows_view.setFrame(rect(0.0, 0.0, 190.0, rows_height));
            for (index, row) in self.rows.iter().enumerate() {
                row.button.setFrame(rect(0.0, rows_height - (index + 1) as f64 * 52.0, 172.0, 44.0));
            }
            let clip = self.scroll.contentView();
            let y = if changed { rows_height - viewport } else { clip.bounds().origin.y.min(rows_height - viewport) };
            clip.scrollToPoint(NSPoint::new(0.0, y.max(0.0)));
            self.scroll.reflectScrolledClipView(&clip);
        }
        for button in [&self.new, &self.open, &self.home] {
            button.setEnabled(!busy);
        }
        for row in &self.rows {
            row.button.setEnabled(!busy);
        }
    }
}

fn button(mtm: MainThreadMarker, title: &str) -> Retained<NSButton> {
    let button = NSButton::initWithFrame(NSButton::alloc(mtm), rect(0.0, 0.0, 1.0, 1.0));
    button.setTitle(&NSString::from_str(title));
    // Safe AppKit state polling avoids an unsafe Objective-C target/selector bridge.
    button.setButtonType(NSButtonType::PushOnPushOff);
    button.setBezelStyle(NSBezelStyle::Push);
    button
}

fn pressed(button: &NSButton) -> bool {
    let pressed = button.isEnabled() && button.state() != 0;
    button.setState(0);
    pressed
}
