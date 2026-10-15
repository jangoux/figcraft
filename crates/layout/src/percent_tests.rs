//! Fixed % sizing: a layer keeps its width / height as a percentage of its parent's.

use super::tests::pos;
use super::*;
use figcraft_doc::{AutoLayout, Document, NodeKind};

fn frame(d: &mut Document, parent: NodeId, w: f64, h: f64) -> NodeId {
    d.create(parent, NodeKind::Frame, "F", 0.0, 0.0, w, h).unwrap()
}

fn rect(d: &mut Document, parent: NodeId, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    d.create(parent, NodeKind::Rectangle, "R", x, y, w, h).unwrap()
}

fn percent(d: &mut Document, id: NodeId, w: Option<f64>, h: Option<f64>) {
    let cl = &mut d.get_mut(id).unwrap().child_layout;
    cl.percent_w = w;
    cl.percent_h = h;
}

fn auto_layout(d: &mut Document, id: NodeId, mode: LayoutMode, padding: [f64; 4]) {
    d.get_mut(id).unwrap().layout = Some(AutoLayout { mode, gap: 10.0, padding, ..Default::default() });
}

#[test]
fn a_percent_child_of_a_regular_frame_follows_its_parent() {
    let mut d = Document::new("t");
    let p = d.first_page().unwrap();
    let f = frame(&mut d, p, 200.0, 100.0);
    let r = rect(&mut d, f, 10.0, 10.0, 20.0, 20.0);
    percent(&mut d, r, Some(50.0), Some(25.0));
    assert!(apply_percent(&mut d, r, &NoMeasure));
    assert_eq!(pos(&d, r), (10.0, 10.0, 100.0, 25.0));
    // Applying again changes nothing.
    assert!(!apply_percent(&mut d, r, &NoMeasure));
    // The parent grows: the child keeps its share; its left/top constraints keep the corner.
    set_size(&mut d, f, 400.0, 200.0);
    apply_constraints(&mut d, f, 200.0, 100.0);
    assert_eq!(pos(&d, r), (10.0, 10.0, 200.0, 50.0));
}

#[test]
fn constraints_hold_their_edge_while_the_percentage_sizes() {
    let mut d = Document::new("t");
    let p = d.first_page().unwrap();
    let f = frame(&mut d, p, 200.0, 100.0);
    // Right edge 10 from the frame's right; centred vertically.
    let r = rect(&mut d, f, 90.0, 40.0, 100.0, 20.0);
    d.get_mut(r).unwrap().constraints.horizontal = HConstraint::Right;
    d.get_mut(r).unwrap().constraints.vertical = VConstraint::Center;
    percent(&mut d, r, Some(50.0), Some(20.0));
    set_size(&mut d, f, 300.0, 200.0);
    apply_constraints(&mut d, f, 200.0, 100.0);
    // 150 wide, right edge at 290; 40 high, centred in 200.
    assert_eq!(pos(&d, r), (140.0, 80.0, 150.0, 40.0));
}

#[test]
fn only_the_percent_axis_follows_the_parent() {
    let mut d = Document::new("t");
    let p = d.first_page().unwrap();
    let f = frame(&mut d, p, 200.0, 100.0);
    let r = rect(&mut d, f, 0.0, 0.0, 100.0, 30.0);
    percent(&mut d, r, Some(50.0), None);
    set_size(&mut d, f, 100.0, 300.0);
    apply_constraints(&mut d, f, 200.0, 100.0);
    // Height: Top constraint, unchanged.
    assert_eq!(pos(&d, r), (0.0, 0.0, 50.0, 30.0));
}

#[test]
fn auto_layout_children_take_a_share_of_the_inner_size() {
    let mut d = Document::new("t");
    let p = d.first_page().unwrap();
    let f = frame(&mut d, p, 220.0, 120.0);
    auto_layout(&mut d, f, LayoutMode::Horizontal, [10.0, 10.0, 10.0, 10.0]);
    let a = rect(&mut d, f, 0.0, 0.0, 20.0, 20.0);
    let b = rect(&mut d, f, 0.0, 0.0, 20.0, 20.0);
    d.get_mut(b).unwrap().child_layout.sizing_h = Sizing::Fill;
    percent(&mut d, a, Some(25.0), Some(50.0));
    layout_frame(&mut d, f, &NoMeasure);
    // Inner 200 x 100: a is 50 x 50, b fills the rest (200 - 50 - 10 gap).
    assert_eq!(pos(&d, a), (10.0, 10.0, 50.0, 50.0));
    assert_eq!(pos(&d, b), (70.0, 10.0, 140.0, 20.0));
    // The frame widens: the percentage follows.
    set_size(&mut d, f, 420.0, 120.0);
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(pos(&d, a), (10.0, 10.0, 100.0, 50.0));
}

#[test]
fn min_and_max_limit_the_percent_size() {
    let mut d = Document::new("t");
    let p = d.first_page().unwrap();
    let f = frame(&mut d, p, 400.0, 100.0);
    auto_layout(&mut d, f, LayoutMode::Horizontal, [0.0; 4]);
    let a = rect(&mut d, f, 0.0, 0.0, 20.0, 20.0);
    percent(&mut d, a, Some(50.0), None);
    d.get_mut(a).unwrap().child_layout.max_w = Some(120.0);
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(pos(&d, a).2, 120.0);
}

#[test]
fn an_absolute_child_measures_the_whole_frame() {
    let mut d = Document::new("t");
    let p = d.first_page().unwrap();
    let f = frame(&mut d, p, 200.0, 100.0);
    auto_layout(&mut d, f, LayoutMode::Horizontal, [10.0; 4]);
    let a = rect(&mut d, f, 0.0, 0.0, 20.0, 20.0);
    d.get_mut(a).unwrap().child_layout.absolute = true;
    percent(&mut d, a, Some(100.0), None);
    assert_eq!(d.percent_basis(a), Some((200.0, 100.0)));
    assert!(apply_percent(&mut d, a, &NoMeasure));
    assert_eq!(pos(&d, a).2, 200.0);
}

#[test]
fn coherent_sizing_drops_percentages_that_cant_apply() {
    let mut d = Document::new("t");
    let p = d.first_page().unwrap();
    // On the page: no parent frame to measure.
    let top = rect(&mut d, p, 0.0, 0.0, 20.0, 20.0);
    percent(&mut d, top, Some(50.0), None);
    assert!(coherent_sizing(&mut d, top));
    assert_eq!(d.node(top).unwrap().child_layout.percent_w, None);
    // In an auto layout frame that hugs its width: no width to share; the height stays.
    let f = frame(&mut d, p, 200.0, 100.0);
    auto_layout(&mut d, f, LayoutMode::Horizontal, [0.0; 4]);
    d.get_mut(f).unwrap().child_layout.sizing_h = Sizing::Hug;
    let a = rect(&mut d, f, 0.0, 0.0, 20.0, 20.0);
    percent(&mut d, a, Some(50.0), Some(50.0));
    coherent_sizing(&mut d, a);
    let cl = &d.node(a).unwrap().child_layout;
    assert_eq!((cl.percent_w, cl.percent_h), (None, Some(50.0)));
    // A Fill axis isn't Fixed: no percentage there.
    d.get_mut(f).unwrap().child_layout.sizing_h = Sizing::Fixed;
    percent(&mut d, a, Some(50.0), Some(50.0));
    d.get_mut(a).unwrap().child_layout.sizing_v = Sizing::Fill;
    coherent_sizing(&mut d, a);
    let cl = &d.node(a).unwrap().child_layout;
    assert_eq!((cl.percent_w, cl.percent_h), (Some(50.0), None));
}

#[test]
fn junk_percentages_never_make_junk_sizes() {
    let mut d = Document::new("t");
    let p = d.first_page().unwrap();
    let f = frame(&mut d, p, 200.0, 100.0);
    let r = rect(&mut d, f, 0.0, 0.0, 20.0, 20.0);
    for v in [f64::NAN, f64::INFINITY, -50.0, 1e300] {
        percent(&mut d, r, Some(v), Some(v));
        apply_percent(&mut d, r, &NoMeasure);
        let (_, _, w, h) = pos(&d, r);
        assert!(w.is_finite() && h.is_finite() && w >= 0.0 && h >= 0.0, "{v}: {w} x {h}");
    }
    // A nested frame (a group's bounds follow its children: no basis).
    let g = d.create(f, NodeKind::Group, "G", 0.0, 0.0, 10.0, 10.0).unwrap();
    let k = rect(&mut d, g, 0.0, 0.0, 5.0, 5.0);
    assert_eq!(d.percent_basis(k), None);
}
