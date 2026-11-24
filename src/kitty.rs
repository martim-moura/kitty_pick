//! Kitty process launch is independent of tools and picker implementation.
use std::{ffi::OsStr, path::Path, process::Command};

use anyhow::{Context as _, Result};

use crate::Context;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Surface {
    Overlay,
    OsWindow,
    Panel,
    Standalone,
    /// Reuse a Kitty instance without the remote-control protocol.
    SingleInstance,
}

/// Build an argument-vector launch. No shell is involved.
/// `source_window` should be the invoking KITTY_WINDOW_ID, not a focus lookup.
pub fn command(
    surface: Surface,
    executable: &Path,
    context: &Context,
    tool: &str,
    source_window: Option<&OsStr>,
) -> Command {
    let mut command = Command::new("kitty");
    match surface {
        Surface::Overlay | Surface::OsWindow | Surface::Panel => {
            command.args([
                "@",
                "launch",
                "--type",
                match surface {
                    Surface::Overlay => "overlay",
                    Surface::OsWindow => "os-window",
                    Surface::Panel => "os-panel",
                    _ => unreachable!(),
                },
            ]);
            command.args(["--title", "Kitty pick"]);
            if let Some(window) = source_window {
                let mut source = std::ffi::OsString::from("id:");
                source.push(window);
                let mut tab = std::ffi::OsString::from("window_id:");
                tab.push(window);
                command
                    .arg("--source-window")
                    .arg(source)
                    .arg("--match")
                    .arg(tab);
            }
            command.arg("--cwd").arg(&context.cwd);
            // Copy only these current caller settings, not all potentially sensitive env.
            for key in ["PATH", "SHELL"] {
                if let Some(value) = std::env::var_os(key) {
                    let mut setting = std::ffi::OsString::from(format!("{key}="));
                    setting.push(value);
                    command.arg("--env").arg(setting);
                }
            }
            if surface == Surface::Panel {
                command.args([
                    "--os-panel",
                    "edge=center-sized",
                    "--os-panel",
                    "lines=28",
                    "--os-panel",
                    "columns=100",
                    "--os-panel",
                    "layer=overlay",
                ]);
            }
        }
        Surface::Standalone | Surface::SingleInstance => {
            if surface == Surface::SingleInstance {
                command.arg("--single-instance");
            }
            command
                .arg("--directory")
                .arg(&context.cwd)
                .args(["--title", "Kitty pick"]);
        }
    }
    command.arg(executable).arg("--cwd").arg(&context.cwd);
    if context.hidden {
        command.arg("--hidden");
    }
    command.arg(tool);
    command
}

/// Return only after Kitty acknowledges the launch, not when the tool exits.
pub fn launch(surface: Surface, context: &Context, tool: &str) -> Result<()> {
    let executable = std::env::current_exe().context("resolve picker executable")?;
    let source = std::env::var_os("KITTY_WINDOW_ID");
    let mut command = command(surface, &executable, context, tool, source.as_deref());
    if matches!(surface, Surface::Standalone | Surface::SingleInstance) {
        // Detach stdio so callers can capture output without keeping pipes open.
        // Reap the launcher asynchronously if this library is embedded in a long-lived app.
        let mut child = command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .context("start Kitty")?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        return Ok(());
    }
    let output = command.output().context("launch Kitty window")?;
    anyhow::ensure!(
        output.status.success(),
        "Kitty launch failed (no fallback attempted): {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlay_pins_source_and_preserves_cwd() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("a ' quoted");
        std::fs::create_dir(&root)?;
        let context = Context::new(&root, true)?;
        let command = command(
            Surface::Overlay,
            Path::new("/a b/kitty-pick"),
            &context,
            "edit",
            Some(OsStr::new("65")),
        );
        let args: Vec<_> = command.get_args().map(|s| s.to_string_lossy()).collect();
        assert!(args.windows(2).any(|w| w == ["--source-window", "id:65"]));
        assert!(args.windows(2).any(|w| w == ["--match", "window_id:65"]));
        assert!(args.iter().any(|a| a == "/a b/kitty-pick"));
        assert!(
            args.iter()
                .any(|a| a.as_ref() == context.cwd.to_string_lossy())
        );
        assert_eq!(&args[args.len() - 2..], ["--hidden", "edit"]);
        Ok(())
    }
}
