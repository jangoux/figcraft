//! Fixed % sizing (a FigCraft extension): a layer's width or height as a percentage of its
//! parent's, kept as the parent resizes. The layer is Fixed on that axis; the percentage is in
//! [`crate::ChildLayout::percent_w`] / [`crate::ChildLayout::percent_h`].

use crate::{AutoLayout, Document, Node, NodeId, StrokeAlign};

/// The largest percentage a layer can have.
pub const MAX_PERCENT: f64 = 10_000.0;

/// A percentage the document can hold: finite, within `[0, MAX_PERCENT]`.
pub fn clean_percent(v: f64) -> Option<f64> {
    v.is_finite().then(|| v.clamp(0.0, MAX_PERCENT))
}

/// Space an auto layout frame keeps inside its edges, `(top, right, bottom, left)`: its padding,
/// sanitised, plus its own inside stroke (per side) when strokes are included in the layout;
/// center and outside strokes never take up space.
pub fn layout_insets(f: &Node, al: &AutoLayout) -> (f64, f64, f64, f64) {
    let fin = |v: f64| if v.is_finite() { v.max(0.0) } else { 0.0 };
    let included = al.strokes_included && f.has_visible_stroke() && f.stroke.align == StrokeAlign::Inside;
    let stroke = if included { f.stroke.sides.unwrap_or([f.stroke.weight; 4]) } else { [0.0; 4] };
    let [pt, pr, pb, pl] = al.padding.map(fin);
    let [st, sr, sb, sl] = stroke.map(fin);
    (pt + st, pr + sr, pb + sb, pl + sl)
}

impl Document {
    /// The size `(width, height)` layer `id`'s percentages are of: its parent frame's inner size
    /// (inside the padding) when it flows in the parent's auto layout, else the parent's whole
    /// size, like CSS. `None` where Fixed % can't apply: no parent frame (a page, a section, a group
    /// whose bounds follow its children).
    pub fn percent_basis(&self, id: NodeId) -> Option<(f64, f64)> {
        let n = self.node(id)?;
        let p = self.node(n.parent?)?;
        if !p.kind.is_frame_like() {
            return None;
        }
        let fin = |v: f64| if v.is_finite() { v.max(0.0) } else { 0.0 };
        let (w, h) = (fin(p.width), fin(p.height));
        match &p.layout {
            Some(al) if !n.child_layout.absolute => {
                let (t, r, b, l) = layout_insets(p, al);
                Some(((w - l - r).max(0.0), (h - t - b).max(0.0)))
            }
            _ => Some((w, h)),
        }
    }
}
