//! A compiled-in tool added without changing the engine or Kitty integration.
use anyhow::Result;
use kitty_pick::{
    Context, Registry,
    effects::{self, Effect},
    picker::{Entry, Picker, Request, SkimPicker, Value},
    tools::Tool,
};

struct EditManifest;

impl Tool for EditManifest {
    fn id(&self) -> &'static str {
        "edit-manifest"
    }
    fn label(&self) -> &'static str {
        "Edit project manifest"
    }

    fn available(&self, context: &Context) -> bool {
        effects::on_path("nvim") && context.cwd.join("Cargo.toml").is_file()
    }

    fn run(&self, context: &Context, picker: &mut dyn Picker) -> Result<Option<Effect>> {
        let path = context.cwd.join("Cargo.toml");
        let request = Request::new(
            "Manifest> ",
            "Enter: edit · Esc: back",
            vec![Entry::new("Cargo.toml", Value::Path(path))],
        );
        let Some(selection) = picker.pick(request)? else {
            return Ok(None);
        };
        let Some(Entry {
            value: Value::Path(path),
            ..
        }) = selection.entries.first()
        else {
            return Ok(None);
        };
        Ok(Some(Effect::Edit(vec![path.clone()])))
    }
}

fn main() -> Result<()> {
    let context = Context::new(std::env::current_dir()?, false)?;
    let mut registry = Registry::builtins();
    registry.register(EditManifest)?;
    if let Some(effect) = registry.menu(&context, &mut SkimPicker)? {
        effects::execute(&effect, &context.cwd)?;
    }
    Ok(())
}
