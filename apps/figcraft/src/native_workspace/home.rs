//! Native home controls. Button state is consumed on the established main-thread host tick;
//! actions carry paths, never a changing recent-list index, and dispatch through FigApp.
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, Message};
use objc2_app_kit::{
    NSBezelStyle, NSBox, NSButton, NSButtonType, NSFont, NSPopUpButton, NSScrollView, NSTextField, NSTextInputContext, NSView, NSWindow,
};
use objc2_foundation::{NSPoint, NSString};

use super::{color, panel, rect, row_pool::RowPool, style};
use figcraft_ui_egui::workspace::{Action, RecentFile};

struct Row {
    file: RecentFile,
    button: style::Button,
    menu: Retained<NSPopUpButton>,
}
impl Drop for Row {
    fn drop(&mut self) {
        self.button.view.removeFromSuperview();
        self.menu.removeFromSuperview();
    }
}

pub(super) struct Controls {
    new: style::Button,
    open: style::Button,
    home: style::Button,
    search: Retained<NSTextField>,
    search_plate: Retained<NSBox>,
    empty: Retained<NSTextField>,
    dialog: super::draft_dialog::Dialog,
    window: Retained<NSWindow>,
    canvas: Retained<NSView>,
    underline: Retained<NSBox>,
    error: Retained<NSTextField>,
    scroll: Retained<NSScrollView>,
    rows_view: Retained<NSView>,
    rows: RowPool<Row>,
    previous_error: Option<String>,
    focus: crate::native_canvas::NativeInputFocus,
    input_context: Option<Retained<NSTextInputContext>>,
    geometry: Option<(f64, f64, bool, bool)>,
    review_action: Option<(String, String)>,
}

impl Controls {
    pub(super) fn install(mtm: MainThreadMarker, left: &NSBox, top: &NSBox, window: &NSWindow, canvas: &NSView) -> Result<Self, String> {
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
        let search_plate = panel(mtm, 0xf7f5f2);
        search_plate.setCornerRadius(12.);
        search_plate.setBorderWidth(1.);
        search_plate.setBorderColor(&color(0xd4d0cc));
        left_content.addSubview(&search_plate);
        let search = NSTextField::textFieldWithString(&NSString::new(), mtm);
        search.setPlaceholderString(Some(&NSString::from_str("搜索")));
        search.setFont(Some(&NSFont::systemFontOfSize(16.)));
        search.setBordered(false);
        search.setBezeled(false);
        search.setDrawsBackground(false);
        left_content.addSubview(&search);
        let empty = super::label(mtm, "无匹配稿件", 14., 0x736e69);
        left_content.addSubview(&empty);
        let dialog = super::draft_dialog::Dialog::new(mtm, canvas)?;
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

        let mut controls = Self {
            new,
            open,
            home,
            search,
            search_plate,
            empty,
            dialog,
            window: window.retain(),
            canvas: canvas.retain(),
            underline,
            error,
            scroll,
            rows_view,
            rows: RowPool::new(),
            previous_error: None,
            focus: Default::default(),
            input_context: None,
            geometry: None,
            // Explicit validation-launch hook opens a real confirmation for a real listed file.
            // It never confirms the action and is absent in ordinary launches.
            review_action: std::env::var("KODMA_S003_REVIEW_ACTION")
                .ok()
                .and_then(|v| v.split_once(':').map(|(kind, path)| (kind.into(), path.into()))),
        };
        if let Ok(kind) = std::env::var("KODMA_S003_UI_STATE")
            && matches!(kind.as_str(), "trash" | "save-error")
        {
            controls.dialog.preview(&kind);
        }
        Ok(controls)
    }

    pub(super) fn editing(&self) -> bool {
        self.search.currentEditor().is_some() || self.dialog.editing()
    }
    pub(super) fn modal(&self) -> bool {
        self.dialog.editing()
    }

    pub(super) fn take_actions(&mut self) -> Vec<Action> {
        let mut actions = Vec::new();
        if self.dialog.editing() {
            if let Some((intent, name)) = self.dialog.poll(&self.window, &self.canvas) {
                use super::draft_dialog::Intent;
                actions.push(match intent {
                    Intent::Rename(path) => Action::RenameRecent(path, name),
                    Intent::Remove(path) => Action::RemoveRecent(path),
                    Intent::Trash(path) => Action::TrashRecent(path),
                    Intent::ForgetMissing(path) => Action::ForgetMissingRecent(path),
                    Intent::Preview => return actions,
                });
            }
            return actions;
        }
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
            let choice = row.menu.indexOfSelectedItem();
            if choice > 0 {
                row.menu.selectItemAtIndex(0);
                use super::draft_dialog::Intent;
                match choice {
                    1 => self.dialog.show(Intent::Rename(row.file.path.clone()), &row.file.name, &self.window),
                    2 => actions.push(Action::SaveAsRecent(row.file.path.clone())),
                    3 => self.dialog.show(
                        if row.file.managed { Intent::Trash(row.file.path.clone()) } else { Intent::Remove(row.file.path.clone()) },
                        &row.file.name,
                        &self.window,
                    ),
                    4 if row.file.managed => self.dialog.show(Intent::ForgetMissing(row.file.path.clone()), &row.file.name, &self.window),
                    _ => {}
                }
            }
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
        if let Some(mtm) = MainThreadMarker::new()
            && let Some(context) = NSTextInputContext::currentInputContext(mtm)
        {
            self.input_context = Some(context);
        }
        if self.focus.tick(self.editing(), self.window.isKeyWindow())
            && let Some(context) = &self.input_context
        {
            context.activate();
        }
        if !is_home && self.dialog.editing() {
            self.dialog.hide(&self.window, &self.canvas);
        }
        if is_home
            && !busy
            && let Some((kind, path)) = self.review_action.take()
            && let Some(file) = recents.iter().find(|f| f.path == path)
        {
            use super::draft_dialog::Intent;
            let intent = match kind.as_str() {
                "trash" if file.managed => Some(Intent::Trash(path)),
                "remove" if !file.managed => Some(Intent::Remove(path)),
                "missing" if file.managed => Some(Intent::ForgetMissing(path)),
                _ => None,
            };
            if let Some(intent) = intent {
                self.dialog.show(intent, &file.name, &self.window);
            }
        }
        let query = self.search.currentEditor().map(|editor| editor.string().to_string()).unwrap_or_else(|| self.search.stringValue().to_string());
        let recents: Vec<_> = recents.into_iter().filter(|file| matches_query(&file.name, &query)).collect();
        self.empty.setHidden(!is_home || query.trim().is_empty() || !recents.is_empty());
        self.empty.setFrame(rect(36., height - 310., 180., 28.));
        self.dialog.layout(width, height);
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
                    let menu = NSPopUpButton::initWithFrame_pullsDown(NSPopUpButton::alloc(mtm), rect(0., 0., 32., 44.), true);
                    populate_menu(&menu, file.managed);
                    menu.setBordered(false);
                    if let Some(items) = menu.menu() {
                        items.setAutoenablesItems(false);
                    }
                    self.rows_view.addSubview(&menu);
                    Row { file: file.clone(), button, menu }
                },
                |row, file| {
                    row.button.pressed();
                    row.button.title(&file.name.chars().take(80).collect::<String>());
                    if row.file.name.is_empty() != file.name.is_empty() {
                        row.button.icon("folder", 0x121212);
                    }
                    row.button.control.setToolTip(Some(&NSString::from_str(&format!("{}\n{}", file.name, file.path))));
                    row.menu.setToolTip(Some(&NSString::from_str(&format!("{}\n{}", file.name, file.path))));
                    row.button.view.setHidden(false);
                    row.menu.setHidden(false);
                    row.menu.selectItemAtIndex(0);
                    if row.file.managed != file.managed {
                        populate_menu(&row.menu, file.managed);
                    }
                    row.file = file;
                },
                |row, file| (!row.button.tracking() && !row.menu.isHighlighted()) || row.file.path == file.path,
            );
            for row in self.rows.inactive() {
                row.button.view.setHidden(true);
                row.menu.setHidden(true);
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
        self.search.setHidden(!is_home);
        self.search_plate.setHidden(!is_home);
        self.search.setEnabled(!busy);
        self.scroll.setHidden(!is_home);
        self.new.view.setHidden(!is_home);
        if changed || self.geometry != Some(key) {
            self.geometry = Some(key);
            self.new.frame(if is_home { 26. } else { 180. }, height - if is_home { 172. } else { 168. }, if is_home { 190. } else { 36. }, 44.);
            self.new.title(if is_home { "新建稿件" } else { "" });
            self.new.icon_offset(if is_home { 55. } else { 10. });
            self.home.frame(42.0, 9.0, 84.0, 42.0);
            self.search_plate.setFrame(rect(26., height - 232., 190., 44.));
            self.search.setFrame(rect(40., height - 224., 160., 28.));
            self.open.title("打开稿件…");
            self.open.frame(26., 62., 190., 44.);
            self.open.leading();
            self.error.setFrame(rect(26.0, 116.0, 190.0, 88.0));
            let bottom = if self.previous_error.is_some() { 218.0 } else { 140.0 };
            let list_top = if is_home { 278. } else { 177. };
            let bottom = if is_home { bottom } else { height - 340. };
            let viewport = (height - list_top - bottom).max(1.0);
            self.scroll.setFrame(rect(26.0, bottom, 190.0, viewport));
            let rows_height = (self.rows.len() as f64 * 52.0).max(viewport);
            self.rows_view.setFrame(rect(0.0, 0.0, 190.0, rows_height));
            for (index, row) in self.rows.iter().enumerate() {
                let y = rows_height - 44. - index as f64 * 52.;
                row.button.frame(0., y, 154., 44.);
                row.menu.setFrame(rect(154., y + 4., 36., 36.));
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
            row.menu.setEnabled(!busy);
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

fn matches_query(name: &str, query: &str) -> bool {
    name.to_lowercase().contains(&query.trim().to_lowercase())
}

fn populate_menu(menu: &NSPopUpButton, managed: bool) {
    menu.removeAllItems();
    for title in ["•••", "重命名…", "另存为…"].into_iter().chain(if managed {
        vec!["移到废纸篓…", "移除失效记录…"]
    } else {
        vec!["从列表移除…"]
    }) {
        menu.addItemWithTitle(&NSString::from_str(title));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn search_accepts_chinese_case_and_clearing() {
        assert!(super::matches_query("阅读 App", "阅读"));
        assert!(super::matches_query("阅读 App", " app "));
        assert!(super::matches_query("阅读 App", ""));
        assert!(!super::matches_query("阅读 App", "不存在"));
    }
}
