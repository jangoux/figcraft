//! Margins: a FigCraft-only extension stored as standard frames. Figma has no margin, so a child of
//! an auto layout frame with a margin sits inside a *wrapper*: a plain auto layout frame (no
//! fills, strokes or effects, one child) whose padding is the margin. Files stay readable by other
//! apps, and the layout engine needs nothing new.
//!
//! A wrapper is recognised by [`Node::margin_wrapper`] first, and by its name and shape as a
//! fallback (files that went through an app that drops the flag): named "<child> · margin", a
//! frame with auto layout, exactly one child and nothing painted.

use crate::{Document, Node, NodeId, NodeKind};

/// The end of a wrapper's name: "<child name> · margin".
pub const MARGIN_SUFFIX: &str = " · margin";

/// The name a wrapper of a child named `child` gets.
pub fn margin_wrapper_name(child: &str) -> String {
    format!("{child}{MARGIN_SUFFIX}")
}

/// `name` without the wrapper suffix (unchanged when it has none).
pub fn strip_margin_suffix(name: &str) -> &str {
    name.strip_suffix(MARGIN_SUFFIX).unwrap_or(name)
}

/// Whether `n` has a wrapper's shape: a frame with auto layout, exactly one child, and no visible
/// fills, strokes or effects. Only the shape is checked (not its sizing or padding), so detecting
/// a wrapper never needs to change it.
pub fn is_plain_wrapper_shape(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Frame)
        && n.layout.is_some()
        && n.children.len() == 1
        && !n.fills.iter().any(|p| p.visible)
        && !n.strokes.iter().any(|p| p.visible)
        && !n.effects.iter().any(|e| e.visible)
}

/// Whether `n` is a margin wrapper by name and shape alone (the fallback for files without the
/// flag).
pub fn looks_like_margin_wrapper(n: &Node) -> bool {
    n.name.ends_with(MARGIN_SUFFIX) && is_plain_wrapper_shape(n)
}

/// Whether node `id` is a margin wrapper: flagged, or (unflagged) named and shaped like one.
pub fn is_margin_wrapper(doc: &Document, id: NodeId) -> bool {
    doc.node(id).is_some_and(|n| n.margin_wrapper || looks_like_margin_wrapper(n))
}

/// The margin wrapper of `id`: `id` itself when it is one, else its parent when that is a wrapper.
pub fn margin_wrapper_of(doc: &Document, id: NodeId) -> Option<NodeId> {
    if is_margin_wrapper(doc, id) {
        return Some(id);
    }
    doc.node(id).and_then(|n| n.parent).filter(|p| is_margin_wrapper(doc, *p))
}

/// The margin `[top, right, bottom, left]` around `id` (the child or its wrapper): the wrapper's
/// padding, sanitised to finite values ≥ 0; zero without a wrapper.
pub fn margin_of(doc: &Document, id: NodeId) -> [f64; 4] {
    margin_wrapper_of(doc, id)
        .and_then(|w| doc.node(w))
        .and_then(|w| w.layout.as_ref())
        .map(|l| l.padding.map(|p| if p.is_finite() { p.max(0.0) } else { 0.0 }))
        .unwrap_or([0.0; 4])
}

/// What stands for `id` among its parent's layers: its margin wrapper when it has one (moving,
/// reordering, deleting or duplicating the child acts on it), else `id`.
pub fn margin_outer(doc: &Document, id: NodeId) -> NodeId {
    margin_wrapper_of(doc, id).filter(|w| *w != id).unwrap_or(id)
}

/// The parent of `id` as the layers panel shows it: past a margin wrapper.
pub fn visible_parent(doc: &Document, id: NodeId) -> Option<NodeId> {
    doc.node(margin_outer(doc, id)).and_then(|n| n.parent)
}
