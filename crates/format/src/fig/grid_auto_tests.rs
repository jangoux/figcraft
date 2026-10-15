//! Responsive grid columns in `.fig` files: written as the plain `1fr` columns they make at the
//! frame's width (so other apps show the same grid), with the setting kept as plugin data that our
//! import restores only while the file still has the columns it was written with.

use figcraft_doc::{AutoColumns, AutoLayout, GridTrack};
use serde_json::json;

use super::*;

/// A fixed 430-wide grid frame "Grid" (gap 10, padding 0, 2 stored columns) with `n` 20×20
/// rectangles, responsive columns of at least 100 (4 columns at this width).
fn auto_grid(n: usize, fit: bool) -> Document {
    let mut doc = Document::new("grid");
    let page = doc.first_page().unwrap();
    let f = doc.create(page, NodeKind::Frame, "Grid", 0.0, 0.0, 430.0, 100.0).unwrap();
    doc.node_mut(f).unwrap().layout = Some(AutoLayout {
        mode: LayoutMode::Grid,
        gap: 10.0,
        cross_gap: 10.0,
        padding: [0.0; 4],
        grid_columns: 2,
        grid_rows: 1,
        column_tracks: vec![GridTrack::fixed(50.0)],
        column_auto: Some(AutoColumns { min: 100.0, fit }),
        ..AutoLayout::default()
    });
    for i in 0..n {
        doc.create(f, NodeKind::Rectangle, &format!("R{i}"), 0.0, 0.0, 20.0, 20.0).unwrap();
    }
    doc
}

fn grid_node(doc: &Document) -> &Node {
    doc.nodes().find(|n| n.name == "Grid").unwrap()
}

/// The written grid frame's node change as JSON.
fn written_grid(doc: &Document) -> Value {
    let (bytes, _) = export(doc, None).unwrap();
    let (schema, message) = decode_canvas(&Archive::open(&bytes).unwrap().canvas).unwrap();
    let msg = schema.decode_message(schema.find("Message").unwrap(), &message).unwrap();
    msg.get("nodeChanges").unwrap().list().iter().map(debug_json).find(|n| n["name"] == "Grid").unwrap()
}

#[test]
fn responsive_columns_are_written_frozen_and_come_back() {
    let doc = auto_grid(5, false);
    let g = written_grid(&doc);
    // Four plain columns, as the frame shows them now.
    assert_eq!(g["gridColumns"]["entries"].as_array().unwrap().len(), 4);
    let kept = g["pluginData"].as_array().unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!((kept[0]["pluginID"].as_str(), kept[0]["key"].as_str()), (Some("figcraft"), Some("gridAuto")));
    let value: Value = serde_json::from_str(kept[0]["value"].as_str().unwrap()).unwrap();
    assert_eq!(value, json!({"min": 100.0, "fit": false, "frozen": 4}));
    let (bytes, warnings) = export(&doc, None).unwrap();
    assert!(warnings.iter().any(|w| w.contains("responsive grid columns")), "{warnings:?}");
    // The stored fixed track isn't written as a fixed column (the grid's columns are all 1fr).
    assert!(!warnings.iter().any(|w| w.contains("fixed or content size")), "{warnings:?}");

    let (back, warnings) = import(&bytes, "x").unwrap();
    assert!(!warnings.iter().any(|w| w.contains("grid")), "{warnings:?}");
    let l = grid_node(&back).layout.clone().unwrap();
    assert_eq!(l.column_auto, Some(AutoColumns { min: 100.0, fit: false }));
    // The frozen count becomes the column count to fall back on.
    assert_eq!(l.grid_columns, 4);
    assert_eq!(figcraft_layout::effective_columns(&back, grid_node(&back).id), 4);

    // auto-fit, fewer children than columns: frozen at the 2 columns the children fill.
    let doc = auto_grid(2, true);
    let (bytes, _) = export(&doc, None).unwrap();
    let (back, _) = import(&bytes, "x").unwrap();
    assert_eq!(grid_node(&back).layout.as_ref().unwrap().column_auto, Some(AutoColumns { min: 100.0, fit: true }));
    assert_eq!(grid_node(&back).layout.as_ref().unwrap().grid_columns, 2);
}

#[test]
fn a_grid_hugging_its_width_writes_its_own_columns_and_nothing_to_keep() {
    let mut doc = auto_grid(3, false);
    let id = grid_node(&doc).id;
    doc.node_mut(id).unwrap().child_layout.sizing_h = Sizing::Hug;
    let g = written_grid(&doc);
    assert_eq!(g["gridColumns"]["entries"].as_array().unwrap().len(), 2);
    assert!(g.get("pluginData").is_none(), "{g}");
    // Without responsive columns there's nothing to keep either.
    let mut doc = auto_grid(3, false);
    doc.node_mut(id).unwrap().layout.as_mut().unwrap().column_auto = None;
    assert!(written_grid(&doc).get("pluginData").is_none());
}

#[test]
fn the_kept_setting_counts_only_while_the_file_has_the_columns_it_was_written_with() {
    let kept = r#"{"min":100,"fit":true,"frozen":4}"#;
    assert_eq!(map::grid_auto(kept, 4), Some(json!({"min": 100.0, "fit": true})));
    // Someone changed the grid elsewhere: their columns win.
    assert_eq!(map::grid_auto(kept, 3), None);
    assert_eq!(map::grid_auto(kept, 5), None);
    // `fit` defaults to auto-fill.
    assert_eq!(map::grid_auto(r#"{"min":80,"frozen":2}"#, 2), Some(json!({"min": 80.0, "fit": false})));
}

#[test]
fn garbage_kept_settings_are_ignored() {
    let long = format!(r#"{{"min":100,"fit":false,"frozen":4,"pad":"{}"}}"#, "x".repeat(1000));
    for kept in [
        "",
        "{",
        "[]",
        "null",
        "4",
        r#"{"min":"100","frozen":4}"#,
        r#"{"min":0,"frozen":4}"#,
        r#"{"min":-5,"frozen":4}"#,
        r#"{"min":1e308,"frozen":4}"#,
        r#"{"min":100,"fit":"yes","frozen":4}"#,
        r#"{"min":100,"fit":1,"frozen":4}"#,
        r#"{"min":100}"#,
        r#"{"min":100,"frozen":-4}"#,
        r#"{"min":100,"frozen":4.5}"#,
        r#"{"min":100,"frozen":"4"}"#,
        long.as_str(),
    ] {
        assert_eq!(map::grid_auto(kept, 4), None, "{kept}");
    }
}

#[test]
fn every_kept_setting_shares_one_plugin_data_list() {
    let mut f: export::Fields = vec![("name", Kv::Str("x".into()))];
    export::stash(&mut f, "stroke", "{}");
    export::stash(&mut f, export::GRID_AUTO_STASH, r#"{"min":1,"fit":false,"frozen":1}"#);
    let lists: Vec<_> = f.iter().filter(|(k, _)| *k == "pluginData").collect();
    assert_eq!(lists.len(), 1);
    match &lists[0].1 {
        Kv::Array(entries) => {
            let keys: Vec<_> = entries.iter().filter_map(|e| e.get("key").and_then(Kv::str)).collect();
            assert_eq!(keys, vec!["stroke", "gridAuto"]);
        }
        other => panic!("{other:?}"),
    }
}
