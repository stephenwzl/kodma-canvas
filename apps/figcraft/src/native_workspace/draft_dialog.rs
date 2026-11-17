//! Native name/confirmation drafts capture a file path, never a changing list position.
use super::{color, label, panel, rect, style};
use objc2::{MainThreadMarker, Message, rc::Retained};
use objc2_app_kit::{NSApplication, NSBox, NSEventType, NSTextField, NSTextInputClient, NSTextView, NSView, NSWindow};
use objc2_foundation::NSString;

#[derive(Clone)]
pub(super) enum Intent {
    Rename(String),
    Remove(String),
    Trash(String),
    ForgetMissing(String),
    Preview,
}
pub(super) struct Dialog {
    view: Retained<NSBox>,
    parent: Retained<NSView>,
    title: Retained<NSTextField>,
    input: Retained<NSTextField>,
    message: Retained<NSTextField>,
    cancel: style::Button,
    confirm: style::Button,
    pub intent: Option<Intent>,
    gate: super::inspector::KeyGate,
}
impl Dialog {
    pub fn new(mtm: MainThreadMarker, parent: &NSView) -> Result<Self, String> {
        let view = panel(mtm, 0xffffff);
        view.setCornerRadius(20.);
        view.setBorderWidth(1.);
        view.setBorderColor(&color(0xdfdcd8));
        let content = view.contentView().ok_or("missing draft dialog")?;
        let title = label(mtm, "", 22., 0x121212);
        let input = NSTextField::textFieldWithString(&NSString::new(), mtm);
        input.setFont(Some(&objc2_app_kit::NSFont::systemFontOfSize(16.)));
        input.setPlaceholderString(Some(&NSString::from_str("稿件名称")));
        let message = label(mtm, "", 14., 0x736e69);
        content.addSubview(&title);
        content.addSubview(&input);
        content.addSubview(&message);
        let cancel = style::Button::new(mtm, &content, "取消", 0xf7f5f2, None, 12.);
        let confirm = style::Button::new(mtm, &content, "保存", 0x121212, None, 12.);
        // Advertise standard dialog keys; the host polling below still respects IME composition.
        cancel.control.setKeyEquivalent(&NSString::from_str("\u{1b}"));
        confirm.control.setKeyEquivalent(&NSString::from_str("\r"));
        confirm.control.setContentTintColor(Some(&color(0xffffff)));
        parent.addSubview(&view);
        view.setHidden(true);
        Ok(Self { view, parent: parent.retain(), title, input, message, cancel, confirm, intent: None, gate: Default::default() })
    }
    pub fn show(&mut self, intent: Intent, name: &str, window: &NSWindow) {
        let rename = matches!(intent, Intent::Rename(_));
        self.title.setStringValue(&NSString::from_str(&match &intent {
            Intent::Rename(_) => "重命名稿件".into(),
            Intent::Trash(_) => format!("将“{}”移到废纸篓？", name.chars().take(14).collect::<String>()),
            Intent::ForgetMissing(_) => "移除失效记录？".into(),
            _ => "从列表移除稿件？".into(),
        }));
        self.input.setStringValue(&NSString::from_str(name));
        self.input.setHidden(!rename);
        self.message.setStringValue(&NSString::from_str(if rename {
            ""
        } else if matches!(&intent, Intent::Trash(_)) {
            "可以从系统废纸篓恢复。"
        } else if matches!(&intent, Intent::ForgetMissing(_)) {
            "仅移除已不存在的文件记录。"
        } else {
            "磁盘上的文件会保留。"
        }));
        self.confirm.title(if rename {
            "保存"
        } else if matches!(&intent, Intent::Trash(_)) {
            "移到废纸篓"
        } else {
            "移除"
        });
        self.intent = Some(intent);
        self.view.removeFromSuperview();
        self.parent.addSubview(&self.view);
        self.view.setHidden(false);
        if rename {
            window.makeFirstResponder(Some(&self.input));
        }
    }
    /// I2 state fixtures: render the real native controls without performing I3/I4 storage work.
    /// The validation launcher must opt in; normal product flows never dispatch a fake delete/save.
    pub fn preview(&mut self, kind: &str) {
        self.intent = Some(Intent::Preview);
        self.input.setHidden(true);
        self.title.setStringValue(&NSString::from_str(if kind == "trash" {
            "将“合成测试稿”移到废纸篓？"
        } else {
            "无法保存“合成测试稿”"
        }));
        self.message.setStringValue(&NSString::from_str(if kind == "trash" {
            "可以从系统废纸篓恢复。"
        } else {
            "磁盘空间不足。修改仍保留在当前稿件中。"
        }));
        self.confirm.title(if kind == "trash" { "移到废纸篓" } else { "重试" });
        self.confirm.enabled(false);
        self.parent.addSubview(&self.view);
        self.view.setHidden(false);
    }
    pub fn layout(&self, width: f64, height: f64) {
        let w = 620.0_f64.min((width - 48.).max(280.));
        self.view.setFrame(rect((width - w) / 2., (height - 300.) / 2., w, 300.));
        self.title.setFrame(rect(28., 230., w - 56., 40.));
        self.input.setFrame(rect(28., 154., w - 56., 42.));
        self.message.setFrame(rect(28., 112., w - 56., 36.));
        self.cancel.frame(w - 308., 28., 136., 44.);
        self.confirm.frame(w - 164., 28., 136., 44.);
    }
    pub fn editing(&self) -> bool {
        self.intent.is_some()
    }
    pub fn hide(&mut self, window: &NSWindow, canvas: &NSView) {
        self.input.abortEditing();
        window.makeFirstResponder(Some(canvas));
        self.intent = None;
        self.view.setHidden(true);
    }
    pub fn poll(&mut self, window: &NSWindow, canvas: &NSView) -> Option<(Intent, String)> {
        let marked = self.input.currentEditor().is_some_and(|e| e.downcast_ref::<NSTextView>().is_some_and(|v| v.hasMarkedText()));
        let event = MainThreadMarker::new().and_then(|mtm| NSApplication::sharedApplication(mtm).currentEvent()).and_then(|e| match e.r#type() {
            NSEventType::KeyDown => Some((e.timestamp().to_bits(), e.keyCode(), true)),
            NSEventType::KeyUp => Some((e.timestamp().to_bits(), e.keyCode(), false)),
            _ => None,
        });
        let key = self.gate.observe(event, marked);
        let name = self
            .input
            .currentEditor()
            .map(|editor| editor.string().to_string())
            .unwrap_or_else(|| self.input.stringValue().to_string())
            .trim()
            .to_string();
        let valid = !name.is_empty();
        if matches!(self.intent, Some(Intent::Preview)) {
            if self.cancel.pressed() || key == Some(53) {
                self.hide(window, canvas);
            }
            return None;
        }
        let rename = matches!(self.intent, Some(Intent::Rename(_)));
        self.confirm.enabled(!rename || valid);
        self.message.setStringValue(&NSString::from_str(if rename && !valid {
            "名称不能为空"
        } else if rename {
            ""
        } else if matches!(self.intent, Some(Intent::Trash(_))) {
            "可以从系统废纸篓恢复。"
        } else if matches!(self.intent, Some(Intent::ForgetMissing(_))) {
            "仅移除已不存在的文件记录。"
        } else {
            "磁盘上的文件会保留。"
        }));
        if self.cancel.pressed() || key == Some(53) {
            self.hide(window, canvas);
        }
        if self.confirm.pressed() || (key == Some(36) && (!rename || valid)) {
            let intent = self.intent.clone()?;
            self.hide(window, canvas);
            return Some((intent, name));
        }
        None
    }
}
