//! Fixed % (a FigCraft extension) through `.fig` files and REST JSON: other apps see the size in
//! px; the percentages come back from plugin data while the size still matches them.

use figcraft_doc::{AutoLayout, Document, Node, NodeKind, Sizing};
use serde_json::{Value, json};

/// A 200 x 100 frame holding a 100 x 25 rectangle at 50% x 25%, and an outlined-stroke-free
/// auto layout frame (padding 10) holding a 50%-wide one.
fn doc_with_percent() -> Document {
    let mut doc = Document::new("Percent");
    let page = doc.first_page().unwrap();
    let f = doc.create(page, NodeKind::Frame, "Card", 0.0, 0.0, 200.0, 100.0).unwrap();
    let r = doc.create(f, NodeKind::Rectangle, "Half", 10.0, 10.0, 100.0, 25.0).unwrap();
    let cl = &mut doc.get_mut(r).unwrap().child_layout;
    (cl.percent_w, cl.percent_h) = (Some(50.0), Some(25.0));
    let row = doc.create(page, NodeKind::Frame, "Row", 0.0, 200.0, 220.0, 60.0).unwrap();
    doc.get_mut(row).unwrap().layout = Some(AutoLayout { padding: [10.0; 4], gap: 0.0, ..AutoLayout::default() });
    let k = doc.create(row, NodeKind::Rectangle, "Wide", 10.0, 10.0, 100.0, 20.0).unwrap();
    doc.get_mut(k).unwrap().child_layout.percent_w = Some(50.0);
    doc
}

fn named<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes().find(|n| n.name == name).unwrap_or_else(|| panic!("no layer {name}"))
}

fn percents(doc: &Document, name: &str) -> (Option<f64>, Option<f64>) {
    let cl = &named(doc, name).child_layout;
    (cl.percent_w, cl.percent_h)
}

fn fig_round_trip(doc: &Document) -> Document {
    let (bytes, _) = crate::fig::export(doc, None).unwrap();
    crate::fig::import(&bytes, "x").unwrap().0
}

#[test]
fn percentages_survive_a_fig_round_trip_as_fixed_sizes() {
    let back = fig_round_trip(&doc_with_percent());
    let half = named(&back, "Half");
    assert_eq!((half.width, half.height), (100.0, 25.0));
    assert_eq!(half.child_layout.sizing_h, Sizing::Fixed);
    assert_eq!(percents(&back, "Half"), (Some(50.0), Some(25.0)));
    // In auto layout: a share of the inner size (220 - 20 padding).
    assert_eq!(percents(&back, "Wide"), (Some(50.0), None));
    // Layers without percentages get none.
    assert_eq!(percents(&back, "Card"), (None, None));
}

/// A REST API file JSON page: a 200 x 100 frame holding a rectangle `w` x 25 whose plugin data
/// says 50% x 25%.
fn rest_file(w: f64) -> Value {
    json!({"name": "Fixture", "schemaVersion": 0, "document": {"id": "0:0", "name": "Document", "type": "DOCUMENT", "children": [
        {"id": "0:1", "name": "Page", "type": "CANVAS", "children": [
            {"id": "1:1", "name": "Card", "type": "FRAME", "relativeTransform": [[1, 0, 0], [0, 1, 0]], "size": {"x": 200, "y": 100}, "fills": [],
             "children": [{"id": "1:2", "name": "Half", "type": "RECTANGLE", "relativeTransform": [[1, 0, 10], [0, 1, 10]], "size": {"x": w, "y": 25},
                           "fills": [], "pluginData": {"figcraft": {"percentSize": "{\"w\":50.0,\"h\":25.0}"}}}]}
        ]}
    ]}})
}

fn rest_import(v: &Value) -> Document {
    crate::rest::import(v, &|_: &str| None).unwrap().0
}

#[test]
fn rest_import_keeps_percentages_only_while_the_size_matches() {
    assert_eq!(percents(&rest_import(&rest_file(100.0)), "Half"), (Some(50.0), Some(25.0)));
    // Resized in another app: the width stays the px it was given; the height still matches.
    let doc = rest_import(&rest_file(130.0));
    assert_eq!(percents(&doc, "Half"), (None, Some(25.0)));
    assert_eq!(named(&doc, "Half").width, 130.0);
}

#[test]
fn junk_percent_plugin_data_is_ignored() {
    for junk in ["", "{", "[1]", "{\"w\":\"x\",\"h\":null}", "{\"w\":1e999}"] {
        let mut v = rest_file(100.0);
        v["document"]["children"][0]["children"][0]["children"][0]["pluginData"]["figcraft"]["percentSize"] = json!(junk);
        let doc = rest_import(&v);
        assert_eq!(percents(&doc, "Half"), (None, None), "{junk}");
    }
}
