//! Margin wrappers (a FigCraft extension stored as standard frames) through `.fig` files and REST
//! JSON: the flag comes back from plugin data, or from the wrapper's name and shape alone, and
//! reading never changes the frame itself.

use figcraft_doc::color::{Color, Paint};
use figcraft_doc::{AutoLayout, ChildLayout, Document, Node, NodeKind, Sizing, margin_wrapper_name};
use serde_json::{Value, json};

/// A row frame holding a margin wrapper (padding `[4, 8, 4, 8]`, Hug × Hug) around a rectangle.
fn doc_with_wrapper(name: &str, flag: bool) -> Document {
    let mut doc = Document::new("Margins");
    let page = doc.first_page().unwrap();
    let row = doc.create(page, NodeKind::Frame, "Row", 0.0, 0.0, 200.0, 60.0).unwrap();
    doc.get_mut(row).unwrap().layout = Some(AutoLayout { padding: [0.0; 4], ..AutoLayout::default() });
    let w = doc.create(row, NodeKind::Frame, name, 0.0, 0.0, 56.0, 28.0).unwrap();
    let wn = doc.get_mut(w).unwrap();
    wn.fills.clear();
    wn.clip = false;
    wn.layout = Some(AutoLayout { padding: [4.0, 8.0, 4.0, 8.0], gap: 0.0, ..AutoLayout::default() });
    wn.child_layout = ChildLayout { sizing_h: Sizing::Hug, sizing_v: Sizing::Hug, ..ChildLayout::default() };
    wn.margin_wrapper = flag;
    doc.create(w, NodeKind::Rectangle, "Box", 8.0, 4.0, 40.0, 20.0).unwrap();
    doc
}

fn named<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes().find(|n| n.name == name).unwrap_or_else(|| panic!("no layer {name}"))
}

fn fig_round_trip(doc: &Document) -> Document {
    let (bytes, _) = crate::fig::export(doc, None).unwrap();
    crate::fig::import(&bytes, "x").unwrap().0
}

#[test]
fn the_flag_survives_a_fig_round_trip_even_renamed() {
    for name in [margin_wrapper_name("Box"), "Renamed by someone".to_string()] {
        let back = fig_round_trip(&doc_with_wrapper(&name, true));
        let w = named(&back, &name);
        // Renamed, only the plugin data the export wrote can bring the flag back.
        assert!(w.margin_wrapper, "{name}: the flag comes back");
        assert_eq!(w.layout.as_ref().unwrap().padding, [4.0, 8.0, 4.0, 8.0]);
        assert_eq!(w.children.len(), 1);
    }
    // A plain frame stays one.
    let back = fig_round_trip(&doc_with_wrapper("Just a frame", false));
    assert!(!named(&back, "Just a frame").margin_wrapper);
}

/// A REST API file JSON page with a row frame holding `wrapper`.
fn rest_file(wrapper: Value) -> Value {
    json!({"name": "Fixture", "schemaVersion": 0, "document": {"id": "0:0", "name": "Document", "type": "DOCUMENT", "children": [
        {"id": "0:1", "name": "Page", "type": "CANVAS", "children": [
            {"id": "1:1", "name": "Row", "type": "FRAME", "relativeTransform": [[1, 0, 0], [0, 1, 0]], "size": {"x": 200, "y": 60},
             "layoutMode": "HORIZONTAL", "fills": [], "children": [wrapper]}
        ]}
    ]}})
}

fn rest_wrapper(name: &str, extra: Value) -> Value {
    let mut w = json!({"id": "1:2", "name": name, "type": "FRAME", "relativeTransform": [[1, 0, 0], [0, 1, 0]], "size": {"x": 56, "y": 28},
        "layoutMode": "HORIZONTAL", "paddingTop": 4, "paddingRight": 8, "paddingBottom": 4, "paddingLeft": 8, "itemSpacing": 0,
        "primaryAxisSizingMode": "AUTO", "counterAxisSizingMode": "AUTO", "fills": [], "clipsContent": false,
        "children": [{"id": "1:3", "name": "Box", "type": "RECTANGLE", "relativeTransform": [[1, 0, 8], [0, 1, 4]], "size": {"x": 40, "y": 20},
                      "fills": [{"type": "SOLID", "color": {"r": 1, "g": 0, "b": 0, "a": 1}}]}]});
    if let (Some(o), Some(e)) = (w.as_object_mut(), extra.as_object()) {
        o.extend(e.clone());
    }
    w
}

fn rest_import(v: &Value) -> Document {
    crate::rest::import(v, &|_: &str| None).unwrap().0
}

/// Everything about a layer but the flag, for comparing healed and unhealed reads.
fn without_flag(n: &Node) -> Node {
    let mut n = n.clone();
    n.margin_wrapper = false;
    n
}

#[test]
fn rest_import_heals_by_name_and_shape_and_changes_nothing_else() {
    let name = margin_wrapper_name("Box");
    let healed = rest_import(&rest_file(rest_wrapper(&name, json!({}))));
    let w = named(&healed, &name);
    assert!(w.margin_wrapper, "named and shaped like a wrapper");
    // The same frame under another name is read identically, flag aside: healing changes nothing
    // else (sizing, padding, children, look).
    let plain = rest_import(&rest_file(rest_wrapper("Plain", json!({}))));
    let p = named(&plain, "Plain");
    assert!(!p.margin_wrapper);
    let mut a = without_flag(w);
    a.name = "Plain".into();
    assert_eq!(a, without_flag(p));
    for (x, y) in w.children.iter().zip(&p.children) {
        assert_eq!(healed.node(*x), plain.node(*y));
    }
}

#[test]
fn rest_import_reads_the_plugin_data_and_refuses_wrong_shapes() {
    // Plugin data marks it whatever its name.
    let doc = rest_import(&rest_file(rest_wrapper("Anything", json!({"pluginData": {"figcraft": {"margin": "1"}}}))));
    assert!(named(&doc, "Anything").margin_wrapper);
    let doc = rest_import(&rest_file(rest_wrapper("Off", json!({"pluginData": {"figcraft": {"margin": "0"}}}))));
    assert!(!named(&doc, "Off").margin_wrapper);
    // The right name with a fill, or a second child: not a wrapper, and left as it is.
    let name = margin_wrapper_name("Box");
    let filled = rest_wrapper(&name, json!({"fills": [{"type": "SOLID", "color": {"r": 0, "g": 0, "b": 1, "a": 1}}]}));
    let doc = rest_import(&rest_file(filled));
    let w = named(&doc, &name);
    assert!(!w.margin_wrapper);
    assert_eq!(w.fills, vec![Paint::solid(Color::from_rgb8_const(0, 0, 255))]);
    let mut two = rest_wrapper(&name, json!({}));
    let kid = two["children"][0].clone();
    let mut kid2 = kid.clone();
    kid2["id"] = json!("1:4");
    two["children"] = json!([kid, kid2]);
    let doc = rest_import(&rest_file(two));
    assert!(!named(&doc, &name).margin_wrapper);
}
