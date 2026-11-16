//! Native inspector projection and guarded commits into the original command runtime.
use crate::FigApp;
use figcraft_doc::{Document, Node, NodeId};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Name,
    X,
    Y,
    Width,
    Height,
    Rotation,
    Opacity,
    Fill,
    Text,
    FontSize,
}
impl Field {
    pub const ALL: [Self; 10] =
        [Self::Name, Self::X, Self::Y, Self::Width, Self::Height, Self::Rotation, Self::Opacity, Self::Fill, Self::Text, Self::FontSize];
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "名称",
            Self::X => "X",
            Self::Y => "Y",
            Self::Width => "W",
            Self::Height => "H",
            Self::Rotation => "旋转",
            Self::Opacity => "不透明度",
            Self::Fill => "填充",
            Self::Text => "文字",
            Self::FontSize => "字号",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::X => "x",
            Self::Y => "y",
            Self::Width => "width",
            Self::Height => "height",
            Self::Rotation => "rotation",
            Self::Opacity => "opacity",
            Self::Fill => "fills",
            Self::Text => "characters",
            Self::FontSize => "fontSize",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    epoch: u64,
    window: u64,
    uid: u64,
    page: u64,
    ids: Vec<NodeId>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    pub field: Field,
    pub value: Option<String>,
    pub enabled: bool,
    pub mixed: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Inspector {
    pub target: Option<Target>,
    pub fields: Vec<Property>,
    /// Read-only appearance observations for the native design sections.
    pub appearance: Vec<(&'static str, String)>,
}
#[derive(Clone, Debug)]
pub struct Commit {
    pub target: Target,
    pub field: Field,
    pub value: String,
}
/// The engine's immutable document identity, without copying or retaining another document.
/// A weak reference keeps its allocation identity unique until the next observation. The
/// engine's repaint revision cannot serve this purpose: reads and view changes bump it too.
#[derive(Clone)]
struct Version(std::sync::Weak<Document>);
impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Weak::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Version {}
impl std::fmt::Debug for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DocumentVersion")
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    window: u64,
    uid: u64,
    page: u64,
    ids: Vec<NodeId>,
    version: Version,
    home: bool,
    readonly: bool,
}
#[derive(Default)]
pub(crate) struct Tracker {
    key: Option<Key>,
    epoch: u64,
}

impl FigApp {
    /// Observe command boundaries as well as frames, so A→B→A and selection round trips
    /// invalidate old drafts even when no intermediate native repaint occurred.
    pub(crate) fn observe_property_target(&mut self) {
        let key = self.session.ds().ok().map(|d| Key {
            window: self.windows.current,
            uid: d.uid,
            page: d.page.0,
            ids: d.selection.iter().copied().take(257).collect(),
            version: Version(std::sync::Arc::downgrade(&d.doc)),
            home: self.ui.home || !self.shows(d.uid),
            readonly: self.session.active_read_only().is_some() || d.preview.is_some(),
        });
        if self.workspace_properties.key != key {
            self.workspace_properties.key = key;
            self.workspace_properties.epoch = self.workspace_properties.epoch.wrapping_add(1);
        }
    }
    pub fn workspace_inspector(&mut self) -> Inspector {
        self.observe_property_target();
        let Some(ds) = self.session.ds().ok().filter(|d| !self.ui.home && self.shows(d.uid)) else {
            return Inspector { target: None, fields: Vec::new(), appearance: Vec::new() };
        };
        if ds.selection.len() > 256 {
            return Inspector { target: None, fields: Vec::new(), appearance: Vec::new() };
        }
        let nodes: Vec<_> = ds.selection.iter().filter_map(|id| ds.doc.node(*id)).collect();
        if nodes.is_empty() || nodes.len() > 256 || nodes.len() != ds.selection.len() {
            return Inspector { target: None, fields: Vec::new(), appearance: Vec::new() };
        }
        let editable = self.session.active_read_only().is_none()
            && ds.preview.is_none()
            && ds.interaction.is_none()
            && ds.vector_edit.is_none()
            && ds.text_edit.is_none()
            && nodes.iter().all(|n| !n.is_page() && !n.locked && ds.doc.ancestors(n.id).iter().all(|id| ds.doc.node(*id).is_some_and(|a| !a.locked)));
        let one = nodes.len() == 1;
        let fields = Field::ALL
            .into_iter()
            .map(|field| {
                let values: Vec<_> = nodes.iter().map(|n| field_value(field, &ds.doc, n)).collect();
                let value = values.first().cloned().flatten();
                let mixed = values.iter().any(|v| *v != value);
                let supported = match field {
                    Field::Fill => nodes.iter().all(|n| simple_fill(n)),
                    Field::Text | Field::FontSize => one && nodes[0].kind.text().is_some_and(|t| t.runs.is_empty() && t.characters.len() <= 65536),
                    Field::Opacity => true,
                    _ => one,
                };
                Property {
                    field,
                    value: if mixed { None } else { value },
                    mixed,
                    enabled: editable && supported && values.iter().all(Option::is_some),
                }
            })
            .collect();
        Inspector {
            appearance: nodes.first().filter(|_| one).map(|n| appearance(n)).unwrap_or_default(),
            target: Some(Target {
                epoch: self.workspace_properties.epoch,
                window: self.windows.current,
                uid: ds.uid,
                page: ds.page.0,
                ids: ds.selection.clone(),
            }),
            fields,
        }
    }
    pub fn workspace_commit_property(&mut self, edit: Commit) -> bool {
        let current = self.workspace_inspector();
        // No fallback to the current file/node: the original owner, page, selection and editing
        // generation must all still match, including after an external edit or round trip.
        if current.target.as_ref() != Some(&edit.target) || self.pending_dialog.is_some() || self.ui.close_tab.is_some() {
            return false;
        }
        let Some(property) = current.fields.iter().find(|p| p.field == edit.field && p.enabled) else {
            return false;
        };
        if property.value.as_deref() == Some(edit.value.as_str()) {
            return true;
        }
        let value = match parsed(edit.field, &edit.value) {
            Ok(v) => v,
            Err(e) => {
                self.ui.last_error = Some(e.into());
                return false;
            }
        };
        self.ui.last_error = None;
        let ids = &edit.target.ids;
        let result = if edit.field == Field::Fill {
            self.run("fill.set", json!({"ids":ids,"index":0,"color":value}))
        } else {
            self.run("node.set", json!({"ids":ids,"props":{edit.field.key():value}}))
        };
        result.is_some()
    }
}
fn simple_fill(n: &Node) -> bool {
    n.fills.len() == 1
        // The native field edits RGB only. Replacing a translucent colour would silently
        // erase its alpha; leave that paint to the full editor instead.
        && matches!(n.fills[0].kind, figcraft_color::PaintKind::Solid { color } if color.a == 1.0)
        && !n.bound.keys().any(|k| k == "fill" || k.starts_with("fills"))
        && n.kind.text().is_none_or(|t| t.runs.is_empty() && t.style.fills.is_none())
}
fn field_value(field: Field, doc: &figcraft_doc::Document, n: &Node) -> Option<String> {
    let description = figcraft_engine::cmd::props::describe(doc, n.id);
    match field {
        Field::Name => Some(n.name.chars().take(4096).collect()),
        Field::Opacity => Some(format!("{}%", number(f64::from(n.opacity) * 100.0))),
        Field::Fill => {
            if simple_fill(n) {
                match n.fills[0].kind {
                    figcraft_color::PaintKind::Solid { color } => Some(format!("#{}", color.to_hex())),
                    _ => None,
                }
            } else {
                None
            }
        }
        Field::Text => n.kind.text().map(|t| t.characters.chars().take(65536).collect()),
        Field::FontSize => n.kind.text().map(|t| t.style.font_size.to_string()),
        _ => description.get(field.key()).and_then(Value::as_f64).map(|v| v.to_string()),
    }
}
fn appearance(n: &Node) -> Vec<(&'static str, String)> {
    use figcraft_doc::{NodeKind, props::EffectKind};
    let kind = match &n.kind {
        NodeKind::Frame => "画板",
        NodeKind::Rectangle => "矩形",
        NodeKind::Text(_) => "文字",
        NodeKind::Group => "组",
        NodeKind::Ellipse { .. } => "椭圆",
        _ => n.kind.label(),
    };
    let corners = if n.corners.radii.iter().all(|v| *v == n.corners.radii[0]) { number(n.corners.radii[0]) } else { "混合".into() };
    let stroke = n
        .strokes
        .iter()
        .find(|p| p.visible)
        .and_then(|p| match p.kind {
            figcraft_color::PaintKind::Solid { color } => Some(format!("#{}", color.to_hex())),
            _ => None,
        })
        .unwrap_or_else(|| "—".into());
    let effect = n
        .effects
        .iter()
        .find(|e| e.visible)
        .map(|e| match &e.kind {
            EffectKind::DropShadow(s) => format!("阴影     {} · {} · {}", number(s.offset.0), number(s.offset.1), number(s.radius)),
            EffectKind::InnerShadow(_) => "内阴影".into(),
            EffectKind::LayerBlur(_) => "图层模糊".into(),
            EffectKind::BackgroundBlur(_) => "背景模糊".into(),
            _ => "效果".into(),
        })
        .unwrap_or_else(|| "—".into());
    vec![
        ("kind", kind.into()),
        ("corner", corners),
        ("stroke", stroke),
        ("strokeWidth", format!("{} px", number(n.stroke.weight))),
        ("effect", effect),
        ("fillOpacity", n.fills.first().map(|p| format!("{}%", number(f64::from(p.opacity) * 100.))).unwrap_or_default()),
    ]
}
fn number(value: f64) -> String {
    format!("{value:.3}").trim_end_matches('0').trim_end_matches('.').to_string()
}

fn parsed(field: Field, input: &str) -> Result<Value, &'static str> {
    if input.len() > 65536 {
        return Err("输入过长");
    }
    match field {
        Field::Name | Field::Text => Ok(json!(input)),
        Field::Fill => figcraft_color::Color::parse_css(input.trim())
            .filter(|c| c.a == 1.0)
            .map(|c| json!(format!("#{}", c.to_hex())))
            .ok_or("请输入不含透明度的颜色"),
        _ => {
            let text = input.trim();
            let (text, percent) = text.strip_suffix('%').map_or((text, false), |v| (v, true));
            let mut n = text.parse::<f64>().ok().filter(|v| v.is_finite()).ok_or("请输入有效数值")?;
            if field == Field::Opacity {
                n /= 100.0;
            } else if percent {
                return Err("该字段不使用百分比");
            }
            if (matches!(field, Field::Width | Field::Height | Field::FontSize) && n <= 0.0) || (field == Field::Opacity && !(0.0..=1.0).contains(&n))
            {
                return Err("数值超出范围");
            }
            Ok(json!(n))
        }
    }
}

#[cfg(test)]
#[path = "workspace_properties_tests.rs"]
mod tests;
