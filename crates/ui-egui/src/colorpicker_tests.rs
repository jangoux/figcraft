//! The colour picker drawn headless: it opens above the window it was opened from, every time.

use egui::{Id, LayerId, Pos2, RawInput, Rect, vec2};
use figcraft_engine::Session;
use serde_json::json;

use super::*;
use crate::{FigApp, NoServices};

fn frame(ctx: &egui::Context, app: &mut FigApp, t: &mut f64, events: Vec<egui::Event>) {
    *t += 1.0 / 60.0;
    let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0))), time: Some(*t), events, ..Default::default() };
    let mut out = ctx.run_ui(input, |ui| app.ui(ui));
    out.textures_delta.clear();
}

fn click(ctx: &egui::Context, app: &mut FigApp, t: &mut f64, at: Pos2) {
    let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    frame(ctx, app, t, vec![egui::Event::PointerMoved(at)]);
    frame(ctx, app, t, vec![ev(true)]);
    frame(ctx, app, t, vec![ev(false)]);
}

#[test]
fn variable_picker_opens_above_the_variables_window_every_time() {
    let mut s = Session::new();
    let c = s.execute("variable.createCollection", &json!({"name": "Primitives"})).unwrap()["id"].clone();
    let v = s.execute("variable.create", &json!({"collection": c, "name": "primary-500", "type": "color", "value": "#FF0000"})).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let mut app = FigApp::new(s, Box::new(NoServices));
    app.ui.home = false;
    app.ui.variables = Some(Default::default());
    let ctx = egui::Context::default();
    let mut t = 0.0;
    let picker = LayerId::new(egui::Order::Middle, Id::new("color_picker"));
    for round in 0..3 {
        let mode = app.session.doc().unwrap().variables.collections[0].modes[0].id.clone();
        app.ui.picker = Some(PickerTarget::Variable { id: v.clone(), mode });
        for _ in 0..3 {
            frame(&ctx, &mut app, &mut t, vec![]);
        }
        assert!(app.ui.picker.is_some(), "round {round}: picker closed");
        let r = ctx.memory(|m| m.area_rect(picker.id)).expect("picker drawn");
        assert_eq!(ctx.layer_id_at(r.center()), Some(picker), "round {round}: the picker is under another window");
        // Close it, then click in the variables window (which raises that window).
        app.ui.picker = None;
        frame(&ctx, &mut app, &mut t, vec![]);
        let w = ctx.memory(|m| m.area_rect(Id::new("variables_window"))).expect("variables window drawn");
        click(&ctx, &mut app, &mut t, w.left_bottom() + vec2(320.0, -40.0));
    }
}
