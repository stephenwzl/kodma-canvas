//! S001 validation host: native controls and the existing egui content view in one NSWindow.
//!
//! winit's macOS delegate assumes its contentView is a WinitView. Replacing/reparenting that
//! view would invalidate that assumption. Keep it and its event loop intact; AppKit controls
//! occupy a reserved area in the same content view. No raw pointers or unsafe bridge are needed.

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem, NSPopUpButton, NSResponder, NSSlider, NSTextField, NSTextInputContext, NSView, NSWindow};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

pub struct Host {
    window: Retained<NSWindow>,
    canvas: Retained<NSView>,
    panel: Retained<NSView>,
    input: Retained<NSTextField>,
    width: Retained<NSSlider>,
    sources: Retained<NSPopUpButton>,
    source_ids: Vec<Retained<NSString>>,
    source_index: isize,
    input_context: Option<Retained<NSTextInputContext>>,
    original_source: Option<Retained<NSString>>,
    requested_source: Option<Retained<NSString>>,
    native_input_focus: NativeInputFocus,
    edit_menu: Option<NativeEditMenu>,
    last_state: Cell<Option<(bool, bool, u32, u32)>>,
}

impl Host {
    pub fn install(title: &str) -> Result<Self, String> {
        let mtm = MainThreadMarker::new().ok_or("native canvas must be installed on the main thread")?;
        let windows = NSApplication::sharedApplication(mtm).windows();
        let mut candidates = windows.iter().filter(|window| window.title().to_string() == title);
        let window = candidates.next().ok_or("could not find the eframe window for native canvas")?;
        if candidates.next().is_some() {
            return Err("ambiguous eframe window for native canvas".into());
        }
        let canvas = window.contentView().ok_or("eframe window has no content view")?;
        let panel = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, 320.0, 500.0));
        let input = NSTextField::textFieldWithString(&NSString::new(), mtm);
        input.setPlaceholderString(Some(&NSString::from_str("输入")));
        let width = NSSlider::initWithFrame(NSSlider::alloc(mtm), rect(16.0, 16.0, 288.0, 28.0));
        width.setMinValue(220.0);
        width.setMaxValue(520.0);
        width.setDoubleValue(320.0);
        let sources = NSPopUpButton::initWithFrame_pullsDown(NSPopUpButton::alloc(mtm), rect(16.0, 16.0, 288.0, 28.0), false);
        sources.addItemWithTitle(&NSString::from_str("输入法"));
        sources.setEnabled(false);
        panel.addSubview(&sources);
        panel.addSubview(&input);
        panel.addSubview(&width);
        canvas.addSubview(&panel);
        // The same WinitView remains the window's content view and owns its original layer.
        window.makeFirstResponder(Some(&canvas));
        log::info!("native_canvas installed in NSWindow {}", window.windowNumber());
        let mut host = Self {
            window,
            canvas,
            panel,
            input,
            width,
            sources,
            source_ids: Vec::new(),
            source_index: -1,
            input_context: None,
            original_source: None,
            requested_source: None,
            native_input_focus: NativeInputFocus::default(),
            edit_menu: None,
            last_state: Cell::new(None),
        };
        host.sync();
        Ok(host)
    }

    pub fn sync(&mut self) -> figcraft_ui_egui::CanvasHost {
        let bounds = self.canvas.bounds();
        let width = self.width.doubleValue().clamp(0.0, (bounds.size.width - 1.0).max(0.0));
        self.panel.setFrame(rect(bounds.size.width - width, 0.0, width, bounds.size.height));
        self.input.setFrame(rect(16.0, (bounds.size.height - 60.0).max(0.0), (width - 32.0).max(0.0), 28.0));
        self.width.setFrame(rect(16.0, (bounds.size.height - 110.0).max(0.0), (width - 32.0).max(0.0), 28.0));
        self.sources.setFrame(rect(16.0, (bounds.size.height - 160.0).max(0.0), (width - 32.0).max(0.0), 28.0));
        let keyboard = self.window.firstResponder().is_some_and(|responder| std::ptr::eq::<NSResponder>(&*responder, &**self.canvas));
        self.sync_input_source(keyboard);
        if let Some(menu) = &self.edit_menu {
            // Hidden native key equivalents must not steal the canvas engine's undo or clipboard.
            menu.set_active(!keyboard && self.input.currentEditor().is_some());
        }
        let state = (keyboard, self.window.isMiniaturized(), bounds.size.width as u32, bounds.size.height as u32);
        if self.last_state.replace(Some(state)) != Some(state) {
            log::info!("native_canvas keyboard={} minimized={} size={}x{}", state.0, state.1, state.2, state.3);
        }
        figcraft_ui_egui::CanvasHost { right_inset: width as f32, keyboard, ..Default::default() }
    }

    /// Install after craft-menu's guarded app menu, preserving its Quit and Services entries.
    pub fn install_edit_menu(&mut self) -> Result<(), String> {
        self.edit_menu = Some(NativeEditMenu::install()?);
        Ok(())
    }

    fn sync_input_source(&mut self, keyboard: bool) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        if let Some(context) = NSTextInputContext::currentInputContext(mtm) {
            self.input_context = Some(context);
        }
        let Some(context) = &self.input_context else { return };
        let editing = !keyboard && self.input.currentEditor().is_some();
        if self.native_input_focus.tick(editing, self.window.isKeyWindow()) {
            context.activate();
        }
        if self.source_ids.is_empty() {
            let Some(sources) = context.keyboardInputSources() else { return };
            self.sources.removeAllItems();
            self.sources.addItemWithTitle(&NSString::from_str("输入法"));
            // Bound the validation menu even if the platform supplies an unexpectedly large list.
            for source in sources.iter().take(64) {
                let title = NSTextInputContext::localizedNameForInputSource(&source, mtm).unwrap_or_else(|| source.clone());
                self.sources.addItemWithTitle(&title);
                self.source_ids.push(source);
            }
            self.sources.selectItemAtIndex(0);
            self.source_index = 0;
            self.sources.setEnabled(!self.source_ids.is_empty());
        }
        let current = context.selectedKeyboardInputSource();
        if self.original_source.is_none() && self.requested_source.is_none() {
            self.original_source = current.clone();
        }
        let index = self.sources.indexOfSelectedItem();
        if index != self.source_index {
            // Only an explicit choice in this validation control selects an already enabled
            // system source. Text still goes through AppKit/winit's real IME, never ui.type.
            if let Ok(index) = usize::try_from(index)
                && let Some(source) = index.checked_sub(1).and_then(|index| self.source_ids.get(index))
            {
                context.setSelectedKeyboardInputSource(Some(source));
                self.requested_source = Some(source.clone());
                self.native_input_focus.source_selected();
                log::info!("native_canvas input_source={source}");
            }
            self.source_index = index;
        } else if let Some(current) = current
            && let Some(index) = self.source_ids.iter().position(|id| **id == *current)
        {
            // Reflect a system/focus-driven source change without issuing another selection.
            let index = (index + 1) as isize;
            self.sources.selectItemAtIndex(index);
            self.source_index = index;
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        // End native field editing before removing its views; the runtime itself is not reset.
        if let (Some(context), Some(source), Some(requested)) = (&self.input_context, &self.original_source, &self.requested_source)
            && context.selectedKeyboardInputSource().as_ref().is_some_and(|current| **current == **requested)
        {
            // Restore only our own last choice, preserving a source subsequently chosen elsewhere.
            context.setSelectedKeyboardInputSource(Some(source));
        }
        self.window.makeFirstResponder(Some(&self.canvas));
        self.panel.removeFromSuperview();
        log::info!("native_canvas detached from NSWindow {}", self.window.windowNumber());
    }
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

/// Standard responder actions belong to AppKit while its field editor owns the keyboard.
/// Keep muda's definitions alive; there is no second command dispatcher or native text model.
pub(super) struct NativeEditMenu {
    parent: Retained<NSMenu>,
    item: Retained<NSMenuItem>,
    _definition: muda::Submenu,
    keys: Vec<(Retained<NSMenuItem>, Retained<NSString>)>,
    active: Cell<Option<bool>>,
}

impl NativeEditMenu {
    pub(super) fn install() -> Result<Self, String> {
        let mtm = MainThreadMarker::new().ok_or("native menu requires the main thread")?;
        let app = NSApplication::sharedApplication(mtm);
        let parent = app.mainMenu().ok_or("no application menu")?;
        let edit = muda::Submenu::with_items(
            "Edit",
            true,
            &[
                &muda::PredefinedMenuItem::undo(None),
                &muda::PredefinedMenuItem::redo(None),
                &muda::PredefinedMenuItem::separator(),
                &muda::PredefinedMenuItem::cut(None),
                &muda::PredefinedMenuItem::copy(None),
                &muda::PredefinedMenuItem::paste(None),
                &muda::PredefinedMenuItem::select_all(None),
            ],
        )
        .map_err(|error| error.to_string())?;
        let bridge = muda::Menu::with_items(&[&edit]).map_err(|error| error.to_string())?;
        // Obtain a retained AppKit item through public safe APIs. Installing then restoring
        // within this synchronous call avoids converting muda's raw NSMenu pointer with unsafe.
        bridge.init_for_nsapp();
        let temporary = app.mainMenu();
        app.setMainMenu(Some(&parent));
        let temporary = temporary.ok_or("native edit menu was not created")?;
        let item = temporary.itemAtIndex(0).ok_or("native edit menu is empty")?;
        temporary.removeItem(&item);
        item.setHidden(true);
        parent.addItem(&item);
        let keys = item
            .submenu()
            .map(|menu| {
                menu.itemArray()
                    .iter()
                    .map(|item| {
                        let key = item.keyEquivalent();
                        (item, key)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let menu = Self { parent, item, _definition: edit, keys, active: Cell::new(None) };
        menu.set_active(false);
        Ok(menu)
    }

    pub(super) fn set_active(&self, active: bool) {
        if self.active.replace(Some(active)) == Some(active) {
            return;
        }
        self.item.setHidden(!active);
        // AppKit can still search a hidden submenu's children for key equivalents. Remove
        // them while the canvas owns keys so Copy/Paste/Undo reach winit unchanged.
        let empty = NSString::new();
        for (item, key) in &self.keys {
            item.setKeyEquivalent(if active { key } else { &empty });
        }
    }
}

impl Drop for NativeEditMenu {
    fn drop(&mut self) {
        self.parent.removeItem(&self.item);
    }
}

/// Defer activation until eframe has disabled the former canvas IME. A source popup can also
/// interrupt a native field without ever giving the canvas keys, so track the field editor itself.
#[derive(Default)]
struct NativeInputFocus {
    editing: bool,
    pending: bool,
}

impl NativeInputFocus {
    fn tick(&mut self, editing: bool, key_window: bool) -> bool {
        let activate = editing && self.pending && key_window;
        self.pending = editing && (!self.editing || (self.pending && !activate));
        self.editing = editing;
        activate
    }

    fn source_selected(&mut self) {
        self.pending = true;
    }
}

#[cfg(test)]
mod tests {
    use super::NativeInputFocus;

    #[test]
    fn a_source_selection_reactivates_an_already_editing_native_field() {
        let mut focus = NativeInputFocus::default();
        assert!(!focus.tick(true, true));
        assert!(focus.tick(true, true));
        assert!(!focus.tick(true, true));
        focus.source_selected();
        assert!(focus.tick(true, true), "the picker can change source without returning keys to the canvas");
        assert!(!focus.tick(true, true), "do not repeatedly activate during composition");
    }

    #[test]
    fn returning_from_a_popup_rearms_the_field_and_does_not_activate_in_background() {
        let mut focus = NativeInputFocus::default();
        assert!(!focus.tick(true, true));
        assert!(focus.tick(true, true));
        assert!(!focus.tick(false, true));
        assert!(!focus.tick(true, false));
        assert!(!focus.tick(true, false));
        assert!(focus.tick(true, true));
        assert!(!focus.tick(true, true));
        focus.source_selected();
        assert!(!focus.tick(false, true), "no activation on a slider or popup");
    }
}
