//! Full-width animated "Map" view of the physical topology.
//!
//! Every physical socket is drawn as one card whose rows are its lanes (the
//! USB 2 lane and the SuperSpeed "companion" lane Windows lists separately).
//! Links are animated with particles whose speed and thickness follow the
//! negotiated link speed.

use std::collections::HashSet;

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind,
    Ui, Vec2,
};
use egui_phosphor::regular as ph;

use super::theme::{self, with_alpha, Palette};
use super::{icons, widgets, Action, App, ViewMode};
use crate::model::{Snapshot, Speed};
use crate::physical::{self, Connector, Physical, PortRef};
use crate::tree::{self, FlatNode, Health};

const CTRL_W: f32 = 260.0;
const CONN_W: f32 = 150.0;
const DEV_W: f32 = 270.0;
const GAP_X: f32 = 64.0;
const LANE_H: f32 = 22.0;

#[derive(Clone, Debug, PartialEq)]
pub enum MKind {
    Controller(usize),
    Connector {
        lanes: Vec<LaneView>,
        type_c: bool,
        internal: bool,
        error: bool,
        slow_lane_hint: bool,
    },
    Device {
        port: PortRef,
        halves: Vec<Speed>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct LaneView {
    pub index: u32,
    pub max: Speed,
    /// Negotiated speed of the device on this lane.
    pub link: Option<Speed>,
}

#[derive(Clone, Debug)]
pub struct MNode {
    pub id: String,
    pub kind: MKind,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub rect: Rect,
    pub label: String,
}

pub struct MapLayout {
    pub nodes: Vec<MNode>,
    pub bounds: Rect,
}

impl Default for MapLayout {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            bounds: Rect::NOTHING,
        }
    }
}

pub struct MapState {
    pub pan: Vec2,
    pub zoom: f32,
    pub needs_fit: bool,
    pub selected: Option<usize>,
    pub help_open: bool,
    pub layout: MapLayout,
}

impl Default for MapState {
    fn default() -> Self {
        Self {
            pan: Vec2::ZERO,
            zoom: 1.0,
            needs_fit: true,
            selected: None,
            help_open: false,
            layout: MapLayout::default(),
        }
    }
}

fn port_id(snap: &Snapshot, r: &PortRef) -> String {
    let c = &snap.controllers[r.0];
    let cid = if c.info.instance_id.is_empty() {
        format!("ctrl{}", r.0)
    } else {
        c.info.instance_id.clone()
    };
    format!("{cid}/{}", tree::port_chain(snap, r.0, &r.1))
}

/// Builds the node graph and computes world-space positions.
pub fn build_layout(snap: &Snapshot, show_empty: bool) -> MapLayout {
    let phys = Physical::build(snap);
    let mut nodes: Vec<MNode> = Vec::new();
    for pc in &phys.controllers {
        let c = &snap.controllers[pc.ci];
        let id = if c.info.instance_id.is_empty() {
            format!("ctrl{}", pc.ci)
        } else {
            c.info.instance_id.clone()
        };
        let ci_node = nodes.len();
        nodes.push(MNode {
            id,
            kind: MKind::Controller(pc.ci),
            parent: None,
            children: vec![],
            rect: Rect::NOTHING,
            label: c.info.display_name().to_string(),
        });
        add_connectors(snap, &phys, &pc.connectors, ci_node, show_empty, &mut nodes);
    }
    // Layout: x by column, y by stacking leaves.
    let mut cursor = 0.0;
    let roots: Vec<usize> = (0..nodes.len())
        .filter(|&i| nodes[i].parent.is_none())
        .collect();
    for r in roots {
        place(&mut nodes, r, 0.0, &mut cursor);
        cursor += 48.0;
    }
    let bounds = nodes.iter().fold(Rect::NOTHING, |b, n| b.union(n.rect));
    MapLayout { nodes, bounds }
}

fn add_connectors(
    snap: &Snapshot,
    phys: &Physical,
    conns: &[Connector],
    parent: usize,
    show_empty: bool,
    nodes: &mut Vec<MNode>,
) {
    for con in conns {
        if con.attached.is_empty() && !show_empty && !con.error {
            continue;
        }
        let lanes: Vec<LaneView> = con
            .lanes
            .iter()
            .map(|l| LaneView {
                index: l.index,
                max: l.max,
                link: tree::port(snap, l.port.0, &l.port.1)
                    .and_then(|p| p.device.as_ref())
                    .map(|d| d.speed),
            })
            .collect();
        let slow_lane_hint = con.attached.last().is_some_and(|a| {
            physical::link_info(snap, phys, a.0, &a.1)
                .is_some_and(|li| li.on_slow_lane && li.best_possible > li.lane_max)
        });
        let cid = nodes.len();
        nodes.push(MNode {
            id: format!("conn:{}", port_id(snap, con.primary())),
            kind: MKind::Connector {
                lanes,
                type_c: con.type_c,
                internal: con.internal,
                error: con.error,
                slow_lane_hint,
            },
            parent: Some(parent),
            children: vec![],
            rect: Rect::NOTHING,
            label: con.label(),
        });
        nodes[parent].children.push(cid);
        if let Some(main) = con.attached.last() {
            let halves: Vec<Speed> = con
                .attached
                .iter()
                .filter_map(|a| {
                    tree::port(snap, a.0, &a.1)
                        .and_then(|p| p.device.as_ref())
                        .map(|d| d.speed)
                })
                .collect();
            let name = tree::port(snap, main.0, &main.1)
                .and_then(|p| p.device.as_ref())
                .map(|d| d.name())
                .unwrap_or_default();
            let di = nodes.len();
            nodes.push(MNode {
                id: port_id(snap, main),
                kind: MKind::Device {
                    port: main.clone(),
                    halves,
                },
                parent: Some(cid),
                children: vec![],
                rect: Rect::NOTHING,
                label: name,
            });
            nodes[cid].children.push(di);
            add_connectors(snap, phys, &con.children, di, show_empty, nodes);
        }
    }
}

fn node_size(n: &MNode) -> Vec2 {
    match &n.kind {
        MKind::Controller(_) => Vec2::new(CTRL_W, 64.0),
        MKind::Connector { lanes, .. } => Vec2::new(CONN_W, 30.0 + LANE_H * lanes.len() as f32),
        MKind::Device { halves, .. } => {
            Vec2::new(DEV_W, if halves.len() > 1 { 74.0 } else { 62.0 })
        }
    }
}

fn place(nodes: &mut Vec<MNode>, i: usize, x: f32, cursor: &mut f32) {
    let size = node_size(&nodes[i]);
    let child_x = x + size.x + GAP_X;
    let children = nodes[i].children.clone();
    if children.is_empty() {
        nodes[i].rect = Rect::from_min_size(Pos2::new(x, *cursor), size);
        *cursor += size.y + 14.0;
        return;
    }
    let start = *cursor;
    for c in &children {
        place(nodes, *c, child_x, cursor);
    }
    let first = nodes[children[0]].rect.center().y;
    let last = nodes[*children.last().unwrap()].rect.center().y;
    let mut cy = (first + last) / 2.0;
    // Keep the parent inside the band its children occupy.
    cy = cy.max(start + size.y / 2.0);
    nodes[i].rect = Rect::from_center_size(Pos2::new(x + size.x / 2.0, cy), size);
    let bottom = nodes[i].rect.bottom() + 14.0;
    if bottom > *cursor {
        *cursor = bottom;
    }
}

struct View {
    origin: Pos2,
    pan: Vec2,
    zoom: f32,
}

impl View {
    fn pos(&self, w: Pos2) -> Pos2 {
        self.origin + self.pan + w.to_vec2() * self.zoom
    }
    fn rect(&self, r: Rect) -> Rect {
        Rect::from_min_max(self.pos(r.min), self.pos(r.max))
    }
}

fn bezier(a: Pos2, b: Pos2, t: f32) -> Pos2 {
    let dx = (b.x - a.x) * 0.5;
    let p1 = Pos2::new(a.x + dx, a.y);
    let p2 = Pos2::new(b.x - dx, b.y);
    let u = 1.0 - t;
    let x = u * u * u * a.x + 3.0 * u * u * t * p1.x + 3.0 * u * t * t * p2.x + t * t * t * b.x;
    let y = u * u * u * a.y + 3.0 * u * u * t * p1.y + 3.0 * u * t * t * p2.y + t * t * t * b.y;
    Pos2::new(x, y)
}

fn curve_points(a: Pos2, b: Pos2) -> Vec<Pos2> {
    (0..=28).map(|i| bezier(a, b, i as f32 / 28.0)).collect()
}

pub(crate) fn link_width(s: Speed) -> f32 {
    match s {
        Speed::Unknown | Speed::Low => 1.5,
        Speed::Full => 2.0,
        Speed::High => 2.8,
        Speed::Super => 3.8,
        Speed::SuperPlus => 4.6,
        Speed::SuperPlus20 => 5.4,
    }
}

fn particle_rate(s: Speed) -> f32 {
    match s {
        Speed::Unknown | Speed::Low => 0.10,
        Speed::Full => 0.16,
        Speed::High => 0.28,
        Speed::Super => 0.48,
        Speed::SuperPlus => 0.66,
        Speed::SuperPlus20 => 0.85,
    }
}

fn fade(c: Color32, f: f32) -> Color32 {
    if f >= 0.999 {
        c
    } else {
        c.gamma_multiply(f)
    }
}

/// Draws an animated link between two screen points.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_link(
    ui: &Ui,
    p: &Palette,
    a: Pos2,
    b: Pos2,
    speed: Option<Speed>,
    zoom: f32,
    t: f64,
    motion: bool,
    f: f32,
    warn: bool,
) {
    let painter = ui.painter();
    let pts = curve_points(a, b);
    match speed {
        None => {
            for s in Shape::dashed_line(
                &pts,
                Stroke::new(1.2 * zoom.max(0.6), fade(p.idle_link, f)),
                4.0 * zoom,
                4.0 * zoom,
            ) {
                painter.add(s);
            }
        }
        Some(s) => {
            let c = p.speed(s);
            let w = link_width(s) * zoom.max(0.5);
            if warn {
                painter.add(Shape::line(
                    pts.clone(),
                    Stroke::new(w + 6.0 * zoom, fade(with_alpha(p.slow_lane, 0.28), f)),
                ));
            }
            painter.add(Shape::line(
                pts.clone(),
                Stroke::new(w + 3.0 * zoom, fade(with_alpha(c, 0.14), f)),
            ));
            painter.add(Shape::line(
                pts,
                Stroke::new(w, fade(with_alpha(c, 0.75), f)),
            ));
            if motion {
                let len = (b - a).length().max(1.0);
                let n = ((len / (70.0 * zoom.max(0.3))).ceil() as usize).clamp(2, 8);
                let rate = particle_rate(s) * 260.0 / (len / zoom.max(0.3)).max(60.0);
                for i in 0..n {
                    let tt = ((t as f32 * rate) + i as f32 / n as f32).fract();
                    let pos = bezier(a, b, tt);
                    let alpha = (tt * std::f32::consts::PI).sin();
                    painter.circle_filled(
                        pos,
                        (w * 0.55 + 1.2).max(1.5),
                        fade(with_alpha(Color32::WHITE, 0.85 * alpha), f),
                    );
                    painter.circle_filled(pos, w * 1.3 + 2.0, fade(with_alpha(c, 0.25 * alpha), f));
                }
            }
        }
    }
}

fn elide(
    painter: &egui::Painter,
    text: &str,
    font: FontId,
    color: Color32,
    max_w: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = LayoutJob::single_section(
        text.to_string(),
        TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    );
    job.wrap.max_width = max_w.max(8.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    painter.layout_job(job)
}

/// Nodes related to `i` (ancestors and descendants) for hover highlighting.
fn related(l: &MapLayout, i: usize) -> HashSet<usize> {
    let mut s = HashSet::new();
    let mut cur = Some(i);
    while let Some(c) = cur {
        s.insert(c);
        cur = l.nodes[c].parent;
    }
    let mut stack = vec![i];
    while let Some(c) = stack.pop() {
        s.insert(c);
        stack.extend(l.nodes[c].children.iter().copied());
    }
    s
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    let Some(snap) = app.snapshot.clone() else {
        ui.centered_and_justified(|ui| {
            ui.add(egui::Spinner::new().size(28.0).color(p.accent));
        });
        return;
    };
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, p.bg);

    // Fit to view.
    if app.map.needs_fit && !app.map.layout.nodes.is_empty() {
        fit(app, rect);
        app.map.needs_fit = false;
    }
    // Pan & zoom.
    if resp.dragged() {
        app.map.pan += resp.drag_delta();
    }
    if resp.hovered() {
        let (scroll, zoom_delta, pointer) =
            ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
        let mut factor = zoom_delta;
        if scroll.y != 0.0 && zoom_delta == 1.0 {
            factor = (scroll.y * 0.0018).exp();
        }
        if factor != 1.0 {
            if let Some(ptr) = pointer {
                zoom_at(app, rect, ptr, factor);
            }
        }
    }
    let t = ui.input(|i| i.time);
    let motion = app.settings.map_motion;
    if motion {
        ui.ctx().request_repaint();
    }
    let view = View {
        origin: rect.min,
        pan: app.map.pan,
        zoom: app.map.zoom,
    };
    let z = app.map.zoom;

    // Background dot grid.
    let spacing = 26.0 * z;
    if spacing > 8.0 {
        let off = Vec2::new(
            (view.pan.x).rem_euclid(spacing),
            (view.pan.y).rem_euclid(spacing),
        );
        let mut y = rect.top() + off.y;
        while y < rect.bottom() {
            let mut x = rect.left() + off.x;
            while x < rect.right() {
                painter.circle_filled(Pos2::new(x, y), 1.0, with_alpha(p.text_faint, 0.18));
                x += spacing;
            }
            y += spacing;
        }
    }

    let layout_owned = std::mem::take(&mut app.map.layout);
    let layout = &layout_owned;
    // Hover detection (topmost = last).
    let pointer = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|pp| rect.contains(*pp));
    let hovered = pointer.and_then(|pp| {
        layout
            .nodes
            .iter()
            .rposition(|n| view.rect(n.rect).contains(pp))
    });
    let focus = hovered.or(app.map.selected);
    let rel = focus.map(|h| related(layout, h));
    let search_active = super::tree_view::filter_active(app);
    let dim = |i: usize| -> f32 {
        let mut f: f32 = 1.0;
        if let Some(r) = &rel {
            if !r.contains(&i) {
                f = 0.35;
            }
        }
        if search_active {
            let n = &layout.nodes[i];
            let id = n.id.trim_start_matches("conn:");
            let m = app
                .flat
                .get(id)
                .is_some_and(|fnode| super::tree_view::node_matches(app, fnode));
            if !m {
                f = f.min(0.25);
            }
        }
        f
    };

    let phys = Physical::build(&snap);
    // --- Links ---
    for (i, n) in layout.nodes.iter().enumerate() {
        let Some(pi) = n.parent else { continue };
        let parent = &layout.nodes[pi];
        let f = dim(i).min(dim(pi));
        let pr = view.rect(parent.rect);
        let cr = view.rect(n.rect);
        match (&parent.kind, &n.kind) {
            // Parent (controller or hub) -> each lane row of a socket.
            (_, MKind::Connector { lanes, .. }) => {
                let halves = match &parent.kind {
                    MKind::Device { halves, .. } => halves.clone(),
                    _ => vec![],
                };
                for (li, lane) in lanes.iter().enumerate() {
                    let ly = cr.top() + (30.0 + LANE_H * li as f32 + LANE_H / 2.0) * z;
                    let from_y = if halves.len() > 1 {
                        // USB 2 lanes come from the USB 2 half, SS lanes from the SS half.
                        let k = if lane.max.is_super() {
                            halves.len() - 1
                        } else {
                            0
                        };
                        pr.top() + pr.height() * (k as f32 + 1.0) / (halves.len() as f32 + 1.0)
                    } else {
                        pr.center().y
                    };
                    draw_link(
                        ui,
                        &p,
                        Pos2::new(pr.right(), from_y),
                        Pos2::new(cr.left(), ly),
                        lane.link,
                        z,
                        t,
                        motion,
                        f,
                        false,
                    );
                }
            }
            // Socket lane rows -> device (two links for USB 3 hubs).
            (
                MKind::Connector {
                    lanes,
                    slow_lane_hint,
                    ..
                },
                MKind::Device { .. },
            ) => {
                let occupied: Vec<(usize, &LaneView)> = lanes
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| l.link.is_some())
                    .collect();
                for (k, (li, lane)) in occupied.iter().enumerate() {
                    let ly = pr.top() + (30.0 + LANE_H * *li as f32 + LANE_H / 2.0) * z;
                    let ty = if occupied.len() > 1 {
                        cr.top() + cr.height() * (k as f32 + 1.0) / (occupied.len() as f32 + 1.0)
                    } else {
                        cr.center().y
                    };
                    draw_link(
                        ui,
                        &p,
                        Pos2::new(pr.right(), ly),
                        Pos2::new(cr.left(), ty),
                        lane.link,
                        z,
                        t,
                        motion,
                        f,
                        *slow_lane_hint,
                    );
                }
            }
            _ => {
                draw_link(
                    ui,
                    &p,
                    Pos2::new(pr.right(), pr.center().y),
                    Pos2::new(cr.left(), cr.center().y),
                    None,
                    z,
                    t,
                    motion,
                    f,
                    false,
                );
            }
        }
    }

    // --- Nodes ---
    let mut clicked_node = None;
    let mut double_clicked = None;
    for (i, n) in layout.nodes.iter().enumerate() {
        let mut r = view.rect(n.rect);
        if !r.intersects(rect) {
            continue;
        }
        let f = dim(i);
        // Arrival: pop in with a pulse ring.
        let arrival = app.arrivals.get(&n.id).map(|a| a.elapsed().as_secs_f32());
        if let Some(age) = arrival {
            let k = (age / 0.5).min(1.0);
            let s = 0.85 + 0.15 * (1.0 - (1.0 - k).powi(3));
            r = Rect::from_center_size(r.center(), r.size() * s);
            if age < 2.5 {
                let ring = (age / 2.5).min(1.0);
                painter.rect_stroke(
                    r.expand(ring * 22.0 * z),
                    CornerRadius::same(14),
                    Stroke::new(2.0, with_alpha(p.ok, 1.0 - ring)),
                    StrokeKind::Outside,
                );
            }
        }
        let selected = app.map.selected == Some(i);
        let hot = hovered == Some(i);
        match &n.kind {
            MKind::Controller(ci) => {
                draw_controller(ui, &p, &snap, *ci, &n.label, r, z, f, hot || selected)
            }
            MKind::Connector {
                lanes,
                type_c,
                internal,
                error,
                slow_lane_hint,
            } => draw_connector(
                ui,
                &p,
                n,
                lanes,
                *type_c,
                *internal,
                *error,
                *slow_lane_hint,
                r,
                z,
                f,
                hot || selected,
            ),
            MKind::Device { port, halves } => {
                let flat = app.flat.get(&n.id).cloned();
                draw_device(
                    app,
                    ui,
                    &snap,
                    &phys,
                    port,
                    halves,
                    flat.as_ref(),
                    r,
                    z,
                    f,
                    hot || selected,
                    t,
                );
            }
        }
        if selected {
            painter.rect_stroke(
                r.expand(3.0),
                CornerRadius::same(14),
                Stroke::new(2.0, p.accent),
                StrokeKind::Outside,
            );
        }
        if hot && resp.clicked() {
            clicked_node = Some(i);
        }
        if hot && resp.double_clicked() {
            double_clicked = Some(i);
        }
    }
    if resp.clicked() {
        app.map.selected = clicked_node;
        if let Some(i) = clicked_node {
            let id = layout.nodes[i].id.trim_start_matches("conn:").to_string();
            if app.flat.by_id.contains_key(&id) {
                app.selected = Some(id);
            }
        }
    }
    let double_id =
        double_clicked.map(|i| layout.nodes[i].id.trim_start_matches("conn:").to_string());
    if let Some(id) = double_id {
        app.view = ViewMode::Tree;
        app.settings.map_view = false;
        app.dispatch(Action::Select(id));
    }

    // Tooltips
    if let Some(h) = hovered {
        let n = &layout.nodes[h];
        if let MKind::Connector { lanes, .. } = &n.kind {
            let lanes = lanes.clone();
            resp.clone()
                .on_hover_ui_at_pointer(|ui| connector_tooltip(ui, &p, &lanes));
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    } else if resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }

    // Keyboard
    let text_focus = ui.ctx().memory(|m| m.focused().is_some());
    if !text_focus && !app.palette_open {
        if ui.input(|i| i.key_pressed(egui::Key::L)) {
            toggle_legend(app);
        }
        let (f_key, plus, minus, esc) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::F),
                i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals),
                i.key_pressed(egui::Key::Minus),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if f_key {
            app.map.needs_fit = true;
        }
        if plus {
            zoom_at(app, rect, rect.center(), 1.2);
        }
        if minus {
            zoom_at(app, rect, rect.center(), 1.0 / 1.2);
        }
        if esc {
            app.map.selected = None;
        }
    }

    app.map.layout = layout_owned;
    overlays(app, ui, rect);
}

pub fn toggle_legend(app: &mut App) {
    app.settings.map_legend = !app.settings.map_legend;
    app.settings.save();
}

fn zoom_at(app: &mut App, rect: Rect, ptr: Pos2, factor: f32) {
    let old = app.map.zoom;
    let new = (old * factor).clamp(0.2, 2.5);
    let world = (ptr - rect.min - app.map.pan) / old;
    app.map.zoom = new;
    app.map.pan = ptr - rect.min - world * new;
}

fn fit(app: &mut App, rect: Rect) {
    let b = app.map.layout.bounds;
    if !b.is_positive() {
        return;
    }
    let margin = 50.0;
    // Keep clear of the legend (left) and toolbar (top).
    let left = if app.settings.map_legend {
        290.0
    } else {
        margin
    };
    let top = 70.0;
    let avail = Vec2::new(rect.width() - left - margin, rect.height() - top - margin);
    let z = (avail.x / b.width())
        .min(avail.y / b.height())
        .clamp(0.3, 1.1);
    app.map.zoom = z;
    let content = b.size() * z;
    app.map.pan = Vec2::new(
        left + ((avail.x - content.x) / 2.0).max(0.0),
        top + ((avail.y - content.y) / 2.0).max(0.0),
    ) - b.min.to_vec2() * z;
}

#[allow(clippy::too_many_arguments)]
fn card_bg(ui: &Ui, p: &Palette, r: Rect, z: f32, f: f32, border: Color32, hot: bool) {
    let painter = ui.painter();
    let cr = CornerRadius::same((12.0 * z).clamp(3.0, 14.0) as u8);
    // Soft shadow
    painter.rect_filled(
        r.translate(Vec2::new(0.0, 4.0 * z)),
        cr,
        fade(Color32::from_black_alpha(if p.dark { 70 } else { 18 }), f),
    );
    painter.rect_filled(
        r,
        cr,
        fade(if hot { p.card_hover } else { p.card }, f.max(0.6)),
    );
    painter.rect_stroke(
        r,
        cr,
        Stroke::new(if hot { 1.5 } else { 1.0 }, fade(border, f)),
        StrokeKind::Inside,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_controller(
    ui: &Ui,
    p: &Palette,
    snap: &Snapshot,
    ci: usize,
    label: &str,
    r: Rect,
    z: f32,
    f: f32,
    hot: bool,
) {
    card_bg(ui, p, r, z, f, if hot { p.accent } else { p.border }, hot);
    let painter = ui.painter();
    let tile = Rect::from_min_size(r.min + Vec2::new(12.0, 12.0) * z, Vec2::splat(40.0 * z));
    painter.rect_filled(
        tile,
        CornerRadius::same((10.0 * z) as u8),
        fade(p.soft(p.accent), f),
    );
    painter.text(
        tile.center(),
        Align2::CENTER_CENTER,
        ph::CPU,
        theme::icon_fill(22.0 * z),
        fade(p.accent, f),
    );
    let x = tile.right() + 10.0 * z;
    let g = elide(
        painter,
        label,
        theme::semibold(13.0 * z),
        fade(p.text, f),
        r.right() - x - 10.0 * z,
    );
    painter.galley(Pos2::new(x, r.top() + 14.0 * z), g, p.text);
    let c = &snap.controllers[ci];
    let mut sub = c.kind().to_string();
    if let (Some(v), Some(d)) = (c.pci_vendor, c.pci_device) {
        sub += &format!(" · PCI {v:04X}:{d:04X}");
    }
    if z > 0.45 {
        painter.text(
            Pos2::new(x, r.top() + 40.0 * z),
            Align2::LEFT_CENTER,
            sub,
            FontId::proportional(11.5 * z),
            fade(p.text_muted, f),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_connector(
    ui: &Ui,
    p: &Palette,
    n: &MNode,
    lanes: &[LaneView],
    type_c: bool,
    internal: bool,
    error: bool,
    slow_hint: bool,
    r: Rect,
    z: f32,
    f: f32,
    hot: bool,
) {
    let border = if error {
        p.error
    } else if hot {
        p.accent
    } else {
        p.border
    };
    card_bg(ui, p, r, z, f, border, hot);
    let painter = ui.painter();
    // Header: socket glyph + label
    let hx = r.left() + 10.0 * z;
    let hy = r.top() + 15.0 * z;
    let usb3 = lanes.iter().any(|l| l.max.is_super());
    let glyph_c = fade(
        if usb3 {
            p.speed(Speed::Super)
        } else {
            p.text_muted
        },
        f,
    );
    if type_c {
        let g = Rect::from_center_size(Pos2::new(hx + 9.0 * z, hy), Vec2::new(18.0 * z, 7.0 * z));
        painter.rect_stroke(
            g,
            CornerRadius::same(255),
            Stroke::new(1.5 * z.max(0.5), glyph_c),
            StrokeKind::Inside,
        );
    } else {
        let g = Rect::from_center_size(Pos2::new(hx + 9.0 * z, hy), Vec2::new(18.0 * z, 9.0 * z));
        painter.rect_stroke(
            g,
            CornerRadius::same((2.0 * z) as u8),
            Stroke::new(1.5 * z.max(0.5), glyph_c),
            StrokeKind::Inside,
        );
        let tongue = Rect::from_min_size(
            g.min + Vec2::new(3.0 * z, 2.0 * z),
            Vec2::new(12.0 * z, 2.5 * z),
        );
        painter.rect_filled(tongue, CornerRadius::ZERO, glyph_c);
    }
    if z > 0.4 {
        let mut label = n.label.clone();
        if internal {
            label += " · int";
        }
        let g = elide(
            painter,
            &label,
            theme::semibold(11.5 * z),
            fade(p.text, f),
            r.right() - hx - 26.0 * z,
        );
        painter.galley(Pos2::new(hx + 24.0 * z, hy - g.size().y / 2.0), g, p.text);
    }
    // Lane rows
    for (i, l) in lanes.iter().enumerate() {
        let row = Rect::from_min_size(
            Pos2::new(r.left() + 6.0 * z, r.top() + (30.0 + LANE_H * i as f32) * z),
            Vec2::new(r.width() - 12.0 * z, (LANE_H - 3.0) * z),
        );
        let active = l.link.is_some();
        let c = p.speed(l.link.unwrap_or(l.max));
        if active {
            painter.rect_filled(row, CornerRadius::same((6.0 * z) as u8), fade(p.soft(c), f));
        }
        let available = slow_hint && !active && l.max.is_super();
        if available {
            for s in Shape::dashed_line(
                &[
                    row.left_top(),
                    row.right_top(),
                    row.right_bottom(),
                    row.left_bottom(),
                    row.left_top(),
                ],
                Stroke::new(1.0, fade(p.slow_lane, f)),
                3.0 * z,
                3.0 * z,
            ) {
                painter.add(s);
            }
        }
        let dot = Pos2::new(row.left() + 9.0 * z, row.center().y);
        if active {
            painter.circle_filled(dot, 4.0 * z, fade(c, f));
        } else {
            painter.circle_stroke(
                dot,
                3.5 * z,
                Stroke::new(
                    1.2,
                    fade(if available { p.slow_lane } else { p.text_faint }, f),
                ),
            );
        }
        if z > 0.45 {
            let lane_name = if l.max.is_super() { "USB 3" } else { "USB 2" };
            let right = if available {
                "free".to_string()
            } else {
                l.max.short().to_string()
            };
            painter.text(
                Pos2::new(dot.x + 9.0 * z, row.center().y),
                Align2::LEFT_CENTER,
                format!("{lane_name} · {}", l.index),
                FontId::proportional(11.0 * z),
                fade(if active { p.text } else { p.text_muted }, f),
            );
            painter.text(
                Pos2::new(row.right() - 6.0 * z, row.center().y),
                Align2::RIGHT_CENTER,
                right,
                FontId::proportional(10.5 * z),
                fade(
                    if available {
                        p.slow_lane
                    } else if active {
                        c
                    } else {
                        p.text_faint
                    },
                    f,
                ),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_device(
    app: &App,
    ui: &Ui,
    snap: &Snapshot,
    phys: &Physical,
    port: &PortRef,
    halves: &[Speed],
    flat: Option<&FlatNode>,
    r: Rect,
    z: f32,
    f: f32,
    hot: bool,
    t: f64,
) {
    let p = &app.p;
    let Some(dev) = tree::port(snap, port.0, &port.1).and_then(|pp| pp.device.as_ref()) else {
        return;
    };
    let health = flat.map(|n| n.health).unwrap_or(Health::Ok);
    let speed_c = p.speed(dev.speed);
    let border = match health {
        Health::Error => p.error,
        Health::Warning => p.warn,
        _ if hot => p.accent,
        _ => p.border,
    };
    card_bg(ui, p, r, z, f, border, hot);
    let painter = ui.painter();
    // Problem pulse
    if health == Health::Error {
        let pulse = ((t * 2.2).sin() as f32 * 0.5 + 0.5) * 0.6;
        painter.rect_stroke(
            r.expand(3.0 + pulse * 4.0),
            CornerRadius::same(14),
            Stroke::new(1.5, with_alpha(p.error, 0.6 - pulse * 0.5)),
            StrokeKind::Outside,
        );
    }
    // Accent stripe in link color
    let stripe = Rect::from_min_size(
        r.min + Vec2::new(0.0, 10.0 * z),
        Vec2::new(3.0 * z.max(0.7), r.height() - 20.0 * z),
    );
    painter.rect_filled(stripe, CornerRadius::same(2), fade(speed_c, f));

    let icon_c = match health {
        Health::Error => p.error,
        Health::Warning => p.warn,
        _ => speed_c,
    };
    let tile = Rect::from_min_size(
        Pos2::new(r.left() + 12.0 * z, r.center().y - 19.0 * z),
        Vec2::splat(38.0 * z),
    );
    painter.rect_filled(
        tile,
        CornerRadius::same((10.0 * z) as u8),
        fade(p.soft(icon_c), f),
    );
    let icon = flat
        .map(|n| icons::for_node(n, Some(dev)))
        .unwrap_or(ph::USB);
    painter.text(
        tile.center(),
        Align2::CENTER_CENTER,
        icon,
        theme::icon_fill(21.0 * z),
        fade(icon_c, f),
    );

    let x = tile.right() + 10.0 * z;
    // Speed pill (right)
    let pill_txt = dev.speed.short();
    let pg = painter.layout_no_wrap(
        pill_txt.to_string(),
        FontId::proportional(10.5 * z),
        fade(speed_c, f),
    );
    let pr = Rect::from_min_size(
        Pos2::new(r.right() - pg.size().x - 22.0 * z, r.top() + 12.0 * z),
        pg.size() + Vec2::new(12.0 * z, 5.0 * z),
    );
    painter.rect_filled(pr, CornerRadius::same(255), fade(p.soft(speed_c), f));
    painter.galley(pr.center() - pg.size() / 2.0, pg, speed_c);

    let name = flat
        .map(|n| app.display_label(n))
        .unwrap_or_else(|| dev.name());
    let g = elide(
        painter,
        &name,
        theme::semibold(13.0 * z),
        fade(p.text, f),
        pr.left() - x - 6.0 * z,
    );
    painter.galley(Pos2::new(x, r.top() + 12.0 * z), g, p.text);
    if z > 0.45 {
        let sub = if halves.len() > 1 {
            "USB 3 hub · two halves (USB 2 + USB 3)".to_string()
        } else {
            let mut s = dev
                .vid_pid()
                .map(|(v, pi)| format!("{v:04X}:{pi:04X}"))
                .unwrap_or_default();
            if let Some(i) = &dev.info {
                let extra: Vec<String> = i
                    .all_volumes()
                    .iter()
                    .flat_map(|v| {
                        v.mount_points
                            .iter()
                            .map(|m| m.trim_end_matches('\\').to_string())
                    })
                    .chain(i.all_com_ports().into_iter().map(String::from))
                    .collect();
                if !extra.is_empty() {
                    s += &format!(" · {}", extra.join(" "));
                }
            }
            s
        };
        let g = elide(
            painter,
            &sub,
            FontId::proportional(11.5 * z),
            fade(p.text_muted, f),
            r.right() - x - 10.0 * z,
        );
        painter.galley(Pos2::new(x, r.top() + 32.0 * z), g, p.text_muted);
        // Hub halves as small link chips.
        if halves.len() > 1 {
            let mut cx = x;
            for h in halves {
                let t = format!("{} link", h.short());
                let gg =
                    painter.layout_no_wrap(t, FontId::proportional(10.0 * z), fade(p.speed(*h), f));
                let cr = Rect::from_min_size(
                    Pos2::new(cx, r.top() + 50.0 * z),
                    gg.size() + Vec2::new(10.0 * z, 4.0 * z),
                );
                painter.rect_filled(cr, CornerRadius::same(255), fade(p.soft(p.speed(*h)), f));
                painter.galley(cr.center() - gg.size() / 2.0, gg, p.text);
                cx = cr.right() + 6.0 * z;
            }
        }
        // "Could be faster" badge
        if physical::explain_speed(snap, phys, port.0, &port.1).is_some() {
            let b = Pos2::new(r.right() - 8.0 * z, r.bottom() - 10.0 * z);
            painter.text(
                b,
                Align2::RIGHT_CENTER,
                format!("{} could be faster", ph::LIGHTNING),
                FontId::proportional(10.5 * z),
                fade(p.slow_lane, f),
            );
        }
    }
}

fn connector_tooltip(ui: &mut Ui, p: &Palette, lanes: &[LaneView]) {
    ui.set_max_width(320.0);
    if lanes.len() > 1 {
        ui.label(RichText::new("One physical socket, two logical ports").strong());
        ui.label(
            RichText::new(
                "Windows lists every USB 3 socket twice: a USB 2 port and a SuperSpeed \"companion\" port. \
                 A device uses exactly one lane – USB 3 devices take the SuperSpeed lane when cable and plug \
                 support it and fall back to the USB 2 lane otherwise.",
            )
            .color(p.text_muted)
            .size(12.0),
        );
    } else {
        ui.label(RichText::new("USB 2-only socket").strong());
        ui.label(
            RichText::new(
                "This port has no SuperSpeed lane, so USB 3 devices run at 480 Mbit/s here.",
            )
            .color(p.text_muted)
            .size(12.0),
        );
    }
    for l in lanes {
        let state = match l.link {
            Some(s) => format!("in use at {}", s.label()),
            None => "free".into(),
        };
        ui.label(
            RichText::new(format!(
                "• Port {} – {} lane, up to {} – {state}",
                l.index,
                if l.max.is_super() {
                    "SuperSpeed"
                } else {
                    "USB 2"
                },
                l.max.short()
            ))
            .size(12.0),
        );
    }
}

fn overlays(app: &mut App, ui: &mut Ui, rect: Rect) {
    let p = app.p;
    // Toolbar (top-right)
    let bar = Rect::from_min_size(
        Pos2::new(rect.right() - 16.0 - 330.0, rect.top() + 14.0),
        Vec2::new(330.0, 40.0),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| {
        egui::Frame::new()
            .fill(p.card)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(6, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if widgets::icon_button(ui, &p, ph::CORNERS_OUT, "Fit to view (F)").clicked() {
                        app.map.needs_fit = true;
                    }
                    if widgets::icon_button(ui, &p, ph::MAGNIFYING_GLASS_MINUS, "Zoom out (-)")
                        .clicked()
                    {
                        zoom_at(app, rect, rect.center(), 1.0 / 1.2);
                    }
                    ui.label(
                        RichText::new(format!("{:.0}%", app.map.zoom * 100.0))
                            .size(12.0)
                            .color(p.text_muted),
                    );
                    if widgets::icon_button(ui, &p, ph::MAGNIFYING_GLASS_PLUS, "Zoom in (+)")
                        .clicked()
                    {
                        zoom_at(app, rect, rect.center(), 1.2);
                    }
                    ui.separator();
                    let empty_tip = if app.settings.map_show_empty {
                        "Hide empty sockets"
                    } else {
                        "Show empty sockets"
                    };
                    if widgets::icon_button(
                        ui,
                        &p,
                        if app.settings.map_show_empty {
                            ph::CIRCLE_DASHED
                        } else {
                            ph::PLUGS_CONNECTED
                        },
                        empty_tip,
                    )
                    .clicked()
                    {
                        app.settings.map_show_empty = !app.settings.map_show_empty;
                        app.settings.save();
                        app.rebuild_map();
                        app.map.needs_fit = true;
                    }
                    let motion_tip = if app.settings.map_motion {
                        "Pause link animation"
                    } else {
                        "Animate links"
                    };
                    if widgets::icon_button(
                        ui,
                        &p,
                        if app.settings.map_motion {
                            ph::PAUSE
                        } else {
                            ph::PLAY
                        },
                        motion_tip,
                    )
                    .clicked()
                    {
                        app.settings.map_motion = !app.settings.map_motion;
                        app.settings.save();
                    }
                    if widgets::icon_button(ui, &p, ph::QUESTION, "What are companion ports?")
                        .clicked()
                    {
                        app.map.help_open = !app.map.help_open;
                    }
                    let tip = if app.settings.map_legend {
                        "Hide legend (L)"
                    } else {
                        "Show legend (L)"
                    };
                    if widgets::toggle_icon_button(
                        ui,
                        &p,
                        ph::LIST_BULLETS,
                        tip,
                        app.settings.map_legend,
                    )
                    .clicked()
                    {
                        toggle_legend(app);
                    }
                });
            });
    });

    // Legend (top-left)
    if app.settings.map_legend {
        let lr = Rect::from_min_size(rect.min + Vec2::new(16.0, 14.0), Vec2::new(250.0, 290.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(lr), |ui| {
            egui::Frame::new().fill(p.card).stroke(Stroke::new(1.0, p.border)).corner_radius(CornerRadius::same(12)).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Legend").font(theme::semibold(13.0)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_button(ui, &p, ph::X, "Hide legend (L)").clicked() {
                            toggle_legend(app);
                        }
                    });
                });
                ui.add_space(2.0);
                for s in [Speed::Low, Speed::Full, Speed::High, Speed::Super, Speed::SuperPlus, Speed::SuperPlus20] {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(28.0, 10.0), Sense::hover());
                        ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(link_width(s), p.speed(s)));
                        ui.label(RichText::new(s.label()).size(11.5).color(p.text_muted));
                    });
                }
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(28.0, 10.0), Sense::hover());
                    for s in Shape::dashed_line(&[r.left_center(), r.right_center()], Stroke::new(1.2, p.idle_link), 4.0, 4.0) {
                        ui.painter().add(s);
                    }
                    ui.label(RichText::new("Idle lane").size(11.5).color(p.text_muted));
                });
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(28.0, 10.0), Sense::hover());
                    ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(6.0, with_alpha(p.slow_lane, 0.4)));
                    ui.label(RichText::new("Device on a slower lane than available").size(11.5).color(p.text_muted));
                });
                ui.add_space(4.0);
                ui.label(RichText::new("Thickness and flow speed follow the link speed. Socket cards list their lanes; hover one to learn more.").size(11.0).color(p.text_faint));
                ui.label(RichText::new("Drag to pan · scroll to zoom · double-click to open details · L hides this").size(11.0).color(p.text_faint));
            });
        });
    }

    // Companion explainer
    if app.map.help_open {
        let w = 520.0_f32.min(rect.width() - 40.0);
        let hr = Rect::from_min_size(
            Pos2::new(rect.center().x - w / 2.0, rect.bottom() - 250.0),
            Vec2::new(w, 230.0),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(hr), |ui| {
            egui::Frame::new().fill(p.card).stroke(Stroke::new(1.0, p.border)).corner_radius(CornerRadius::same(14)).inner_margin(egui::Margin::same(16)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Companion ports, explained").font(theme::semibold(15.0)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_button(ui, &p, ph::X, "Close").clicked() {
                            app.map.help_open = false;
                        }
                        if ui.link("Open the full guide").clicked() {
                            app.map.help_open = false;
                            app.dispatch(Action::OpenLesson(super::learn::Chapter::Companion));
                        }
                    });
                });
                // Diagram: controller -> two lanes -> one socket
                let (d, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 90.0), Sense::hover());
                let t = ui.input(|i| i.time);
                let ctrl = Rect::from_center_size(Pos2::new(d.left() + 50.0, d.center().y), Vec2::new(80.0, 44.0));
                let sock = Rect::from_center_size(Pos2::new(d.right() - 70.0, d.center().y), Vec2::new(110.0, 70.0));
                ui.painter().rect_filled(ctrl, CornerRadius::same(8), p.soft(p.accent));
                ui.painter().text(ctrl.center(), Align2::CENTER_CENTER, "Controller", FontId::proportional(12.0), p.accent);
                ui.painter().rect_stroke(sock, CornerRadius::same(10), Stroke::new(1.0, p.border), StrokeKind::Inside);
                ui.painter().text(Pos2::new(sock.center().x, sock.top() + 11.0), Align2::CENTER_CENTER, "one physical socket", FontId::proportional(10.5), p.text_muted);
                for (k, (s, name)) in [(Speed::High, "USB 2 lane · port 3"), (Speed::Super, "USB 3 lane · port 9")].iter().enumerate() {
                    let y = sock.top() + 32.0 + k as f32 * 22.0;
                    let row = Rect::from_min_size(Pos2::new(sock.left() + 4.0, y - 9.0), Vec2::new(sock.width() - 8.0, 18.0));
                    ui.painter().rect_filled(row, CornerRadius::same(5), p.soft(p.speed(*s)));
                    ui.painter().text(row.center(), Align2::CENTER_CENTER, *name, FontId::proportional(10.5), p.text);
                    draw_link(ui, &p, Pos2::new(ctrl.right(), ctrl.center().y - 8.0 + k as f32 * 16.0), Pos2::new(row.left(), y), Some(*s), 1.0, t, true, 1.0, false);
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "USB 3 kept USB 2's wires and added new ones, so the controller sees two separate ports behind each blue socket. \
                         Windows (and USBTreeView) list them separately – the SuperSpeed port is the USB 2 port's \"companion\". \
                         A device always uses one lane: if a USB 3 device shows up on the USB 2 lane, the cable, an adapter or a \
                         half-inserted plug is usually to blame.",
                    )
                    .size(12.0)
                    .color(p.text_muted),
                );
            });
        });
    }

    inspector(app, ui, rect);
}

fn inspector(app: &mut App, ui: &mut Ui, rect: Rect) {
    let p = app.p;
    let Some(i) = app.map.selected else { return };
    let Some(n) = app.map.layout.nodes.get(i).cloned() else {
        return;
    };
    let Some(snap) = app.snapshot.clone() else {
        return;
    };
    let w = 330.0;
    let ir = Rect::from_min_size(
        Pos2::new(rect.right() - w - 16.0, rect.top() + 66.0),
        Vec2::new(w, rect.height() - 90.0),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(ir), |ui| {
        egui::Frame::new()
            .fill(p.card)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(CornerRadius::same(14))
            .shadow(egui::Shadow { offset: [0, 10], blur: 30, spread: 0, color: Color32::from_black_alpha(if p.dark { 110 } else { 35 }) })
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.set_width(w - 28.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(match n.kind {
                        MKind::Controller(_) => "HOST CONTROLLER",
                        MKind::Connector { .. } => "SOCKET",
                        MKind::Device { .. } => "DEVICE",
                    }).size(10.5).color(p.text_faint).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_button(ui, &p, ph::X, "Close (Esc)").clicked() {
                            app.map.selected = None;
                        }
                    });
                });
                let title = app.flat.get(&n.id).map(|f| app.display_label(f)).unwrap_or(n.label.clone());
                ui.add(egui::Label::new(RichText::new(title).font(theme::semibold(16.0))).wrap());
                ui.add_space(4.0);
                match &n.kind {
                    MKind::Device { port, halves } => {
                        let phys = Physical::build(&snap);
                        if let Some(d) = tree::port(&snap, port.0, &port.1).and_then(|pp| pp.device.as_ref()) {
                            ui.horizontal_wrapped(|ui| {
                                widgets::chip(ui, &p, ph::LIGHTNING, d.speed.label(), p.speed(d.speed));
                                if let Some((v, pi)) = d.vid_pid() {
                                    widgets::chip(ui, &p, "", &format!("{v:04X}:{pi:04X}"), p.text_muted);
                                }
                            });
                            if halves.len() > 1 {
                                ui.label(RichText::new("A USB 3 hub is two hubs in one box: a USB 2 hub and a SuperSpeed hub, each linked through its own lane of the upstream socket. The map shows them merged.").size(12.0).color(p.text_muted));
                            }
                            if let Some(li) = physical::link_info(&snap, &phys, port.0, &port.1) {
                                ui.add_space(6.0);
                                widgets::speed_ladder(ui, &p, d.speed, li.best_possible, d.max_capable_speed().max(d.speed), li.connector_max, li.path_limit.as_ref().map(|x| x.0));
                            }
                            if let Some(why) = physical::explain_speed(&snap, &phys, port.0, &port.1) {
                                ui.add_space(4.0);
                                ui.add(egui::Label::new(RichText::new(format!("{}  {why}", ph::LIGHTBULB)).size(12.0).color(p.warn)).wrap());
                            }
                            for ins in app.insights.iter().filter(|x| x.node_id == n.id && !x.title.contains("could be faster")) {
                                ui.add(egui::Label::new(RichText::new(format!("{}  {}", ph::WARNING, ins.title)).size(12.0).color(p.warn)).wrap());
                            }
                        }
                    }
                    MKind::Connector { lanes, .. } => connector_tooltip(ui, &p, lanes),
                    MKind::Controller(ci) => {
                        let c = &snap.controllers[*ci];
                        ui.label(RichText::new(c.kind()).color(p.text_muted));
                        let n_dev = app.map.layout.nodes.iter().filter(|m| matches!(m.kind, MKind::Device { .. }) && root_of(&app.map.layout, m) == i).count();
                        ui.label(RichText::new(format!("{n_dev} devices")).color(p.text_muted));
                    }
                }
                ui.add_space(8.0);
                let id = n.id.trim_start_matches("conn:").to_string();
                ui.horizontal(|ui| {
                    if app.flat.by_id.contains_key(&id) && widgets::action_button(ui, &p, ph::ARROW_SQUARE_OUT, "Open details", false).clicked() {
                        app.view = ViewMode::Tree;
                        app.settings.map_view = false;
                        app.dispatch(Action::Select(id.clone()));
                    }
                    if let MKind::Device { port, .. } = &n.kind {
                        if app.is_live() {
                            if let Some(info) = tree::port(&snap, port.0, &port.1).and_then(|pp| pp.device.as_ref()).and_then(|d| d.info.as_ref()) {
                                if !info.instance_id.is_empty() && widgets::action_button(ui, &p, ph::EJECT, "Eject", false).clicked() {
                                    app.dispatch(Action::SafelyRemove { id: info.instance_id.clone(), name: n.label.clone() });
                                }
                            }
                        }
                    }
                });
            });
    });
}

fn root_of(l: &MapLayout, n: &MNode) -> usize {
    let mut cur = l.nodes.iter().position(|x| x.id == n.id).unwrap_or(0);
    while let Some(p) = l.nodes[cur].parent {
        cur = p;
    }
    cur
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_has_no_overlaps_and_merges_hub_halves() {
        let s = crate::demo::snapshot();
        let l = build_layout(&s, true);
        let devices: Vec<_> = l
            .nodes
            .iter()
            .filter(|n| matches!(n.kind, MKind::Device { .. }))
            .collect();
        // 8 devices in the demo, but the USB 3 hub's two halves become one node.
        assert_eq!(devices.len(), 9);
        assert!(devices
            .iter()
            .any(|n| matches!(&n.kind, MKind::Device { halves, .. } if halves.len() == 2)));
        for (i, a) in l.nodes.iter().enumerate() {
            for b in l.nodes.iter().skip(i + 1) {
                assert!(
                    !a.rect.shrink(1.0).intersects(b.rect.shrink(1.0)),
                    "{} overlaps {}",
                    a.label,
                    b.label
                );
            }
        }
        // Children are to the right of parents.
        for n in &l.nodes {
            if let Some(pi) = n.parent {
                assert!(n.rect.left() > l.nodes[pi].rect.right());
            }
        }
    }

    #[test]
    fn hiding_empty_sockets_shrinks_map() {
        let s = crate::demo::snapshot();
        assert!(build_layout(&s, false).nodes.len() < build_layout(&s, true).nodes.len());
    }
}
