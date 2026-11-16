//! Standard editing keys route synchronously by the actual AppKit responder, not a frame-old
//! focus flag. Canvas commands retain their originating document and use the original runtime.
use figcraft_ui_egui::workspace::Action;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSResponder, NSTextView};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

const HELPER: isize = 0x4b4f444d;
const EDITS: [(&str, &str, &str, isize); 6] = [
    ("edit.undo", "Undo", "Cmd+Z", 0),
    ("edit.redo", "Redo", "Cmd+Shift+Z", 1),
    ("edit.cut", "Cut", "Cmd+X", 3),
    ("edit.copy", "Copy", "Cmd+C", 4),
    ("edit.paste", "Paste", "Cmd+V", 5),
    ("select.all", "Select All", "Cmd+A", 6),
];
#[derive(Clone, Copy)]
struct Owned {
    uid: u64,
    id: &'static str,
}
pub(super) struct Menu {
    driver: craft_menu::NativeMenu,
    _helper: crate::native_canvas::NativeEditMenu,
    owner: Arc<AtomicU64>,
    queue: Arc<Mutex<VecDeque<Owned>>>,
}
impl Menu {
    pub fn install(ctx: &egui::Context, window_number: isize) -> Result<Self, String> {
        let mut children = Vec::new();
        for (id, label, key, _) in EDITS {
            let chord = craft_menu::Chord::parse(key).map_err(|e| e.to_string())?;
            let mut item = craft_menu::Item::new(id, label).shortcut(chord);
            // A layout shortcut is dispatched synchronously through on_command, including while a
            // field is active. The callback decides from AppKit's current first responder.
            item.layout_shortcut = true;
            children.push(craft_menu::Node::Item(item));
        }
        let bar = craft_menu::MenuBar { menus: vec![craft_menu::Menu::new("Edit", children).role(craft_menu::MenuRole::Edit)], ..Default::default() };
        let layout = craft_menu::mac_layout(&bar, &craft_menu::LayoutOptions::new("Kodma"));
        let mut driver = craft_menu::mac::install(ctx, &layout.bar).ok_or("could not install workspace edit menu")?;
        let helper = crate::native_canvas::NativeEditMenu::install()?;
        helper.dispatch_helper(HELPER);
        let owner = Arc::new(AtomicU64::new(0));
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let source = owner.clone();
        let output = queue.clone();
        driver.on_command(Box::new(move |command| {
            let Some(&(id, _, _, index)) = EDITS.iter().find(|(id, _, _, _)| *id == command.id) else { return false };
            let Some(mtm) = MainThreadMarker::new() else { return true };
            let app = NSApplication::sharedApplication(mtm);
            let Some(window) = app.keyWindow() else { return true };
            let responder = window.firstResponder();
            if responder.as_ref().is_some_and(|r| r.downcast_ref::<NSTextView>().is_some()) {
                if let Some(menu) = app.mainMenu().and_then(|m| m.itemWithTag(HELPER)).and_then(|i| i.submenu()) {
                    menu.performActionForItemAtIndex(index);
                }
            } else if window.windowNumber() == window_number
                && responder.as_ref().zip(window.contentView().as_ref()).is_some_and(|(r, v)| std::ptr::eq::<NSResponder>(&**r, &***v))
            {
                let uid = source.load(Ordering::Relaxed);
                let mut pending = output.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                if uid != 0 && pending.len() < 64 {
                    pending.push_back(Owned { uid, id });
                }
            }
            true
        }));
        Ok(Self { driver, _helper: helper, owner, queue })
    }
    pub fn owner(&self, uid: Option<u64>) {
        self.owner.store(uid.unwrap_or(0), Ordering::Relaxed);
    }
    pub fn input(&mut self, raw: &mut egui::RawInput) {
        self.driver.raw_input(raw);
    }
    pub fn actions(&mut self, ctx: &egui::Context) -> Vec<Action> {
        // Preserve the shared menu's built-in Quit/Minimize handling and its original close guard.
        let _ = self.driver.take_clicks(ctx);
        self.queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).drain(..).map(|o| Action::OwnedCommand(o.uid, o.id)).collect()
    }
}
impl Drop for Menu {
    fn drop(&mut self) {
        self.owner(None);
        self.queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
    }
}
