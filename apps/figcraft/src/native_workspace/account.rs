//! Native Kodma account panel, independent of drafts and editor commands.
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{NSAccessibility, NSBox, NSLineBreakMode, NSTextField, NSView, NSWindow};
use objc2_foundation::NSString;

use crate::kodma_account::{Action, Controller, Problem, Status};

use super::{color, label, panel, rect, style};

pub(super) struct Account {
    controller: Controller,
    scrim: Retained<NSBox>,
    card: Retained<NSBox>,
    paper: Retained<NSBox>,
    title: Retained<NSTextField>,
    identity: Retained<NSTextField>,
    message: Retained<NSTextField>,
    close: style::Button,
    primary: style::Button,
    secondary: style::Button,
    last: Option<Status>,
    geometry: Option<(f64, f64)>,
    open: bool,
}

impl Account {
    pub fn install(mtm: MainThreadMarker, canvas: &NSView) -> Result<Self, String> {
        let scrim = panel(mtm, 0x231e1b);
        scrim.setFillColor(&color(0x231e1b).colorWithAlphaComponent(0.24));
        let view = scrim.contentView().ok_or("missing account scrim")?;
        let card = panel(mtm, 0xffffff);
        card.setCornerRadius(20.);
        card.setBorderWidth(1.);
        card.setBorderColor(&color(0xdfdcd8));
        let content = card.contentView().ok_or("missing account settings")?;
        let title = label(mtm, "Kodma 账号", 22., 0x121212);
        content.addSubview(&title);
        let close = style::Button::new(mtm, &content, "", 0xffffff, None, 8.);
        close.icon("close", 0x736e69);
        close.tooltip("关闭");
        close.control.setAccessibilityLabel(Some(&NSString::from_str("关闭")));
        close.control.setKeyEquivalent(&NSString::from_str("\u{1b}"));
        let paper = panel(mtm, 0xf7f5f2);
        paper.setCornerRadius(10.);
        content.addSubview(&paper);
        let identity = label(mtm, "", 15., 0x121212);
        identity.setSelectable(true);
        identity.setMaximumNumberOfLines(1);
        identity.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        paper.contentView().ok_or("missing account identity")?.addSubview(&identity);
        let message = label(mtm, "", 14., 0x736e69);
        content.addSubview(&message);
        let primary = style::Button::new(mtm, &content, "在浏览器登录", 0x121212, None, 10.);
        let secondary = style::Button::new(mtm, &content, "退出登录", 0xf7f5f2, None, 10.);
        view.addSubview(&card);
        scrim.setHidden(true);
        canvas.addSubview(&scrim);
        Ok(Self {
            controller: Controller::start(),
            scrim,
            card,
            paper,
            title,
            identity,
            message,
            close,
            primary,
            secondary,
            last: None,
            geometry: None,
            open: false,
        })
    }

    pub fn open(&mut self, window: &NSWindow) {
        self.open = true;
        self.scrim.setHidden(false);
        window.makeFirstResponder(None);
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn tick(&mut self, window: &NSWindow, canvas: &NSView) {
        self.controller.poll();
        if !self.open {
            return;
        }
        if self.close.pressed() {
            if self.controller.status() == &Status::Waiting {
                self.controller.perform(Action::Cancel);
            }
            self.open = false;
            self.scrim.setHidden(true);
            window.makeFirstResponder(Some(canvas));
            return;
        }
        let visible = self.last.as_ref().unwrap_or(self.controller.status());
        let (_, _, action, secondary) = presentation(visible);
        let offer_account_site = visible == &Status::RevocationUnconfirmed;
        if self.primary.pressed()
            && let Some((_, action)) = action
        {
            self.controller.perform(action);
        }
        if self.secondary.pressed() {
            if offer_account_site {
                let _ = webbrowser::open(crate::kodma_account::ISSUER);
            } else if secondary {
                self.controller.perform(Action::Logout);
            }
        }
    }

    pub fn layout(&mut self, width: f64, height: f64) {
        if !self.open {
            return;
        }
        let status = self.controller.status();
        let changed = self.last.as_ref() != Some(status);
        if !changed && self.geometry == Some((width, height)) {
            return;
        }
        self.geometry = Some((width, height));
        self.last = Some(status.clone());
        let (identity, message, primary, secondary) = presentation(status);
        let w = 480.;
        let h = 220.;
        self.scrim.setFrame(rect(0., 0., width, height));
        self.card.setFrame(rect((width - w) / 2., (height - h) / 2., w, h));
        self.title.setFrame(rect(28., h - 67., w - 98., 36.));
        self.close.frame(w - 58., h - 61., 34., 34.);
        self.paper.setFrame(rect(28., 96., w - 56., 48.));
        self.identity.setFrame(rect(12., 11., w - 80., 25.));
        self.paper.setHidden(identity.is_none());
        if let Some(identity) = identity {
            let identity = NSString::from_str(identity);
            self.identity.setStringValue(&identity);
            self.identity.setToolTip(Some(&identity));
        } else {
            self.identity.setStringValue(&NSString::new());
            self.identity.setToolTip(None);
        }
        self.message.setFrame(rect(28., if identity.is_some() { 63. } else { 95. }, w - 56., 29.));
        self.message.setStringValue(&NSString::from_str(message));
        self.message.setHidden(message.is_empty());
        self.primary.frame(w - 172., 21., 144., 36.);
        self.primary.view.setHidden(primary.is_none());
        self.primary.enabled(primary.is_some());
        if let Some((title, action)) = primary {
            self.primary.title(title);
            let quiet = matches!(action, Action::Logout | Action::Cancel);
            self.primary.paint(if quiet { 0xf7f5f2 } else { 0x121212 });
            self.primary.control.setContentTintColor(Some(&color(if quiet { 0x121212 } else { 0xffffff })));
        }
        self.secondary.frame(w - 304., 21., 120., 36.);
        self.secondary.view.setHidden(!secondary);
        self.secondary.enabled(secondary);
        self.secondary.title(if status == &Status::RevocationUnconfirmed { "账号网站" } else { "退出登录" });
    }
}

impl Drop for Account {
    fn drop(&mut self) {
        self.scrim.removeFromSuperview();
    }
}

type Presentation<'a> = (Option<&'a str>, &'static str, Option<(&'static str, Action)>, bool);

fn presentation(status: &Status) -> Presentation<'_> {
    match status {
        Status::Checking => (None, "正在验证会话…", None, false),
        Status::SignedOut => (None, "", Some(("在浏览器登录", Action::Login)), false),
        Status::Waiting => (None, "等待浏览器登录…", Some(("取消", Action::Cancel)), false),
        Status::SignedIn(identity) => (Some(identity), "", Some(("退出登录", Action::Logout)), false),
        Status::Offline(identity) => (Some(identity), "暂时无法验证会话", Some(("重试", Action::Retry)), true),
        Status::Failed(Problem::Login) => (None, "登录失败", Some(("重试", Action::Login)), false),
        Status::Failed(Problem::Timeout) => (None, "登录超时", Some(("重试", Action::Login)), false),
        Status::Failed(Problem::Keychain) => (None, "钥匙串暂不可用", Some(("重试", Action::Retry)), false),
        Status::Failed(Problem::Reauthenticate) => (None, "无法恢复会话", Some(("重新登录", Action::Login)), true),
        Status::SigningOut => (None, "正在退出…", None, false),
        Status::RevocationUnconfirmed => (None, "已在本机退出。可在账号网站撤销会话。", Some(("在浏览器登录", Action::Login)), true),
    }
}
