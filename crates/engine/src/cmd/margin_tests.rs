use figcraft_color::{Color, Paint};
use figcraft_doc::{HConstraint, Node, NodeId, Sizing, is_margin_wrapper};
use serde_json::{Value, json};

use crate::{EngineError, Session, params};

fn id(v: &Value) -> NodeId {
    params::id_of(&v["id"]).unwrap()
}

fn ex(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

fn node(s: &Session, id: NodeId) -> Node {
    s.doc().unwrap().node(id).unwrap().clone()
}

fn bounds(s: &Session, id: NodeId) -> [f64; 4] {
    let b = s.doc().unwrap().world_bounds(id).unwrap();
    [b.x0, b.y0, b.width(), b.height()]
}

fn wrapper(r: &Value) -> NodeId {
    params::id_of(&r["wrapper"]).unwrap()
}

fn get(s: &mut Session, id: NodeId) -> Value {
    ex(s, "layout.getMargin", json!({"id": id}))
}

/// A horizontal auto layout frame (padding 0, gap 10) with two 40×20 rectangles.
fn al_frame(s: &mut Session) -> (NodeId, NodeId, NodeId) {
    let f = id(&ex(s, "frame.create", json!({"x": 0, "y": 0, "width": 300, "height": 100, "props": {"layout": "horizontal", "gap": 10}})));
    let a = id(&ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 40, "height": 20, "parent": f})));
    let b = id(&ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 40, "height": 20, "parent": f})));
    (f, a, b)
}

#[test]
fn set_get_update_and_remove() {
    let mut s = Session::new();
    let (f, a, b) = al_frame(&mut s);
    let r = ex(&mut s, "layout.setMargin", json!({"id": a, "margin": [4, 3, 2, 1]}));
    let w = wrapper(&r);
    let wn = node(&s, w);
    assert!(wn.margin_wrapper && is_margin_wrapper(s.doc().unwrap(), w));
    assert_eq!(wn.name, "Rectangle · margin");
    assert!(wn.fills.is_empty() && !wn.clip);
    assert_eq!(wn.layout.as_ref().unwrap().padding, [4.0, 3.0, 2.0, 1.0]);
    assert_eq!((wn.child_layout.sizing_h, wn.child_layout.sizing_v), (Sizing::Hug, Sizing::Hug));
    assert_eq!(wn.children, vec![a]);
    assert_eq!(node(&s, f).children, vec![w, b], "the wrapper takes the child's place in the order");
    assert_eq!(s.selection(), vec![a], "the child stays selected");
    // The wrapper hugs the child plus the margin; the next child moves along.
    assert_eq!(bounds(&s, w), [0.0, 0.0, 44.0, 26.0]);
    assert_eq!(bounds(&s, a), [1.0, 4.0, 40.0, 20.0]);
    assert_eq!(bounds(&s, b)[0], 54.0);
    // getMargin from the child or the wrapper.
    for target in [a, w] {
        let g = get(&mut s, target);
        assert_eq!(g["margin"], json!([4.0, 3.0, 2.0, 1.0]));
        assert_eq!(params::id_of(&g["wrapper"]), Some(w));
        assert_eq!(params::id_of(&g["child"]), Some(a));
    }
    assert_eq!(get(&mut s, b), json!({"margin": [0.0, 0.0, 0.0, 0.0], "wrapper": null, "child": b}));
    // Update: one number for every side, through the child or the wrapper; no second wrapper.
    ex(&mut s, "layout.setMargin", json!({"id": a, "margin": 8}));
    ex(&mut s, "layout.setMargin", json!({"id": w, "margin": [8, 8, 8, "9"]}));
    assert_eq!(node(&s, w).layout.unwrap().padding, [8.0, 8.0, 8.0, 9.0]);
    assert_eq!(node(&s, w).children, vec![a]);
    assert_eq!(node(&s, f).children, vec![w, b]);
    // A renamed child renames the wrapper on the next margin edit.
    ex(&mut s, "node.set", json!({"id": a, "props": {"name": "Card"}}));
    ex(&mut s, "layout.setMargin", json!({"id": a, "margin": 6}));
    assert_eq!(node(&s, w).name, "Card · margin");
    // 0 on every side removes the plain wrapper.
    let r = ex(&mut s, "layout.setMargin", json!({"id": a, "margin": [0, 0, 0, 0]}));
    assert_eq!(r["wrapper"], Value::Null);
    assert!(!s.doc().unwrap().contains(w));
    assert_eq!(node(&s, f).children, vec![a, b]);
    assert_eq!(bounds(&s, a), [0.0, 0.0, 40.0, 20.0]);
    assert_eq!(bounds(&s, b)[0], 50.0);
    assert_eq!(s.selection(), vec![a]);
    // 0 without a wrapper changes nothing.
    ex(&mut s, "layout.setMargin", json!({"id": a, "margin": 0}));
    assert_eq!(node(&s, f).children, vec![a, b]);
}

#[test]
fn values_are_clamped_and_junk_refused() {
    let mut s = Session::new();
    let (_, a, _) = al_frame(&mut s);
    let r = ex(&mut s, "layout.setMargin", json!({"id": a, "margin": [-5, 1e308, 2, 0]}));
    assert_eq!(node(&s, wrapper(&r)).layout.unwrap().padding, [0.0, 1e6, 2.0, 0.0]);
    let undo = s.ds().unwrap().history.undo.len();
    for bad in [json!({"id": a}), json!({"id": a, "margin": [1, 2]}), json!({"id": a, "margin": [1, 2, 3, "x"]}), json!({"id": a, "margin": "wide"})]
    {
        assert!(matches!(s.execute("layout.setMargin", &bad), Err(EngineError::BadParams { .. })), "{bad}");
    }
    assert!(s.execute("layout.setMargin", &json!({"id": 99999, "margin": 1})).is_err());
    assert!(s.execute("layout.getMargin", &json!({"id": "nope"})).is_err());
    assert_eq!(s.ds().unwrap().history.undo.len(), undo);
}

#[test]
fn only_auto_layout_children_get_a_margin() {
    let mut s = Session::new();
    let f = id(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 100, "height": 100})));
    let a = id(&ex(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 10, "height": 10, "parent": f})));
    let loose = id(&ex(&mut s, "shape.rectangle", json!({"x": 200, "y": 0, "width": 10, "height": 10})));
    let page = node(&s, loose).parent.unwrap();
    for target in [a, loose, page] {
        let e = s.execute("layout.setMargin", &json!({"id": target, "margin": 4}));
        assert!(matches!(e, Err(EngineError::BadParams { .. })), "{target}: {e:?}");
    }
    assert_eq!(node(&s, f).children, vec![a]);
}

#[test]
fn undo_restores_the_document_exactly() {
    let mut s = Session::new();
    let (_, a, _) = al_frame(&mut s);
    ex(&mut s, "node.set", json!({"id": a, "props": {"sizingH": "fill", "minWidth": 30, "constraints": {"horizontal": "right"}}}));
    let before = serde_json::to_value(s.doc().unwrap()).unwrap();
    ex(&mut s, "layout.setMargin", json!({"id": a, "margin": [1, 2, 3, 4]}));
    let wrapped = serde_json::to_value(s.doc().unwrap()).unwrap();
    assert_ne!(before, wrapped);
    s.undo().unwrap();
    assert_eq!(serde_json::to_value(s.doc().unwrap()).unwrap(), before);
    s.redo().unwrap();
    assert_eq!(serde_json::to_value(s.doc().unwrap()).unwrap(), wrapped);
    // Removing it is one undo step too.
    ex(&mut s, "layout.setMargin", json!({"id": a, "margin": 0}));
    s.undo().unwrap();
    assert_eq!(serde_json::to_value(s.doc().unwrap()).unwrap(), wrapped);
}

#[test]
fn fill_moves_to_the_wrapper_and_back() {
    let mut s = Session::new();
    let (f, a, b) = al_frame(&mut s);
    ex(&mut s, "node.set", json!({"id": a, "props": {"sizingH": "fill", "sizingV": "fill"}}));
    let w = wrapper(&ex(&mut s, "layout.setMargin", json!({"id": a, "margin": 5})));
    let (wn, an) = (node(&s, w), node(&s, a));
    assert_eq!((wn.child_layout.sizing_h, wn.child_layout.sizing_v), (Sizing::Fill, Sizing::Fill));
    assert_eq!((an.child_layout.sizing_h, an.child_layout.sizing_v), (Sizing::Fill, Sizing::Fill), "the child fills the wrapper");
    // The wrapper takes the space the child had; the child fills it inside the margin.
    assert_eq!(bounds(&s, w), [0.0, 0.0, 250.0, 100.0]);
    assert_eq!(bounds(&s, a), [5.0, 5.0, 240.0, 90.0]);
    assert_eq!(bounds(&s, b)[0], 260.0);
    ex(&mut s, "layout.setMargin", json!({"id": a, "margin": 0}));
    let an = node(&s, a);
    assert_eq!((an.child_layout.sizing_h, an.child_layout.sizing_v), (Sizing::Fill, Sizing::Fill));
    assert_eq!(bounds(&s, a), [0.0, 0.0, 250.0, 100.0]);
    assert_eq!(node(&s, f).children, vec![a, b]);
}

#[test]
fn a_child_that_only_filled_the_wrapper_keeps_its_size() {
    let mut s = Session::new();
    let (_, a, _) = al_frame(&mut s);
    let w = wrapper(&ex(&mut s, "layout.setMargin", json!({"id": a, "margin": 5})));
    // The wrapper made fixed, the child filling it: removing the margin can't fill the parent.
    ex(&mut s, "node.set", json!({"id": w, "props": {"sizingH": "fixed", "width": 100}}));
    ex(&mut s, "node.set", json!({"id": a, "props": {"sizingH": "fill"}}));
    assert_eq!(node(&s, a).width, 90.0);
    ex(&mut s, "layout.setMargin", json!({"id": a, "margin": 0}));
    let an = node(&s, a);
    assert_eq!((an.child_layout.sizing_h, an.width), (Sizing::Fixed, 90.0));
}

#[test]
fn absolute_position_and_constraints_move_to_the_wrapper_and_back() {
    let mut s = Session::new();
    let (_, a, b) = al_frame(&mut s);
    ex(&mut s, "node.set", json!({"id": b, "props": {"absolute": true, "constraints": {"horizontal": "right"}}}));
    ex(&mut s, "node.set", json!({"id": b, "props": {"x": 200, "y": 50}}));
    let at = bounds(&s, b);
    let w = wrapper(&ex(&mut s, "layout.setMargin", json!({"id": b, "margin": [4, 0, 0, 6]})));
    let (wn, bn) = (node(&s, w), node(&s, b));
    assert!(wn.child_layout.absolute && !bn.child_layout.absolute);
    assert_eq!((wn.constraints.horizontal, bn.constraints.horizontal), (HConstraint::Right, HConstraint::Left));
    assert_eq!(bounds(&s, w)[..2], at[..2], "the wrapper sits where the child was");
    assert_eq!(bounds(&s, b), [at[0] + 6.0, at[1] + 4.0, at[2], at[3]], "the margin pushes the child in");
    assert_eq!(bounds(&s, a)[0], 0.0, "the flow ignores the absolute wrapper");
    ex(&mut s, "layout.setMargin", json!({"id": b, "margin": 0}));
    let bn = node(&s, b);
    assert!(bn.child_layout.absolute);
    assert_eq!(bn.constraints.horizontal, HConstraint::Right);
    assert_eq!(bounds(&s, b), at, "back where it was");
}

#[test]
fn grid_placement_moves_to_the_wrapper_and_back() {
    let mut s = Session::new();
    let f = id(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 200, "height": 200, "props": {"layout": "grid"}})));
    let a = id(&ex(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 20, "height": 20, "parent": f})));
    ex(&mut s, "node.set", json!({"id": a, "props": {"gridColumn": 1, "gridRow": 1, "gridColumnSpan": 1, "gridAlignH": "center"}}));
    let cl = node(&s, a).child_layout;
    let w = wrapper(&ex(&mut s, "layout.setMargin", json!({"id": a, "margin": 3})));
    let (wcl, acl) = (node(&s, w).child_layout, node(&s, a).child_layout);
    assert_eq!(
        (wcl.grid_column, wcl.grid_row, wcl.grid_column_span, wcl.grid_align_h),
        (cl.grid_column, cl.grid_row, cl.grid_column_span, cl.grid_align_h)
    );
    assert_eq!((acl.grid_column, acl.grid_row, acl.grid_align_h), (None, None, None));
    ex(&mut s, "layout.setMargin", json!({"id": w, "margin": 0}));
    assert_eq!(node(&s, a).child_layout, cl);
}

#[test]
fn a_wrapper_given_a_look_of_its_own_stays_as_a_frame() {
    let mut s = Session::new();
    let (f, a, b) = al_frame(&mut s);
    let w = wrapper(&ex(&mut s, "layout.setMargin", json!({"id": a, "margin": 5})));
    ex(&mut s, "node.set", json!({"id": w, "props": {"fill": "#ff0000", "opacity": 0.5}}));
    let r = ex(&mut s, "layout.setMargin", json!({"id": a, "margin": 0}));
    assert_eq!(params::id_of(&r["kept"]), Some(w));
    let wn = node(&s, w);
    assert!(!wn.margin_wrapper && !is_margin_wrapper(s.doc().unwrap(), w));
    assert_eq!(wn.name, "Rectangle");
    assert_eq!(wn.layout.as_ref().unwrap().padding, [0.0; 4]);
    assert_eq!((wn.fills, wn.opacity, wn.children), (vec![Paint::solid(Color::from_rgb8_const(255, 0, 0))], 0.5, vec![a]), "the rest is untouched");
    assert_eq!(node(&s, f).children, vec![w, b]);
    assert_eq!(get(&mut s, a)["wrapper"], Value::Null);
    // A wrapper given a second layer is kept the same way.
    let w2 = wrapper(&ex(&mut s, "layout.setMargin", json!({"id": b, "margin": 5})));
    ex(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 5, "height": 5, "parent": w2}));
    ex(&mut s, "layout.setMargin", json!({"id": w2, "margin": 0}));
    assert!(s.doc().unwrap().contains(w2));
    assert_eq!(node(&s, w2).children.len(), 2);
    assert!(!node(&s, w2).margin_wrapper);
}

#[test]
fn renders_like_a_frame_with_padding_built_by_hand() {
    let margin = [5.0, 6.0, 7.0, 8.0];
    let mut s = Session::new();
    let (f, a, b) = al_frame(&mut s);
    let w = wrapper(&ex(&mut s, "layout.setMargin", json!({"id": a, "margin": margin})));
    let mut h = Session::new();
    let (hf, ha, hb) = al_frame(&mut h);
    let hw = id(&ex(&mut h, "object.frameSelection", json!({"ids": [ha]})));
    ex(&mut h, "node.set", json!({"id": hw, "props": {"layout": "horizontal", "gap": 0, "padding": margin, "fills": [], "clipsContent": false}}));
    ex(&mut h, "node.set", json!({"id": hw, "props": {"sizingH": "hug", "sizingV": "hug"}}));
    for (x, y) in [(w, hw), (a, ha), (b, hb)] {
        assert_eq!(bounds(&s, x), bounds(&h, y));
    }
    let pixels = |s: &Session, f| figcraft_render::render_node(s.doc().unwrap(), f, 1.0).unwrap().data().to_vec();
    assert_eq!(pixels(&s, f), pixels(&h, hf));
}

#[test]
fn min_max_stay_on_the_child() {
    // Known limitation: min/max stay on the child, so they bound the child, not the child plus
    // its margin. A filling child with a max width leaves the wrapper wider than max + margin.
    let mut s = Session::new();
    let (_, a, _) = al_frame(&mut s);
    ex(&mut s, "node.set", json!({"id": a, "props": {"sizingH": "fill", "maxWidth": 100}}));
    assert_eq!(node(&s, a).width, 100.0);
    let w = wrapper(&ex(&mut s, "layout.setMargin", json!({"id": a, "margin": 10})));
    let (wn, an) = (node(&s, w), node(&s, a));
    assert_eq!((wn.child_layout.max_w, an.child_layout.max_w), (None, Some(100.0)));
    assert_eq!(an.width, 100.0, "the child keeps its max");
    assert_eq!(wn.width, 250.0, "the wrapper fills the free space, past max + margin (120)");
}

#[test]
fn inside_instances_only_the_padding_changes() {
    let mut s = Session::new();
    let (f, a, b) = al_frame(&mut s);
    let w = wrapper(&ex(&mut s, "layout.setMargin", json!({"id": a, "margin": 4})));
    let comp = id(&ex(&mut s, "component.create", json!({"ids": [f]})));
    let inst = id(&ex(&mut s, "instance.create", json!({"component": comp, "x": 0, "y": 300})));
    let kids = node(&s, inst).children;
    let (iw, ib) = (kids[0], kids[1]);
    assert_eq!(node(&s, iw).source, Some(w));
    let ia = node(&s, iw).children[0];
    ex(&mut s, "layout.setMargin", json!({"id": ia, "margin": 9}));
    let iwn = node(&s, iw);
    assert_eq!(iwn.layout.as_ref().unwrap().padding, [9.0; 4]);
    assert!(iwn.overrides.contains("padding"));
    assert_eq!(node(&s, w).layout.unwrap().padding, [4.0; 4], "the main keeps its margin");
    // 0 keeps the wrapper (layers can't come and go in an instance); new wrappers are refused.
    ex(&mut s, "layout.setMargin", json!({"id": ia, "margin": 0}));
    assert!(s.doc().unwrap().contains(iw));
    assert!(matches!(s.execute("layout.setMargin", &json!({"id": ib, "margin": 3})), Err(EngineError::BadParams { .. })));
    let _ = b;
}
