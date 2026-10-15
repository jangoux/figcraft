//! Font sizes in rem: sizes typed as `"1.5rem"` are the file's root font size (16 px by default)
//! times that, follow the root when it changes, stay in rem in code and read bound number
//! variables in rem.

use serde_json::{Value, json};

use crate::Session;
use crate::params::id_of;
use figcraft_doc::{NodeId, StyleKind, TextStyle};

fn run(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

fn text(s: &mut Session, size: Value) -> NodeId {
    let t = id_of(&run(s, "text.create", json!({"text": "Hello", "x": 0, "y": 0}))["id"]).unwrap();
    run(s, "node.set", json!({"ids": [t], "props": {"fontSize": size}}));
    t
}

fn style(s: &Session, t: NodeId) -> TextStyle {
    s.doc().unwrap().node(t).unwrap().kind.text().unwrap().style.clone()
}

fn css(s: &Session, t: NodeId) -> String {
    crate::codegen::code_for(s.doc().unwrap(), t, crate::codegen::CodeLang::Css)
}

#[test]
fn a_size_in_rem_is_the_root_size_times_it() {
    let mut s = Session::new();
    let t = text(&mut s, json!("1.5rem"));
    assert_eq!((style(&s, t).font_size, style(&s, t).rem(16.0)), (24.0, Some(1.5)));
    for (v, px) in [(json!(" 0.875REM "), 14.0), (json!("2 rem"), 32.0)] {
        run(&mut s, "node.set", json!({"ids": [t], "props": {"fontSize": v}}));
        assert_eq!(style(&s, t).font_size, px, "{v}");
    }
    // Pixels again, by number or "px".
    for v in [json!(20), json!("20px")] {
        run(&mut s, "node.set", json!({"ids": [t], "props": {"fontSize": v}}));
        assert_eq!((style(&s, t).font_size, style(&s, t).rem(16.0)), (20.0, None), "{v}");
    }
    // Clamped to 1–5000 px, the rem value with it.
    run(&mut s, "node.set", json!({"ids": [t], "props": {"fontSize": "1000rem"}}));
    assert_eq!((style(&s, t).font_size, style(&s, t).rem(16.0)), (5000.0, Some(312.5)));
}

#[test]
fn junk_sizes_are_refused() {
    let mut s = Session::new();
    let t = text(&mut s, json!(12));
    for v in [json!("rem"), json!("NaNrem"), json!("-1rem"), json!("0rem"), json!("1e400rem"), json!("infpx"), json!("é"), json!("ⅹrem")] {
        assert!(s.execute("node.set", &json!({"ids": [t], "props": {"fontSize": v}})).is_err(), "{v}");
        assert_eq!(style(&s, t).font_size, 12.0, "{v}");
    }
    for v in [json!(0), json!("abc"), json!(-3), json!(null)] {
        assert!(s.execute("file.rootFontSize", &json!({"size": v})).is_err() || s.doc().unwrap().root_font_size() == 16.0, "{v}");
    }
}

#[test]
fn changing_the_root_resizes_what_is_in_rem_only() {
    let mut s = Session::new();
    let rem = text(&mut s, json!("1.5rem"));
    let px = text(&mut s, json!(24));
    let st = run(&mut s, "style.create", json!({"type": "text", "name": "Body", "from": rem}))["id"].as_str().unwrap().to_string();
    assert_eq!(run(&mut s, "file.rootFontSize", json!({}))["size"], json!(16.0));
    run(&mut s, "file.rootFontSize", json!({"size": 20}));
    assert_eq!(style(&s, rem).font_size, 30.0);
    assert_eq!(style(&s, rem).rem(20.0), Some(1.5));
    assert_eq!(style(&s, px).font_size, 24.0, "a px size stays");
    let StyleKind::Text { style: def } = &s.doc().unwrap().styles.iter().find(|x| x.id == st).unwrap().kind else { panic!() };
    assert_eq!(def.font_size, 30.0, "a text style in rem follows too");
    assert!(s.doc().unwrap().node(rem).unwrap().height > 0.0);
    s.undo().unwrap();
    assert_eq!((s.doc().unwrap().root_font_size(), style(&s, rem).font_size), (16.0, 24.0), "one undo step");
}

#[test]
fn a_one_pixel_step_leaves_rem() {
    let mut s = Session::new();
    let t = text(&mut s, json!("1rem"));
    run(&mut s, "text.sizeUp", json!({"ids": [t]}));
    assert_eq!((style(&s, t).font_size, style(&s, t).rem(16.0)), (17.0, None));
    run(&mut s, "file.rootFontSize", json!({"size": 10}));
    assert_eq!(style(&s, t).font_size, 17.0);
}

#[test]
fn code_keeps_rem_and_rem_exports_use_the_file_root() {
    let mut s = Session::new();
    let t = text(&mut s, json!("0.875rem"));
    assert!(css(&s, t).contains("font-size: 0.875rem;"), "{}", css(&s, t));
    run(&mut s, "node.set", json!({"ids": [t], "props": {"fontSize": 20}}));
    assert!(css(&s, t).contains("font-size: 20px;"), "{}", css(&s, t));
    run(&mut s, "file.rootFontSize", json!({"size": 10}));
    let code = run(&mut s, "inspect.code", json!({"id": t, "unit": "rem"}))["code"].as_str().unwrap_or_default().to_string();
    assert!(code.contains("font-size: 2rem;"), "{code}");
}

#[test]
fn a_bound_number_reads_in_rem_when_the_size_is_in_rem() {
    let mut s = Session::new();
    run(&mut s, "variable.createCollection", json!({"name": "C"}));
    run(&mut s, "variable.create", json!({"collection": "C", "name": "size", "type": "number", "value": 2}));
    let t = text(&mut s, json!("1rem"));
    run(&mut s, "variable.bind", json!({"ids": [t], "field": "fontSize", "variable": "size"}));
    assert_eq!((style(&s, t).font_size, style(&s, t).rem(16.0)), (32.0, Some(2.0)));
    let px = text(&mut s, json!(12));
    run(&mut s, "variable.bind", json!({"ids": [px], "field": "fontSize", "variable": "size"}));
    assert_eq!(style(&s, px).font_size, 2.0, "pixels as before");
    assert!(css(&s, t).contains("font-size: var(--size, 2rem);"), "{}", css(&s, t));
}

#[test]
fn the_root_and_rem_sizes_survive_saving() {
    let mut s = Session::new();
    let t = text(&mut s, json!("1.25rem"));
    run(&mut s, "file.rootFontSize", json!({"size": 18}));
    let bytes = figcraft_format::save_bytes(s.doc().unwrap(), None).unwrap();
    let (doc, _) = figcraft_format::load_bytes(&bytes).unwrap();
    assert_eq!(doc.root_font_size(), 18.0);
    assert_eq!(doc.node(t).unwrap().kind.text().unwrap().style.rem(18.0), Some(1.25));
}
