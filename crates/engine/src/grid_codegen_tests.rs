//! Code for grid auto layout frames: CSS grid that lays out like the canvas (tracks, gaps, cells,
//! spans, alignment in the cell, filling children, hugging frames).

use serde_json::{Value, json};

use crate::Session;
use crate::codegen::{CodeLang, code_for};
use crate::params::id_of;

fn id(v: Value) -> figcraft_doc::NodeId {
    id_of(&v["id"]).unwrap()
}

fn ex(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

/// A 300×200 grid frame "Grid" (`columns` columns, gap 10 / 6, padding 8) with `n` 40×30
/// rectangles named "Item".
fn grid(s: &mut Session, columns: u32, n: usize) -> (figcraft_doc::NodeId, Vec<figcraft_doc::NodeId>) {
    let f = id(ex(s, "frame.create", json!({"x": 0, "y": 0, "width": 300, "height": 200, "name": "Grid"})));
    let kids: Vec<_> =
        (0..n).map(|_| id(ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 40, "height": 30, "parent": f, "name": "Item"})))).collect();
    ex(s, "layout.add", json!({"ids": [f]}));
    let props = json!({"layout": "grid", "gridColumns": columns, "gridRows": 1, "gap": 10, "crossGap": 6, "padding": 8,
                       "sizingH": "fixed", "sizingV": "fixed", "width": 300, "height": 200});
    ex(s, "node.set", json!({"id": f, "props": props}));
    (f, kids)
}

/// The declarations of the rule for `class` in the generated stylesheet.
fn rule<'a>(code: &'a str, class: &str) -> &'a str {
    let start = code.find(&format!(".{class} {{\n")).unwrap_or_else(|| panic!("no .{class} in {code}"));
    let body = &code[start..];
    &body[..body.find("}\n").unwrap()]
}

#[test]
fn grid_frames_become_css_grid_with_their_tracks_and_gaps() {
    let mut s = Session::new();
    let (f, _) = grid(&mut s, 3, 5);
    let code = code_for(s.doc().unwrap(), f, CodeLang::Html);
    let g = rule(&code, "grid");
    assert!(g.contains("display: grid;") && !g.contains("display: flex"), "{g}");
    assert!(g.contains("grid-template-columns: repeat(3, 1fr);"), "{g}");
    // Five children in three columns: two rows.
    assert!(g.contains("grid-template-rows: repeat(2, 1fr);"), "{g}");
    assert!(g.contains("column-gap: 10px;") && g.contains("row-gap: 6px;") && g.contains("padding: 8px 8px 8px 8px;"), "{g}");
    assert!(g.contains("justify-content: start;") && g.contains("align-content: start;"), "{g}");
    assert!(g.contains("justify-items: start;") && g.contains("align-items: start;"), "{g}");
    assert!(!g.contains("flex-direction") && !g.contains("justify-content: flex-start"), "{g}");
    // One child per cell in reading order: CSS auto-placement does the same, no lines needed.
    assert!(!code.contains("grid-column") && !code.contains("grid-row:"), "{code}");
    let item = rule(&code, "item");
    assert!(!item.contains("position: absolute") && item.contains("width: 40px;"), "{item}");
}

#[test]
fn fixed_hug_and_fill_tracks_become_px_auto_and_fr() {
    let mut s = Session::new();
    let (f, _) = grid(&mut s, 3, 3);
    ex(&mut s, "layout.setTrack", json!({"ids": [f], "axis": "columns", "index": 0, "sizing": "fixed", "value": 100}));
    ex(&mut s, "layout.setTrack", json!({"ids": [f], "axis": "columns", "index": 1, "sizing": "hug"}));
    ex(&mut s, "layout.setTrack", json!({"ids": [f], "axis": "columns", "index": 2, "sizing": "fill", "value": 2}));
    ex(&mut s, "layout.setTrack", json!({"ids": [f], "axis": "rows", "index": 0, "sizing": "fixed", "value": 50.5}));
    let code = code_for(s.doc().unwrap(), f, CodeLang::Css);
    assert!(code.contains("grid-template-columns: 100px auto 2fr;"), "{code}");
    assert!(code.contains("grid-template-rows: 50.5px;"), "{code}");
}

#[test]
fn hugging_frames_size_fill_tracks_to_their_content() {
    let mut s = Session::new();
    let (f, _) = grid(&mut s, 2, 4);
    ex(&mut s, "node.set", json!({"id": f, "props": {"sizingH": "hug", "sizingV": "hug"}}));
    let code = code_for(s.doc().unwrap(), f, CodeLang::Html);
    let g = rule(&code, "grid");
    // CSS would make every `1fr` track as big as the largest; FigCraft sizes each to its content.
    assert!(g.contains("grid-template-columns: repeat(2, auto);") && g.contains("grid-template-rows: repeat(2, auto);"), "{g}");
    assert!(g.contains("width: fit-content;") && !g.contains("width: 300px") && !g.contains("height:"), "{g}");
    // Children asking to fill a hugging axis keep their own size, as in the canvas.
    let kid = s.doc().unwrap().node(f).unwrap().children[0];
    ex(&mut s, "node.set", json!({"id": kid, "props": {"name": "Wide"}}));
    s.doc_mut().unwrap().get_mut(kid).unwrap().child_layout.sizing_h = figcraft_doc::Sizing::Fill;
    let code = code_for(s.doc().unwrap(), f, CodeLang::Html);
    let wide = rule(&code, "wide");
    assert!(wide.contains("width: 40px;") && !wide.contains("justify-self"), "{wide}");
}

#[test]
fn anchored_and_spanning_children_get_lines_and_so_do_the_others() {
    let mut s = Session::new();
    let (f, kids) = grid(&mut s, 3, 4);
    ex(&mut s, "node.set", json!({"id": kids[0], "props": {"name": "Wide", "gridColumnSpan": 2}}));
    ex(&mut s, "node.set", json!({"id": kids[1], "props": {"name": "Pinned", "gridColumn": 2, "gridRow": 1}}));
    let doc = s.doc().unwrap();
    let geometry = figcraft_layout::grid_geometry(doc, f).unwrap();
    let code = code_for(doc, f, CodeLang::Html);
    // Every child sits where the canvas puts it, 1-based lines with spans.
    for c in &geometry.cells {
        let name = &doc.node(c.id).unwrap().name;
        let class = match name.as_str() {
            "Wide" => "wide".to_string(),
            "Pinned" => "pinned".to_string(),
            _ => {
                let i = kids.iter().position(|k| *k == c.id).unwrap();
                if i == 2 { "item".to_string() } else { "item-2".to_string() }
            }
        };
        let r = rule(&code, &class);
        assert!(r.contains(&format!("grid-column: {} / span {};", c.column + 1, c.column_span)), "{class}: {r}");
        assert!(r.contains(&format!("grid-row: {} / span {};", c.row + 1, c.row_span)), "{class}: {r}");
    }
    let wide = rule(&code, "wide");
    assert!(wide.contains("grid-column: 1 / span 2;") && wide.contains("grid-row: 1 / span 1;"), "{wide}");
    let pinned = rule(&code, "pinned");
    assert!(pinned.contains("grid-column: 3 / span 1;") && pinned.contains("grid-row: 2 / span 1;"), "{pinned}");
}

#[test]
fn fill_children_stretch_in_their_cell_and_alignment_maps_to_self() {
    let mut s = Session::new();
    let (f, kids) = grid(&mut s, 2, 3);
    ex(&mut s, "node.set", json!({"id": f, "props": {"mainAlign": "center", "crossAlign": "max"}}));
    ex(&mut s, "node.set", json!({"id": kids[0], "props": {"name": "Filler", "sizingH": "fill", "sizingV": "fill"}}));
    ex(&mut s, "node.set", json!({"id": kids[1], "props": {"name": "Own", "gridAlignH": "max", "gridAlignV": "min"}}));
    let code = code_for(s.doc().unwrap(), f, CodeLang::Html);
    let g = rule(&code, "grid");
    assert!(g.contains("justify-items: center;") && g.contains("align-items: end;"), "{g}");
    let filler = rule(&code, "filler");
    assert!(filler.contains("justify-self: stretch;") && filler.contains("align-self: stretch;"), "{filler}");
    assert!(!filler.contains("width:") && !filler.contains("height:"), "the cell sizes it: {filler}");
    let own = rule(&code, "own");
    assert!(own.contains("justify-self: end;") && own.contains("align-self: start;") && own.contains("width: 40px;"), "{own}");
    // The frame's alignment covers the rest.
    let item = rule(&code, "item");
    assert!(!item.contains("-self"), "{item}");
}

#[test]
fn flex_frames_are_unchanged() {
    let mut s = Session::new();
    let (f, _) = grid(&mut s, 2, 2);
    ex(&mut s, "node.set", json!({"id": f, "props": {"layout": "horizontal"}}));
    let code = code_for(s.doc().unwrap(), f, CodeLang::Css);
    assert!(code.contains("display: flex;") && code.contains("gap: 10px;") && !code.contains("grid"), "{code}");
}
