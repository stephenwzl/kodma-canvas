//! Native page/layer rows. Only visible rows own AppKit controls; disclosure is UI state.
use super::{color, rect, row_pool::RowPool, style};
use figcraft_ui_egui::workspace::{Action, Snapshot};
use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained};
use objc2_app_kit::{NSFont, NSScrollView, NSView};
use objc2_foundation::NSPoint;
use std::collections::HashSet;
#[derive(Clone, PartialEq)]
struct Item {
    id: u64,
    name: String,
    icon: &'static str,
    depth: usize,
    branch: bool,
    open: bool,
    selected: bool,
    enabled: bool,
}
struct Row {
    owner: Option<(u64, u64)>,
    item: Item,
    button: style::Button,
    disclosure: style::Button,
}
impl Drop for Row {
    fn drop(&mut self) {
        self.button.view.removeFromSuperview();
        self.disclosure.view.removeFromSuperview();
    }
}
struct List {
    scroll: Retained<NSScrollView>,
    content: Retained<NSView>,
    rows: RowPool<Row>,
    data: Vec<Item>,
    range: (usize, usize),
    geometry: Option<(f64, f64, f64)>,
    owner: Option<(u64, u64)>,
}
impl List {
    fn new(mtm: MainThreadMarker, parent: &NSView) -> Self {
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), rect(0., 0., 190., 44.));
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDrawsBackground(false);
        let content = NSView::initWithFrame(NSView::alloc(mtm), rect(0., 0., 190., 44.));
        scroll.setDocumentView(Some(&content));
        parent.addSubview(&scroll);
        Self { scroll, content, rows: RowPool::new(), data: Vec::new(), range: (0, 0), geometry: None, owner: None }
    }
    fn sync(&mut self, data: Vec<Item>, y: f64, height: f64, stride: f64, owner: Option<(u64, u64)>, busy: bool) {
        let reset = self.owner != owner;
        self.owner = owner;
        let height = height.max(1.);
        let total = (data.len() as f64 * stride).max(height);
        let changed = self.data != data;
        let geometry = self.geometry != Some((y, height, total));
        if geometry {
            self.scroll.setFrame(rect(26., y, 190., height));
            self.content.setFrame(rect(0., 0., 190., total));
            self.geometry = Some((y, height, total));
        }
        let clip = self.scroll.contentView();
        if reset || geometry {
            clip.scrollToPoint(NSPoint::new(0., if reset { total - height } else { clip.bounds().origin.y.clamp(0., total - height) }));
            self.scroll.reflectScrolledClipView(&clip);
        }
        let start = ((total - clip.bounds().origin.y - height).max(0.) / stride).floor() as usize;
        let end = (start + (height / stride).ceil() as usize + 1).min(data.len());
        if reset || changed || geometry || self.range != (start, end) {
            if let Some(mtm) = MainThreadMarker::new() {
                self.rows.rebind(
                    data.iter().enumerate().take(end).skip(start),
                    |(_, item)| {
                        let button = style::Button::new(mtm, &self.content, &item.name, if item.selected { 0xeceae6 } else { 0xf7f5f2 }, None, 7.);
                        button.leading();
                        button.icon(item.icon, 0x736e69);
                        button.control.setFont(Some(&NSFont::systemFontOfSize(if stride == 44. { 16. } else { 15. })));
                        button.control.setContentTintColor(Some(&color(0x242220)));
                        let disclosure = style::Button::new(mtm, &self.content, "", if item.selected { 0xeceae6 } else { 0xf7f5f2 }, None, 0.);
                        disclosure.icon(if item.open { "chevronDown" } else { "chevron" }, 0x736e69);
                        Row { owner, item: (*item).clone(), button, disclosure }
                    },
                    |row, (i, item)| {
                        row.button.pressed();
                        row.disclosure.pressed();
                        let icon_changed = row.item.icon != item.icon || row.item.name.is_empty() != item.name.is_empty();
                        row.button.title(&item.name);
                        if icon_changed {
                            row.button.icon(item.icon, 0x736e69);
                        }
                        if row.item.open != item.open {
                            row.disclosure.icon(if item.open { "chevronDown" } else { "chevron" }, 0x736e69);
                        }
                        row.item = item.clone();
                        row.owner = owner;
                        let bg = if item.selected { 0xeceae6 } else { 0xf7f5f2 };
                        row.button.paint(bg);
                        row.disclosure.paint(bg);
                        row.button.view.setHidden(false);
                        let row_y = total - i as f64 * stride - if stride == 44. { 44. } else { 34. };
                        row.button.frame(0., row_y, 190., if stride == 44. { 44. } else { 34. });
                        let indent = if stride == 44. { 0. } else { item.depth.min(8) as f64 * 12. + 14. };
                        row.button.icon_offset(14. + indent);
                        row.button.control.setFrame(rect(
                            if stride == 44. { 49. } else { 34. + indent },
                            0.,
                            (142. - indent).max(1.),
                            if stride == 44. { 44. } else { 34. },
                        ));
                        row.disclosure.frame(4. + item.depth.min(8) as f64 * 12., row_y + 7., 20., 20.);
                        row.disclosure.icon_offset(2.);
                        row.disclosure.view.setHidden(!item.branch);
                    },
                    |row, (_, item)| (row.owner == owner && row.item.id == item.id) || (!row.button.tracking() && !row.disclosure.tracking()),
                );
                for row in self.rows.inactive() {
                    row.button.view.setHidden(true);
                    row.disclosure.view.setHidden(true);
                }
            }
            self.data = data;
            self.range = (start, end);
        }
        for row in self.rows.iter() {
            row.button.enabled(!busy && row.item.enabled);
            row.disclosure.enabled(!busy && row.item.enabled);
        }
    }
}
pub(super) struct Tree {
    pages: List,
    layers: List,
    owner: Option<(u64, u64)>,
    expanded: HashSet<(u64, u64, u64)>,
    selected: Vec<u64>,
}
impl Tree {
    pub fn new(mtm: MainThreadMarker, parent: &NSView) -> Self {
        Self { pages: List::new(mtm, parent), layers: List::new(mtm, parent), owner: None, expanded: HashSet::new(), selected: Vec::new() }
    }
    pub fn actions(&mut self) -> Vec<Action> {
        let Some((uid, page)) = self.owner else { return Vec::new() };
        let mut actions = Vec::new();
        for row in self.pages.rows.iter() {
            if row.button.pressed() {
                actions.push(Action::Page(uid, row.item.id));
            }
        }
        for row in self.layers.rows.iter() {
            if row.disclosure.pressed() {
                let key = (uid, page, row.item.id);
                if !self.expanded.remove(&key) && self.expanded.len() < 65536 {
                    self.expanded.insert(key);
                }
            }
            if row.button.pressed() {
                actions.push(Action::Layer(uid, page, row.item.id));
            }
        }
        actions
    }
    pub fn sync(&mut self, s: &Snapshot, h: f64) -> f64 {
        let owner = s.active.zip(s.page).filter(|_| !s.home);
        let reset = self.owner != owner;
        self.owner = owner;
        let open_files: HashSet<_> = s.documents.iter().map(|d| d.id).collect();
        let live: HashSet<_> = s.navigation.layers.iter().map(|n| n.id).collect();
        // Agent edits may delete expanded branches in a long-lived document. Do not retain
        // those disclosure keys indefinitely, or grow unbounded across many open files.
        self.expanded.retain(|(uid, page, node)| open_files.contains(uid) && (Some((*uid, *page)) != owner || live.contains(node)));
        let selected: Vec<_> = s.selection.iter().take(10000).map(|n| n.id).collect();
        if (reset || selected != self.selected)
            && let Some((uid, page)) = owner
        {
            let parents: std::collections::HashMap<_, _> = s.navigation.layers.iter().map(|n| (n.id, n.parent)).collect();
            for node in &selected {
                let mut current = *node;
                for _ in 0..128 {
                    let Some(parent) = parents.get(&current).copied() else { break };
                    if parent == page {
                        break;
                    }
                    if self.expanded.len() < 65536 {
                        self.expanded.insert((uid, page, parent));
                    }
                    current = parent;
                }
            }
        }
        self.selected = selected;
        let page_height = (s.navigation.pages.len().max(1) as f64 * 44.).min(112.);
        let divider = 191. + page_height;
        self.pages.scroll.setHidden(s.home);
        self.layers.scroll.setHidden(s.home);
        let pages = s
            .navigation
            .pages
            .iter()
            .map(|p| Item {
                id: p.id,
                name: p.name.clone(),
                icon: "copy",
                depth: 0,
                branch: false,
                open: false,
                selected: p.current && s.navigation.pages.len() > 1,
                enabled: !p.divider,
            })
            .collect();
        self.pages.sync(pages, h - 155. - page_height, page_height, 44., owner, s.busy);
        let mut hidden = None;
        let mut layers = Vec::new();
        if let Some((uid, page)) = owner {
            for n in &s.navigation.layers {
                if hidden.is_some_and(|depth| n.depth > depth) {
                    continue;
                }
                hidden = None;
                let open = self.expanded.contains(&(uid, page, n.id));
                if n.children && !open {
                    hidden = Some(n.depth);
                }
                layers.push(Item {
                    id: n.id,
                    name: n.name.clone(),
                    icon: match n.kind.as_str() {
                        "FRAME" | "GROUP" | "COMPONENT" | "INSTANCE" => "layerFrame",
                        "TEXT" => "layerText",
                        _ => "layerShape",
                    },
                    depth: n.depth,
                    branch: n.children,
                    open,
                    selected: n.selected,
                    enabled: true,
                });
            }
        }
        let top = divider + 62.;
        self.layers.sync(layers, 120., (h - top - 120.).max(1.), 38., owner, s.busy);
        divider
    }
}
