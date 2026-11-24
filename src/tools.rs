use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::{
    Context,
    effects::Effect,
    picker::{Picker, Request, Value},
    source::{Directories, Files, display_path},
};

pub trait Tool {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    fn available(&self, _context: &Context) -> bool {
        true
    }
    fn run(&self, context: &Context, picker: &mut dyn Picker) -> Result<Option<Effect>>;
}

pub struct Edit;

impl Tool for Edit {
    fn id(&self) -> &'static str {
        "edit"
    }
    fn label(&self) -> &'static str {
        "Choose file (edit)"
    }
    fn available(&self, _: &Context) -> bool {
        crate::effects::on_path("nvim")
    }

    fn run(&self, context: &Context, picker: &mut dyn Picker) -> Result<Option<Effect>> {
        let mut request = Request::new(
            "Files> ",
            "Tab: select multiple · Enter: edit · Esc: back",
            Files {
                root: context.cwd.clone(),
                hidden: context.hidden,
            },
        );
        request.multi = true;
        let Some(selection) = picker.pick(request)? else {
            return Ok(None);
        };
        let paths = selection
            .entries
            .into_iter()
            .map(|entry| match entry.value {
                Value::Path(path) => Ok(path),
                _ => bail!("expected a file candidate"),
            })
            .collect::<Result<Vec<_>>>()?;
        if paths.is_empty() {
            return Ok(None);
        }
        Ok(Some(Effect::Edit(paths)))
    }
}

pub struct Browse;

impl Tool for Browse {
    fn id(&self) -> &'static str {
        "browse"
    }
    fn label(&self) -> &'static str {
        "Browse directories (shell / edit)"
    }
    fn run(&self, context: &Context, picker: &mut dyn Picker) -> Result<Option<Effect>> {
        browse(context, picker, false)
    }
}

/// Browse without side effects, returning a directory to the caller.
pub fn choose_directory(context: &Context, picker: &mut dyn Picker) -> Result<Option<PathBuf>> {
    match browse(context, picker, true)? {
        Some(Effect::Directory(path)) => Ok(Some(path)),
        None => Ok(None),
        _ => unreachable!("selection-only browser returns directory effects"),
    }
}

fn browse(context: &Context, picker: &mut dyn Picker, select_only: bool) -> Result<Option<Effect>> {
    let mut cwd = context.cwd.clone();
    loop {
        let actions = if select_only {
            "Ctrl-o: choose"
        } else {
            "Ctrl-o: shell · Ctrl-e: Neovim"
        };
        let mut request = Request::new(
            "Directories> ",
            format!(
                "{}\nEnter: descend · Alt-h: parent · {actions} · Esc: back",
                display_path(&cwd)
            ),
            Directories {
                root: cwd.clone(),
                hidden: context.hidden,
            },
        );
        request.bindings = vec!["ctrl-o:accept(open)".into(), "alt-h:accept(parent)".into()];
        if !select_only {
            request.bindings.push("ctrl-e:accept(edit)".into());
        }
        let Some(selection) = picker.pick(request)? else {
            return Ok(None);
        };
        if selection.action.as_deref() == Some("parent") {
            if let Some(parent) = cwd.parent() {
                cwd = parent.into();
            }
            continue;
        }
        let Some(entry) = selection.entries.into_iter().next() else {
            continue;
        };
        let Value::Path(path) = entry.value else {
            bail!("expected a directory candidate")
        };
        // Revalidate after selection: a directory may have been removed while browsing.
        anyhow::ensure!(
            path.is_dir(),
            "directory no longer exists: {}",
            display_path(&path)
        );
        match selection.action.as_deref() {
            Some("open") if select_only => return Ok(Some(Effect::Directory(path))),
            Some("open") => return Ok(Some(Effect::Shell(path))),
            Some("edit") => return Ok(Some(Effect::Edit(vec![path]))),
            _ => cwd = path,
        }
    }
}
