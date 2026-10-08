//! UI journeys: drive the real egui app through its AccessKit tree with egui_kittest.
//!
//! UI steps: `click: {label}`, `fill: {label, text}`, `type: {text}`, `key: {key, command, shift}`,
//! `drag: {from: [x, y], to: [x, y]}` and `click_at: [x, y]` (page points on the canvas),
//! `expect_ui: {label, exists}`, `focus: {label}` (keyboard focus), `scroll: {dx, dy}` (mouse wheel over
//! the canvas, screen points), `run: {}`.
//! Journey options: `startup_template_picker: true` starts the app as the desktop binary does;
//! print jobs go to `<out>/print-spool/job-<n>.pdf`.
//! Observation steps are shared with headless journeys: `expect`, `let`, `dump`, `expect_pdf`,
//! `expect_png`, `expect_roundtrip` (they query the app's session).

use crate::runner::{self, Ctx, substitute};
use anyhow::{Result, anyhow, bail};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use newpub_app::NewpubApp;
use newpub_engine::Session;
use serde_json::Value;

fn key_from(name: &str) -> Option<egui::Key> {
    egui::Key::from_name(name)
}

fn settle(h: &mut Harness<'_, NewpubApp>) {
    // The app does not animate; a few steps let windows open and layouts settle.
    h.run_steps(3);
}

fn pt(v: &Value) -> Result<(f64, f64)> {
    let a = v.as_array().ok_or_else(|| anyhow!("expected [x, y]"))?;
    let n = |i: usize| {
        a.get(i).and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(newpub_engine::core::units::parse_length)))
    };
    Ok((n(0).ok_or_else(|| anyhow!("bad x"))?, n(1).ok_or_else(|| anyhow!("bad y"))?))
}

fn find<'a>(h: &'a Harness<'_, NewpubApp>, label: &'a str) -> Result<egui_kittest::Node<'a>> {
    // Prefer interactive nodes when several share a label (e.g. a text field and its caption).
    let all: Vec<_> = h.query_all_by_label(label).collect();
    if all.is_empty() {
        bail!("no UI element labelled {label:?}");
    }
    let pick = all.iter().position(|n| n.accesskit_node().role() == egui::accesskit::Role::TextInput).unwrap_or(0);
    Ok(all.into_iter().nth(pick).expect("index in range"))
}

fn ui_step(h: &mut Harness<'_, NewpubApp>, ctx: &mut Ctx, step: &Value) -> Result<()> {
    let m = step.as_object().ok_or_else(|| anyhow!("step must be a map"))?;
    let (name, raw) = m.iter().find(|(k, _)| *k != "as").ok_or_else(|| anyhow!("empty step"))?;
    let args = substitute(raw, ctx)?;
    match name.as_str() {
        "click" => {
            let label = args.get("label").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("click needs label"))?;
            find(h, label)?.click();
            settle(h);
        }
        "fill" => {
            let label = args.get("label").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("fill needs label"))?;
            let text = args.get("text").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("fill needs text"))?;
            find(h, label)?.focus();
            settle(h);
            h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
            h.run_steps(1);
            find(h, label)?.type_text(text);
            settle(h);
        }
        "type" => {
            let text = args.get("text").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("type needs text"))?;
            h.event(egui::Event::Text(text.to_string()));
            settle(h);
        }
        "key" => {
            let k = args.get("key").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("key needs key"))?;
            let key = key_from(k).ok_or_else(|| anyhow!("unknown key {k}"))?;
            let mut mods = egui::Modifiers::NONE;
            if args.get("command").and_then(|v| v.as_bool()).unwrap_or(false) {
                mods |= egui::Modifiers::COMMAND;
            }
            if args.get("shift").and_then(|v| v.as_bool()).unwrap_or(false) {
                mods |= egui::Modifiers::SHIFT;
            }
            h.key_press_modifiers(mods, key);
            settle(h);
        }
        "drag" => {
            let (x0, y0) = pt(args.get("from").ok_or_else(|| anyhow!("drag needs from"))?)?;
            let (x1, y1) = pt(args.get("to").ok_or_else(|| anyhow!("drag needs to"))?)?;
            let a = h.state().page_to_screen(x0, y0);
            let b = h.state().page_to_screen(x1, y1);
            h.hover_at(a);
            h.run_steps(1);
            h.drag_at(a);
            h.run_steps(1);
            for i in 1..=4 {
                h.hover_at(a + (b - a) * (i as f32 / 4.0));
                h.run_steps(1);
            }
            h.drop_at(b);
            settle(h);
        }
        "click_at" => {
            let (x, y) = pt(&args)?;
            let p = h.state().page_to_screen(x, y);
            h.hover_at(p);
            h.run_steps(1);
            h.drag_at(p);
            h.run_steps(1);
            h.drop_at(p);
            settle(h);
        }
        "expect_ui" => {
            let label = args.get("label").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("expect_ui needs label"))?;
            let want = args.get("exists").and_then(|v| v.as_bool()).unwrap_or(true);
            let got = h.query_by_label(label).is_some();
            if got != want {
                bail!("UI element {label:?} exists = {got}, expected {want}");
            }
        }
        "focus" => {
            let label = args.get("label").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("focus needs label"))?;
            find(h, label)?.focus();
            settle(h);
        }
        "scroll" => {
            // Mouse-wheel scroll over the canvas centre, in screen points (positive = content moves up/left).
            let dx = args.get("dx").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let dy = args.get("dy").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let c = h.state().page_to_screen(0.0, 0.0);
            h.hover_at(c + egui::Vec2::new(40.0, 40.0));
            h.run_steps(1);
            h.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::Vec2::new(-dx, -dy),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            });
            settle(h);
        }
        "run" => settle(h),
        "expect" if args.get("query").and_then(|q| q.get("q")).and_then(|q| q.as_str()) == Some("view") => {
            // App view state is answered by the app, not the engine.
            let spec = args.as_object().ok_or_else(|| anyhow!("expect needs a map"))?;
            let mut actual = h.state().view_state();
            if let Some(p) = spec.get("path").and_then(|p| p.as_str()) {
                actual = runner::pointer(&actual, p)?.clone();
            }
            runner::check_matchers(&actual, spec)?;
            if let Some(Value::String(var)) = spec.get("as") {
                ctx.vars.insert(var.clone(), actual);
            }
        }
        "expect" | "let" | "dump" | "expect_pdf" | "expect_png" | "expect_image" | "expect_html"
        | "expect_roundtrip" => {
            let s: &mut Session = &mut h.state_mut().session;
            runner::run_step(s, ctx, step)?;
        }
        other => bail!("{other:?} is not a UI journey step (UI journeys drive the app only through its UI)"),
    }
    Ok(())
}

/// Runs a UI journey. Errors carry the 1-based failing step.
pub fn run_ui_journey(script: &Value, steps: &[Value], ctx: &mut Ctx) -> Result<(), (usize, anyhow::Error)> {
    let mut session = Session::bundled();
    session.base_dir = ctx.out.clone();
    let mut app = NewpubApp::new(session);
    app.startup_picker = script.get("startup_template_picker").and_then(|v| v.as_bool()).unwrap_or(false);
    app.print_spool = Some(ctx.out.join("print-spool"));
    let mut h = Harness::builder()
        .with_size(egui::Vec2::new(1280.0, 860.0))
        .with_max_steps(64)
        .build_ui_state(|ui, app: &mut NewpubApp| app.ui(ui), app);
    settle(&mut h);
    for (i, step) in steps.iter().enumerate() {
        ui_step(&mut h, ctx, step).map_err(|e| (i + 1, e))?;
    }
    // Screenshots of the final document for the gallery.
    let shots = ctx.out.join("screens");
    let _ = std::fs::create_dir_all(&shots);
    let s = &mut h.state_mut().session;
    for p in 0..s.doc().pages.len().min(4) {
        if let Ok(png) = s.page_png(p, 48.0) {
            let _ = std::fs::write(shots.join(format!("page-{}.png", p + 1)), png);
        }
    }
    Ok(())
}
