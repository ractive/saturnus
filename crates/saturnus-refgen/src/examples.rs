//! Examples generated on the emulator: each curated input
//! ([`crate::reference::ExampleSpec`]) runs through `eval` from the same
//! booted state, and its typed input and result (or the calculator's
//! error) are recorded, as `data/commands/examples-<model>.json`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use saturnus::Model;
use saturnus_mcp::emulator::Emulator;
use saturnus_mcp::object::Object;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::catalog::{Catalog, model_name};
use crate::reference::{ExampleSpec, Reference};

/// Emulated-time limit of each evaluation.
const LIMIT: Duration = Duration::from_secs(20);
/// Set on the 49G before every example: RPN (-95 clear), CAS silent mode
/// (-120 set), and an empty stack (the boot leaves what SERVER typed in
/// algebraic mode).
pub const SETUP_49G: &str = "-120. SF";
/// Most stack levels recorded.
const LEVELS: usize = 16;

/// One generated example.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Example {
    /// Run first, not shown as input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
    /// The arguments' source.
    pub input: String,
    /// What ran on them.
    pub run: String,
    /// The input stack typed, level 1 first.
    pub stack: Vec<Value>,
    /// The stack afterwards typed, level 1 first (on an error: as the
    /// calculator left it, display text only in `display`).
    pub result: Vec<Value>,
    /// The display text of the stack afterwards, level 1 first.
    pub display: Vec<String>,
    /// The calculator's error message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Object types in and out, deepest level first, e.g. `real real →
    /// real`.
    pub effect: String,
}

/// A model's examples file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Examples {
    /// `48sx`, `48gx` or `49g`.
    pub model: String,
    /// Examples by command.
    pub examples: BTreeMap<String, Vec<Example>>,
    /// Commands without examples and why.
    pub skipped: BTreeMap<String, String>,
}

/// An object as JSON, without the raw nibbles of objects the decoder does
/// not know (graphics), which would only bloat the file.
fn object_json(o: &Object) -> Result<Value> {
    let mut v = serde_json::to_value(o)?;
    strip_hex(&mut v);
    Ok(v)
}

fn strip_hex(v: &mut Value) {
    match v {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("unknown") {
                map.remove("hex");
                map.remove("truncated");
            }
            for x in map.values_mut() {
                strip_hex(x);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(strip_hex),
        _ => {}
    }
}

/// The type word of a typed object in an effect.
fn type_word(v: &Value) -> String {
    let t = v.get("type").and_then(Value::as_str).unwrap_or("?");
    if t == "unknown" {
        v.get("kind").and_then(Value::as_str).map_or_else(
            || "object".to_string(),
            |k| k.to_lowercase().replace(' ', "_"),
        )
    } else {
        t.to_string()
    }
}

/// `real real → real` from inputs and results given level 1 first.
pub fn effect(input: &[Value], result: &[Value], error: Option<&str>) -> String {
    let side =
        |v: &[Value]| -> String { v.iter().rev().map(type_word).collect::<Vec<_>>().join(" ") };
    let out = match error {
        Some(e) => format!("error: {e}"),
        None => side(result),
    };
    format!("{} → {}", side(input), out).trim().to_string()
}

/// Runs examples on one model, each from the same booted state (on the
/// 49G with [`SETUP_49G`]).
#[derive(Debug)]
pub struct Runner {
    emu: Emulator,
    state: PathBuf,
}

impl Drop for Runner {
    fn drop(&mut self) {
        // Best effort: a leftover file in the temporary directory.
        let _ = std::fs::remove_file(&self.state);
    }
}

impl Runner {
    /// Boot `model` from `rom` and keep its state.
    pub fn new(model: Model, rom: &Path) -> Result<Self> {
        let mut emu = crate::catalog::boot(model, rom)?;
        if model == Model::Hp49g {
            // RPN entry, and the CAS switches modes itself instead of
            // asking (a question box would wait for a key).
            emu.start_server()?;
            let reply = emu.run_command(SETUP_49G)?;
            emu.stop_server()?;
            if let Some(e) = reply.error {
                anyhow::bail!("cannot set up the 49G: {e}");
            }
        }
        let state = std::env::temp_dir().join(format!(
            "saturnus-refgen-{}-{}-examples.state",
            std::process::id(),
            model_name(model)
        ));
        emu.save_state(&state, false)?;
        Ok(Runner { emu, state })
    }

    /// Run `spec` for command `name`.
    pub fn run(&mut self, name: &str, spec: &ExampleSpec) -> Result<Example> {
        self.emu.load_state(&self.state)?;
        self.emu.start_server()?;
        let run = spec.run.clone().unwrap_or_else(|| name.to_string());
        let mut example = Example {
            setup: spec.setup.clone(),
            input: spec.input.clone(),
            run: run.clone(),
            stack: Vec::new(),
            result: Vec::new(),
            display: Vec::new(),
            error: None,
            effect: String::new(),
        };
        if let Some(setup) = &spec.setup {
            match self.emu.eval(setup, 0, LIMIT) {
                Ok(Ok(_)) => self.emu.clear_stack()?,
                Ok(Err(e)) => {
                    example.error = Some(format!("in setup: {}", e.error));
                    return Ok(example);
                }
                Err(e) => {
                    example.error = Some(format!("in setup: {e:#}"));
                    return Ok(example);
                }
            }
        }
        if !spec.input.trim().is_empty() {
            match self.emu.eval(&spec.input, LEVELS, LIMIT) {
                Ok(Ok(levels)) => {
                    example.stack = levels
                        .levels
                        .iter()
                        .map(object_json)
                        .collect::<Result<_>>()?;
                }
                Ok(Err(e)) => {
                    example.error = Some(format!("in input: {}", e.error));
                    return Ok(example);
                }
                Err(e) => {
                    example.error = Some(format!("in input: {e:#}"));
                    return Ok(example);
                }
            }
        }
        match self.emu.eval(&run, LEVELS, LIMIT) {
            Ok(Ok(levels)) => {
                example.result = levels
                    .levels
                    .iter()
                    .map(object_json)
                    .collect::<Result<_>>()?;
                example.display = levels.display;
            }
            Ok(Err(e)) => {
                example.display = e.display;
                example.error = Some(e.error);
            }
            Err(e) => example.error = Some(format!("{e:#}")),
        }
        example.effect = effect(&example.stack, &example.result, example.error.as_deref());
        Ok(example)
    }
}

/// Generate the examples of every command in `catalog` that `reference`
/// has inputs for. `only` limits the run to those names (for checking a
/// batch while writing it).
pub fn generate(
    model: Model,
    rom: &Path,
    catalog: &Catalog,
    reference: &Reference,
    only: Option<&[String]>,
) -> Result<Examples> {
    let mut runner = Runner::new(model, rom)?;
    let m = model_name(model);
    let mut out = Examples {
        model: m.to_string(),
        examples: BTreeMap::new(),
        skipped: BTreeMap::new(),
    };
    for cmd in &catalog.commands {
        if only.is_some_and(|o| !o.contains(&cmd.name)) {
            continue;
        }
        let Some(entry) = reference.commands.get(&cmd.name) else {
            out.skipped
                .insert(cmd.name.clone(), "no reference entry yet".into());
            continue;
        };
        let specs: Vec<&ExampleSpec> = entry.examples.iter().filter(|s| s.on(m)).collect();
        if specs.is_empty() {
            let why = entry
                .skip
                .clone()
                .unwrap_or_else(|| "no example for this model".into());
            out.skipped.insert(cmd.name.clone(), why);
            continue;
        }
        let mut list = Vec::new();
        for spec in specs {
            list.push(
                runner
                    .run(&cmd.name, spec)
                    .with_context(|| format!("example of {} ({})", cmd.name, spec.input))?,
            );
        }
        out.examples.insert(cmd.name.clone(), list);
    }
    Ok(out)
}

/// The examples as the file holds them: pretty JSON and a final newline.
pub fn to_file_text(examples: &Examples) -> Result<String> {
    Ok(serde_json::to_string_pretty(examples)? + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn effects_read_deepest_first() {
        let input = [json!({"type": "real"}), json!({"type": "list"})];
        let result = [json!({"type": "unknown", "kind": "Graphic"})];
        assert_eq!(effect(&input, &result, None), "list real → graphic");
        assert_eq!(
            effect(&input, &[], Some("Bad Argument Type")),
            "list real → error: Bad Argument Type"
        );
        assert_eq!(effect(&[], &[], None), "→");
    }

    #[test]
    fn unknown_objects_lose_their_nibbles() {
        let mut v = json!([{"type": "unknown", "hex": "abc", "truncated": true, "nibbles": 3}]);
        strip_hex(&mut v);
        assert_eq!(v, json!([{"type": "unknown", "nibbles": 3}]));
    }
}
