//! Native home composition. Agent entry points stay disabled until the service is wired.
use super::{label, panel, rect, style};
use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained};
use objc2_app_kit::{NSBox, NSImageView, NSTextField};
pub(super) struct Hero {
    illustration: Retained<NSImageView>,
    composer: Retained<NSBox>,
    hint: Retained<NSTextField>,
    attach: style::Button,
    start: style::Button,
    shortcuts: Vec<(Retained<NSBox>, Retained<NSImageView>, Retained<NSTextField>)>,
}
impl Hero {
    pub fn install(mtm: MainThreadMarker, home: &NSBox) -> Result<Self, String> {
        let parent = home.contentView().ok_or("missing home content")?;
        let illustration = style::picture(mtm, &parent, include_bytes!("../../../../assets/kodma/home-illustration.png"));
        let composer = panel(mtm, 0xffffff);
        composer.setCornerRadius(20.);
        composer.setBorderWidth(1.);
        composer.setBorderColor(&super::color(0xd4d0cc));
        parent.addSubview(&composer);
        let body = composer.contentView().ok_or("missing composer content")?;
        let hint = label(mtm, "描述你的想法…", 18., 0x9c9690);
        body.addSubview(&hint);
        let attach = style::Button::new(mtm, &body, "", 0xffffff, None, 0.);
        attach.icon("attach", 0x736e69);
        attach.enabled(false);
        let start = style::Button::new(mtm, &body, "开始创作", 0x121212, None, 22.);
        start.control.setContentTintColor(Some(&super::color(0xffffff)));
        start.enabled(false);
        start.control.setToolTip(Some(&objc2_foundation::NSString::from_str("Agent 服务尚未连接")));
        let mut shortcuts = Vec::new();
        for (title, icon, color) in [("做一个 App", "app", 0xaadaff), ("改进页面", "copy", 0xffb3a7), ("梳理流程", "flow", 0xb9edca)] {
            let plate = panel(mtm, color);
            plate.setCornerRadius(10.);
            parent.addSubview(&plate);
            let image = NSImageView::initWithFrame(NSImageView::alloc(mtm), rect(0., 0., 20., 20.));
            if let Some(icon) = style::icon(icon, 0x121212, 20.) {
                image.setImage(Some(&icon));
            }
            parent.addSubview(&image);
            let text = label(mtm, title, 18., 0x121212);
            parent.addSubview(&text);
            shortcuts.push((plate, image, text));
        }
        Ok(Self { illustration, composer, hint, attach, start, shortcuts })
    }
    pub fn layout(&self, width: f64, height: f64) {
        let available = (width - 156.).max(300.);
        let composer_width = available.min(920.);
        self.illustration.setFrame(rect((width - 506.).max(650.), height - 395., 416., 330.));
        self.composer.setFrame(rect(78., height - 604., composer_width, 172.));
        self.hint.setFrame(rect(24., 120., composer_width - 48., 30.));
        self.attach.frame(16., 20., 36., 44.);
        self.start.frame(composer_width - 162., 20., 138., 44.);
        for (index, (plate, image, text)) in self.shortcuts.iter().enumerate() {
            let x = 82. + index as f64 * 278.;
            let y = height - 726.;
            plate.setFrame(rect(x, y + 6., 36., 36.));
            image.setFrame(rect(x + 8., y + 14., 20., 20.));
            text.setFrame(rect(x + 50., y + 8., 200., 30.));
        }
    }
}
