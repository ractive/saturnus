//! Copying between two devices, or from a folder to a device
//! (`kb/docs/saturnus-tx.md`, "Two devices at once"): a get from one into
//! memory and a put to the other, never a byte bridge between the links,
//! so each side keeps its own settings and errors.
//!
//! [`plan`] turns a selection into steps (a get and a put per variable, a
//! tree get and a tree put per directory), names what is in use on the
//! target (the host asks "Replace X?" first and sets
//! [`CopyStep::replace`]), and warns when the objects belong to the other
//! family: a 48 object going to a 49G or back. HP's own kit warned the
//! same way (wiki: protocols/xmodem-hp, Conn4x warns on a header
//! mismatch). A binary file with the other family's header is stored as a
//! string (wiki: protocols/hp-object-format, "Binary sends to the
//! calculator"; the emulated 48GX to 49G copy of `just tx-traces` shows
//! it), and a program's binary form holds ROM addresses of its own model,
//! so across families the plan copies each variable's text form, which
//! the target compiles. [`header_check`] tells the same for one file's
//! bytes (a file from a folder).

use satx_core::Model;
use satx_core::object::{BinaryHeader, Family};
use satx_core::reply::Entry;

use crate::device::{Format, Outcome, Request, Result, TreeItem, TxError};

/// The object family a model reads and writes; `None` for an unknown
/// model.
pub fn family(model: Model) -> Option<Family> {
    match model {
        Model::Hp48Sx | Model::Hp48Gx => Some(Family::Hp48),
        Model::Hp49G => Some(Family::Hp49),
        _ => None,
    }
}

/// Objects of one family going to a calculator of the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mismatch {
    /// The objects' family.
    pub from: Family,
    /// The target calculator's.
    pub to: Family,
}

impl Mismatch {
    /// The warning before a copy between calculators, which goes as text.
    pub fn message(&self) -> &'static str {
        match self.to {
            Family::Hp49 => {
                "These are HP 48 objects. They go to the HP 49G as text, which it \
                 compiles; a program may not run the same there."
            }
            Family::Hp48 => {
                "These are HP 49G objects. They go to the HP 48 as text, which it \
                 compiles; many 49G commands do not exist on a 48."
            }
        }
    }

    /// The warning before a binary file of the other family is put: the
    /// calculator stores it as a string.
    pub fn file_message(&self) -> &'static str {
        match self.to {
            Family::Hp49 => {
                "This is an HP 48 file: the HP 49G stores it as a string, not as the \
                 object."
            }
            Family::Hp48 => {
                "This is an HP 49G file: an HP 48 stores it as a string, not as the \
                 object."
            }
        }
    }
}

/// The family of a binary transfer file (`HPHP48-x`, `HPHP49-x`); `None`
/// for any other file.
pub fn file_family(data: &[u8]) -> Option<Family> {
    BinaryHeader::parse(data).map(|h| h.family)
}

/// Whether `data`, put to a `to`, crosses families. Only binary transfer
/// files are told: a `%%HP:` text file is compiled by the target itself.
pub fn header_check(data: &[u8], to: Model) -> Option<Mismatch> {
    let from = file_family(data)?;
    let to = family(to)?;
    (from != to).then_some(Mismatch { from, to })
}

/// One variable or directory to copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyStep {
    /// Its name on both sides.
    pub name: String,
    /// A directory: copied with everything in it.
    pub directory: bool,
    /// Binary for the same family, text across families.
    pub format: Format,
    /// Replace a variable of that name on the target: set by the host
    /// after asking (a directory is never replaced).
    pub replace: bool,
}

impl CopyStep {
    /// The request that gets it from the source directory `from`.
    pub fn get(&self, from: &[String]) -> Request {
        if self.directory {
            let mut dir = from.to_vec();
            dir.push(self.name.clone());
            Request::GetTree {
                dir,
                format: self.format,
            }
        } else {
            Request::Get {
                dir: from.to_vec(),
                name: self.name.clone(),
                format: self.format,
            }
        }
    }

    /// The request that puts what [`CopyStep::get`] brought into the
    /// target directory `to`.
    pub fn put(&self, to: &[String], got: Outcome) -> Result<Request> {
        match (self.directory, got) {
            (false, Outcome::File(file)) => Ok(Request::Put {
                dir: to.to_vec(),
                name: self.name.clone(),
                data: file.data,
                replace: self.replace,
                as_string: false,
            }),
            (true, Outcome::Tree(tree)) => Ok(Request::PutTree {
                dir: to.to_vec(),
                name: Some(self.name.clone()),
                items: tree.items,
                replace: false,
                as_string: false,
            }),
            (_, other) => Err(TxError::Calc(satx_core::Error::Reply(format!(
                "copy {}: the get brought {other:?}",
                self.name
            )))),
        }
    }
}

/// What a copy does and what to ask first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyPlan {
    /// The steps, in the selection's order.
    pub steps: Vec<CopyStep>,
    /// Names of the selection the target has already: a variable asks
    /// "Replace X?"; a directory must be purged first (its own question).
    pub in_use: Vec<String>,
    /// The objects cross families, so the steps copy text; `None` when
    /// the source is a folder ([`header_check`] tells each file then) or a
    /// model is unknown.
    pub mismatch: Option<Mismatch>,
}

/// The plan for copying `selection` (entries of the source listing) from
/// a `from` (`None`: a folder) to a `to` whose target directory holds
/// `target`.
pub fn plan(from: Option<Model>, to: Model, selection: &[Entry], target: &[Entry]) -> CopyPlan {
    let mismatch = match (from.and_then(family), family(to)) {
        (Some(from), Some(to)) if from != to => Some(Mismatch { from, to }),
        _ => None,
    };
    let format = if mismatch.is_some() {
        Format::Text
    } else {
        Format::Binary
    };
    let steps: Vec<CopyStep> = selection
        .iter()
        .map(|e| CopyStep {
            name: e.name.clone(),
            directory: e.is_directory(),
            format,
            replace: false,
        })
        .collect();
    let in_use = steps
        .iter()
        .filter(|s| target.iter().any(|e| e.name == s.name))
        .map(|s| s.name.clone())
        .collect();
    CopyPlan {
        steps,
        in_use,
        mismatch,
    }
}

/// The families of a tree's files that differ from a `to`'s.
pub fn tree_mismatch(items: &[TreeItem], to: Model) -> Option<Mismatch> {
    items
        .iter()
        .filter_map(|i| i.data.as_deref())
        .find_map(|d| header_check(d, to))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::device::{File, Tree};

    fn entry(name: &str, kind: &str) -> Entry {
        Entry {
            name: name.into(),
            size: 10.0,
            kind: kind.into(),
            checksum: 1,
        }
    }

    #[test]
    fn a_48_object_going_to_a_49g_is_warned_about() {
        let sel = [entry("PRG", "Program"), entry("D", "Directory")];
        let p = plan(
            Some(Model::Hp48Gx),
            Model::Hp49G,
            &sel,
            &[entry("PRG", "Program")],
        );
        assert_eq!(
            p.mismatch,
            Some(Mismatch {
                from: Family::Hp48,
                to: Family::Hp49
            })
        );
        assert!(p.mismatch.unwrap().message().contains("HP 48 objects"));
        assert_eq!(p.in_use, ["PRG"]);
        assert_eq!(p.steps.len(), 2);
        assert!(p.steps[1].directory);
        // Across families as text, which the target compiles.
        assert!(p.steps.iter().all(|s| s.format == Format::Text));
        let back = plan(Some(Model::Hp49G), Model::Hp48Sx, &sel, &[]);
        assert!(back.mismatch.unwrap().message().contains("HP 49G objects"));
        let same = plan(Some(Model::Hp48Sx), Model::Hp48Gx, &sel, &[]);
        assert_eq!(same.mismatch, None);
        assert!(same.steps.iter().all(|s| s.format == Format::Binary));
        assert_eq!(plan(None, Model::Hp49G, &sel, &[]).mismatch, None);
        assert_eq!(
            plan(Some(Model::Unknown), Model::Hp49G, &sel, &[]).mismatch,
            None
        );
    }

    #[test]
    fn headers_tell_the_family() {
        let f48 = b"HPHP48-R\x2c\x2a";
        let f49 = b"HPHP49-C\x2c\x2a";
        assert_eq!(header_check(f48, Model::Hp48Sx), None);
        assert_eq!(header_check(f48, Model::Hp49G).unwrap().from, Family::Hp48);
        assert!(
            header_check(f48, Model::Hp49G)
                .unwrap()
                .file_message()
                .contains("string")
        );
        assert_eq!(header_check(f49, Model::Hp48Gx).unwrap().to, Family::Hp48);
        assert_eq!(
            header_check(b"%%HP: T(3)A(R)F(.);\r\n1", Model::Hp49G),
            None
        );
        let items = [
            TreeItem {
                path: vec!["D".into()],
                data: None,
            },
            TreeItem {
                path: vec!["D".into(), "X".into()],
                data: Some(f49.to_vec()),
            },
        ];
        assert!(tree_mismatch(&items, Model::Hp48Sx).is_some());
        assert!(tree_mismatch(&items, Model::Hp49G).is_none());
    }

    #[test]
    fn steps_make_their_requests() {
        let var = CopyStep {
            name: "X".into(),
            directory: false,
            format: Format::Binary,
            replace: true,
        };
        let home = vec!["HOME".to_string()];
        assert!(matches!(var.get(&home), Request::Get { name, .. } if name == "X"));
        let file = Outcome::File(File {
            name: "X".into(),
            file_name: "X.hp".into(),
            format: Format::Binary,
            data: vec![1],
        });
        assert!(matches!(
            var.put(&home, file).unwrap(),
            Request::Put { replace: true, data, .. } if data == [1]
        ));
        let dir = CopyStep {
            name: "D".into(),
            directory: true,
            format: Format::Text,
            replace: false,
        };
        assert!(matches!(
            dir.get(&home),
            Request::GetTree { dir, format: Format::Text } if dir == ["HOME", "D"]
        ));
        let tree = Outcome::Tree(Tree {
            dir: vec!["HOME".into(), "D".into()],
            items: Vec::new(),
        });
        assert!(matches!(
            dir.put(&home, tree).unwrap(),
            Request::PutTree { name: Some(n), .. } if n == "D"
        ));
        assert!(dir.put(&home, Outcome::Restored).is_err());
    }
}
