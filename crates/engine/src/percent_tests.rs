//! Fixed % through commands: setting it, following the parent, edits that resize the layer
//! itself, instances, inspect and code.

use serde_json::{Value, json};

use crate::Session;
use crate::params::id_of;
use figcraft_doc::{NodeId, Sizing};

fn ex(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap()
}

fn id(v: Value) -> NodeId {
    id_of(&v["id"]).unwrap()
}

fn frame(s: &mut Session, parent: Option<NodeId>, w: f64, h: f64) -> NodeId {
    let mut p = json!({"x": 0, "y": 0, "width": w, "height": h});
    if let Some(parent) = parent {
        p["parent"] = json!(parent);
    }
    id(ex(s, "frame.create", p))
}

fn rect(s: &mut Session, parent: NodeId, w: f64, h: f64) -> NodeId {
    id(ex(s, "shape.rectangle", json!({"parent": parent, "x": 10, "y": 10, "width": w, "height": h})))
}

fn set(s: &mut Session, node: NodeId, props: Value) {
    ex(s, "node.set", json!({"id": node, "props": props}));
}

fn size(s: &Session, node: NodeId) -> (f64, f64) {
    let n = s.doc().unwrap().node(node).unwrap();
    (n.width, n.height)
}

fn percent(s: &Session, node: NodeId) -> (Option<f64>, Option<f64>) {
    let cl = &s.doc().unwrap().node(node).unwrap().child_layout;
    (cl.percent_w, cl.percent_h)
}

#[test]
fn a_percentage_sizes_the_layer_and_follows_its_parent() {
    let mut s = Session::new();
    let f = frame(&mut s, None, 200.0, 100.0);
    let r = rect(&mut s, f, 20.0, 20.0);
    set(&mut s, r, json!({"widthPercent": 50, "heightPercent": "25%"}));
    assert_eq!(size(&s, r), (100.0, 25.0));
    assert_eq!(percent(&s, r), (Some(50.0), Some(25.0)));
    set(&mut s, f, json!({"width": 400, "height": 40}));
    assert_eq!(size(&s, r), (200.0, 10.0));
    // One undo puts the frame and the layer back.
    s.undo().unwrap();
    assert_eq!(size(&s, r), (100.0, 25.0));
    // Null goes back to px at the current size.
    set(&mut s, r, json!({"widthPercent": null}));
    set(&mut s, f, json!({"width": 300}));
    assert_eq!((size(&s, r).0, percent(&s, r).0), (100.0, None));
}

#[test]
fn resizing_the_layer_itself_changes_its_percentage() {
    let mut s = Session::new();
    let f = frame(&mut s, None, 200.0, 100.0);
    let r = rect(&mut s, f, 20.0, 20.0);
    set(&mut s, r, json!({"widthPercent": 50}));
    // A typed width (or a drag) is a new share of the same parent.
    set(&mut s, r, json!({"width": 50}));
    assert_eq!(percent(&s, r).0, Some(25.0));
    assert_eq!(s.doc().unwrap().node(r).unwrap().child_layout.sizing_h, Sizing::Fixed);
    set(&mut s, f, json!({"width": 400}));
    assert_eq!(size(&s, r).0, 100.0);
}

#[test]
fn sizing_percent_keeps_the_current_size_and_other_sizings_clear_it() {
    let mut s = Session::new();
    let f = frame(&mut s, None, 200.0, 100.0);
    let r = rect(&mut s, f, 50.0, 20.0);
    set(&mut s, r, json!({"sizingH": "percent", "sizingV": "Fixed %"}));
    assert_eq!(percent(&s, r), (Some(25.0), Some(20.0)));
    assert_eq!(size(&s, r), (50.0, 20.0));
    set(&mut s, r, json!({"sizingH": "fixed"}));
    assert_eq!(percent(&s, r), (None, Some(20.0)));
}

#[test]
fn auto_layout_children_share_the_inner_size_and_a_hugging_parent_fixes() {
    let mut s = Session::new();
    let f = frame(&mut s, None, 220.0, 100.0);
    set(&mut s, f, json!({"layout": "horizontal", "padding": 10, "gap": 0, "sizingH": "hug"}));
    let a = rect(&mut s, f, 20.0, 20.0);
    let w = size(&s, f).0;
    set(&mut s, a, json!({"widthPercent": 100}));
    // The frame hugged its width: it became fixed at its size, the child fills its inside.
    let fl = &s.doc().unwrap().node(f).unwrap().child_layout;
    assert_eq!(fl.sizing_h, Sizing::Fixed);
    assert_eq!(size(&s, a).0, w - 20.0);
    set(&mut s, f, json!({"width": 420}));
    assert_eq!(size(&s, a).0, 400.0);
    // The frame hugs again: the child keeps its size in px.
    set(&mut s, f, json!({"sizingH": "hug"}));
    assert_eq!(percent(&s, a).0, None);
}

#[test]
fn fixed_percent_needs_a_parent_frame() {
    let mut s = Session::new();
    let page = s.doc().unwrap().current_page.unwrap();
    let r = rect(&mut s, page, 20.0, 20.0);
    assert!(s.execute("node.set", &json!({"id": r, "props": {"widthPercent": 50}})).is_err());
    let f = frame(&mut s, None, 100.0, 100.0);
    let r = rect(&mut s, f, 20.0, 20.0);
    for junk in [json!("abc"), json!([1]), json!({"a": 1})] {
        assert!(s.execute("node.set", &json!({"id": r, "props": {"widthPercent": junk}})).is_err(), "{junk}");
    }
    // Out of range values are held in range.
    set(&mut s, r, json!({"widthPercent": -5}));
    assert_eq!(size(&s, r).0, 0.0);
}

#[test]
fn instances_size_percent_layers_from_their_own_frame() {
    let mut s = Session::new();
    let f = frame(&mut s, None, 200.0, 100.0);
    let r = rect(&mut s, f, 20.0, 20.0);
    set(&mut s, r, json!({"widthPercent": 50}));
    let c = id(ex(&mut s, "component.create", json!({"ids": [f]})));
    let i = id(ex(&mut s, "instance.create", json!({"component": c, "x": 300, "y": 0})));
    let copy = s.doc().unwrap().node(i).unwrap().children[0];
    set(&mut s, i, json!({"width": 400}));
    assert_eq!(size(&s, copy).0, 200.0);
    // The main changes its percentage: the instance follows at its own width.
    set(&mut s, r, json!({"widthPercent": 25}));
    assert_eq!(size(&s, r).0, 50.0);
    assert_eq!(size(&s, copy).0, 100.0);
}

#[test]
fn inspect_and_code_show_the_percentage() {
    let mut s = Session::new();
    let f = frame(&mut s, None, 200.0, 100.0);
    let r = rect(&mut s, f, 20.0, 20.0);
    set(&mut s, r, json!({"widthPercent": 50}));
    let n = crate::inspect::node_json(s.doc().unwrap(), r, 0);
    assert_eq!(n["widthPercent"], json!(50.0));
    let css = ex(&mut s, "inspect.code", json!({"id": r}))["code"].as_str().unwrap().to_string();
    assert!(css.contains("width: 50%;"), "{css}");
    assert!(css.contains("height: 20px;"), "{css}");
}

fn drag(s: &mut Session, path: &[(f64, f64)]) {
    use crate::tools::{PointerEvent, PointerKind};
    let (first, last) = (path[0], path[path.len() - 1]);
    s.pointer(&PointerEvent::at(PointerKind::Down, first.0, first.1, 1.0)).unwrap();
    for p in path {
        s.pointer(&PointerEvent::at(PointerKind::Move, p.0, p.1, 1.0)).unwrap();
    }
    s.pointer(&PointerEvent::at(PointerKind::Up, last.0, last.1, 1.0)).unwrap();
}

#[test]
fn dragging_the_parent_smaller_and_larger_keeps_the_share() {
    let mut s = Session::new();
    let f = frame(&mut s, None, 400.0, 200.0);
    let r = rect(&mut s, f, 20.0, 20.0);
    set(&mut s, r, json!({"widthPercent": 50, "heightPercent": 25}));
    ex(&mut s, "select.set", json!({"ids": [f]}));
    // The frame's bottom-right corner: in, past the start, back in.
    drag(&mut s, &[(400.0, 200.0), (300.0, 150.0), (200.0, 100.0)]);
    assert_eq!(size(&s, f), (200.0, 100.0));
    assert_eq!(size(&s, r), (100.0, 25.0), "after shrinking");
    ex(&mut s, "select.set", json!({"ids": [f]}));
    drag(&mut s, &[(200.0, 100.0), (500.0, 300.0), (300.0, 120.0)]);
    assert_eq!(size(&s, f), (300.0, 120.0));
    assert_eq!(size(&s, r), (150.0, 30.0), "after growing then shrinking");
    assert_eq!(percent(&s, r), (Some(50.0), Some(25.0)));
}

#[test]
fn a_75_percent_child_follows_its_parent_from_1000_down_to_100() {
    // Typed at once, step by step, and dragged on the canvas; regular frame and auto layout.
    for auto_layout in [false, true] {
        for how in ["set", "steps", "drag"] {
            let mut s = Session::new();
            let f = frame(&mut s, None, 1000.0, 200.0);
            if auto_layout {
                set(&mut s, f, json!({"layout": "horizontal", "padding": 0, "gap": 0}));
            }
            let r = rect(&mut s, f, 20.0, 20.0);
            set(&mut s, r, json!({"widthPercent": 75}));
            assert_eq!(size(&s, r).0, 750.0, "{how} al={auto_layout}: start");
            match how {
                "set" => set(&mut s, f, json!({"width": 100})),
                "steps" => {
                    for w in (1..10).rev() {
                        set(&mut s, f, json!({"width": w * 100}));
                    }
                }
                _ => {
                    ex(&mut s, "select.set", json!({"ids": [f]}));
                    drag(&mut s, &[(1000.0, 200.0), (700.0, 200.0), (400.0, 200.0), (100.0, 200.0)]);
                }
            }
            assert_eq!(size(&s, f).0, 100.0, "{how} al={auto_layout}: frame");
            assert_eq!(size(&s, r).0, 75.0, "{how} al={auto_layout}: child");
            assert_eq!(percent(&s, r).0, Some(75.0), "{how} al={auto_layout}: percent");
        }
    }
}
