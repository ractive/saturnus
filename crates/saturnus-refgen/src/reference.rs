//! The curated half of the reference, `crates/saturnus-cli/data/commands/reference.json`:
//! our own description of every command, its stack effect, and the
//! inputs the generator runs ([`crate::examples`]). Written by hand (by
//! us, never copied from HP's manuals), kept sorted by name.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One example to run: `setup` (not shown as input), then `input` (the
/// arguments, shown as the input stack), then `run` (default: the
/// command's name).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExampleSpec {
    /// Source run first, e.g. `5. 'X' STO`; its stack is cleared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
    /// The arguments as RPL source, e.g. `3. 4.`; may be empty.
    #[serde(default)]
    pub input: String,
    /// What runs on them; the command's name when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// Only on these models (`48sx`, `48gx`, `49g`, `28s`); all when
    /// absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    /// What the example shows, where the input and result do not say it
    /// (`kb/docs/example-comments.md`), shown under it. Never run and never
    /// in `examples-<model>.json`: `scripts/commands-json.py` and `saturnus
    /// ref` join it to the generated example.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

impl ExampleSpec {
    /// Whether the example runs on `model`.
    pub fn on(&self, model: &str) -> bool {
        self.models
            .as_ref()
            .is_none_or(|m| m.iter().any(|x| x == model))
    }
}

/// The curated entry of one command.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// What the mnemonic stands for ("Residual" for RSD), only where that
    /// says more than the mnemonic itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// One or two sentences, ours.
    pub description: String,
    /// The stack effect, arguments before `→`, results after, level 1
    /// last, e.g. `x y → x+y`; derived from the runs and checked against
    /// the manuals' stack diagrams.
    pub stack: String,
    /// A remark shown on its own line after the description, such as a
    /// ROM bug.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The category where the ROM's menus give none (the catalog's own
    /// category always wins).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Inputs to run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<ExampleSpec>,
    /// Why there is no example (interactive, plotting, I/O, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip: Option<String>,
    /// The text of a model on which the command differs from what
    /// `description`, `stack` and `note` say (the 28S's RND takes one argument),
    /// by model.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub per_model: BTreeMap<String, ModelText>,
}

/// A model's own text for a command.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelText {
    /// What the mnemonic stands for on that model, where it differs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// One or two sentences, ours; the shared one when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The stack effect on that model; the shared one when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    /// That model's note (a ROM bug, say), shown after the description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The whole curated file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    /// Entries by command name.
    pub commands: BTreeMap<String, Entry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn examples_select_models() {
        let all = ExampleSpec::default();
        assert!(all.on("48sx"));
        let gx = ExampleSpec {
            models: Some(vec!["48gx".into(), "49g".into()]),
            ..ExampleSpec::default()
        };
        assert!(!gx.on("48sx") && gx.on("49g"));
    }

    #[test]
    fn the_file_round_trips() {
        // Field for field (the file's key order is its own): nothing is
        // dropped, nothing absent is filled in.
        let text = include_str!("../../saturnus-cli/data/commands/reference.json");
        let parsed: Reference = serde_json::from_str(text).unwrap();
        let raw: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(serde_json::to_value(&parsed).unwrap(), raw);
        // A model's own note alone: description and stack are the shared ones.
        let own = &parsed.commands["HMS+"].per_model["48sx"];
        assert!(own.description.is_none() && own.stack.is_none() && own.note.is_some());
        // An example's comment is kept with its spec.
        assert_eq!(
            parsed.commands["HMS+"].examples[0].comment.as_deref(),
            Some("1h30m plus 2h45m gives 4h15m")
        );
    }
}
