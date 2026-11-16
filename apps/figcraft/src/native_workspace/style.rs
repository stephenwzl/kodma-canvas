//! Kodma's native visual tokens and original primitive icons. No OS or product icon set.
use super::{color, panel, rect};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, rc::Retained};
use objc2_app_kit::{NSBox, NSButton, NSButtonType, NSCellImagePosition, NSFont, NSImage, NSImageView, NSLineBreakMode, NSTextAlignment, NSView};
use objc2_foundation::{NSData, NSSize, NSString};
use std::{cell::RefCell, io::Cursor};

pub(super) struct Button {
    pub view: Retained<NSBox>,
    pub control: Retained<NSButton>,
    hit: Retained<NSButton>,
    icon: RefCell<Option<Retained<NSImageView>>>,
}
impl Button {
    pub fn new(mtm: MainThreadMarker, parent: &NSView, title: &str, bg: u32, border: Option<u32>, radius: f64) -> Self {
        let view = panel(mtm, bg);
        view.setCornerRadius(radius);
        if let Some(border) = border {
            view.setBorderWidth(1.0);
            view.setBorderColor(&color(border));
        }
        let control = NSButton::initWithFrame(NSButton::alloc(mtm), rect(0.0, 0.0, 1.0, 1.0));
        control.setTitle(&NSString::from_str(title));
        control.setFont(Some(&NSFont::systemFontOfSize(15.0)));
        if let Some(cell) = control.cell() {
            cell.setWraps(false);
            cell.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        }
        control.setButtonType(NSButtonType::PushOnPushOff);
        control.setBordered(false);
        control.setContentTintColor(Some(&color(0x121212)));
        if let Some(content) = view.contentView() {
            content.addSubview(&control);
        }
        // Paint and icon views must not leave dead hit regions in a rounded button.
        let hit = NSButton::initWithFrame(NSButton::alloc(mtm), rect(0., 0., 1., 1.));
        hit.setTitle(&NSString::new());
        hit.setBordered(false);
        hit.setTransparent(true);
        hit.setButtonType(NSButtonType::PushOnPushOff);
        if let Some(content) = view.contentView() {
            content.addSubview(&hit);
        }
        parent.addSubview(&view);
        Self { view, control, hit, icon: RefCell::new(None) }
    }
    pub fn frame(&self, x: f64, y: f64, w: f64, h: f64) {
        self.view.setFrame(rect(x, y, w, h));
        self.hit.setFrame(rect(0., 0., w, h));
        let has_icon = self.icon.borrow().as_ref().is_some_and(|v| !v.isHidden()) && !self.control.title().to_string().is_empty();
        let inset = if has_icon { 40. } else { 8. };
        self.control.setFrame(rect(inset, 0.0, (w - inset - 8.).max(0.0), h));
        if let Some(icon) = self.icon.borrow().as_ref() {
            icon.setFrame(rect(14., (h - 16.) / 2., 16., 16.));
        }
    }
    pub fn leading(&self) {
        self.control.setAlignment(NSTextAlignment::Left);
    }
    pub fn title(&self, text: &str) {
        if self.control.title().to_string() != text {
            self.control.setTitle(&NSString::from_str(text));
            let f = self.view.frame();
            self.frame(f.origin.x, f.origin.y, f.size.width, f.size.height);
        }
    }
    pub fn paint(&self, bg: u32) {
        self.view.setFillColor(&color(bg));
    }
    pub fn show_icon(&self, visible: bool) {
        if let Some(icon) = self.icon.borrow().as_ref() {
            icon.setHidden(!visible);
        }
    }
    pub fn icon_offset(&self, x: f64) {
        if let Some(icon) = self.icon.borrow().as_ref() {
            let mut frame = icon.frame();
            frame.origin.x = x;
            icon.setFrame(frame);
        }
    }
    pub fn enabled(&self, value: bool) {
        self.control.setEnabled(value);
        self.hit.setEnabled(value);
    }
    pub fn pressed(&self) -> bool {
        let pressed = self.control.state() != 0 || self.hit.state() != 0;
        self.control.setState(0);
        self.hit.setState(0);
        pressed && self.control.isEnabled()
    }
    pub fn icon(&self, name: &str, rgb: u32) {
        if let (Some(image), Some(mtm)) = (icon(name, rgb, 16.0), MainThreadMarker::new()) {
            if self.control.title().to_string().is_empty() {
                self.control.setImage(Some(&image));
                self.control.setImagePosition(NSCellImagePosition::ImageOnly);
                return;
            }
            if let Some(view) = self.icon.borrow().as_ref() {
                view.setImage(Some(&image));
                return;
            }
            let view = NSImageView::initWithFrame(NSImageView::alloc(mtm), rect(14., 12., 16., 16.));
            view.setImage(Some(&image));
            if let Some(content) = self.view.contentView() {
                content.addSubview(&view);
                content.addSubview(&self.hit);
            }
            *self.icon.borrow_mut() = Some(view);
        }
    }
}
pub(super) fn picture(mtm: MainThreadMarker, parent: &NSView, bytes: &[u8]) -> Retained<NSImageView> {
    let view = NSImageView::initWithFrame(NSImageView::alloc(mtm), rect(0.0, 0.0, 1.0, 1.0));
    let data = NSData::with_bytes(bytes);
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        view.setImage(Some(&image));
    }
    parent.addSubview(&view);
    view
}
/// Original 16-unit paths, rasterised at 3x for AppKit. Lines use round caps.
pub(super) fn icon(name: &str, rgb: u32, size: f64) -> Option<Retained<NSImage>> {
    let mut lines: Vec<([f64; 2], [f64; 2])> = Vec::new();
    let mut path = |points: &[[f64; 2]]| {
        for pair in points.windows(2) {
            lines.push((pair[0], pair[1]));
        }
    };
    match name {
        "chevron" => path(&[[6., 4.], [10., 8.], [6., 12.]]),
        "chevronDown" => path(&[[4., 6.], [8., 10.], [12., 6.]]),
        "layerFrame" => {
            path(&[[5., 1.], [3., 15.]]);
            path(&[[12., 1.], [10., 15.]]);
            path(&[[1., 5.], [15., 5.]]);
            path(&[[1., 11.], [15., 11.]]);
        }
        "layerText" => {
            path(&[[3., 3.], [13., 3.]]);
            path(&[[8., 3.], [8., 14.]]);
            path(&[[5., 14.], [11., 14.]]);
        }
        "layerShape" => path(&[[3., 3.], [13., 3.], [13., 13.], [3., 13.], [3., 3.]]),
        "folder" => path(&[[1.5, 4.5], [6.0, 4.0], [7.0, 2.8], [14.2, 3.5], [13.5, 13.0], [2.5, 14.0], [1.5, 4.5]]),
        "plus" => {
            path(&[[8.0, 2.0], [8.0, 14.0]]);
            path(&[[2.0, 8.0], [14.0, 8.0]]);
        }
        "search" => {
            for i in 0..24 {
                let a = i as f64 * std::f64::consts::TAU / 24.0;
                let b = (i + 1) as f64 * std::f64::consts::TAU / 24.0;
                path(&[[6.5 + 4.3 * a.cos(), 6.5 + 4.3 * a.sin()], [6.5 + 4.3 * b.cos(), 6.5 + 4.3 * b.sin()]]);
            }
            path(&[[10.0, 10.0], [14.0, 14.0]]);
        }
        "home" => {
            path(&[[1.5, 7.0], [8.0, 2.0], [14.5, 7.0]]);
            path(&[[3.5, 6.0], [3.5, 14.0], [12.5, 14.0], [12.5, 6.0]]);
        }
        "close" => {
            path(&[[4.0, 4.0], [12.0, 12.0]]);
            path(&[[12.0, 4.0], [4.0, 12.0]]);
        }
        "grid" => {
            for (x, y) in [(2., 2.), (9., 2.), (2., 9.), (9., 9.)] {
                path(&[[x, y], [x + 5., y], [x + 5., y + 5.], [x, y + 5.], [x, y]]);
            }
        }
        "settings" => {
            for i in 0..16 {
                let a = i as f64 * std::f64::consts::TAU / 16.;
                let b = (i + 1) as f64 * std::f64::consts::TAU / 16.;
                let r = if i % 2 == 0 { 7. } else { 5.7 };
                let q = if (i + 1) % 2 == 0 { 7. } else { 5.7 };
                path(&[[8. + r * a.cos(), 8. + r * a.sin()], [8. + q * b.cos(), 8. + q * b.sin()]]);
            }
            for i in 0..20 {
                let a = i as f64 * std::f64::consts::TAU / 20.;
                let b = (i + 1) as f64 * std::f64::consts::TAU / 20.;
                path(&[[8. + 2.4 * a.cos(), 8. + 2.4 * a.sin()], [8. + 2.4 * b.cos(), 8. + 2.4 * b.sin()]]);
            }
        }
        "attach" => path(&[
            [6., 10.],
            [11., 5.],
            [12., 5.],
            [12., 7.],
            [6., 13.],
            [3., 13.],
            [2., 10.],
            [3., 7.],
            [9., 1.],
            [12., 1.],
            [14., 4.],
            [14., 7.],
            [8., 14.],
        ]),
        "spark" => path(&[[8., 1.], [10., 6.], [15., 8.], [10., 10.], [8., 15.], [6., 10.], [1., 8.], [6., 6.], [8., 1.]]),
        "copy" => {
            path(&[[5., 2.], [14., 2.], [14., 11.], [11., 11.]]);
            path(&[[2., 5.], [11., 5.], [11., 14.], [2., 14.], [2., 5.]]);
        }
        "flow" => {
            path(&[[8., 3.], [8., 8.], [3., 8.], [3., 12.]]);
            path(&[[8., 8.], [13., 8.], [13., 12.]]);
            for (x, y) in [(6., 1.), (1., 12.), (11., 12.)] {
                path(&[[x, y], [x + 4., y], [x + 4., y + 3.], [x, y + 3.], [x, y]]);
            }
        }
        "app" => {
            path(&[[4., 2.], [12., 3.], [10., 14.], [2., 13.], [4., 2.]]);
            path(&[[12., 0.5], [14.5, 2.], [12., 5.]]);
        }
        "save" => {
            path(&[[2., 1.], [12., 1.], [14., 3.], [14., 15.], [2., 15.], [2., 1.]]);
            path(&[[5., 1.], [5., 6.], [11., 6.], [11., 1.]]);
            path(&[[5., 15.], [5., 10.], [11., 10.], [11., 15.]]);
        }
        "pointer" => path(&[[3., 1.], [13., 9.], [9., 10.], [7., 15.], [3., 1.]]),
        "hand" => path(&[
            [3., 8.],
            [3., 5.],
            [5., 7.],
            [5., 2.],
            [7., 2.],
            [7., 6.],
            [7., 1.],
            [9., 1.],
            [9., 6.],
            [9., 2.],
            [11., 2.],
            [11., 7.],
            [11., 4.],
            [13., 4.],
            [13., 10.],
            [11., 14.],
            [6., 14.],
            [3., 8.],
        ]),
        "undo" => {
            path(&[[5., 3.], [2., 6.], [5., 9.]]);
            path(&[[2., 6.], [10., 6.], [13., 8.], [13., 11.], [11., 13.]]);
        }
        "redo" => {
            path(&[[11., 3.], [14., 6.], [11., 9.]]);
            path(&[[14., 6.], [6., 6.], [3., 8.], [3., 11.], [5., 13.]]);
        }
        "play" => path(&[[4., 2.], [13., 8.], [4., 14.], [4., 2.]]),
        "rect" => path(&[[2., 2.], [14., 2.], [14., 14.], [2., 14.], [2., 2.]]),
        "arrow" => {
            path(&[[2., 8.], [14., 8.]]);
            path(&[[10., 4.], [14., 8.], [10., 12.]]);
        }
        _ => path(&[[2., 8.], [14., 8.]]),
    }
    let extent = (size * 3.0).round().clamp(1.0, 192.0) as u32;
    let mut raster = image::RgbaImage::new(extent, extent);
    for y in 0..extent {
        for x in 0..extent {
            let px = (f64::from(x) + 0.5) * 16. / f64::from(extent);
            let py = (f64::from(y) + 0.5) * 16. / f64::from(extent);
            let mut coverage = 0.0_f64;
            for (a, b) in &lines {
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let d = dx * dx + dy * dy;
                let t = if d > 0.0 { ((px - a[0]) * dx + (py - a[1]) * dy) / d } else { 0.0 };
                let t = t.clamp(0., 1.);
                let dist = ((px - a[0] - t * dx).powi(2) + (py - a[1] - t * dy).powi(2)).sqrt();
                coverage = coverage.max(((0.75 - dist) * 3.0 + 0.5).clamp(0., 1.));
            }
            raster.put_pixel(x, y, image::Rgba([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, (coverage * 255.) as u8]));
        }
    }
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(raster).write_to(&mut png, image::ImageFormat::Png).ok()?;
    let image = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(png.get_ref()))?;
    image.setSize(NSSize::new(size, size));
    Some(image)
}
