//! The requests that change the calculator or walk its directories:
//! make, rename and purge (with the count), get and put a directory tree,
//! the memory check, backup, restore and the graphics screen. Each runs as
//! a sequence of `satx-core` machine operations on the device's session
//! (see [`crate::device`]).
//!
//! Wiki references (calculator-knowledgebase): `protocols/server-commands`
//! (`ARCHIVE` to `:0:`, `RESTORE`, `G D` and its free-memory header, a
//! missing port object purged without error).

use std::collections::{HashMap, HashSet, hash_map};

use satx_core::calc::validate_name;
use satx_core::grob::Grob;
use satx_core::machine::{Op, Reply};
use satx_core::reply::{Entry, Listing};

use crate::device::{
    Backup, Ctx, Event, Memory, Outcome, Pict, Result, Tree, TreeItem, TxError, cd, expect_done,
    get, list_dir, put_in, unexpected,
};
use crate::names::FileKind;
use crate::{File, Format};

/// `name` as the calculator takes it, else [`TxError::BadName`].
fn check_name(name: &str) -> Result<()> {
    validate_name(name).map_err(|_| TxError::BadName(name.to_string()))
}

/// `dir` and below it `rest`.
fn join(dir: &[String], rest: &[String]) -> Vec<String> {
    dir.iter().chain(rest).cloned().collect()
}

/// A path for messages: the names with `/` between them.
fn shown(path: &[String]) -> String {
    path.join("/")
}

pub(crate) async fn mkdir(ctx: &Ctx, dir: &[String], name: String) -> Result<Outcome> {
    ctx.session(|_| ())?;
    check_name(&name)?;
    let listing = list_dir(ctx, dir).await?;
    if listing.entries.iter().any(|e| e.name == name) {
        return Err(TxError::NameClash(name));
    }
    expect_done(ctx.op(Op::Mkdir(name.clone())).await?)?;
    Ok(Outcome::Created(name))
}

pub(crate) async fn rename(ctx: &Ctx, dir: &[String], from: String, to: String) -> Result<Outcome> {
    ctx.session(|_| ())?;
    check_name(&from)?;
    check_name(&to)?;
    let listing = list_dir(ctx, dir).await?;
    if !listing.entries.iter().any(|e| e.name == from) {
        return Err(TxError::NotFound(from));
    }
    if from != to {
        if listing.entries.iter().any(|e| e.name == to) {
            return Err(TxError::NameClash(to));
        }
        expect_done(
            ctx.op(Op::Rename {
                from: from.clone(),
                to: to.clone(),
            })
            .await?,
        )?;
    }
    Ok(Outcome::Renamed { from, to })
}

/// How many variables the directory `dir` holds, at every level (its
/// directories count as variables too).
async fn count_tree(ctx: &Ctx, dir: &[String]) -> Result<usize> {
    let mut count = 0;
    let mut todo = vec![dir.to_vec()];
    while let Some(path) = todo.pop() {
        let listing = list_dir(ctx, &path).await?;
        count += listing.entries.len();
        for e in listing.entries.iter().filter(|e| e.is_directory()) {
            todo.push(join(&path, std::slice::from_ref(&e.name)));
        }
    }
    Ok(count)
}

pub(crate) async fn purge(
    ctx: &Ctx,
    dir: &[String],
    name: String,
    confirmed: Option<usize>,
) -> Result<Outcome> {
    ctx.session(|_| ())?;
    check_name(&name)?;
    let listing = list_dir(ctx, dir).await?;
    let entry = listing
        .entries
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| TxError::NotFound(name.clone()))?;
    let mut count = 0;
    if entry.is_directory() {
        count = count_tree(ctx, &join(dir, std::slice::from_ref(&name))).await?;
        if count > 0 && confirmed != Some(count) {
            return Err(TxError::NotEmpty { name, count });
        }
        cd(ctx, dir).await?;
    }
    // satx-core's remove lists once more and sends `PGDIR` for a
    // directory, `PURGE` for a variable.
    expect_done(ctx.op(Op::Remove(name.clone())).await?)?;
    Ok(Outcome::Purged { name, count })
}

pub(crate) async fn get_tree(ctx: &Ctx, dir: Vec<String>, format: Format) -> Result<Outcome> {
    ctx.session(|_| ())?;
    let mut items = Vec::new();
    // Below `dir`; a directory's variables are got before its
    // directories are opened, so each directory is entered once.
    let mut todo: Vec<Vec<String>> = vec![Vec::new()];
    while let Some(rel) = todo.pop() {
        let here = join(&dir, &rel);
        let listing = list_dir(ctx, &here).await?;
        let mut dirs = Vec::new();
        for e in listing.entries {
            let path = join(&rel, std::slice::from_ref(&e.name));
            ctx.event(Event::Item(join(&dir, &path)));
            if e.is_directory() {
                items.push(TreeItem {
                    path: path.clone(),
                    data: None,
                });
                dirs.push(path);
                continue;
            }
            match get(ctx, &here, e.name, format).await? {
                Outcome::File(File { data, .. }) => items.push(TreeItem {
                    path,
                    data: Some(data),
                }),
                other => {
                    return Err(TxError::Calc(satx_core::Error::Reply(format!(
                        "GET: unexpected result {other:?}"
                    ))));
                }
            }
        }
        todo.extend(dirs.into_iter().rev());
    }
    // The order got: a directory's variables, then each of its
    // directories in turn; every directory comes before its contents.
    Ok(Outcome::Tree(Tree { dir, items }))
}

/// [`crate::Request::PutTree`] without its directory.
pub(crate) struct PutTree {
    pub(crate) name: Option<String>,
    pub(crate) items: Vec<TreeItem>,
    pub(crate) replace: bool,
    pub(crate) as_string: bool,
}

/// A tree's items sorted for a put: the directories to have (below the
/// root, parents first), the files, and every directory's children (its
/// directories and files) in the tree's order, as `(parent, name)`.
struct Layout {
    dirs: Vec<Vec<String>>,
    files: Vec<(Vec<String>, String, Vec<u8>)>,
    children: Vec<(Vec<String>, String)>,
}

fn layout(items: Vec<TreeItem>, as_string: bool) -> Result<Layout> {
    let mut dirs: Vec<Vec<String>> = Vec::new();
    let mut seen: HashSet<Vec<String>> = HashSet::new();
    let mut files = Vec::new();
    let mut children = Vec::new();
    let add_dir = |path: &[String],
                   dirs: &mut Vec<Vec<String>>,
                   children: &mut Vec<(Vec<String>, String)>| {
        for n in 1..=path.len() {
            let prefix = path[..n].to_vec();
            if !dirs.contains(&prefix) {
                children.push((path[..n - 1].to_vec(), path[n - 1].clone()));
                dirs.push(prefix);
            }
        }
    };
    for item in items {
        let Some((name, parent)) = item.path.split_last() else {
            return Err(TxError::BadName(String::new()));
        };
        for part in &item.path {
            check_name(part)?;
        }
        if !seen.insert(item.path.clone()) {
            return Err(TxError::Duplicate(shown(&item.path)));
        }
        add_dir(parent, &mut dirs, &mut children);
        match item.data {
            None => add_dir(&item.path, &mut dirs, &mut children),
            Some(data) => {
                if FileKind::of(&data) == FileKind::Other && !as_string {
                    return Err(TxError::NotAnObject);
                }
                children.push((parent.to_vec(), name.clone()));
                files.push((parent.to_vec(), name.clone(), data));
            }
        }
    }
    for (parent, name, _) in &files {
        if dirs.contains(&join(parent, std::slice::from_ref(name))) {
            return Err(TxError::Duplicate(shown(&join(
                parent,
                std::slice::from_ref(name),
            ))));
        }
    }
    Ok(Layout {
        dirs,
        files,
        children,
    })
}

pub(crate) async fn put_tree(ctx: &Ctx, dir: &[String], tree: PutTree) -> Result<Outcome> {
    ctx.session(|_| ())?;
    if let Some(name) = &tree.name {
        check_name(name)?;
    }
    let Layout {
        dirs,
        files,
        children,
    } = layout(tree.items, tree.as_string)?;
    let root: Vec<String> = join(dir, tree.name.as_slice());

    // What exists already, by directory below the root: nothing in a new
    // directory; in `dir` itself, the directories the tree shares with it.
    let mut existing: HashMap<Vec<String>, Vec<Entry>> = HashMap::new();
    let top = list_dir(ctx, dir).await?;
    if let Some(name) = &tree.name {
        if top.entries.iter().any(|e| &e.name == name) {
            return Err(TxError::NameClash(name.clone()));
        }
    } else {
        existing.insert(Vec::new(), top.entries);
        for path in &dirs {
            let Some((name, parent)) = path.split_last() else {
                continue;
            };
            let Some(entry) = existing
                .get(parent)
                .and_then(|entries| entries.iter().find(|e| &e.name == name))
            else {
                continue;
            };
            if !entry.is_directory() {
                return Err(TxError::NotADirectory(shown(&join(dir, path))));
            }
            let listing: Listing = list_dir(ctx, &join(dir, path)).await?;
            existing.insert(path.clone(), listing.entries);
        }
        let mut in_use = Vec::new();
        for (parent, name, _) in &files {
            let found = existing
                .get(parent)
                .and_then(|entries| entries.iter().find(|e| &e.name == name));
            match found {
                Some(e) if e.is_directory() => {
                    return Err(TxError::IsDirectory(shown(&join(
                        parent,
                        std::slice::from_ref(name),
                    ))));
                }
                Some(_) => in_use.push(shown(&join(parent, std::slice::from_ref(name)))),
                None => {}
            }
        }
        if !in_use.is_empty() && !tree.replace {
            return Err(TxError::NamesInUse(in_use));
        }
    }

    let mut made = 0;
    if let Some(name) = &tree.name {
        ctx.event(Event::Item(root.clone()));
        cd(ctx, dir).await?;
        expect_done(ctx.op(Op::Mkdir(name.clone())).await?)?;
        made += 1;
        existing.insert(Vec::new(), Vec::new());
    }
    let mut data: HashMap<Vec<String>, Vec<u8>> = files
        .into_iter()
        .map(|(parent, name, bytes)| (join(&parent, std::slice::from_ref(&name)), bytes))
        .collect();
    let mut stored = 0;
    // Directory by directory, parents first. The calculator puts a new
    // variable first in its directory, so each directory's children go in
    // the tree's order backwards: a tree got and put back keeps its order.
    for parent in std::iter::once(Vec::new()).chain(dirs) {
        let here: Vec<&String> = children
            .iter()
            .filter(|(p, _)| *p == parent)
            .map(|(_, n)| n)
            .collect();
        if here.is_empty() {
            continue;
        }
        let at = join(&root, &parent);
        cd(ctx, &at).await?;
        let entries = existing.get(&parent).cloned().unwrap_or_default();
        for name in here.into_iter().rev() {
            let path = join(&parent, std::slice::from_ref(name));
            ctx.event(Event::Item(join(&root, &path)));
            if let Some(bytes) = data.remove(&path) {
                put_in(ctx, &entries, name.clone(), bytes, tree.replace).await?;
                stored += 1;
            } else if let hash_map::Entry::Vacant(new) = existing.entry(path) {
                expect_done(ctx.op(Op::Mkdir(name.clone())).await?)?;
                made += 1;
                new.insert(Vec::new());
            }
        }
    }
    Ok(Outcome::TreeStored {
        directories: made,
        variables: stored,
    })
}

pub(crate) async fn memory(ctx: &Ctx) -> Result<Memory> {
    ctx.session(|_| ())?;
    let listing = list_dir(ctx, &["HOME".to_string()]).await?;
    let home = listing.entries.iter().map(|e| e.size).sum();
    let free = match listing.free {
        Some(free) => free,
        // The 48S/SX's listing has no header.
        None => match ctx.op(Op::Mem).await? {
            Reply::Real(free) => free,
            other => return Err(unexpected("MEM", &other)),
        },
    };
    Ok(Memory::new(free, home))
}

pub(crate) async fn backup(ctx: &Ctx) -> Result<Outcome> {
    let model = ctx.model()?;
    let memory = memory(ctx).await?;
    if !memory.fits() {
        return Err(TxError::NoRoom {
            needed: memory.needed,
            free: memory.free,
        });
    }
    // In HOME since the memory check.
    match ctx.op(Op::Backup).await? {
        Reply::Data(data) => Ok(Outcome::Backup(Backup {
            model,
            data,
            memory,
        })),
        other => Err(unexpected("backup", &other)),
    }
}

pub(crate) async fn restore(ctx: &Ctx, data: Vec<u8>) -> Result<Outcome> {
    ctx.session(|_| ())?;
    cd(ctx, &["HOME".to_string()]).await?;
    let restored = ctx.op(Op::Restore(data)).await;
    match restored {
        Ok(reply) => {
            expect_done(reply)?;
            ctx.restarted();
            Ok(Outcome::Restored)
        }
        Err(e) => {
            // The calculator may be anywhere now.
            ctx.session(|s| s.cwd = None)?;
            Err(e)
        }
    }
}

pub(crate) async fn pict(ctx: &Ctx) -> Result<Outcome> {
    ctx.session(|_| ())?;
    match ctx.op(Op::PictFile).await? {
        Reply::Data(data) => {
            let grob = Grob::from_file(&data)?;
            Ok(Outcome::Pict(Pict {
                width: grob.width,
                height: grob.height,
                data,
            }))
        }
        other => Err(unexpected("PICT", &other)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn item(path: &[&str], data: Option<&[u8]>) -> TreeItem {
        TreeItem {
            path: path.iter().map(|s| s.to_string()).collect(),
            data: data.map(<[u8]>::to_vec),
        }
    }

    const BIN: &[u8] = b"HPHP48-R\x2c\x2a\x20\x00\x00";

    #[test]
    fn a_layout_has_every_parent_before_its_children() {
        let l = layout(
            vec![
                item(&["A", "B", "X"], Some(BIN)),
                item(&["Y"], Some(BIN)),
                item(&["C"], None),
                item(&["A"], None),
            ],
            false,
        )
        .unwrap();
        let dirs: Vec<String> = l.dirs.iter().map(|d| shown(d)).collect();
        assert_eq!(dirs, ["A", "A/B", "C"]);
        let children: Vec<String> = l
            .children
            .iter()
            .map(|(p, n)| shown(&join(p, std::slice::from_ref(n))))
            .collect();
        assert_eq!(children, ["A", "A/B", "A/B/X", "Y", "C"]);
        assert_eq!(l.files.len(), 2);
        assert_eq!(l.files[0].0, ["A", "B"]);
        assert!(l.files[1].0.is_empty());
    }

    #[test]
    fn a_layout_refuses_bad_names_duplicates_and_non_objects() {
        let bad = layout(vec![item(&["1X"], Some(BIN))], false);
        assert!(matches!(bad, Err(TxError::BadName(n)) if n == "1X"));
        let dup = layout(
            vec![item(&["A"], Some(BIN)), item(&["A"], Some(BIN))],
            false,
        );
        assert!(matches!(dup, Err(TxError::Duplicate(n)) if n == "A"));
        let both = layout(vec![item(&["A"], None), item(&["A"], Some(BIN))], false);
        assert!(matches!(both, Err(TxError::Duplicate(_))));
        let parent = layout(
            vec![item(&["A"], Some(BIN)), item(&["A", "B"], Some(BIN))],
            false,
        );
        assert!(matches!(parent, Err(TxError::Duplicate(_))));
        let note = layout(vec![item(&["N"], Some(b"hello"))], false);
        assert!(matches!(note, Err(TxError::NotAnObject)));
        assert!(layout(vec![item(&["N"], Some(b"hello"))], true).is_ok());
        assert!(matches!(
            layout(vec![item(&[], None)], false),
            Err(TxError::BadName(_))
        ));
    }

    #[test]
    fn a_backup_needs_twice_home_and_the_margin() {
        let m = Memory::new(10_000.0, 4_000.0);
        assert_eq!(m.needed, 9_024.0);
        assert!(m.fits());
        assert!(!Memory::new(9_000.0, 4_000.0).fits());
    }
}
