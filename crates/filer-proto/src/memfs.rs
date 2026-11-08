//! An in-memory [`FileSystem`] for the test server: drives, directories
//! and files in a map, DOS's rules for the errors.

use std::collections::BTreeMap;

use crate::command::{DirEntry, DosDateTime, NameKind, attr};
use crate::error::FsError;
use crate::server::FileSystem;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Node {
    Dir {
        modified: DosDateTime,
    },
    File {
        data: Vec<u8>,
        modified: DosDateTime,
        attributes: u8,
    },
}

/// Files in memory, keyed by canonical path (`C:\SUB\FILE.TXT`).
#[derive(Clone, Debug)]
pub struct MemoryFs {
    drives: Vec<u8>,
    nodes: BTreeMap<Vec<u8>, Node>,
    read_only: Vec<u8>,
    /// Bytes each drive holds, for the free space.
    pub capacity: u32,
    /// The time new files and directories get.
    pub now: DosDateTime,
}

impl MemoryFs {
    /// Empty drives with these letters (`b"ACD"`), 1 MiB each.
    pub fn new(drives: &[u8]) -> Self {
        MemoryFs {
            drives: drives.to_ascii_uppercase(),
            nodes: BTreeMap::new(),
            read_only: Vec::new(),
            capacity: 1 << 20,
            now: DosDateTime::new(2026, 1, 1, 12, 0, 0).unwrap_or_default(),
        }
    }

    /// Make a drive write-protected (the ROM drive D:): writes fail with
    /// [`FsError::WRITE_PROTECTED`].
    pub fn set_read_only(&mut self, drive: u8) {
        self.read_only.push(drive.to_ascii_uppercase());
    }

    /// Add a file (and its missing parent directories) at a canonical path.
    pub fn insert_file(&mut self, path: &[u8], data: Vec<u8>) {
        let path = path.to_ascii_uppercase();
        self.insert_parents(&path);
        self.nodes.insert(
            path,
            Node::File {
                data,
                modified: self.now,
                attributes: attr::ARCHIVE,
            },
        );
    }

    /// Add a directory (and its parents).
    pub fn insert_dir(&mut self, path: &[u8]) {
        let path = path.to_ascii_uppercase();
        self.insert_parents(&path);
        self.nodes
            .entry(path)
            .or_insert(Node::Dir { modified: self.now });
    }

    /// A file's contents.
    pub fn file(&self, path: &[u8]) -> Option<&[u8]> {
        match self.nodes.get(&path.to_ascii_uppercase()) {
            Some(Node::File { data, .. }) => Some(data),
            _ => None,
        }
    }

    /// A file's time.
    pub fn modified(&self, path: &[u8]) -> Option<DosDateTime> {
        match self.nodes.get(&path.to_ascii_uppercase()) {
            Some(Node::File { modified, .. } | Node::Dir { modified }) => Some(*modified),
            None => None,
        }
    }

    /// Every path, files and directories, in order.
    pub fn paths(&self) -> Vec<Vec<u8>> {
        self.nodes.keys().cloned().collect()
    }

    fn insert_parents(&mut self, path: &[u8]) {
        let mut parent = parent(path);
        while let Some(p) = parent {
            if p.len() <= 3 {
                break;
            }
            self.nodes
                .entry(p.clone())
                .or_insert(Node::Dir { modified: self.now });
            parent = self::parent(&p);
        }
    }

    fn writable(&self, path: &[u8]) -> Result<(), FsError> {
        match path.first() {
            Some(d) if self.read_only.contains(d) => Err(FsError::WRITE_PROTECTED),
            _ => Ok(()),
        }
    }

    /// The parent must be a directory.
    fn parent_exists(&self, path: &[u8]) -> Result<(), FsError> {
        match parent(path) {
            Some(p) if self.kind(&p) == NameKind::Directory => Ok(()),
            _ => Err(FsError::PATH_NOT_FOUND),
        }
    }

    fn used(&self, drive: u8) -> u64 {
        self.nodes
            .iter()
            .filter(|(p, _)| p.first() == Some(&drive))
            .map(|(_, n)| match n {
                // Clusters of 512 bytes, as the recorded free space moved.
                Node::File { data, .. } => (data.len() as u64).div_ceil(512) * 512,
                Node::Dir { .. } => 512,
            })
            .sum()
    }

    fn children(&self, dir: &[u8]) -> impl Iterator<Item = (&Vec<u8>, &Node)> {
        let dir = dir.to_vec();
        self.nodes
            .iter()
            .filter(move |(p, _)| parent(p).as_deref() == Some(dir.as_slice()))
    }
}

/// `C:\A\B` to `C:\A`, `C:\A` to `C:\`, `C:\` to `None`.
fn parent(path: &[u8]) -> Option<Vec<u8>> {
    if path.len() <= 3 {
        return None;
    }
    let i = path.iter().rposition(|&b| b == b'\\')?;
    Some(if i <= 2 {
        path[..3].to_vec()
    } else {
        path[..i].to_vec()
    })
}

fn base_name(path: &[u8]) -> Vec<u8> {
    let i = path.iter().rposition(|&b| b == b'\\').map_or(0, |i| i + 1);
    path[i..].to_vec()
}

/// 8.3: a base of 1-8 characters, an optional extension of 1-3.
fn valid_83(name: &[u8]) -> bool {
    let mut parts = name.split(|&b| b == b'.');
    let base = parts.next().unwrap_or_default();
    let ext = parts.next();
    parts.next().is_none()
        && (1..=8).contains(&base.len())
        && ext.is_none_or(|e| (1..=3).contains(&e.len()))
        && !name.iter().any(|&b| b == b'*' || b == b'?')
}

impl FileSystem for MemoryFs {
    fn drives(&self) -> Vec<u8> {
        self.drives.clone()
    }

    fn volume(&self, drive: u8) -> Result<(Vec<u8>, u32), FsError> {
        let free = u64::from(self.capacity).saturating_sub(self.used(drive));
        Ok((vec![b' '; 11], u32::try_from(free).unwrap_or(u32::MAX)))
    }

    fn kind(&self, path: &[u8]) -> NameKind {
        if path.len() == 3 && self.drives.contains(&path[0]) {
            return NameKind::Directory;
        }
        match self.nodes.get(path) {
            Some(Node::Dir { .. }) => NameKind::Directory,
            Some(Node::File { .. }) => NameKind::File,
            None => NameKind::Nothing,
        }
    }

    fn list(&self, dir: &[u8]) -> Result<Vec<DirEntry>, FsError> {
        if self.kind(dir) != NameKind::Directory {
            return Err(FsError::PATH_NOT_FOUND);
        }
        let mut entries = Vec::new();
        if dir.len() > 3 {
            for name in [&b"."[..], b".."] {
                entries.push(DirEntry {
                    attributes: attr::DIRECTORY,
                    modified: self.now,
                    size: 0,
                    name: name.to_vec(),
                });
            }
        }
        for (path, node) in self.children(dir) {
            entries.push(match node {
                Node::Dir { modified } => DirEntry {
                    attributes: attr::DIRECTORY,
                    modified: *modified,
                    size: 0,
                    name: base_name(path),
                },
                Node::File {
                    data,
                    modified,
                    attributes,
                } => DirEntry {
                    attributes: *attributes,
                    modified: *modified,
                    size: u32::try_from(data.len()).unwrap_or(u32::MAX),
                    name: base_name(path),
                },
            });
        }
        Ok(entries)
    }

    fn read(&self, path: &[u8]) -> Result<(Vec<u8>, DosDateTime), FsError> {
        match self.nodes.get(path) {
            Some(Node::File { data, modified, .. }) => Ok((data.clone(), *modified)),
            Some(Node::Dir { .. }) => Err(FsError::ACCESS_DENIED),
            None => Err(FsError::NOT_FOUND),
        }
    }

    fn create(&mut self, path: &[u8], overwrite: bool) -> Result<(), FsError> {
        self.writable(path)?;
        self.parent_exists(path)?;
        if !valid_83(&base_name(path)) {
            return Err(FsError::INVALID_NAME);
        }
        match self.nodes.get(path) {
            Some(Node::Dir { .. }) => return Err(FsError::ACCESS_DENIED),
            Some(Node::File { attributes, .. }) if attributes & attr::READ_ONLY != 0 => {
                return Err(FsError::ACCESS_DENIED);
            }
            Some(Node::File { .. }) if !overwrite => return Err(FsError::EXISTS),
            _ => {}
        }
        self.nodes.insert(
            path.to_vec(),
            Node::File {
                data: Vec::new(),
                modified: self.now,
                attributes: attr::ARCHIVE,
            },
        );
        Ok(())
    }

    fn store(
        &mut self,
        path: &[u8],
        data: &[u8],
        modified: Option<DosDateTime>,
    ) -> Result<(), FsError> {
        self.writable(path)?;
        let old = match self.nodes.get(path) {
            Some(Node::File { data, .. }) => data.len() as u64,
            _ => 0,
        };
        let grows = (data.len() as u64).saturating_sub(old);
        let used = self.used(path.first().copied().unwrap_or(b'C'));
        if used + grows > u64::from(self.capacity) {
            return Err(FsError::DISK_FULL);
        }
        let attributes = match self.nodes.get(path) {
            Some(Node::File { attributes, .. }) => *attributes,
            _ => attr::ARCHIVE,
        };
        self.nodes.insert(
            path.to_vec(),
            Node::File {
                data: data.to_vec(),
                modified: modified.unwrap_or(self.now),
                attributes,
            },
        );
        Ok(())
    }

    fn delete(&mut self, path: &[u8]) -> Result<(), FsError> {
        self.writable(path)?;
        match self.nodes.get(path) {
            Some(Node::File { attributes, .. }) if attributes & attr::READ_ONLY == 0 => {
                self.nodes.remove(path);
                Ok(())
            }
            Some(_) => Err(FsError::ACCESS_DENIED),
            None if self.kind(path) == NameKind::Directory => Err(FsError::ACCESS_DENIED),
            None => Err(FsError::NOT_FOUND),
        }
    }

    fn make_dir(&mut self, path: &[u8]) -> Result<(), FsError> {
        self.writable(path)?;
        self.parent_exists(path)?;
        if self.kind(path) != NameKind::Nothing {
            return Err(FsError::CANNOT_MKDIR);
        }
        if !valid_83(&base_name(path)) {
            return Err(FsError::INVALID_NAME);
        }
        self.nodes
            .insert(path.to_vec(), Node::Dir { modified: self.now });
        Ok(())
    }

    fn remove_dir(&mut self, path: &[u8]) -> Result<(), FsError> {
        self.writable(path)?;
        match self.nodes.get(path) {
            Some(Node::Dir { .. }) => {}
            _ => return Err(FsError::PATH_NOT_FOUND),
        }
        if self.children(path).next().is_some() {
            return Err(FsError::ACCESS_DENIED);
        }
        self.nodes.remove(path);
        Ok(())
    }

    fn rename(&mut self, from: &[u8], to: &[u8]) -> Result<(), FsError> {
        self.writable(from)?;
        if !self.nodes.contains_key(from) {
            return Err(FsError::NOT_FOUND);
        }
        self.parent_exists(to)?;
        if !valid_83(&base_name(to)) {
            return Err(FsError::INVALID_NAME);
        }
        if self.kind(to) != NameKind::Nothing || to.starts_with(&[from, b"\\"].concat()) {
            return Err(FsError::ACCESS_DENIED);
        }
        // The node and, for a directory, everything below it.
        let below = [from, b"\\"].concat();
        let moving: Vec<Vec<u8>> = self
            .nodes
            .keys()
            .filter(|p| p.as_slice() == from || p.starts_with(&below))
            .cloned()
            .collect();
        for old in moving {
            if let Some(node) = self.nodes.remove(&old) {
                let mut new = to.to_vec();
                new.extend_from_slice(&old[from.len()..]);
                self.nodes.insert(new, node);
            }
        }
        Ok(())
    }

    fn attributes(&self, path: &[u8]) -> Result<u8, FsError> {
        match self.nodes.get(path) {
            Some(Node::File { attributes, .. }) => Ok(*attributes),
            Some(Node::Dir { .. }) => Ok(attr::DIRECTORY),
            None => Err(FsError::NOT_FOUND),
        }
    }

    fn set_attributes(&mut self, path: &[u8], value: u8) -> Result<(), FsError> {
        self.writable(path)?;
        match self.nodes.get_mut(path) {
            Some(Node::File { attributes, .. }) => {
                *attributes = value & !(attr::DIRECTORY | attr::VOLUME);
                Ok(())
            }
            Some(Node::Dir { .. }) => Err(FsError::ACCESS_DENIED),
            None => Err(FsError::NOT_FOUND),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn dos_rules() {
        let mut fs = MemoryFs::new(b"CD");
        fs.set_read_only(b'D');
        fs.insert_file(b"C:\\SUB\\A.TXT", b"a".to_vec());
        assert_eq!(fs.kind(b"C:\\SUB"), NameKind::Directory);
        assert_eq!(fs.remove_dir(b"C:\\SUB"), Err(FsError::ACCESS_DENIED));
        assert_eq!(fs.create(b"C:\\SUB\\A.TXT", false), Err(FsError::EXISTS));
        assert_eq!(
            fs.create(b"C:\\NO\\A.TXT", false),
            Err(FsError::PATH_NOT_FOUND)
        );
        assert_eq!(
            fs.create(b"C:\\TOOLONGNAME", false),
            Err(FsError::INVALID_NAME)
        );
        assert_eq!(fs.create(b"D:\\A", false), Err(FsError::WRITE_PROTECTED));
        assert_eq!(fs.delete(b"C:\\B"), Err(FsError::NOT_FOUND));
        fs.rename(b"C:\\SUB", b"C:\\NEW").unwrap();
        assert_eq!(fs.file(b"C:\\NEW\\A.TXT"), Some(&b"a"[..]));
        let names: Vec<_> = fs
            .list(b"C:\\NEW")
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, [&b"."[..], b"..", b"A.TXT"]);
        assert_eq!(fs.list(b"C:\\").unwrap().len(), 1);
        fs.capacity = 1024;
        assert_eq!(
            fs.store(b"C:\\BIG", &[0; 2048], None),
            Err(FsError::DISK_FULL)
        );
        assert_eq!(parent(b"C:\\A"), Some(b"C:\\".to_vec()));
        assert_eq!(parent(b"C:\\"), None);
    }
}
