//! Native page and account chrome; no fabricated account or collaboration state.
use super::{label, panel, rect, style};
use figcraft_ui_egui::workspace::Snapshot;
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{NSBox, NSTextField};
pub(super) struct Navigation {
    page_title: Retained<NSTextField>,
    tree: super::navigation_tree::Tree,
    layers_title: Retained<NSTextField>,
    add_page: style::Button,
    owner: Option<u64>,
    account: style::Button,
    rules: Vec<Retained<NSBox>>,
    design: Retained<NSBox>,
    agent: style::Button,
    share: style::Button,
    header_rule: Retained<NSBox>,
    top_rule: Retained<NSBox>,
    edge: Retained<NSBox>,
}
impl Navigation {
    pub fn install(mtm: MainThreadMarker, left: &NSBox, right: &NSBox, top: &NSBox) -> Result<Self, String> {
        let nav = left.contentView().ok_or("missing navigation")?;
        let inspector = right.contentView().ok_or("missing inspector")?;
        let tabs = top.contentView().ok_or("missing tabs")?;
        let page_title = label(mtm, "页面", 14., 0x736e69);
        nav.addSubview(&page_title);
        let tree = super::navigation_tree::Tree::new(mtm, &nav);
        let layers_title = label(mtm, "图层", 14., 0x736e69);
        nav.addSubview(&layers_title);
        let add_page = style::Button::new(mtm, &nav, "", 0xf7f5f2, None, 0.);
        add_page.icon("plus", 0x736e69);
        let account = style::Button::new(mtm, &nav, "我的账户", 0xf7f5f2, None, 12.);
        account.icon("grid", 0x736e69);
        account.leading();
        account.enabled(false);
        let mut rules = Vec::new();
        for _ in 0..2 {
            let rule = panel(mtm, 0xdfdcd8);
            nav.addSubview(&rule);
            rules.push(rule);
        }
        let design = panel(mtm, 0xf7f5f2);
        design.setCornerRadius(10.);
        inspector.addSubview(&design);
        // Host title is added after this plate during layout through its existing retained view.
        let agent = style::Button::new(mtm, &inspector, "Kodma Agent", 0xffffff, None, 10.);
        agent.icon("spark", 0x736e69);
        agent.enabled(false);
        let share = style::Button::new(mtm, &inspector, "分享", 0x121212, None, 18.);
        share.control.setContentTintColor(Some(&super::color(0xffffff)));
        share.enabled(false);
        let header_rule = panel(mtm, 0xdfdcd8);
        inspector.addSubview(&header_rule);
        let top_rule = panel(mtm, 0xdfdcd8);
        tabs.addSubview(&top_rule);
        let edge = panel(mtm, 0xdfdcd8);
        inspector.addSubview(&edge);
        Ok(Self { page_title, tree, layers_title, add_page, owner: None, account, rules, design, agent, share, header_rule, top_rule, edge })
    }
    pub fn actions(&mut self) -> Vec<figcraft_ui_egui::workspace::Action> {
        let mut actions = self.tree.actions();
        if self.add_page.pressed()
            && let Some(uid) = self.owner
        {
            actions.push(figcraft_ui_egui::workspace::Action::OwnedCommand(uid, "page.add"));
        }
        actions
    }
    pub fn sync(&mut self, s: &Snapshot, w: f64, h: f64) {
        self.owner = if s.home { None } else { s.active };
        let divider = self.tree.sync(s, h);
        self.page_title.setHidden(s.home);
        self.layers_title.setHidden(s.home);
        self.add_page.view.setHidden(s.home);
        self.add_page.enabled(!s.busy);
        self.account.view.setHidden(true);
        self.agent.view.setHidden(true);
        self.share.view.setHidden(true);
        self.page_title.setFrame(rect(36., h - 142., 140., 26.));
        self.add_page.frame(185., h - 148., 36., 36.);
        self.layers_title.setFrame(rect(36., h - divider - 48., 150., 26.));
        self.account.frame(26., 32., 160., 44.);
        for (index, rule) in self.rules.iter().enumerate() {
            rule.setHidden(s.home || index == 1);
            rule.setFrame(rect(26., if index == 0 { h - divider } else { 104. }, 190., 1.));
        }
        let body = h - 84.;
        self.edge.setFrame(rect(0., 0., 1., body));
        self.design.setFrame(rect(24., body - 60., 65., 40.));
        self.agent.frame(100., body - 60., 175., 40.);
        self.share.frame(322., body - 58., 62., 36.);
        self.header_rule.setFrame(rect(24., body - 80., 360., 1.));
        self.top_rule.setFrame(rect(42., 0., (w - 325.).max(0.), 1.));
    }
}
