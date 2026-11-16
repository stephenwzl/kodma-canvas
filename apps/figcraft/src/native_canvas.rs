//! S001 validation host: native controls and the existing egui content view in one NSWindow.
//!
//! winit's macOS delegate assumes its contentView is a WinitView. Replacing/reparenting that
//! view would invalidate that assumption. Keep it and its event loop intact; AppKit controls
//! occupy a reserved area in the same content view. No raw pointers or unsafe bridge are needed.

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSResponder, NSSlider, NSTextField, NSView, NSWindow};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

pub struct Host {
    window: Retained<NSWindow>,
    canvas: Retained<NSView>,
    panel: Retained<NSView>,
    input: Retained<NSTextField>,
    width: Retained<NSSlider>,
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
        panel.addSubview(&input);
        panel.addSubview(&width);
        canvas.addSubview(&panel);
        // The same WinitView remains the window's content view and owns its original layer.
        window.makeFirstResponder(Some(&canvas));
        log::info!("native_canvas installed in NSWindow {}", window.windowNumber());
        let host = Self { window, canvas, panel, input, width, last_state: Cell::new(None) };
        host.sync();
        Ok(host)
    }

    pub fn sync(&self) -> figcraft_ui_egui::CanvasHost {
        let bounds = self.canvas.bounds();
        let width = self.width.doubleValue().clamp(0.0, (bounds.size.width - 1.0).max(0.0));
        self.panel.setFrame(rect(bounds.size.width - width, 0.0, width, bounds.size.height));
        self.input.setFrame(rect(16.0, (bounds.size.height - 60.0).max(0.0), (width - 32.0).max(0.0), 28.0));
        self.width.setFrame(rect(16.0, (bounds.size.height - 110.0).max(0.0), (width - 32.0).max(0.0), 28.0));
        let keyboard = self.window.firstResponder().is_some_and(|responder| std::ptr::eq::<NSResponder>(&*responder, &**self.canvas));
        let state = (keyboard, self.window.isMiniaturized(), bounds.size.width as u32, bounds.size.height as u32);
        if self.last_state.replace(Some(state)) != Some(state) {
            log::info!("native_canvas keyboard={} minimized={} size={}x{}", state.0, state.1, state.2, state.3);
        }
        figcraft_ui_egui::CanvasHost { right_inset: width as f32, keyboard }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        // End native field editing before removing its views; the runtime itself is not reset.
        self.window.makeFirstResponder(Some(&self.canvas));
        self.panel.removeFromSuperview();
    }
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}
