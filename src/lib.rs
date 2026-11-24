//! Typed terminal tools, streaming sources, skim picking, and deferred effects.
pub mod effects;
pub mod kitty;
pub mod picker;
pub mod source;
pub mod tools;

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use picker::{Entry, Picker, Request, Value};
use tools::Tool;

/// Immutable invocation context. No process-wide cwd mutation is needed.
#[derive(Clone, Debug)]
pub struct Context {
    pub cwd: PathBuf,
    pub hidden: bool,
}

impl Context {
    pub fn new(cwd: impl AsRef<Path>, hidden: bool) -> Result<Self> {
        let cwd = cwd
            .as_ref()
            .canonicalize()
            .context("resolve working directory")?;
        anyhow::ensure!(cwd.is_dir(), "not a directory: {}", cwd.display());
        Ok(Self { cwd, hidden })
    }
}

/// A registry of compiled-in tools. IDs, not labels, identify actions.
#[derive(Default)]
pub struct Registry {
    tools: Vec<Box<dyn Tool>>,
}

impl Registry {
    pub fn register(&mut self, tool: impl Tool + 'static) -> Result<()> {
        if self.tools.iter().any(|t| t.id() == tool.id()) {
            bail!("duplicate tool ID: {}", tool.id());
        }
        self.tools.push(Box::new(tool));
        Ok(())
    }

    pub fn builtins() -> Self {
        let mut registry = Self::default();
        registry.register(tools::Edit).expect("unique built-in ID");
        registry
            .register(tools::Browse)
            .expect("unique built-in ID");
        registry
    }

    pub fn available<'a>(&'a self, context: &'a Context) -> impl Iterator<Item = &'a dyn Tool> {
        self.tools
            .iter()
            .map(Box::as_ref)
            .filter(|t| t.available(context))
    }

    pub fn run(
        &self,
        id: &str,
        context: &Context,
        picker: &mut dyn Picker,
    ) -> Result<Option<effects::Effect>> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.id() == id)
            .with_context(|| format!("unknown tool: {id}"))?;
        anyhow::ensure!(tool.available(context), "tool unavailable: {id}");
        tool.run(context, picker)
    }

    /// Cancellation in a tool returns here; cancellation in this menu exits.
    pub fn menu(
        &self,
        context: &Context,
        picker: &mut dyn Picker,
    ) -> Result<Option<effects::Effect>> {
        loop {
            let entries: Vec<_> = self
                .available(context)
                .map(|t| Entry::new(t.label(), Value::Tool(t.id().into())))
                .collect();
            anyhow::ensure!(!entries.is_empty(), "no tools available");
            let request = Request::new("Tools> ", "Enter: choose · Esc: quit", entries);
            let Some(selection) = picker.pick(request)? else {
                return Ok(None);
            };
            let Some(Entry {
                value: Value::Tool(id),
                ..
            }) = selection.entries.first()
            else {
                continue;
            };
            if let Some(effect) = self.run(id, context, picker)? {
                return Ok(Some(effect));
            }
        }
    }
}
