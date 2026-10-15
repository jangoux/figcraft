//! Responsive grid columns (`repeat(auto-fill | auto-fit, minmax(min, 1fr))`): the column count
//! follows the frame's width.

use super::tests::pos;
use super::*;
use figcraft_doc::color::{Color, Paint};
use figcraft_doc::{AutoColumns, AutoLayout, Document, GridTrack, NodeKind, StrokeAlign};

/// A grid frame `w` wide (padding 0, gap `gap`, two fixed columns stored) with `n` 20×20 children
/// and responsive columns of at least `min`.
fn auto_grid(w: f64, gap: f64, min: f64, fit: bool, n: usize) -> (Document, NodeId, Vec<NodeId>) {
    let mut d = Document::new("t");
    let p = d.first_page().unwrap();
    let f = d.create(p, NodeKind::Frame, "G", 0.0, 0.0, w, 100.0).unwrap();
    d.get_mut(f).unwrap().layout = Some(AutoLayout {
        mode: LayoutMode::Grid,
        gap,
        cross_gap: gap,
        padding: [0.0; 4],
        grid_columns: 2,
        grid_rows: 1,
        column_tracks: vec![GridTrack::fixed(50.0), GridTrack::fill(2.0)],
        column_auto: Some(AutoColumns { min, fit }),
        ..Default::default()
    });
    let kids = (0..n).map(|i| d.create(f, NodeKind::Rectangle, &format!("R{i}"), 0.0, 0.0, 20.0, 20.0).unwrap()).collect();
    (d, f, kids)
}

fn set_width(d: &mut Document, id: NodeId, w: f64) {
    let h = d.node(id).unwrap().height;
    set_size(d, id, w, h);
}

fn cols(d: &Document, f: NodeId) -> u32 {
    effective_columns(d, f)
}

fn widths(d: &Document, f: NodeId) -> Vec<f64> {
    grid_geometry(d, f).unwrap().columns.iter().map(|(a, b)| b - a).collect()
}

#[test]
fn a_fill_child_grid_counts_its_columns_from_the_width_its_parent_gives_it() {
    // A vertical stack with a grid filling its width: resizing only the stack reflows the grid.
    let (mut d, g, kids) = auto_grid(100.0, 10.0, 100.0, false, 4);
    let p = d.first_page().unwrap();
    let stack = d.create(p, NodeKind::Frame, "Stack", 0.0, 0.0, 450.0, 300.0).unwrap();
    d.get_mut(stack).unwrap().layout = Some(AutoLayout { mode: LayoutMode::Vertical, padding: [10.0; 4], ..Default::default() });
    d.reparent(g, stack, None).unwrap();
    d.get_mut(g).unwrap().child_layout.sizing_h = Sizing::Fill;
    layout_frame(&mut d, stack, &NoMeasure);
    // 450 − 20 padding = 430 = 4·100 + 3·10.
    assert_eq!(d.node(g).unwrap().width, 430.0);
    assert_eq!(cols(&d, g), 4);
    assert_eq!(pos(&d, kids[3]).1, 0.0, "one row");
    // Narrower parent: the grid takes its new width before it counts columns.
    set_width(&mut d, stack, 240.0);
    layout_frame(&mut d, stack, &NoMeasure);
    assert_eq!(d.node(g).unwrap().width, 220.0);
    assert_eq!(cols(&d, g), 2);
    // Two 1fr rows share the 100 px height: row 2 starts at 45 + 10.
    assert_eq!((pos(&d, kids[2]).0, pos(&d, kids[2]).1), (0.0, 55.0), "the third child wraps to row 2");
}

#[test]
fn the_column_count_changes_exactly_at_the_breakpoints() {
    // Breakpoint for n columns: n·100 + (n − 1)·10.
    for (w, want) in
        [(50.0, 1), (100.0, 1), (209.99, 1), (210.0, 2), (210.01, 2), (319.99, 2), (320.0, 3), (320.01, 3), (429.99, 3), (430.0, 4), (430.01, 4)]
    {
        let (mut d, f, _) = auto_grid(w, 10.0, 100.0, false, 6);
        layout_frame(&mut d, f, &NoMeasure);
        assert_eq!(cols(&d, f), want, "width {w}");
        // Columns are equal and never narrower than the minimum.
        let ws = widths(&d, f);
        assert_eq!(ws.len(), want as usize);
        assert!(ws.iter().all(|x| *x >= 100.0 - 1e-9), "width {w}: {ws:?}");
        if w >= 100.0 {
            assert!(ws.iter().all(|x| (x - ws[0]).abs() < 1e-9), "width {w}: {ws:?}");
            assert!((ws.iter().sum::<f64>() + 10.0 * (want - 1) as f64 - w).abs() < 1e-9, "width {w}: fills the frame");
        }
    }
    // A frame narrower than the minimum keeps one column of the minimum width (it overflows).
    let (mut d, f, _) = auto_grid(50.0, 10.0, 100.0, false, 2);
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(widths(&d, f), vec![100.0]);
}

#[test]
fn gap_zero_padding_and_included_strokes_are_counted() {
    let (mut d, f, _) = auto_grid(300.0, 0.0, 100.0, false, 3);
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(cols(&d, f), 3);
    set_width(&mut d, f, 299.99);
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(cols(&d, f), 2);

    // 340 wide, padding 10 a side, gap 10: inner 320 → 3 columns.
    let (mut d, f, _) = auto_grid(340.0, 10.0, 100.0, false, 3);
    d.get_mut(f).unwrap().layout.as_mut().unwrap().padding = [10.0; 4];
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(cols(&d, f), 3);
    assert_eq!(grid_geometry(&d, f).unwrap().columns[0].0, 10.0);
    // A 5 px inside stroke the layout includes takes 10 more: inner 310 → 2 columns.
    {
        let n = d.get_mut(f).unwrap();
        n.strokes = vec![Paint::solid(Color::BLACK)];
        n.stroke.weight = 5.0;
        n.stroke.align = StrokeAlign::Inside;
    }
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(cols(&d, f), 2);
    // Not included: back to 3.
    d.get_mut(f).unwrap().layout.as_mut().unwrap().strokes_included = false;
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(cols(&d, f), 3);
}

#[test]
fn auto_fill_keeps_empty_columns_and_auto_fit_drops_them() {
    // 430 wide: room for 4 columns, 2 children.
    let (mut d, f, kids) = auto_grid(430.0, 10.0, 100.0, false, 2);
    for k in &kids {
        d.get_mut(*k).unwrap().child_layout.sizing_h = Sizing::Fill;
    }
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(cols(&d, f), 4);
    assert_eq!(pos(&d, kids[1]), (110.0, 0.0, 100.0, 20.0));
    // auto-fit: two columns of (430 − 10) / 2.
    d.get_mut(f).unwrap().layout.as_mut().unwrap().column_auto = Some(AutoColumns { min: 100.0, fit: true });
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(cols(&d, f), 2);
    assert_eq!(pos(&d, kids[1]), (220.0, 0.0, 210.0, 20.0));
    // A spanning child counts its columns.
    d.get_mut(kids[0]).unwrap().child_layout.grid_column_span = 2;
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(cols(&d, f), 3);
    // More children than columns: auto-fit is auto-fill.
    let (mut d, f, _) = auto_grid(430.0, 10.0, 100.0, true, 9);
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(cols(&d, f), 4);
}

#[test]
fn one_child_and_no_children() {
    let (mut d, f, kids) = auto_grid(430.0, 10.0, 100.0, false, 1);
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(cols(&d, f), 4);
    assert_eq!(pos(&d, kids[0]).0, 0.0);
    d.get_mut(f).unwrap().layout.as_mut().unwrap().column_auto = Some(AutoColumns { min: 100.0, fit: true });
    d.get_mut(kids[0]).unwrap().child_layout.sizing_h = Sizing::Fill;
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(cols(&d, f), 1);
    assert_eq!(pos(&d, kids[0]).2, 430.0, "the only child stretches across");

    for fit in [false, true] {
        let (mut d, f, _) = auto_grid(430.0, 10.0, 100.0, fit, 0);
        layout_frame(&mut d, f, &NoMeasure);
        assert_eq!(cols(&d, f), if fit { 1 } else { 4 });
        assert!(grid_geometry(&d, f).unwrap().cells.is_empty());
    }
}

#[test]
fn spans_are_clamped_and_anchors_past_the_last_column_are_placed_automatically() {
    // 210 wide: 2 columns.
    let (mut d, f, kids) = auto_grid(210.0, 10.0, 100.0, false, 3);
    d.get_mut(kids[0]).unwrap().child_layout.grid_column_span = 6;
    d.get_mut(kids[1]).unwrap().child_layout.grid_column = Some(5);
    layout_frame(&mut d, f, &NoMeasure);
    let g = grid_geometry(&d, f).unwrap();
    let cell = |id: NodeId| g.cells.iter().find(|c| c.id == id).copied().unwrap();
    assert_eq!((cell(kids[0]).column, cell(kids[0]).row, cell(kids[0]).column_span), (0, 0, 2), "span clamped to 2");
    // Column 5 doesn't exist: placed in reading order like an unanchored child.
    assert_eq!((cell(kids[1]).column, cell(kids[1]).row), (0, 1));
    assert_eq!((cell(kids[2]).column, cell(kids[2]).row), (1, 1));
    // Wide enough for column 5: the anchor holds.
    set_width(&mut d, f, 650.0);
    layout_frame(&mut d, f, &NoMeasure);
    let g = grid_geometry(&d, f).unwrap();
    assert_eq!(g.columns.len(), 6);
    assert_eq!(g.cells.iter().find(|c| c.id == kids[1]).map(|c| c.column), Some(5));

    // A fixed-count grid keeps clamping an anchor to its last column (files lay out as before).
    let (mut d, f, kids) = auto_grid(210.0, 10.0, 100.0, false, 2);
    d.get_mut(f).unwrap().layout.as_mut().unwrap().column_auto = None;
    d.get_mut(kids[1]).unwrap().child_layout.grid_column = Some(5);
    d.get_mut(kids[1]).unwrap().child_layout.grid_row = Some(0);
    layout_frame(&mut d, f, &NoMeasure);
    let g = grid_geometry(&d, f).unwrap();
    assert_eq!(g.cells.iter().find(|c| c.id == kids[1]).map(|c| (c.column, c.row)), Some((1, 0)));
}

#[test]
fn resizing_the_frame_reflows_the_children() {
    let (mut d, f, kids) = auto_grid(430.0, 10.0, 100.0, false, 5);
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!((pos(&d, kids[3]).0, pos(&d, kids[3]).1), (330.0, 0.0));
    // Two 1fr rows share the 100 px height.
    assert_eq!((pos(&d, kids[4]).0, pos(&d, kids[4]).1), (0.0, 55.0));
    set_width(&mut d, f, 320.0);
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(cols(&d, f), 3);
    assert_eq!((pos(&d, kids[3]).0, pos(&d, kids[3]).1), (0.0, 55.0));
    set_width(&mut d, f, 650.0);
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(cols(&d, f), 6);
    assert_eq!(grid_geometry(&d, f).unwrap().rows.len(), 1);
}

#[test]
fn a_hugging_width_falls_back_to_the_column_count_and_tracks() {
    let (mut d, f, _) = auto_grid(430.0, 10.0, 100.0, false, 3);
    d.get_mut(f).unwrap().child_layout.sizing_h = Sizing::Hug;
    layout_frame(&mut d, f, &NoMeasure);
    assert!(auto_columns(d.node(f).unwrap()).is_none());
    assert_eq!(cols(&d, f), 2);
    // The stored tracks: 50 px fixed, then a 2fr track hugging its content.
    assert_eq!(widths(&d, f), vec![50.0, 20.0]);
}

#[test]
fn junk_minimums_fall_back_to_the_column_count() {
    for min in [f64::NAN, f64::INFINITY, 0.0, -10.0] {
        let (mut d, f, _) = auto_grid(430.0, 10.0, min, false, 3);
        layout_frame(&mut d, f, &NoMeasure);
        assert_eq!(cols(&d, f), 2, "min {min}");
    }
    // Huge minimums and gaps stay one column; junk gaps count as 0.
    let (mut d, f, _) = auto_grid(430.0, 1e308, 1e308, false, 3);
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(cols(&d, f), 1);
    let (mut d, f, _) = auto_grid(300.0, f64::NAN, 100.0, false, 3);
    layout_frame(&mut d, f, &NoMeasure);
    assert_eq!(cols(&d, f), 3);
}
