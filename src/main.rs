use std::{io::Write, path::PathBuf, process::ExitCode};

use anyhow::{Context as _, Result};
use clap::{Parser, Subcommand, ValueEnum};
use kitty_pick::{
    Context, Registry, effects,
    kitty::{self, Surface},
    picker::SkimPicker,
    tools,
};

#[derive(Parser)]
#[command(version, about = "Contextual terminal tools powered by skim")]
struct Cli {
    #[arg(long, global = true, default_value = ".")]
    cwd: PathBuf,
    /// Include hidden files and directories (ignore rules still apply to files).
    #[arg(long, global = true)]
    hidden: bool,
    #[command(subcommand)]
    command: Option<Action>,
}

#[derive(Clone, Copy, ValueEnum)]
enum LaunchTool {
    Menu,
    Edit,
    Browse,
}

impl LaunchTool {
    fn name(self) -> &'static str {
        match self {
            Self::Menu => "menu",
            Self::Edit => "edit",
            Self::Browse => "browse",
        }
    }
}

#[derive(Subcommand)]
enum Action {
    /// Pick a tool, then run its picker (default).
    Menu,
    /// Choose one or more files to open in Neovim.
    Edit,
    /// Browse directories, then open a shell or Neovim.
    Browse,
    /// Browse and write one raw NUL-delimited directory to stdout; cancel exits 130.
    ChooseDir,
    /// List currently available tool IDs and labels without opening a TTY.
    Tools,
    /// Start this program in a Kitty surface. Does not wait for the tool to exit.
    Launch {
        #[arg(long, value_enum, default_value = "overlay")]
        surface: Surface,
        #[arg(value_enum, default_value = "menu")]
        tool: LaunchTool,
    },
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    let context = Context::new(&cli.cwd, cli.hidden)?;
    let registry = Registry::builtins();
    let action = cli.command.unwrap_or(Action::Menu);
    match action {
        Action::Launch { surface, tool } => {
            kitty::launch(surface, &context, tool.name())?;
            return Ok(ExitCode::SUCCESS);
        }
        Action::Tools => {
            for tool in registry.available(&context) {
                println!("{}\t{}", tool.id(), tool.label());
            }
            return Ok(ExitCode::SUCCESS);
        }
        _ => {}
    }
    let mut picker = SkimPicker;
    if matches!(action, Action::ChooseDir) {
        let Some(path) = tools::choose_directory(&context, &mut picker)? else {
            return Ok(ExitCode::from(130));
        };
        let mut stdout = std::io::stdout().lock();
        stdout
            .write_all(path.as_os_str().as_encoded_bytes())
            .context("write selected directory")?;
        stdout.write_all(b"\0")?;
        stdout.flush()?;
        return Ok(ExitCode::SUCCESS);
    }
    let effect = match action {
        Action::Menu => registry.menu(&context, &mut picker)?,
        Action::Edit => registry.run("edit", &context, &mut picker)?,
        Action::Browse => registry.run("browse", &context, &mut picker)?,
        _ => unreachable!(),
    };
    if let Some(effect) = effect {
        effects::execute(&effect, &context.cwd)?;
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("kitty-pick: {error:#}");
            ExitCode::FAILURE
        }
    }
}
