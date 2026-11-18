//! Native field drafts contain a captured runtime target, never an editable document copy.
use super::{RIGHT, color, label, panel, rect};
use figcraft_ui_egui::workspace_properties::{Commit, Field, Inspector, Target};
use objc2::{MainThreadMarker, MainThreadOnly, Message, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSBox, NSEventType, NSFont, NSScrollView, NSTextField, NSTextInputClient, NSTextInputContext, NSTextView, NSView, NSWindow,
};
use objc2_foundation::NSString;
struct Control {
    field: Field,
    input: Retained<NSTextField>,
    caption: Retained<NSTextField>,
    plate: Retained<NSBox>,
}
struct Draft {
    index: usize,
    target: Target,
    original: String,
}
pub(super) struct Properties {
    scroll: Retained<NSScrollView>,
    content: Retained<NSView>,
    controls: Vec<Control>,
    sections: Vec<(Retained<NSTextField>, f64)>,
    rules: Vec<(Retained<NSBox>, f64)>,
    observations: Vec<(&'static str, Retained<NSTextField>, f64, f64, f64)>,
    swatch: Retained<NSBox>,
    stroke_swatch: Retained<NSBox>,
    corner_plate: Retained<NSBox>,
    corner_caption: Retained<NSTextField>,
    draft: Option<Draft>,
    context: Option<Retained<NSTextInputContext>>,
    focus: crate::native_canvas::NativeInputFocus,
    geometry: Option<(f64, f64)>,
    window: Retained<NSWindow>,
    canvas: Retained<NSView>,
    gate: KeyGate,
}
impl Properties {
    pub(super) fn install(mtm: MainThreadMarker, right: &NSBox, window: &NSWindow, canvas: &NSView) -> Result<Self, String> {
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), rect(0.0, 0.0, RIGHT, 500.0));
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDrawsBackground(false);
        let content = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, RIGHT, 660.0));
        // Only the native inspector gets a backing store. Static labels/plates must survive
        // Metal canvas repaints; this does not replace or reparent the original render layer.
        content.setWantsLayer(true);
        scroll.setDocumentView(Some(&content));
        let mut controls = Vec::new();
        for field in Field::ALL {
            let input = NSTextField::textFieldWithString(&NSString::new(), mtm);
            input.setBezeled(false);
            input.setBordered(false);
            input.setDrawsBackground(false);
            input.setFont(Some(&NSFont::systemFontOfSize(16.0)));
            input.setTextColor(Some(&color(0x121212)));
            input.setBackgroundColor(Some(&color(0xf7f5f2)));
            input.setPlaceholderString(Some(&NSString::from_str(field.label())));
            let caption = label(mtm, if field == Field::Opacity { "透明度" } else { field.label() }, 14.0, 0x736e69);
            let plate = panel(mtm, 0xf7f5f2);
            plate.setCornerRadius(10.);
            content.addSubview(&plate);
            content.addSubview(&caption);
            content.addSubview(&input);
            controls.push(Control { field, input, caption, plate });
        }
        let corner_plate = panel(mtm, 0xf7f5f2);
        corner_plate.setCornerRadius(10.);
        content.addSubview(&corner_plate);
        let mut sections = Vec::new();
        for (text, y) in [("位置与尺寸", 187.), ("外观", 382.), ("填充", 526.), ("描边", 629.), ("效果", 734.), ("更多属性", 850.)] {
            let title = label(mtm, text, 17., 0x121212);
            content.addSubview(&title);
            sections.push((title, y - 80.));
        }
        let mut rules = Vec::new();
        for y in [159., 354., 498., 610., 713., 826.] {
            let rule = panel(mtm, 0xdfdcd8);
            content.addSubview(&rule);
            rules.push((rule, y - 80.));
        }
        let mut observations = Vec::new();
        for (key, x, y, w) in [
            ("kind", 310., 113., 74.),
            ("corner", 89., 436., 100.),
            ("stroke", 64., 667., 220.),
            ("strokeWidth", 329., 668., 65.),
            ("effect", 24., 776., 350.),
            ("fillOpacity", 329., 565., 65.),
        ] {
            let ink = matches!(key, "corner" | "stroke" | "effect");
            let value = label(mtm, "", if ink { 16. } else { 14. }, if ink { 0x121212 } else { 0x736e69 });
            content.addSubview(&value);
            observations.push((key, value, x, y - 80., w));
        }
        let corner = label(mtm, "圆角", 14., 0x736e69);
        content.addSubview(&corner);
        let swatch = panel(mtm, 0xf7f5f2);
        swatch.setCornerRadius(6.);
        swatch.setBorderWidth(1.);
        swatch.setBorderColor(&color(0xdfdcd8));
        content.addSubview(&swatch);
        let stroke_swatch = panel(mtm, 0xf7f5f2);
        stroke_swatch.setCornerRadius(6.);
        content.addSubview(&stroke_swatch);
        let header = right.contentView().ok_or("native inspector has no content view")?;
        header.addSubview(&scroll);
        Ok(Self {
            scroll,
            content,
            controls,
            sections,
            rules,
            observations,
            swatch,
            stroke_swatch,
            corner_plate,
            corner_caption: corner,
            draft: None,
            context: None,
            focus: Default::default(),
            geometry: None,
            window: window.retain(),
            canvas: canvas.retain(),
            gate: Default::default(),
        })
    }
    pub(super) fn editing(&self) -> bool {
        self.controls.iter().any(|c| c.input.currentEditor().is_some())
    }
    fn cancel(&mut self, restore_canvas: bool) {
        if self.draft.take().is_some() {
            if let Some(context) = &self.context {
                context.discardMarkedText();
            }
            let clicked_canvas = self.window.firstResponder().is_some_and(|r| std::ptr::eq::<objc2_app_kit::NSResponder>(&*r, &**self.canvas))
                && MainThreadMarker::new()
                    .and_then(|mtm| NSApplication::sharedApplication(mtm).currentEvent())
                    .is_some_and(|e| e.r#type() == NSEventType::LeftMouseDown);
            if restore_canvas || clicked_canvas {
                self.window.makeFirstResponder(Some(&self.canvas));
            } else {
                // An external update invalidated this edit without a canvas click. Keep the
                // keyboard on the window: a late Return/Delete must not edit the canvas.
                self.window.makeFirstResponder(None);
            }
            for control in &self.controls {
                control.input.abortEditing();
            }
        }
    }
    /// Navigation wins over blur: a click on Home/tab/close cancels, never commits its old field.
    pub(super) fn take_commit(&mut self, view: &Inspector, navigation: bool) -> Option<Commit> {
        if navigation || self.draft.as_ref().is_some_and(|d| view.target.as_ref() != Some(&d.target)) {
            self.cancel(navigation);
            return None;
        }
        // A new editor can already have received Return before the first host tick (notably
        // after Tab). Capture its target before consuming that key or the confirmation is lost.
        let editing = self.controls.iter().position(|c| c.input.currentEditor().is_some());
        if self.draft.is_none()
            && let (Some(index), Some(target)) = (editing, view.target.clone())
        {
            let original = view.fields.iter().find(|p| p.field == self.controls[index].field).and_then(|p| p.value.clone()).unwrap_or_default();
            self.draft = Some(Draft { index, target, original });
        }
        let marked = self
            .controls
            .iter()
            .filter_map(|c| c.input.currentEditor())
            .any(|editor| editor.downcast_ref::<NSTextView>().is_some_and(|view| view.hasMarkedText()));
        let event =
            MainThreadMarker::new().and_then(|mtm| NSApplication::sharedApplication(mtm).currentEvent()).and_then(|event| match event.r#type() {
                NSEventType::KeyDown => Some((event.timestamp().to_bits(), event.keyCode(), true)),
                NSEventType::KeyUp => Some((event.timestamp().to_bits(), event.keyCode(), false)),
                _ => None,
            });
        let key = self.gate.observe(event, marked);
        if let Some(key) = key
            && let Some(draft) = self.draft.take()
        {
            if key == 53 {
                self.draft = Some(draft);
                self.cancel(true);
                return None;
            }
            let control = &self.controls[draft.index];
            control.input.validateEditing();
            let value = control.input.stringValue().to_string();
            self.window.makeFirstResponder(Some(&self.canvas));
            if key == 48
                && let Some(next) =
                    self.controls.iter().skip(draft.index + 1).chain(self.controls.iter().take(draft.index)).find(|c| c.input.isEnabled())
            {
                self.window.makeFirstResponder(Some(&next.input));
            }
            if value != draft.original {
                return Some(Commit { target: draft.target, field: control.field, value });
            }
        }
        let editing = self.controls.iter().position(|c| c.input.currentEditor().is_some());
        if self.draft.as_ref().is_some_and(|d| Some(d.index) != editing) {
            let draft = self.draft.take()?;
            let control = self.controls.get(draft.index)?;
            let value = control.input.stringValue().to_string();
            if value != draft.original {
                return Some(Commit { target: draft.target, field: control.field, value });
            }
        }
        None
    }
    pub(super) fn sync(&mut self, view: &Inspector, busy: bool, width: f64, height: f64, window: &NSWindow) {
        if self.draft.as_ref().is_some_and(|d| view.target.as_ref() != Some(&d.target)) {
            self.cancel(false);
        }
        if self.geometry != Some((width, height)) {
            let height_changed = self.geometry.is_none_or(|(_, old_height)| old_height != height);
            self.geometry = Some((width, height));
            let columns = (width - 48.) / (RIGHT - 48.);
            let align = |x: f64| 24. + (x - 24.) * columns;
            let viewport = (height - 82.0).max(1.0);
            self.scroll.setFrame(rect(0.0, 0.0, width, viewport));
            self.content.setFrame(rect(0.0, 0.0, width, 1060.0_f64.max(viewport)));
            let top = self.content.frame().size.height;
            for control in &self.controls {
                let (x, y, w) = match control.field {
                    Field::Name => (24., 27., 280.),
                    Field::X => (24., 148., 172.),
                    Field::Y => (212., 148., 172.),
                    Field::Width => (24., 201., 172.),
                    Field::Height => (212., 201., 172.),
                    Field::Opacity => (212., 345., 172.),
                    Field::Fill => (64., 484., 245.),
                    Field::Rotation => (24., 815., 172.),
                    Field::Text => (24., 878., 360.),
                    Field::FontSize => (24., 941., 172.),
                };
                let (x, w) = match control.field {
                    Field::Fill => (x, align(x + w) - x),
                    _ => (align(x), w * columns),
                };
                let plain = matches!(control.field, Field::Name | Field::Fill);
                control.plate.setHidden(plain);
                control.plate.setFrame(rect(x, top - y - 42., w, 42.));
                control.caption.setHidden(plain);
                control.caption.setFrame(caption_rect(x, top - y - 32.));
                control.input.setFrame(rect(x + if plain { 0. } else { 65. }, top - y - 32., w - if plain { 0. } else { 77. }, 26.));
                if control.field == Field::Name {
                    control.input.setFont(Some(&NSFont::systemFontOfSize(21.)));
                }
            }
            for (label, y) in &self.sections {
                label.setFrame(rect(24., top - y - 28., width - 48., 28.));
            }
            for (rule, y) in &self.rules {
                rule.setFrame(rect(24., top - y - 1., width - 48., 1.));
            }
            for (key, label, x, y, w) in &self.observations {
                let left = if matches!(*key, "stroke" | "corner") { *x } else { align(*x) };
                label.setFrame(rect(left, top - y - 26., align(x + w) - left, 26.));
            }
            self.corner_plate.setFrame(rect(24., top - 387., 172. * columns, 42.));
            self.corner_caption.setFrame(caption_rect(12., top - 384.));
            self.swatch.setFrame(rect(24., top - 508., 24., 24.));
            self.stroke_swatch.setFrame(rect(24., top - 611., 24., 24.));
            if height_changed {
                let clip = self.scroll.contentView();
                clip.scrollToPoint(objc2_foundation::NSPoint::new(0.0, (self.content.frame().size.height - viewport).max(0.0)));
                self.scroll.reflectScrolledClipView(&clip);
            }
        }
        for control in &self.controls {
            let property = view.fields.iter().find(|p| p.field == control.field);
            let enabled = !busy && property.is_some_and(|p| p.enabled);
            control.input.setEnabled(enabled);
            if control.input.currentEditor().is_none() {
                let value = property.and_then(|p| p.value.as_deref()).unwrap_or("");
                control.input.setStringValue(&NSString::from_str(value));
                let placeholder = if property.is_some_and(|p| p.mixed) { "混合" } else { "" };
                control.input.setPlaceholderString(Some(&NSString::from_str(placeholder)));
            }
        }
        for (key, label, _, _, _) in &self.observations {
            label.setStringValue(&NSString::from_str(view.appearance.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str()).unwrap_or("")));
        }
        let stroke = view.appearance.iter().find(|(k, _)| *k == "stroke").and_then(|(_, v)| u32::from_str_radix(v.trim_start_matches('#'), 16).ok());
        self.stroke_swatch.setFillColor(&color(stroke.unwrap_or(0xf7f5f2)));
        let value = view
            .fields
            .iter()
            .find(|p| p.field == Field::Fill)
            .and_then(|p| p.value.as_deref())
            .and_then(|v| u32::from_str_radix(v.trim_start_matches('#'), 16).ok());
        self.swatch.setFillColor(&color(value.unwrap_or(0xf7f5f2)));
        if let Some(mtm) = MainThreadMarker::new()
            && let Some(context) = NSTextInputContext::currentInputContext(mtm)
        {
            self.context = Some(context);
        }
        if self.focus.tick(self.editing(), window.isKeyWindow())
            && let Some(context) = &self.context
        {
            context.activate();
        }
    }
}
impl Drop for Properties {
    fn drop(&mut self) {
        self.cancel(true);
    }
}

fn caption_rect(x: f64, y: f64) -> objc2_foundation::NSRect {
    // Labels paint above the inputs. Their transparent area still participates in AppKit
    // hit testing, so it must stop before the value and the neighboring opacity field.
    rect(x + 12., y, 48., 24.)
}

/// Poll only the original AppKit key event; never synthesize canvas input. A native IME owns
/// Return/Tab/Escape while marked text is active. The paired key-up cannot confirm it later.
#[derive(Default)]
pub(super) struct KeyGate {
    seen: Option<u64>,
    marked: bool,
    key_up: Option<u16>,
}
impl KeyGate {
    pub(super) fn observe(&mut self, event: Option<(u64, u16, bool)>, marked: bool) -> Option<u16> {
        let composing = self.marked || marked;
        self.marked = marked;
        let (serial, key, down) = event?;
        if self.seen == Some(serial) {
            return None;
        }
        self.seen = Some(serial);
        if !matches!(key, 36 | 48 | 53 | 76) {
            return None;
        }
        if !down && self.key_up == Some(key) {
            self.key_up = None;
            return None;
        }
        if down {
            self.key_up = Some(key);
        }
        if composing {
            return None;
        }
        Some(if key == 76 { 36 } else { key })
    }
}
#[cfg(test)]
mod tests {
    use super::KeyGate;
    #[test]
    fn field_labels_leave_their_value_and_neighbor_hit_regions_clear() {
        let caption = super::caption_rect(24., 0.);
        assert!(caption.origin.x + caption.size.width <= 24. + 65., "caption covers the first value characters");
        let corner = super::caption_rect(12., 0.);
        assert!(corner.origin.x + corner.size.width <= 89., "corner label covers radius or opacity input");
    }
    #[test]
    fn ime_confirmation_and_its_key_up_never_submit_a_property() {
        let mut gate = KeyGate::default();
        assert_eq!(gate.observe(Some((1, 0, true)), true), None);
        assert_eq!(gate.observe(Some((2, 36, true)), false), None);
        assert_eq!(gate.observe(Some((3, 36, false)), false), None);
        assert_eq!(gate.observe(Some((4, 36, true)), false), Some(36));
        assert_eq!(gate.observe(Some((4, 36, true)), false), None);
        assert_eq!(gate.observe(Some((5, 36, false)), false), None);
    }
    #[test]
    fn a_key_up_can_confirm_when_the_tick_did_not_see_the_key_down() {
        let mut gate = KeyGate::default();
        assert_eq!(gate.observe(Some((1, 48, false)), false), Some(48));
        assert_eq!(gate.observe(Some((2, 53, true)), true), None);
        assert_eq!(gate.observe(Some((3, 53, false)), false), None);
    }
}
