//! S001 validation host: native controls and the existing egui content view in one NSWindow.
//!
//! winit's macOS delegate assumes its contentView is a WinitView. Replacing/reparenting that
//! view would invalidate that assumption. Keep it and its event loop intact; AppKit controls
//! occupy a reserved area in the same content view. No raw pointers or unsafe bridge are needed.

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSPopUpButton, NSResponder, NSSlider, NSTextField, NSTextInputContext, NSView, NSWindow};
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
    activate_native_context: bool,
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
            activate_native_context: false,
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
        let state = (keyboard, self.window.isMiniaturized(), bounds.size.width as u32, bounds.size.height as u32);
        if self.last_state.replace(Some(state)) != Some(state) {
            log::info!("native_canvas keyboard={} minimized={} size={}x{}", state.0, state.1, state.2, state.3);
        }
        figcraft_ui_egui::CanvasHost { right_inset: width as f32, keyboard }
    }

    fn sync_input_source(&mut self, keyboard: bool) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        if let Some(context) = NSTextInputContext::currentInputContext(mtm) {
            self.input_context = Some(context);
        }
        let Some(context) = &self.input_context else { return };
        if keyboard || self.input.currentEditor().is_none() {
            self.activate_native_context = false;
        } else {
            if self.activate_native_context && self.window.isKeyWindow() {
                context.activate();
                self.activate_native_context = false;
            }
            // Let eframe disable the former canvas IME first, then activate the native field's
            // real text context on the following host tick. No text or source is synthesized.
            if self.last_state.get().is_some_and(|state| state.0) {
                self.activate_native_context = true;
            }
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
    }
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}
