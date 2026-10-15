//! Responsive grid columns through the engine: `layout.setGridAuto` / `layout.clearGridAuto`,
//! undo, inspect, code and instances.

use figcraft_doc::{AutoColumns, GridTrack, NodeId};
use serde_json::{Value, json};

use crate::Session;
use crate::codegen::{CodeLang, code_for};
use crate::params::id_of;

fn id(v: &Value) -> NodeId {
    id_of(&v["id"]).unwrap()
}

fn ex(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

/// A fixed 430×200 grid frame "Grid" (gap 10, padding 0, two stored columns: 50 px and 2fr) with
/// `n` 20×20 rectangles named "Item".
fn grid(s: &mut Session, n: usize) -> (NodeId, Vec<NodeId>) {
    let f = id(&ex(s, "frame.create", json!({"x": 0, "y": 0, "width": 430, "height": 200, "name": "Grid"})));
    let kids =
        (0..n).map(|_| id(&ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 20, "height": 20, "parent": f, "name": "Item"})))).collect();
    ex(s, "layout.add", json!({"ids": [f]}));
    let props = json!({"layout": "grid", "gridRows": 1, "gap": 10, "crossGap": 10, "padding": 0, "sizingH": "fixed", "sizingV": "fixed",
                       "width": 430, "height": 200, "columnTracks": [{"sizing": "fixed", "value": 50}, {"sizing": "fill", "value": 2}]});
    ex(s, "node.set", json!({"id": f, "props": props}));
    (f, kids)
}

fn cols(s: &Session, f: NodeId) -> u32 {
    figcraft_layout::effective_columns(s.doc().unwrap(), f)
}

fn auto(s: &Session, f: NodeId) -> Option<AutoColumns> {
    s.doc().unwrap().node(f).unwrap().layout.as_ref().unwrap().column_auto
}

#[test]
fn set_and_clear_keep_the_stored_columns_and_undo() {
    let mut s = Session::new();
    let (f, _) = grid(&mut s, 3);
    assert_eq!(cols(&s, f), 2);
    ex(&mut s, "layout.setGridAuto", json!({"id": f, "min": 100}));
    assert_eq!(auto(&s, f), Some(AutoColumns { min: 100.0, fit: false }));
    assert_eq!(cols(&s, f), 4);
    let l = s.doc().unwrap().node(f).unwrap().layout.clone().unwrap();
    assert_eq!((l.grid_columns, l.column_tracks.clone()), (2, vec![GridTrack::fixed(50.0), GridTrack::fill(2.0)]), "stored untouched");
    ex(&mut s, "layout.setGridAuto", json!({"id": f, "min": 200, "fit": true}));
    // 430 holds 2 columns of 200; 3 children fill them.
    assert_eq!(cols(&s, f), 2);
    // Clearing restores the stored count and tracks.
    ex(&mut s, "layout.clearGridAuto", json!({"id": f}));
    assert_eq!(auto(&s, f), None);
    let g = figcraft_layout::grid_geometry(s.doc().unwrap(), f).unwrap();
    assert_eq!(g.columns.len(), 2);
    assert_eq!(g.columns[0], (0.0, 50.0));
    // Undo steps back through each change.
    s.undo().unwrap();
    assert_eq!(auto(&s, f), Some(AutoColumns { min: 200.0, fit: true }));
    s.undo().unwrap();
    assert_eq!(auto(&s, f), Some(AutoColumns { min: 100.0, fit: false }));
    assert_eq!(cols(&s, f), 4);
    s.undo().unwrap();
    assert_eq!(auto(&s, f), None);
    assert_eq!(cols(&s, f), 2);
    s.redo().unwrap();
    assert_eq!(cols(&s, f), 4);
}

#[test]
fn bad_frames_and_junk_parameters_are_refused_without_a_change() {
    let mut s = Session::new();
    let (f, kids) = grid(&mut s, 2);
    for p in [
        json!({"id": f}),
        json!({"id": f, "min": 0}),
        json!({"id": f, "min": -5}),
        json!({"id": f, "min": 1e308}),
        json!({"id": f, "min": "NaN"}),
        json!({"id": f, "min": "100"}),
        json!({"id": f, "min": [100]}),
        json!({"id": f, "min": null}),
        json!({"id": f, "min": true}),
        json!({"id": f, "min": 100, "fit": "yes"}),
        json!({"id": f, "min": 100, "fit": 1}),
        json!({"id": 999_999, "min": 100}),
    ] {
        assert!(s.execute("layout.setGridAuto", &p).is_err(), "{p}");
        assert_eq!(auto(&s, f), None, "{p}");
    }
    // Not a grid frame: a rectangle, or a flex frame.
    let err = s.execute("layout.setGridAuto", &json!({"id": kids[0], "min": 100})).unwrap_err().to_string();
    assert!(err.contains("not a grid auto layout frame"), "{err}");
    let stack = id(&ex(&mut s, "frame.create", json!({"x": 500, "y": 0, "width": 100, "height": 100})));
    ex(&mut s, "layout.add", json!({"ids": [stack]}));
    assert!(s.execute("layout.setGridAuto", &json!({"id": stack, "min": 100})).is_err());
    assert!(s.execute("layout.clearGridAuto", &json!({"id": stack})).is_err());
    // A grid hugging its width can't count columns from it.
    ex(&mut s, "node.set", json!({"id": f, "props": {"sizingH": "hug"}}));
    let err = s.execute("layout.setGridAuto", &json!({"id": f, "min": 100})).unwrap_err().to_string();
    assert!(err.contains("hugs its width"), "{err}");
    assert_eq!(auto(&s, f), None);
    // One bad frame refuses the whole edit.
    ex(&mut s, "node.set", json!({"id": f, "props": {"sizingH": "fixed"}}));
    assert!(s.execute("layout.setGridAuto", &json!({"ids": [f, kids[0]], "min": 100})).is_err());
    assert_eq!(auto(&s, f), None);
}

#[test]
fn a_frame_that_starts_hugging_falls_back_and_columns_tracks_are_refused_while_auto() {
    let mut s = Session::new();
    let (f, _) = grid(&mut s, 3);
    ex(&mut s, "layout.setGridAuto", json!({"id": f, "min": 100}));
    let err = s.execute("layout.setTrack", &json!({"ids": [f], "axis": "columns", "index": 0, "sizing": "hug"})).unwrap_err().to_string();
    assert!(err.contains("clearGridAuto"), "{err}");
    // Rows stay editable.
    ex(&mut s, "layout.setTrack", json!({"ids": [f], "axis": "rows", "index": 0, "sizing": "fixed", "value": 40}));
    // Hugging later: the stored columns lay out, auto mode stays set for when it stops.
    ex(&mut s, "node.set", json!({"id": f, "props": {"sizingH": "hug"}}));
    assert_eq!(cols(&s, f), 2);
    assert!(auto(&s, f).is_some());
    ex(&mut s, "node.set", json!({"id": f, "props": {"sizingH": "fixed", "width": 430}}));
    assert_eq!(cols(&s, f), 4);
}

#[test]
fn resizing_a_fill_parent_reflows_the_grid() {
    let mut s = Session::new();
    let (g, kids) = grid(&mut s, 4);
    let stack = id(&ex(&mut s, "frame.create", json!({"x": 0, "y": 300, "width": 450, "height": 400, "name": "Stack"})));
    ex(&mut s, "node.reparent", json!({"ids": [g], "parent": stack}));
    ex(&mut s, "layout.add", json!({"ids": [stack]}));
    ex(
        &mut s,
        "node.set",
        json!({"id": stack, "props": {"layout": "vertical", "padding": 10, "sizingH": "fixed", "sizingV": "fixed", "width": 450}}),
    );
    ex(&mut s, "node.set", json!({"id": g, "props": {"sizingH": "fill"}}));
    ex(&mut s, "layout.setGridAuto", json!({"id": g, "min": 100}));
    assert_eq!(s.doc().unwrap().node(g).unwrap().width, 430.0);
    assert_eq!(cols(&s, g), 4);
    // Only the parent changes; the grid follows.
    ex(&mut s, "node.set", json!({"id": stack, "props": {"width": 240}}));
    assert_eq!(s.doc().unwrap().node(g).unwrap().width, 220.0);
    assert_eq!(cols(&s, g), 2);
    let g2 = figcraft_layout::grid_geometry(s.doc().unwrap(), g).unwrap();
    assert_eq!(g2.cells.iter().find(|c| c.id == kids[2]).map(|c| (c.column, c.row)), Some((0, 1)));
}

#[test]
fn inspect_reports_the_setting_and_the_columns_now() {
    let mut s = Session::new();
    let (f, _) = grid(&mut s, 3);
    let layout = ex(&mut s, "node.get", json!({"id": f}))["layout"].clone();
    assert_eq!(layout["effectiveColumns"], json!(2));
    assert!(layout.get("columnAuto").is_none(), "{layout}");
    ex(&mut s, "layout.setGridAuto", json!({"id": f, "min": 100, "fit": true}));
    let layout = ex(&mut s, "node.get", json!({"id": f}))["layout"].clone();
    assert_eq!(layout["columnAuto"], json!({"min": 100.0, "fit": true}));
    assert_eq!(layout["effectiveColumns"], json!(3));
    // The Dev Mode readout.
    let rows = ex(&mut s, "inspect.properties", json!({"id": f})).to_string();
    assert!(rows.contains("Auto 3 × 1"), "{rows}");
}

#[test]
fn code_writes_auto_fill_and_auto_fit() {
    let mut s = Session::new();
    let (f, kids) = grid(&mut s, 3);
    ex(&mut s, "node.set", json!({"id": kids[0], "props": {"name": "Wide", "gridColumnSpan": 2}}));
    ex(&mut s, "layout.setGridAuto", json!({"id": f, "min": 120.5}));
    let code = code_for(s.doc().unwrap(), f, CodeLang::Html);
    assert!(code.contains("grid-template-columns: repeat(auto-fill, minmax(120.5px, 1fr));"), "{code}");
    assert!(code.contains("grid-template-rows: 1fr;") && code.contains("grid-auto-rows: 1fr;"), "{code}");
    // Cells move with the width: spans only, no lines.
    assert!(code.contains("grid-column: span 2;") && !code.contains(" / span"), "{code}");
    ex(&mut s, "layout.setGridAuto", json!({"id": f, "min": 120.5, "fit": true}));
    let code = code_for(s.doc().unwrap(), f, CodeLang::Css);
    assert!(code.contains("grid-template-columns: repeat(auto-fit, minmax(120.5px, 1fr));"), "{code}");
    // Hugging: the stored tracks.
    ex(&mut s, "node.set", json!({"id": f, "props": {"sizingH": "hug"}}));
    let code = code_for(s.doc().unwrap(), f, CodeLang::Css);
    assert!(code.contains("grid-template-columns: 50px auto;") && !code.contains("auto-fit"), "{code}");
}

#[test]
fn instances_override_responsive_columns_and_keep_them_through_main_edits() {
    let mut s = Session::new();
    let (f, _) = grid(&mut s, 3);
    let main = id(&ex(&mut s, "component.create", json!({"ids": [f]})));
    let inst = id(&ex(&mut s, "instance.create", json!({"component": main, "x": 600, "y": 0})));
    ex(&mut s, "layout.setGridAuto", json!({"id": inst, "min": 100}));
    assert_eq!(auto(&s, inst), Some(AutoColumns { min: 100.0, fit: false }));
    assert_eq!(auto(&s, main), None);
    assert!(s.doc().unwrap().node(inst).unwrap().overrides.iter().any(|g| g == "gridAuto"));
    // The main changes something else: the instance keeps its columns.
    ex(&mut s, "node.set", json!({"id": main, "props": {"gap": 4}}));
    assert_eq!(auto(&s, inst), Some(AutoColumns { min: 100.0, fit: false }));
    // An instance without the override follows the main.
    let other = id(&ex(&mut s, "instance.create", json!({"component": main, "x": 0, "y": 600})));
    ex(&mut s, "layout.setGridAuto", json!({"id": main, "min": 150, "fit": true}));
    assert_eq!(auto(&s, other), Some(AutoColumns { min: 150.0, fit: true }));
    assert_eq!(auto(&s, inst), Some(AutoColumns { min: 100.0, fit: false }));
}

#[test]
fn files_without_the_setting_load_unchanged_and_with_it_round_trip() {
    let old = r#"{"mode":"grid","gap":4,"grid_columns":3,"grid_rows":2}"#;
    let l: figcraft_doc::AutoLayout = serde_json::from_str(old).unwrap();
    assert_eq!(l.column_auto, None);
    assert!(!serde_json::to_string(&l).unwrap().contains("column_auto"), "not written when unset");
    let l = figcraft_doc::AutoLayout { column_auto: Some(AutoColumns { min: 80.0, fit: true }), ..l };
    let back: figcraft_doc::AutoLayout = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
    assert_eq!(back, l);
}
