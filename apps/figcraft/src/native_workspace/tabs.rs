//! Presentation-only tabs; identity and close/save continuations remain in the original runtime.
use super::{
    home::{button, pressed},
    rect,
};
use figcraft_ui_egui::{
    windows::CloseAnswer,
    workspace::{Action, Snapshot},
};
use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained};
use objc2_app_kit::{NSBox, NSButton, NSScrollView, NSView};
use objc2_foundation::{NSPoint, NSString};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

struct Tab {
    uid: u64,
    select: Retained<NSButton>,
    close: Retained<NSButton>,
}
struct Question {
    uid: u64,
    future: Pin<Box<dyn Future<Output = rfd::MessageDialogResult>>>,
}
pub(super) struct Tabs {
    scroll: Retained<NSScrollView>,
    content: Retained<NSView>,
    back: Retained<NSButton>,
    forward: Retained<NSButton>,
    save: Retained<NSButton>,
    save_as: Retained<NSButton>,
    rows: Vec<Tab>,
    active: Option<u64>,
    geometry: Option<(f64, usize)>,
    question: Option<Question>,
    alert: rfd::AsyncMessageDialog,
}
impl Tabs {
    pub(super) fn install(mtm: MainThreadMarker, top: &NSBox, alert: rfd::AsyncMessageDialog) -> Result<Self, String> {
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), rect(0.0, 0.0, 1.0, 1.0));
        scroll.setHasHorizontalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDrawsBackground(false);
        let content = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, 1.0, 1.0));
        scroll.setDocumentView(Some(&content));
        let back = button(mtm, "‹");
        let forward = button(mtm, "›");
        back.setToolTip(Some(&NSString::from_str("向前滚动标签")));
        forward.setToolTip(Some(&NSString::from_str("向后滚动标签")));
        let save = button(mtm, "保存");
        let save_as = button(mtm, "另存为…");
        let view = top.contentView().ok_or("native tabs have no content view")?;
        view.addSubview(&scroll);
        view.addSubview(&back);
        view.addSubview(&forward);
        view.addSubview(&save);
        view.addSubview(&save_as);
        Ok(Self { scroll, content, back, forward, save, save_as, rows: Vec::new(), active: None, geometry: None, question: None, alert })
    }
    pub(super) fn take_actions(&mut self) -> Vec<Action> {
        let mut actions = Vec::new();
        let direction = i32::from(pressed(&self.forward)) - i32::from(pressed(&self.back));
        if direction != 0 {
            let clip = self.scroll.contentView();
            let max_x = (self.content.frame().size.width - clip.bounds().size.width).max(0.0);
            clip.scrollToPoint(NSPoint::new((clip.bounds().origin.x + f64::from(direction) * 220.0).clamp(0.0, max_x), 0.0));
            self.scroll.reflectScrolledClipView(&clip);
        }
        if let Some(question) = &mut self.question {
            let mut ctx = Context::from_waker(Waker::noop());
            if let Poll::Ready(answer) = question.future.as_mut().poll(&mut ctx) {
                let answer = match answer {
                    rfd::MessageDialogResult::Yes => CloseAnswer::Save,
                    rfd::MessageDialogResult::No => CloseAnswer::DontSave,
                    rfd::MessageDialogResult::Custom(ref s) if s == "保存" => CloseAnswer::Save,
                    rfd::MessageDialogResult::Custom(ref s) if s == "不保存" => CloseAnswer::DontSave,
                    _ => CloseAnswer::Cancel,
                };
                actions.push(Action::AnswerClose(question.uid, answer));
                self.question = None;
            }
        }
        for tab in &self.rows {
            if pressed(&tab.close) {
                actions.push(Action::Close(tab.uid));
            }
            if pressed(&tab.select) {
                actions.push(Action::Switch(tab.uid));
            }
        }
        if let Some(uid) = self.active {
            if pressed(&self.save) {
                actions.push(Action::Save(uid));
            }
            if pressed(&self.save_as) {
                actions.push(Action::SaveAs(uid));
            }
        }
        actions
    }
    pub(super) fn sync(&mut self, snapshot: &Snapshot, width: f64) {
        let ids_changed = self.rows.iter().map(|r| r.uid).ne(snapshot.documents.iter().map(|d| d.id));
        if ids_changed {
            for row in &self.rows {
                row.select.removeFromSuperview();
                row.close.removeFromSuperview();
            }
            self.rows.clear();
            if let Some(mtm) = MainThreadMarker::new() {
                for doc in &snapshot.documents {
                    let select = button(mtm, "");
                    let close = button(mtm, "×");
                    close.setToolTip(Some(&NSString::from_str(&format!("关闭 {}", doc.name))));
                    self.content.addSubview(&select);
                    self.content.addSubview(&close);
                    self.rows.push(Tab { uid: doc.id, select, close });
                }
            }
        }
        let viewport = (width - 402.0).max(1.0);
        let geometry_changed = self.geometry != Some((width, self.rows.len()));
        if geometry_changed {
            self.geometry = Some((width, self.rows.len()));
            self.scroll.setFrame(rect(166.0, 4.0, viewport, 58.0));
            self.content.setFrame(rect(0.0, 0.0, (self.rows.len() as f64 * 220.0).max(viewport), 58.0));
            self.back.setFrame(rect(140.0, 9.0, 22.0, 42.0));
            self.forward.setFrame(rect(width - 230.0, 9.0, 22.0, 42.0));
            self.save.setFrame(rect(width - 190.0, 9.0, 72.0, 42.0));
            self.save_as.setFrame(rect(width - 110.0, 9.0, 100.0, 42.0));
        }
        let active = (!snapshot.home).then_some(snapshot.active).flatten();
        for (index, (row, doc)) in self.rows.iter().zip(&snapshot.documents).enumerate() {
            let name: String = doc.name.chars().take(60).collect();
            let title = format!("{}{}{}", if active == Some(doc.id) { "● " } else { "" }, name, if doc.dirty { " •" } else { "" });
            row.select.setTitle(&NSString::from_str(&title));
            row.select.setToolTip(Some(&NSString::from_str(&doc.name)));
            row.select.setFrame(rect(index as f64 * 220.0, 9.0, 180.0, 42.0));
            row.close.setFrame(rect(index as f64 * 220.0 + 182.0, 9.0, 30.0, 42.0));
            row.select.setEnabled(!snapshot.busy);
            row.close.setEnabled(!snapshot.busy);
            if active == Some(row.uid) && (active != self.active || ids_changed) {
                self.content.scrollRectToVisible(rect(index as f64 * 220.0, 0.0, 220.0, 58.0));
            }
        }
        self.active = active;
        let clip = self.scroll.contentView();
        self.back.setEnabled(!snapshot.busy && clip.bounds().origin.x > 0.5);
        self.forward.setEnabled(!snapshot.busy && clip.bounds().origin.x + viewport < self.content.frame().size.width - 0.5);
        self.save.setEnabled(active.is_some() && !snapshot.busy);
        self.save_as.setEnabled(active.is_some() && !snapshot.busy);
        if self.question.is_none()
            && let Some(uid) = snapshot.close_question
            && let Some(doc) = snapshot.documents.iter().find(|d| d.id == uid)
        {
            self.question = Some(Question {
                uid,
                future: Box::pin(
                    self.alert
                        .clone()
                        .set_title("保存修改？")
                        .set_description(format!("是否保存对“{}”的修改？", doc.name))
                        .set_level(rfd::MessageLevel::Warning)
                        .set_buttons(rfd::MessageButtons::YesNoCancelCustom("保存".into(), "不保存".into(), "取消".into()))
                        .show(),
                ),
            });
        }
        // Do not reset the document view frame or scroll position every tick: AppKit's
        // user-driven horizontal scroll must survive stable presentation updates.
        if geometry_changed {
            let clip = self.scroll.contentView();
            let max_x = (self.content.frame().size.width - viewport).max(0.0);
            clip.scrollToPoint(NSPoint::new(clip.bounds().origin.x.clamp(0.0, max_x), 0.0));
            self.scroll.reflectScrolledClipView(&clip);
        }
    }
}
