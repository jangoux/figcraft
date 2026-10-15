//! Colour aliases with an opacity on top: a fixed percentage or a number variable's, following
//! the aliased colour across modes, through bound layers, deletes, undo and the export formats.

use serde_json::json;

use crate::Session;
use crate::params::id_of;
use figcraft_doc::{VariableValue, Variables};

fn alpha(vars: &Variables, id: &str, modes: &std::collections::BTreeMap<String, String>) -> Option<(String, f32)> {
    match vars.resolve(id, modes)? {
        VariableValue::Color(c) => Some((c.with_alpha(1.0).to_hex(), (c.a * 100.0).round())),
        _ => None,
    }
}

fn tokens() -> Session {
    let mut s = Session::new();
    s.execute("variable.createCollection", &json!({"name": "Primitives", "modes": ["Light", "Dark"]})).unwrap();
    s.execute(
        "variable.create",
        &json!({"collection": "Primitives", "name": "primary-500", "type": "color", "values": {"Light": "#FF0000", "Dark": "#0000FF80"}}),
    )
    .unwrap();
    s.execute("variable.create", &json!({"collection": "Primitives", "name": "opacity/disabled", "type": "number", "value": 40})).unwrap();
    s.execute("variable.createCollection", &json!({"name": "Semantic"})).unwrap();
    s.execute("variable.create", &json!({"collection": "Semantic", "name": "primary/muted", "type": "color", "value": "#000000"})).unwrap();
    s
}

fn id(s: &Session, name: &str) -> String {
    s.doc().unwrap().variables.get(name).unwrap().id.clone()
}

#[test]
fn a_fixed_opacity_scales_the_aliased_colour_in_every_mode() {
    let mut s = tokens();
    s.execute("variable.set", &json!({"variable": "primary/muted", "value": {"alias": "primary-500", "opacity": 0.5}})).unwrap();
    let vars = &s.doc().unwrap().variables;
    let light = Default::default();
    assert_eq!(alpha(vars, "primary/muted", &light), Some(("FF0000".into(), 50.0)));
    // The target's own alpha multiplies: 50% of 50%.
    let coll = vars.collection("Primitives").unwrap();
    let dark = [(coll.id.clone(), coll.modes[1].id.clone())].into_iter().collect();
    assert_eq!(alpha(vars, "primary/muted", &dark), Some(("0000FF".into(), 25.0)));
    // Stored by id, as an alias with an opacity.
    let target = id(&s, "primary-500");
    let v = s.doc().unwrap().variables.get("primary/muted").unwrap().values.values().next().cloned();
    assert_eq!(v, Some(VariableValue::AliasOpacity { variable: target, opacity: 0.5, opacity_variable: None }));
    // No opacity: a plain alias again.
    s.execute("variable.set", &json!({"variable": "primary/muted", "value": {"alias": "primary-500", "opacity": null}})).unwrap();
    assert!(matches!(s.doc().unwrap().variables.get("primary/muted").unwrap().values.values().next(), Some(VariableValue::Alias(_))));
}

#[test]
fn an_opacity_variable_holds_a_percentage_and_bound_layers_follow_it() {
    let mut s = tokens();
    let r = id_of(&s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10})).unwrap()["id"]).unwrap();
    s.execute("variable.set", &json!({"variable": "primary/muted", "value": {"alias": "primary-500", "opacity": {"alias": "opacity/disabled"}}}))
        .unwrap();
    s.execute("variable.bind", &json!({"ids": [r], "field": "fill", "variable": "primary/muted"})).unwrap();
    let fill = |s: &Session| {
        s.doc().unwrap().node(r).unwrap().fills.first().and_then(|p| p.solid_color()).map(|c| (c.with_alpha(1.0).to_hex(), (c.a * 100.0).round()))
    };
    assert_eq!(fill(&s), Some(("FF0000".into(), 40.0)));
    s.execute("variable.set", &json!({"variable": "opacity/disabled", "value": 70})).unwrap();
    assert_eq!(fill(&s), Some(("FF0000".into(), 70.0)));
    s.execute("variable.set", &json!({"variable": "primary-500", "mode": "Light", "value": "#00FF00"})).unwrap();
    assert_eq!(fill(&s), Some(("00FF00".into(), 70.0)));
    // Undo steps back through the edits.
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(fill(&s), Some(("FF0000".into(), 70.0)));
    // Deleting the opacity variable keeps the colour the alias shows.
    s.execute("variable.delete", &json!({"variable": "opacity/disabled"})).unwrap();
    let v = s.doc().unwrap().variables.get("primary/muted").unwrap().values.values().next().cloned();
    assert!(matches!(v, Some(VariableValue::Color(c)) if c.to_hex() == "FF0000" && (c.a - 0.7).abs() < 1e-6), "{v:?}");
}

#[test]
fn bad_opacities_are_refused_or_clamped() {
    let mut s = tokens();
    let set = |s: &mut Session, value| s.execute("variable.set", &json!({"variable": "primary/muted", "value": value}));
    assert!(set(&mut s, json!({"alias": "primary-500", "opacity": {"alias": "primary-500"}})).is_err(), "not a number variable");
    assert!(set(&mut s, json!({"alias": "primary-500", "opacity": {"alias": "nope"}})).is_err());
    assert!(set(&mut s, json!({"alias": "primary-500", "opacity": "lots"})).is_err());
    assert!(s.execute("variable.set", &json!({"variable": "opacity/disabled", "value": {"alias": "opacity/disabled", "opacity": 0.5}})).is_err());
    set(&mut s, json!({"alias": "primary-500", "opacity": 7})).unwrap();
    assert_eq!(alpha(&s.doc().unwrap().variables, "primary/muted", &Default::default()), Some(("FF0000".into(), 100.0)));
    // Loops through an alias with an opacity are refused too.
    assert!(s.execute("variable.set", &json!({"variable": "primary-500", "value": {"alias": "primary/muted"}})).is_err());
    // variable.create takes one as well.
    s.execute(
        "variable.create",
        &json!({"collection": "Semantic", "name": "ghost", "type": "color", "value": {"alias": "primary-500", "opacity": 0.1}}),
    )
    .unwrap();
    assert_eq!(alpha(&s.doc().unwrap().variables, "ghost", &Default::default()), Some(("FF0000".into(), 10.0)));
}

#[test]
fn exports_mix_in_css_and_resolve_elsewhere() {
    let mut s = tokens();
    s.execute("variable.set", &json!({"variable": "primary/muted", "value": {"alias": "primary-500", "opacity": 0.4}})).unwrap();
    s.execute("variable.create", &json!({"collection": "Semantic", "name": "primary/disabled", "type": "color", "value": {"alias": "primary-500", "opacity": {"alias": "opacity/disabled"}}}))
        .unwrap();
    let css = s.execute("variable.export", &json!({"format": "css"})).unwrap()["text"].as_str().unwrap().to_string();
    assert!(css.contains("--primary-muted: color-mix(in srgb, var(--primary-500) 40%, transparent);"), "{css}");
    assert!(css.contains("--primary-disabled: color-mix(in srgb, var(--primary-500) calc(var(--opacity-disabled) * 1%), transparent);"), "{css}");
    // The variables JSON has no such alias: the colour it resolves to.
    let text = s.execute("variable.export", &json!({})).unwrap()["text"].as_str().unwrap().to_string();
    let j: serde_json::Value = serde_json::from_str(&text).unwrap();
    let muted = id(&s, "primary/muted");
    let vals = &j["meta"]["variables"][&muted]["valuesByMode"];
    let v = vals.as_object().unwrap().values().next().unwrap();
    assert_eq!((v["r"].as_f64(), v["a"].as_f64().map(|a| (a * 100.0).round())), (Some(1.0), Some(40.0)), "{v}");
    // Saved files keep it.
    let back: Variables = serde_json::from_value(serde_json::to_value(&s.doc().unwrap().variables).unwrap()).unwrap();
    assert_eq!(back, s.doc().unwrap().variables);
    // A .fig export writes the resolved colour.
    assert!(figcraft_format::fig::export(s.doc().unwrap(), None).is_ok());
}
