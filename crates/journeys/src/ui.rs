//! UI journeys: drive the real egui app through AccessKit with egui_kittest.

use crate::runner::Ctx;
use anyhow::anyhow;
use serde_json::Value;

/// Runs a UI journey. Errors carry the 1-based failing step.
pub fn run_ui_journey(_steps: &[Value], _ctx: &mut Ctx) -> Result<(), (usize, anyhow::Error)> {
    Err((1, anyhow!("UI journey harness not built yet")))
}
