//! S002 native frame. All values are observations of the one existing FigApp/Session.
//! NSWindow and WinitView remain owned by eframe; no reparenting or replacement GPU layer.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSApplication, NSBox, NSBoxType, NSColor, NSFont, NSImageView, NSResponder, NSTextField,
    NSTitlePosition, NSView, NSWindow,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use figcraft_ui_egui::CanvasHost;
use figcraft_ui_egui::workspace::Snapshot;

mod edit_menu;
mod hero;
mod home;
mod inspector;
mod navigation;
mod navigation_tree;
mod style;
mod tabs;
mod toolbar;

const LEFT: f64 = 242.0;
const TOP: f64 = 84.0;
const RIGHT: f64 = 408.0;
// canvas/selection: editor overlay only, never an object paint or serialized property.
const CANVAS_SELECTION: egui::Color32 = egui::Color32::from_rgb(0x67, 0x38, 0xef);

pub struct Host {
    window: Retained<NSWindow>,
    canvas: Retained<NSView>,
    left: Retained<NSBox>,
    top: Retained<NSBox>,
    right: Retained<NSBox>,
    home: Retained<NSBox>,
    brand: Retained<NSImageView>,
    hero: hero::Hero,
    toolbar: toolbar::Toolbar,
    navigation: navigation::Navigation,
    drafts: Retained<NSTextField>,
    controls: home::Controls,
    edit_menu: Option<edit_menu::Menu>,
    tabs: tabs::Tabs,
    design: Retained<NSTextField>,
    properties: inspector::Properties,
    greeting: Retained<NSTextField>,
    headline: Retained<NSTextField>,
    geometry: Option<(f64, f64, bool)>,
}

impl Host {
    pub fn install(title: &str, alert: rfd::AsyncMessageDialog) -> Result<Self, String> {
        let mtm = MainThreadMarker::new().ok_or("native workspace requires the main thread")?;
        let windows = NSApplication::sharedApplication(mtm).windows();
        let mut candidates = windows.iter().filter(|window| window.title().to_string() == title);
        let window = candidates.next().ok_or("could not find the workspace window")?;
        if candidates.next().is_some() {
            return Err("ambiguous workspace window".into());
        }
        let canvas = window.contentView().ok_or("workspace has no content view")?;
        let left = panel(mtm, 0xf7f5f2);
        let top = panel(mtm, 0xffffff);
        let right = panel(mtm, 0xffffff);
        let home = panel(mtm, 0xffffff);
        let navigation = navigation::Navigation::install(mtm, &left, &right, &top)?;
        let brand =
            style::picture(mtm, &*left.contentView().ok_or("missing navigation content")?, include_bytes!("../../../assets/kodma/wordmark.png"));
        let hero = hero::Hero::install(mtm, &home)?;
        let drafts = label(mtm, "稿件", 14.0, 0x736e69);
        let design = label(mtm, "设计", 16.0, 0x121212);
        let greeting = label(mtm, "今天，", 66.0, 0x736e69);
        let headline = label(mtm, "想把什么画出来？", 68.0, 0x121212);
        add_labels(&left, &[&drafts])?;
        add_labels(&right, &[&design])?;
        let properties = inspector::Properties::install(mtm, &right, &window, &canvas)?;
        add_labels(&home, &[&greeting, &headline])?;
        let controls = home::Controls::install(mtm, &left, &top, &home)?;
        let tabs = tabs::Tabs::install(mtm, &top, alert)?;
        // Add as siblings of the original rendering view's contents, never its replacement.
        for view in [&*left, &*top, &*right, &*home] {
            canvas.addSubview(view);
        }
        let toolbar = toolbar::Toolbar::install(mtm, &canvas)?;
        window.setAutorecalculatesKeyViewLoop(true);
        window.recalculateKeyViewLoop();
        window.makeFirstResponder(Some(&canvas));
        log::info!("native_workspace installed in NSWindow {}, canvas_flipped={}", window.windowNumber(), canvas.isFlipped());
        Ok(Self {
            window,
            canvas,
            left,
            top,
            right,
            home,
            brand,
            hero,
            toolbar,
            navigation,
            drafts,
            controls,
            edit_menu: None,
            tabs,
            design,
            properties,
            greeting,
            headline,
            geometry: None,
        })
    }

    pub fn install_edit_menu(&mut self, ctx: &egui::Context) -> Result<(), String> {
        self.edit_menu = Some(edit_menu::Menu::install(ctx, self.window.windowNumber())?);
        Ok(())
    }

    pub fn editing_properties(&self) -> bool {
        self.properties.editing()
    }

    pub fn take_actions(
        &mut self,
        ctx: &egui::Context,
        inspector: &figcraft_ui_egui::workspace_properties::Inspector,
    ) -> Vec<figcraft_ui_egui::workspace::Action> {
        let mut actions = self.edit_menu.as_mut().map(|m| m.actions(ctx)).unwrap_or_default();
        actions.extend(self.toolbar.actions());
        actions.extend(self.tabs.take_actions());
        actions.extend(self.controls.take_actions());
        actions.extend(self.navigation.actions());
        let navigation = !actions.is_empty();
        if let Some(commit) = self.properties.take_commit(inspector, navigation) {
            actions.push(figcraft_ui_egui::workspace::Action::Property(commit));
        }
        if navigation {
            self.window.makeFirstResponder(Some(&self.canvas));
        }
        actions
    }

    pub fn raw_input(&mut self, raw: &mut egui::RawInput) {
        if let Some(menu) = &mut self.edit_menu {
            menu.input(raw);
        }
    }

    /// Main-thread presentation only; field drafts dispatch through the original runtime.
    pub fn sync(
        &mut self,
        snapshot: Snapshot,
        recents: Vec<figcraft_ui_egui::workspace::RecentFile>,
        error: Option<String>,
        inspector: figcraft_ui_egui::workspace_properties::Inspector,
    ) -> CanvasHost {
        if let Some(menu) = &self.edit_menu {
            menu.owner(if snapshot.home || snapshot.busy { None } else { snapshot.active });
        }
        let bounds = self.canvas.bounds();
        let w = bounds.size.width;
        let h = bounds.size.height;
        // Size parents before child scroll views: AppKit otherwise clamps their initial
        // top offset against a zero-sized viewport and opens the inspector at its bottom.
        if self.geometry != Some((w, h, snapshot.home)) {
            self.geometry = Some((w, h, snapshot.home));
            self.left.setFrame(rect(0.0, 0.0, LEFT, h));
            // WinitView is flipped on this backend; native panel content views are not.
            // Convert only the panel origins, preserving AppKit's native text coordinates.
            let (top_y, body_y) = panel_origins(self.canvas.isFlipped(), h);
            self.top.setFrame(rect(LEFT, top_y, (w - LEFT).max(0.0), TOP));
            self.right.setFrame(rect(w - RIGHT, body_y, RIGHT, (h - TOP).max(0.0)));
            self.home.setFrame(rect(LEFT, body_y, (w - LEFT).max(0.0), (h - TOP).max(0.0)));
            self.right.setHidden(snapshot.home);
            self.home.setHidden(!snapshot.home);
            self.brand.setFrame(rect(36.0, h - 77.0, 140.0, 33.0));
            self.drafts.setFrame(rect(36.0, h - 160.0, 180.0, 24.0));
            self.drafts.setHidden(true);
            self.design.setFrame(rect(40.0, h - TOP - 58.0, 65.0, 30.0));
            self.greeting.setFrame(rect(78.0, h - 314.0, 500.0, 100.0));
            self.headline.setFrame(rect(78.0, h - 399.0, 650.0, 90.0));
        }
        self.controls.sync(
            recents,
            error,
            snapshot.busy,
            snapshot.home,
            snapshot.documents.iter().find(|d| Some(d.id) == snapshot.active).map(|d| d.name.as_str()),
            w,
            h,
        );
        self.navigation.sync(&snapshot, w, h);
        self.hero.layout(w - LEFT, h - TOP);
        self.toolbar.sync(&snapshot, w, h, self.canvas.isFlipped());
        self.tabs.sync(&snapshot, (w - LEFT).max(0.0));
        self.properties.sync(&inspector, snapshot.busy, (h - TOP).max(0.0), &self.window);
        let busy = snapshot.busy;
        CanvasHost {
            accent: Some(CANVAS_SELECTION),
            left_inset: LEFT as f32,
            top_inset: TOP as f32,
            right_inset: RIGHT as f32,
            respect_home: true,
            keyboard: canvas_keyboard(
                self.window.isKeyWindow() && !busy,
                self.window.firstResponder().is_some_and(|r| std::ptr::eq::<NSResponder>(&*r, &**self.canvas)),
            ),
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.window.makeFirstResponder(Some(&self.canvas));
        for panel in [&self.left, &self.top, &self.right, &self.home] {
            panel.removeFromSuperview();
        }
        self.toolbar.view.removeFromSuperview();
        log::info!("native_workspace detached from NSWindow {}", self.window.windowNumber());
    }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

fn color(rgb: u32) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(
        f64::from((rgb >> 16) & 255) / 255.0,
        f64::from((rgb >> 8) & 255) / 255.0,
        f64::from(rgb & 255) / 255.0,
        1.0,
    )
}

fn panel(mtm: MainThreadMarker, rgb: u32) -> Retained<NSBox> {
    let panel = NSBox::initWithFrame(NSBox::alloc(mtm), rect(0.0, 0.0, 1.0, 1.0));
    panel.setBoxType(NSBoxType::Custom);
    // Workspace tokens are currently light; native controls must share their contrast.
    panel.setAppearance(NSAppearance::appearanceNamed(&NSString::from_str("NSAppearanceNameAqua")).as_deref());
    panel.setTitlePosition(NSTitlePosition::NoTitle);
    panel.setBorderWidth(0.0);
    panel.setContentViewMargins(NSSize::new(0.0, 0.0));
    panel.setFillColor(&color(rgb));
    panel
}

fn label(mtm: MainThreadMarker, text: &str, size: f64, rgb: u32) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    label.setFont(Some(&NSFont::systemFontOfSize(size)));
    label.setTextColor(Some(&color(rgb)));
    label.setSelectable(false);
    label.setMaximumNumberOfLines(0);
    label
}

fn add_labels(panel: &NSBox, labels: &[&NSTextField]) -> Result<(), String> {
    // NSBox owns a content view. Adding labels directly to the box can replace that view,
    // making the last label's frame the container frame and moving text outside its region.
    let content = panel.contentView().ok_or("native panel has no content view")?;
    for label in labels {
        content.addSubview(label);
    }
    Ok(())
}

fn panel_origins(flipped: bool, height: f64) -> (f64, f64) {
    if flipped { (0.0, TOP) } else { (height - TOP, 0.0) }
}

fn canvas_keyboard(key_window: bool, canvas_responder: bool) -> bool {
    key_window && canvas_responder
}

#[cfg(test)]
mod tests {
    use super::{canvas_keyboard, panel_origins};
    #[test]
    fn a_picker_key_window_keeps_keys_out_of_the_underlying_canvas() {
        assert!(!canvas_keyboard(false, true));
        assert!(!canvas_keyboard(true, false));
        assert!(canvas_keyboard(true, true));
    }
    #[test]
    fn native_panels_use_the_actual_parent_coordinate_direction() {
        assert_eq!(panel_origins(true, 960.0), (0.0, 84.0));
        assert_eq!(panel_origins(false, 960.0), (876.0, 0.0));
        assert_eq!(panel_origins(true, 600.0), (0.0, 84.0));
    }
}
