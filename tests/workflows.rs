use std::collections::VecDeque;

use anyhow::Result;
use kitty_pick::{
    Context, Registry,
    effects::Effect,
    picker::{Entry, Picker, Request, Selection, Value},
    tools::{self, Tool},
};

#[derive(Default)]
struct FakePicker {
    replies: VecDeque<Option<Selection>>,
    prompts: Vec<String>,
    candidates: Vec<Vec<Entry>>,
}

impl Picker for FakePicker {
    fn pick(&mut self, request: Request) -> Result<Option<Selection>> {
        self.prompts.push(request.prompt);
        let mut entries = Vec::new();
        request.source.produce(&mut |entry| {
            entries.push(entry);
            true
        })?;
        self.candidates.push(entries);
        Ok(self.replies.pop_front().expect("unexpected picker request"))
    }
}

fn selected(value: Value, action: Option<&str>) -> Option<Selection> {
    Some(Selection {
        action: action.map(str::to_owned),
        entries: vec![Entry::new("label is not identity", value)],
    })
}

#[test]
fn directory_navigation_and_selection_are_typed() -> Result<()> {
    let temp = tempfile::tempdir()?;
    std::fs::create_dir(temp.path().join("child"))?;
    let context = Context::new(temp.path(), false)?;
    let child = context.cwd.join("child");
    let mut picker = FakePicker {
        replies: [
            selected(Value::Path(child.clone()), None),
            Some(Selection {
                action: Some("parent".into()),
                entries: vec![],
            }),
            selected(Value::Path(child.clone()), Some("open")),
        ]
        .into(),
        ..Default::default()
    };
    assert_eq!(tools::choose_directory(&context, &mut picker)?, Some(child));
    assert_eq!(picker.prompts.len(), 3);
    assert_eq!(
        picker.candidates[1][0].value,
        Value::Path(context.cwd.join("child"))
    );
    assert_eq!(picker.candidates[2][0].value, Value::Path(context.cwd));
    Ok(())
}

#[test]
fn browsing_can_open_directory_in_editor() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let context = Context::new(temp.path(), false)?;
    let mut picker = FakePicker {
        replies: [selected(Value::Path(context.cwd.clone()), Some("edit"))].into(),
        ..Default::default()
    };
    assert_eq!(
        tools::Browse.run(&context, &mut picker)?,
        Some(Effect::Edit(vec![context.cwd]))
    );
    Ok(())
}

#[test]
fn menu_tool_cancel_returns_to_menu_then_exits() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let context = Context::new(temp.path(), false)?;
    let mut picker = FakePicker {
        replies: [selected(Value::Tool("browse".into()), None), None, None].into(),
        ..Default::default()
    };
    assert_eq!(Registry::builtins().menu(&context, &mut picker)?, None);
    assert_eq!(picker.prompts, ["Tools> ", "Directories> ", "Tools> "]);
    Ok(())
}

#[test]
fn registry_rejects_duplicate_ids() -> Result<()> {
    let mut registry = Registry::default();
    registry.register(tools::Browse)?;
    assert!(registry.register(tools::Browse).is_err());
    Ok(())
}

#[test]
fn file_edit_preserves_multi_selection_and_metacharacters() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let context = Context::new(temp.path(), false)?;
    let paths = vec![context.cwd.join("a\n'b"), context.cwd.join("-c quit")];
    let mut picker = FakePicker {
        replies: [Some(Selection {
            action: None,
            entries: paths
                .iter()
                .map(|p| Entry::new("display only", Value::Path(p.clone())))
                .collect(),
        })]
        .into(),
        ..Default::default()
    };
    assert_eq!(
        tools::Edit.run(&context, &mut picker)?,
        Some(Effect::Edit(paths))
    );
    Ok(())
}
