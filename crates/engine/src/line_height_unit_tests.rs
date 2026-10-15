//! Line height units: pixels, a percent of the font size (Figma's) and a unitless multiplier
//! (CSS `line-height: 1.5`), and number variables bound to the line height, which read in the
//! unit the field is in.

use serde_json::{Value, json};

use crate::Session;
use crate::params::id_of;
use figcraft_doc::{LineHeight, NodeId};

fn run(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

fn text(s: &mut Session) -> NodeId {
    let t = id_of(&run(s, "text.create", json!({"text": "A\nB", "x": 0, "y": 0}))["id"]).unwrap();
    run(s, "node.set", json!({"ids": [t], "props": {"fontSize": 20}}));
    t
}

fn lh(s: &Session, t: NodeId) -> LineHeight {
    s.doc().unwrap().node(t).unwrap().kind.text().unwrap().style.line_height
}

fn number_var(s: &mut Session, value: f64) {
    run(s, "variable.createCollection", json!({"name": "C"}));
    run(s, "variable.create", json!({"collection": "C", "name": "lh", "type": "number", "value": value}));
}

#[test]
fn a_multiplier_is_typed_with_x_and_spaces_lines_like_the_same_percent() {
    let mut s = Session::new();
    let t = text(&mut s);
    for (v, want) in
        [(json!("1.5x"), LineHeight::Multiplier(1.5)), (json!("2×"), LineHeight::Multiplier(2.0)), (json!(" 1.25X "), LineHeight::Multiplier(1.25))]
    {
        run(&mut s, "node.set", json!({"ids": [t], "props": {"lineHeight": v}}));
        assert_eq!(lh(&s, t), want, "{v}");
    }
    run(&mut s, "node.set", json!({"ids": [t], "props": {"lineHeight": "1.5x"}}));
    let times = s.doc().unwrap().node(t).unwrap().height;
    run(&mut s, "node.set", json!({"ids": [t], "props": {"lineHeight": "150%"}}));
    let pct = s.doc().unwrap().node(t).unwrap().height;
    assert_eq!(times, pct, "on the canvas 1.5× and 150% both take each run's own size");
    run(&mut s, "node.set", json!({"ids": [t], "props": {"lineHeight": 30}}));
    assert_eq!(times, s.doc().unwrap().node(t).unwrap().height, "1.5 × 20 px = 30 px lines");
    assert_eq!(LineHeight::Multiplier(1.5).label(), "1.5×");
    assert_eq!(serde_json::to_value(LineHeight::Multiplier(1.5)).unwrap(), json!({"unit": "multiplier", "value": 1.5}));
}

#[test]
fn junk_line_heights_are_refused() {
    let mut s = Session::new();
    let t = text(&mut s);
    for v in [json!("x"), json!("NaNx"), json!("infx"), json!("inf%"), json!({"unit": "multiplier", "value": "a"}), json!(f64::MAX.to_string() + "x")]
    {
        let r = s.execute("node.set", &json!({"ids": [t], "props": {"lineHeight": v}}));
        assert!(r.is_err() || lh(&s, t).resolve(20.0, 1.2).is_finite(), "{v}");
    }
    assert!(s.execute("node.set", &json!({"ids": [t], "props": {"lineHeight": "NaNx"}})).is_err());
}

#[test]
fn css_keeps_the_multiplier_unitless_and_the_percent_a_percent() {
    let mut s = Session::new();
    let t = text(&mut s);
    let css = |s: &Session| crate::codegen::code_for(s.doc().unwrap(), t, crate::codegen::CodeLang::Css);
    run(&mut s, "node.set", json!({"ids": [t], "props": {"lineHeight": "1.5x"}}));
    assert!(css(&s).contains("line-height: 1.5;"), "{}", css(&s));
    run(&mut s, "node.set", json!({"ids": [t], "props": {"lineHeight": "150%"}}));
    assert!(css(&s).contains("line-height: 150%;"), "{}", css(&s));
}

#[test]
fn a_bound_number_reads_in_the_unit_the_line_height_is_in() {
    let mut s = Session::new();
    number_var(&mut s, 24.0);
    let t = text(&mut s);
    // Pixels (and Auto) read it as pixels, as before.
    run(&mut s, "variable.bind", json!({"ids": [t], "field": "lineHeight", "variable": "lh"}));
    assert_eq!(lh(&s, t), LineHeight::Pixels(24.0));
    // Switching the unit keeps the binding and reads the number again.
    run(&mut s, "variable.set", json!({"variable": "lh", "value": 150}));
    run(&mut s, "node.set", json!({"ids": [t], "props": {"lineHeight": "0%"}}));
    assert_eq!(lh(&s, t), LineHeight::Percent(150.0));
    run(&mut s, "variable.set", json!({"variable": "lh", "value": 1.5}));
    run(&mut s, "node.set", json!({"ids": [t], "props": {"lineHeight": "0x"}}));
    assert_eq!(lh(&s, t), LineHeight::Multiplier(1.5));
    assert!(s.doc().unwrap().node(t).unwrap().bound.contains_key("lineHeight"), "still bound");
    run(&mut s, "variable.set", json!({"variable": "lh", "value": 2}));
    assert_eq!(lh(&s, t), LineHeight::Multiplier(2.0), "later values keep the unit");
    // The CSS variable carries the unit-less value.
    let css = crate::codegen::code_for(s.doc().unwrap(), t, crate::codegen::CodeLang::Css);
    assert!(css.contains("line-height: var(--lh, 2);"), "{css}");
}

#[test]
fn a_text_style_bound_number_reads_in_the_style_unit() {
    let mut s = Session::new();
    number_var(&mut s, 140.0);
    let t = text(&mut s);
    run(&mut s, "node.set", json!({"ids": [t], "props": {"lineHeight": "120%"}}));
    let st = run(&mut s, "style.create", json!({"type": "text", "name": "Body", "from": t}))["id"].as_str().unwrap().to_string();
    run(&mut s, "style.bind", json!({"style": st, "field": "lineHeight", "variable": "lh"}));
    let style_lh = |s: &Session| match &s.doc().unwrap().styles.iter().find(|x| x.id == st).unwrap().kind {
        figcraft_doc::StyleKind::Text { style } => Some(style.line_height),
        _ => None,
    };
    assert_eq!(style_lh(&s), Some(LineHeight::Percent(140.0)));
}
