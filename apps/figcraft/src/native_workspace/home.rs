//! Native home controls. Button state is consumed on the established main-thread host tick;
//! actions carry paths, never a changing recent-list index, and dispatch through FigApp.
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSBezelStyle, NSBox, NSButton, NSButtonType, NSFont, NSScrollView, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSString};

use super::{color, panel, rect, row_pool::RowPool, style};
use figcraft_ui_egui::workspace::{Action, RecentFile};

struct Row {
    file: RecentFile,
    button: style::Button,
}
impl Drop for Row {
    fn drop(&mut self) {
        self.button.view.removeFromSuperview();
    }
}

pub(super) struct Controls {
    new: style::Button,
    open: style::Button,
    home: style::Button,
    search: style::Button,
    settings: style::Button,
    underline: Retained<NSBox>,
    error: Retained<NSTextField>,
    scroll: Retained<NSScrollView>,
    rows_view: Retained<NSView>,
    rows: RowPool<Row>,
    previous_error: Option<String>,
    geometry: Option<(f64, f64, bool, bool)>,
}

impl Controls {
    pub(super) fn install(mtm: MainThreadMarker, left: &NSBox, top: &NSBox, _home_panel: &NSBox) -> Result<Self, String> {
        let left_content = left.contentView().ok_or("missing navigation")?;
        let top_content = top.contentView().ok_or("missing tabs")?;
        let new = style::Button::new(mtm, &left_content, "新建稿件", 0xffffff, Some(0xdfdcd8), 12.);
        new.icon("plus", 0x121212);
        new.control.setFont(Some(&NSFont::systemFontOfSize(14.)));
        let open = style::Button::new(mtm, &left_content, "打开稿件…", 0xf7f5f2, None, 12.);
        open.icon("folder", 0x121212);
        open.leading();
        let home = style::Button::new(mtm, &top_content, "首页", 0xf7f5f2, None, 12.);
        home.icon("home", 0x121212);
        let underline = panel(mtm, 0x121212);
        top_content.addSubview(&underline);
        home.control.setFont(Some(&NSFont::systemFontOfSize(14.)));
        let search = style::Button::new(mtm, &left_content, "搜索", 0xf7f5f2, Some(0xd4d0cc), 12.);
        search.icon("search", 0x736e69);
        search.leading();
        search.control.setFont(Some(&NSFont::systemFontOfSize(16.)));
        search.enabled(false);
        let settings = style::Button::new(mtm, &left_content, "设置", 0xf7f5f2, None, 12.);
        settings.icon("settings", 0x736e69);
        settings.leading();
        settings.enabled(false);
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

        left.addSubview(&scroll);
        left.addSubview(&error);

        Ok(Self {
            new,
            open,
            home,
            search,
            settings,
            underline,
            error,
            scroll,
            rows_view,
            rows: RowPool::new(),
            previous_error: None,
            geometry: None,
        })
    }

    pub(super) fn take_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        if self.new.pressed() {
            actions.push(Action::New);
        }
        if self.open.pressed() {
            actions.push(Action::Open);
        }
        if self.home.pressed() {
            actions.push(Action::Home);
        }
        for row in self.rows.iter() {
            if row.button.pressed() {
                actions.push(Action::Recent(row.file.path.clone()));
            }
        }
        actions
    }

    pub(super) fn sync(
        &mut self,
        recents: Vec<RecentFile>,
        error: Option<String>,
        busy: bool,
        is_home: bool,
        active_name: Option<&str>,
        width: f64,
        height: f64,
    ) {
        let changed = self.rows.iter().map(|r| &r.file).ne(recents.iter());
        if changed && let Some(mtm) = MainThreadMarker::new() {
            self.rows.rebind(
                recents,
                |file| {
                    let title: String = file.name.chars().take(80).collect();
                    let button = style::Button::new(mtm, &self.rows_view, &title, 0xf7f5f2, None, 12.);
                    button.icon("folder", 0x121212);
                    button.leading();
                    button.control.setFont(Some(&NSFont::systemFontOfSize(16.)));
                    Row { file: file.clone(), button }
                },
                |row, file| {
                    row.button.pressed();
                    row.button.title(&file.name.chars().take(80).collect::<String>());
                    if row.file.name.is_empty() != file.name.is_empty() {
                        row.button.icon("folder", 0x121212);
                    }
                    row.button.control.setToolTip(Some(&NSString::from_str(&file.path)));
                    row.button.view.setHidden(false);
                    row.file = file;
                },
                |row, file| !row.button.tracking() || row.file.path == file.path,
            );
            for row in self.rows.inactive() {
                row.button.view.setHidden(true);
            }
        }
        if self.previous_error != error {
            self.error.setStringValue(&NSString::from_str(error.as_deref().unwrap_or_default()));
            self.error.setHidden(error.is_none());
            self.error.setToolTip(error.as_deref().map(NSString::from_str).as_deref());
            self.previous_error = error;
        }
        self.home.paint(if is_home { 0xf7f5f2 } else { 0xffffff });
        let page_changed = self.geometry.is_some_and(|g| g.3 != is_home);
        let key = (width, height, self.previous_error.is_some(), is_home);
        self.home.show_icon(!is_home);
        self.underline.setHidden(!is_home);
        self.underline.setFrame(rect(58., 9., 52., 3.));
        self.new.paint(if is_home { 0xffffff } else { 0xf7f5f2 });
        self.new.view.setBorderWidth(if is_home { 1. } else { 0. });
        self.search.view.setHidden(!is_home);
        self.scroll.setHidden(!is_home);
        self.new.view.setHidden(!is_home);
        if changed || self.geometry != Some(key) {
            self.geometry = Some(key);
            self.new.frame(if is_home { 26. } else { 180. }, height - if is_home { 172. } else { 168. }, if is_home { 190. } else { 36. }, 44.);
            self.new.title(if is_home { "新建稿件" } else { "" });
            self.new.icon_offset(if is_home { 55. } else { 10. });
            self.home.frame(42.0, 9.0, 84.0, 42.0);
            self.search.frame(26., height - 232., 190., 44.);
            self.settings.frame(if is_home { 26. } else { 180. }, 24., if is_home { 140. } else { 36. }, 44.);
            self.settings.title(if is_home { "设置" } else { "" });
            self.open.title("");
            self.open.frame(180., 74., 36., 36.);
            self.error.setFrame(rect(26.0, 16.0, 190.0, 180.0));
            let bottom = if self.previous_error.is_some() { 208.0 } else { 140.0 };
            let list_top = if is_home { 278. } else { 177. };
            let bottom = if is_home { bottom } else { height - 340. };
            let viewport = (height - list_top - bottom).max(1.0);
            self.scroll.setFrame(rect(26.0, bottom, 190.0, viewport));
            let rows_height = (self.rows.len() as f64 * 52.0).max(viewport);
            self.rows_view.setFrame(rect(0.0, 0.0, 190.0, rows_height));
            for (index, row) in self.rows.iter().enumerate() {
                row.button.frame(0.0, rows_height - 44.0 - index as f64 * 52.0, 190.0, 44.0);
            }
            let clip = self.scroll.contentView();
            let y = if changed || page_changed { rows_height - viewport } else { clip.bounds().origin.y.min(rows_height - viewport) };
            clip.scrollToPoint(NSPoint::new(0.0, y.max(0.0)));
            self.scroll.reflectScrolledClipView(&clip);
        }
        for button in [&self.new, &self.open, &self.home] {
            button.enabled(!busy);
        }
        for row in self.rows.iter() {
            row.button.enabled(!busy);
            row.button.paint(if !is_home && active_name == Some(row.file.name.as_str()) { 0xeeece8 } else { 0xf7f5f2 });
        }
    }
}

pub(super) fn button(mtm: MainThreadMarker, title: &str) -> Retained<NSButton> {
    let button = NSButton::initWithFrame(NSButton::alloc(mtm), rect(0.0, 0.0, 1.0, 1.0));
    button.setTitle(&NSString::from_str(title));
    // Safe AppKit state polling avoids an unsafe Objective-C target/selector bridge.
    button.setButtonType(NSButtonType::PushOnPushOff);
    button.setBezelStyle(NSBezelStyle::Push);
    button.setBordered(false);
    button.setFont(Some(&NSFont::systemFontOfSize(14.0)));
    button
}

pub(super) fn pressed(button: &NSButton) -> bool {
    let pressed = button.isEnabled() && button.state() != 0;
    button.setState(0);
    pressed
}
