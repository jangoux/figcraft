//! Margin wrappers: recognised by the flag first, by name and shape as a fallback.

use crate::*;
use figcraft_color::{Color, Paint};

/// A row frame holding `name · margin` (auto layout, no fill) around a rectangle; returns
/// `(doc, wrapper, child)`.
fn wrapped(flag: bool) -> (Document, NodeId, NodeId) {
    let mut doc = Document::new("d");
    let page = doc.first_page().unwrap();
    let row = doc.create(page, NodeKind::Frame, "Row", 0.0, 0.0, 200.0, 100.0).unwrap();
    doc.get_mut(row).unwrap().layout = Some(AutoLayout::default());
    let w = doc.create(row, NodeKind::Frame, &margin_wrapper_name("Box"), 0.0, 0.0, 40.0, 40.0).unwrap();
    let wn = doc.get_mut(w).unwrap();
    wn.fills.clear();
    wn.layout = Some(AutoLayout { padding: [4.0, 3.0, 2.0, 1.0], ..AutoLayout::default() });
    wn.margin_wrapper = flag;
    let c = doc.create(w, NodeKind::Rectangle, "Box", 0.0, 0.0, 20.0, 20.0).unwrap();
    (doc, w, c)
}

#[test]
fn the_flag_marks_a_wrapper() {
    let (mut doc, w, c) = wrapped(true);
    // The flag wins even where the name and shape no longer match.
    let wn = doc.get_mut(w).unwrap();
    wn.name = "Renamed".into();
    wn.fills = vec![Paint::solid(Color::WHITE)];
    assert!(is_margin_wrapper(&doc, w));
    assert!(!is_margin_wrapper(&doc, c));
    assert_eq!(margin_wrapper_of(&doc, c), Some(w));
    assert_eq!(margin_wrapper_of(&doc, w), Some(w));
    assert_eq!(margin_of(&doc, c), [4.0, 3.0, 2.0, 1.0]);
}

#[test]
fn name_and_shape_mark_an_unflagged_wrapper() {
    let (doc, w, c) = wrapped(false);
    assert!(is_margin_wrapper(&doc, w));
    assert_eq!(margin_wrapper_of(&doc, c), Some(w));
    assert_eq!(margin_of(&doc, w), [4.0, 3.0, 2.0, 1.0]);
    // A hidden fill paints nothing: still plain.
    let mut doc = doc;
    let mut hidden = Paint::solid(Color::WHITE);
    hidden.visible = false;
    doc.get_mut(w).unwrap().fills = vec![hidden];
    assert!(is_margin_wrapper(&doc, w));
}

#[test]
fn the_name_alone_is_not_enough() {
    // A visible fill.
    let (mut doc, w, c) = wrapped(false);
    doc.get_mut(w).unwrap().fills = vec![Paint::solid(Color::WHITE)];
    assert!(!is_margin_wrapper(&doc, w));
    assert_eq!(margin_wrapper_of(&doc, c), None);
    assert_eq!(margin_of(&doc, c), [0.0; 4]);
    // Two children.
    let (mut doc, w, _) = wrapped(false);
    doc.create(w, NodeKind::Rectangle, "Other", 0.0, 0.0, 5.0, 5.0).unwrap();
    assert!(!is_margin_wrapper(&doc, w));
    // A visible stroke, or an effect.
    let (mut doc, w, _) = wrapped(false);
    doc.get_mut(w).unwrap().strokes = vec![Paint::solid(Color::BLACK)];
    assert!(!is_margin_wrapper(&doc, w));
    let (mut doc, w, _) = wrapped(false);
    doc.get_mut(w).unwrap().effects = vec![Effect::drop_shadow()];
    assert!(!is_margin_wrapper(&doc, w));
    // No auto layout.
    let (mut doc, w, _) = wrapped(false);
    doc.get_mut(w).unwrap().layout = None;
    assert!(!is_margin_wrapper(&doc, w));
    // Not a frame (an instance or a group with the right name).
    let (mut doc, w, _) = wrapped(false);
    doc.get_mut(w).unwrap().kind = NodeKind::Group;
    assert!(!is_margin_wrapper(&doc, w));
    // The right shape without the suffix.
    let (mut doc, w, _) = wrapped(false);
    doc.get_mut(w).unwrap().name = "Box margin".into();
    assert!(!is_margin_wrapper(&doc, w));
}

#[test]
fn old_files_load_unchanged_and_the_flag_round_trips() {
    let (doc, w, _) = wrapped(false);
    let json = serde_json::to_value(doc.node(w).unwrap()).unwrap();
    assert!(json.get("margin_wrapper").is_none(), "left out when false");
    let back: Node = serde_json::from_value(json).unwrap();
    assert!(!back.margin_wrapper);
    let (doc, w, _) = wrapped(true);
    let json = serde_json::to_value(doc.node(w).unwrap()).unwrap();
    let back: Node = serde_json::from_value(json).unwrap();
    assert!(back.margin_wrapper);
}

#[test]
fn names() {
    assert_eq!(margin_wrapper_name("Card"), "Card · margin");
    assert_eq!(strip_margin_suffix("Card · margin"), "Card");
    assert_eq!(strip_margin_suffix("Card"), "Card");
    assert_eq!(strip_margin_suffix(" · margin"), "");
}
