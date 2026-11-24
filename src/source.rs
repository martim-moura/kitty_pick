use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use ignore::WalkBuilder;

use crate::picker::{Entry, Value};

/// A cancellable stream. Return as soon as `emit` returns false.
/// Never invoke `emit` again after cancellation.
pub trait Source: Send {
    fn produce(self: Box<Self>, emit: &mut dyn FnMut(Entry) -> bool) -> Result<()>;
}

impl Source for Vec<Entry> {
    fn produce(self: Box<Self>, emit: &mut dyn FnMut(Entry) -> bool) -> Result<()> {
        for entry in *self {
            if !emit(entry) {
                break;
            }
        }
        Ok(())
    }
}

/// Escape terminal control characters without changing the underlying path.
pub fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    let mut label = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_control() || ch == '\\' {
            label.extend(ch.escape_default());
        } else {
            label.push(ch);
        }
    }
    label
}

fn path_entry(path: PathBuf, root: &Path) -> Entry {
    let label = display_path(path.strip_prefix(root).unwrap_or(&path));
    Entry::new(label, Value::Path(path))
}

pub struct Files {
    pub root: PathBuf,
    pub hidden: bool,
}

impl Source for Files {
    fn produce(self: Box<Self>, emit: &mut dyn FnMut(Entry) -> bool) -> Result<()> {
        let walk = WalkBuilder::new(&self.root)
            .hidden(!self.hidden)
            .follow_links(false)
            .build();
        for entry in walk {
            let entry = entry.context("walk files")?;
            // Include file symlinks, but never descend symlink directories.
            if (entry.file_type().is_some_and(|t| t.is_file())
                || (entry.path_is_symlink() && entry.path().is_file()))
                && !emit(path_entry(entry.into_path(), &self.root))
            {
                break;
            }
        }
        Ok(())
    }
}

/// Immediate children only. Hidden entries are opt-in; ignore files are not
/// applied to navigation. Directory symlinks can be entered explicitly.
pub struct Directories {
    pub root: PathBuf,
    pub hidden: bool,
}

impl Source for Directories {
    fn produce(self: Box<Self>, emit: &mut dyn FnMut(Entry) -> bool) -> Result<()> {
        if !emit(Entry::new(
            "./ (current directory)",
            Value::Path(self.root.clone()),
        )) {
            return Ok(());
        }
        if let Some(parent) = self.root.parent()
            && !emit(Entry::new(
                "../ (parent directory)",
                Value::Path(parent.into()),
            ))
        {
            return Ok(());
        }
        for child in std::fs::read_dir(&self.root).context("read directory")? {
            let child = child.context("read directory entry")?;
            if !self.hidden && child.file_name().as_encoded_bytes().starts_with(b".") {
                continue;
            }
            let kind = child.file_type()?;
            if (kind.is_dir() || (kind.is_symlink() && child.path().is_dir()))
                && !emit(path_entry(child.path(), &self.root))
            {
                break;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn collect(source: impl Source + 'static) -> Result<Vec<Entry>> {
        let mut entries = Vec::new();
        Box::new(source).produce(&mut |entry| {
            entries.push(entry);
            true
        })?;
        Ok(entries)
    }

    #[test]
    fn traversal_policy() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path();
        std::fs::create_dir(root.join(".git"))?;
        std::fs::create_dir(root.join("child"))?;
        for name in ["visible", ".secret", "ignored", "child/nested"] {
            std::fs::write(root.join(name), "")?;
        }
        std::fs::write(root.join(".gitignore"), "ignored\n")?;
        let files = collect(Files {
            root: root.into(),
            hidden: false,
        })?;
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|e| e.label == "visible"));
        assert!(files.iter().any(|e| e.label == "child/nested"));
        let dirs = collect(Directories {
            root: root.into(),
            hidden: false,
        })?;
        assert_eq!(dirs.len(), 3); // current, parent, child
        let hidden = collect(Files {
            root: root.into(),
            hidden: true,
        })?;
        assert!(hidden.iter().any(|e| e.label == ".secret"));
        assert!(!hidden.iter().any(|e| e.label == "ignored"));
        Ok(())
    }

    #[test]
    fn cancellation_stops_emission() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut calls = 0;
        Box::new(Directories {
            root: temp.path().into(),
            hidden: false,
        })
        .produce(&mut |_| {
            calls += 1;
            false
        })?;
        assert_eq!(calls, 1);
        Ok(())
    }

    #[test]
    fn controls_are_display_only() {
        let path = Path::new("a\n\x1b\\b");
        assert_eq!(display_path(path), r"a\n\u{1b}\\b");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_directories_are_not_traversed() -> Result<()> {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("file");
        std::fs::write(&path, "")?;
        symlink(temp.path(), temp.path().join("loop"))?;
        symlink(&path, temp.path().join("file-link"))?;
        let entries = collect(Files {
            root: temp.path().into(),
            hidden: false,
        })?;
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.value == Value::Path(path.clone())));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_identity_is_not_the_lossy_label() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};
        // APFS rejects invalid UTF-8 filenames, so test representation without I/O.
        let path = PathBuf::from(OsString::from_vec(vec![b'x', 0xff]));
        let entry = path_entry(path.clone(), Path::new("/"));
        assert_eq!(entry.value, Value::Path(path));
        assert!(entry.label.contains('\u{fffd}'));
    }
}
