//! The curated half of the reference, `data/commands/reference.json`:
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
    /// Only on these models (`48sx`, `48gx`, `49g`); all when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
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
    /// One or two sentences, ours.
    pub description: String,
    /// The stack effect, arguments before `→`, results after, level 1
    /// last, e.g. `x y → x+y`; derived from the runs and checked against
    /// the manuals' stack diagrams.
    pub stack: String,
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
}
