//! Local-workspace home composition; Agent entry points belong to their own milestone.
use super::{rect, style};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{NSBox, NSImageView};
pub(super) struct Hero {
    illustration: Retained<NSImageView>,
}
impl Hero {
    pub fn install(mtm: MainThreadMarker, home: &NSBox) -> Result<Self, String> {
        let parent = home.contentView().ok_or("missing home content")?;
        let illustration = style::picture(mtm, &parent, include_bytes!("../../../../assets/kodma/home-illustration.png"));
        Ok(Self { illustration })
    }
    pub fn layout(&self, width: f64, height: f64) {
        let scale = ((width - 120.) / 1040.).clamp(0.55, 1.);
        self.illustration.setFrame(rect(width - 60. - 416. * scale, height * 0.5 - 150. * scale, 416. * scale, 330. * scale));
    }
}
