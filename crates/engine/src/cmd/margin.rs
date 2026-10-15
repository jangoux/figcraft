//! Margins on auto layout children, a FigCraft-only extension stored as standard frames: the child
//! sits in a plain auto layout wrapper whose padding is the margin ([`figcraft_doc::margin`]), so
//! files stay Figma-compatible and the layout engine needs nothing new.

use figcraft_doc::*;
use figcraft_geom::Affine;
use serde_json::{Value, json};

use super::CommandSpec;
use super::structure::{remove_frame, wrap_in_frame};
use crate::components::structure_locked;
use crate::params::*;
use crate::{Result, Session, bad, spec};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "layout.setMargin",
            "Set margin",
            "{id, margin: n | [top, right, bottom, left]} (a child of an auto layout frame, or its margin wrapper; values are clamped to 0..1e6. The first margin wraps the child in a plain auto layout frame named \"<child> · margin\" whose padding is the margin, and moves the child's Fill sizing, absolute position, constraints and grid cell to it; later margins change the padding; 0 on every side removes the wrapper, giving them back, or, when the wrapper has been given fills, strokes, effects or layers of its own, keeps it as an ordinary frame with no padding. Inside instances only the padding of an existing wrapper changes)",
            true,
            set_margin
        ),
        spec!(
            "layout.getMargin",
            "Get margin",
            "{id} (a layer or its margin wrapper) → {margin: [top, right, bottom, left], wrapper: id | null, child: id}",
            false,
            get_margin
        ),
    ]
}

const SET: &str = "layout.setMargin";

/// Largest margin accepted on a side.
const MAX_MARGIN: f64 = 1e6;

/// `margin` from params: one number for every side or `[top, right, bottom, left]`, each finite and
/// clamped to `0..=MAX_MARGIN`.
fn margin_param(p: &Value) -> Result<[f64; 4]> {
    const HELP: &str = "`margin` must be a number or [top, right, bottom, left]";
    let m = match get(p, "margin") {
        Some(Value::Array(a)) => {
            let sides: Vec<f64> = a.iter().map(f64_of).collect::<Option<_>>().ok_or_else(|| bad(SET, HELP))?;
            <[f64; 4]>::try_from(sides).map_err(|_| bad(SET, HELP))?
        }
        Some(v) => [f64_of(v).ok_or_else(|| bad(SET, HELP))?; 4],
        None => return Err(bad(SET, HELP)),
    };
    Ok(m.map(|x| if x.is_finite() { x.clamp(0.0, MAX_MARGIN) } else { 0.0 }))
}

/// A wrapper's own layout: one child, no gap, padding `m`.
fn wrapper_layout(m: [f64; 4]) -> AutoLayout {
    AutoLayout { padding: m, gap: 0.0, cross_gap: 0.0, ..AutoLayout::default() }
}

fn set_margin(s: &mut Session, p: &Value) -> Result<Value> {
    let id = req_id(SET, p, "id")?;
    let m = margin_param(p)?;
    let doc = s.doc()?;
    doc.get(id)?;
    let zero = m.iter().all(|x| *x == 0.0);
    let Some(w) = margin_wrapper_of(doc, id) else {
        if zero {
            return Ok(json!({"id": id, "wrapper": null, "margin": m}));
        }
        return add_wrapper(s, id, m);
    };
    let wn = doc.get(w)?;
    let child = if w == id { wn.children.first().copied() } else { Some(id) };
    // Inside an instance, layers can't be added or removed: only the padding changes (an override).
    let locked = wn.parent.is_some_and(|parent| structure_locked(doc, parent));
    let plain = is_plain_wrapper_shape(wn);
    let doc = s.doc_mut()?;
    if locked {
        let n = doc.get_mut(w)?;
        n.layout.get_or_insert_with(|| wrapper_layout(m)).padding = m;
        if n.source.is_some() {
            n.overrides.insert("padding".into());
        }
        return Ok(json!({"id": child, "wrapper": w, "margin": m}));
    }
    match (zero, plain, child) {
        (true, true, Some(c)) => remove_wrapper(s, w, c),
        (true, _, _) => {
            // A wrapper given a look or layers of its own stays, as an ordinary frame.
            let n = doc.get_mut(w)?;
            if let Some(l) = n.layout.as_mut() {
                l.padding = [0.0; 4];
            }
            n.margin_wrapper = false;
            n.name = strip_margin_suffix(&n.name).to_string();
            Ok(json!({"id": child, "wrapper": null, "margin": m, "kept": w}))
        }
        (false, _, _) => {
            let name = child.and_then(|c| doc.node(c)).map(|c| margin_wrapper_name(&c.name));
            let n = doc.get_mut(w)?;
            n.layout.get_or_insert_with(|| wrapper_layout(m)).padding = m;
            n.margin_wrapper = true;
            if let Some(name) = name {
                n.name = name;
            }
            Ok(json!({"id": child, "wrapper": w, "margin": m}))
        }
    }
}

/// Wrap `id` in a new margin wrapper with padding `m`, moving its layout role (Fill per axis,
/// absolute position, constraints, grid cell) to the wrapper.
fn add_wrapper(s: &mut Session, id: NodeId, m: [f64; 4]) -> Result<Value> {
    let doc = s.doc()?;
    let n = doc.get(id)?;
    let parent = n.parent.ok_or_else(|| bad(SET, "a page has no margin"))?;
    if !doc.node(parent).is_some_and(|p| p.layout.is_some() && p.kind.is_frame_like()) {
        return Err(bad(SET, format!("\"{}\" is not in an auto layout frame: margins apply to auto layout children", n.name)));
    }
    if structure_locked(doc, parent) {
        return Err(bad(SET, format!("\"{}\" is inside an instance: add the margin in its main component", n.name)));
    }
    let (name, cl, constraints) = (margin_wrapper_name(&n.name), n.child_layout.clone(), n.constraints);
    let w = wrap_in_frame(s, id, &name)?;
    let doc = s.doc_mut()?;
    let fill = |s: Sizing| if s == Sizing::Fill { Sizing::Fill } else { Sizing::Hug };
    let wn = doc.get_mut(w)?;
    wn.fills.clear();
    wn.clip = false;
    wn.layout = Some(wrapper_layout(m));
    wn.margin_wrapper = true;
    wn.constraints = constraints;
    wn.child_layout = ChildLayout {
        sizing_h: fill(cl.sizing_h),
        sizing_v: fill(cl.sizing_v),
        absolute: cl.absolute,
        grid_column: cl.grid_column,
        grid_row: cl.grid_row,
        grid_column_span: cl.grid_column_span,
        grid_row_span: cl.grid_row_span,
        grid_align_h: cl.grid_align_h,
        grid_align_v: cl.grid_align_v,
        ..ChildLayout::default()
    };
    let cn = doc.get_mut(id)?;
    cn.constraints = Constraints::default();
    cn.child_layout = ChildLayout {
        sizing_h: cl.sizing_h,
        sizing_v: cl.sizing_v,
        min_w: cl.min_w,
        max_w: cl.max_w,
        min_h: cl.min_h,
        max_h: cl.max_h,
        ..ChildLayout::default()
    };
    s.set_selection(vec![id]);
    Ok(json!({"id": id, "wrapper": w, "margin": m}))
}

/// Remove plain wrapper `w`, giving its layout role back to `child`.
fn remove_wrapper(s: &mut Session, w: NodeId, child: NodeId) -> Result<Value> {
    let doc = s.doc_mut()?;
    let wn = doc.get(w)?;
    let (wcl, constraints) = (wn.child_layout.clone(), wn.constraints);
    let [top, _, _, left] = wn.layout.as_ref().map(|l| l.padding).unwrap_or_default();
    let cn = doc.get_mut(child)?;
    let own = cn.child_layout.clone();
    // Fill comes back from the wrapper; a child that only filled the wrapper keeps its size.
    let back = |wrapper: Sizing, own: Sizing| match (wrapper, own) {
        (Sizing::Fill, _) => Sizing::Fill,
        (_, Sizing::Fill) => Sizing::Fixed,
        (_, own) => own,
    };
    cn.child_layout = ChildLayout {
        sizing_h: back(wcl.sizing_h, own.sizing_h),
        sizing_v: back(wcl.sizing_v, own.sizing_v),
        absolute: wcl.absolute,
        grid_column: wcl.grid_column,
        grid_row: wcl.grid_row,
        grid_column_span: wcl.grid_column_span,
        grid_row_span: wcl.grid_row_span,
        grid_align_h: wcl.grid_align_h,
        grid_align_v: wcl.grid_align_v,
        ..own
    };
    cn.constraints = constraints;
    remove_frame(s, w)?;
    // An absolute child goes back to where the wrapper was, as the margin no longer pushes it in.
    if wcl.absolute {
        let (dx, dy) = (if left.is_finite() { left } else { 0.0 }, if top.is_finite() { top } else { 0.0 });
        let cn = s.doc_mut()?.get_mut(child)?;
        cn.transform = Affine::translate((-dx, -dy)) * cn.transform;
    }
    Ok(json!({"id": child, "wrapper": null, "margin": [0.0, 0.0, 0.0, 0.0]}))
}

fn get_margin(s: &mut Session, p: &Value) -> Result<Value> {
    let id = req_id("layout.getMargin", p, "id")?;
    let doc = s.doc()?;
    doc.get(id)?;
    let w = margin_wrapper_of(doc, id);
    let child = match w {
        Some(w) if w == id => doc.get(w)?.children.first().copied(),
        _ => Some(id),
    };
    Ok(json!({"margin": margin_of(doc, id), "wrapper": w, "child": child}))
}

#[cfg(test)]
#[path = "margin_tests.rs"]
mod tests;
