use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context as _, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Edit(Vec<PathBuf>),
    Shell(PathBuf),
    /// A result for the caller
    Directory(PathBuf),
}

/// Check executable availability
pub fn on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| {
            let path = dir.join(program);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                path.metadata()
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            }
            #[cfg(not(unix))]
            path.is_file()
        })
    })
}

pub fn command(effect: &Effect, cwd: &Path) -> Result<Command> {
    let mut command = match effect {
        Effect::Edit(paths) => {
            anyhow::ensure!(!paths.is_empty(), "no files selected");
            let editor_cwd = if paths.len() == 1 && paths[0].is_dir() {
                paths[0].as_path()
            } else {
                cwd
            };
            let mut command = Command::new("nvim");
            command.arg("--").args(paths).current_dir(editor_cwd);
            command
        }
        Effect::Shell(path) => {
            let shell = std::env::var_os("SHELL")
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "/bin/sh".into());
            let mut command = Command::new(shell);
            command.arg("-i").current_dir(path);
            command
        }
        Effect::Directory(_) => anyhow::bail!("directory result must be handled by caller"),
    };
    // Skim does not own the terminal anymore when this command runs.
    command
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    Ok(command)
}

/// Replace the picker process on Unix, so there is no idle wrapper process.
pub fn execute(effect: &Effect, cwd: &Path) -> Result<()> {
    let mut command = command(effect, cwd)?;
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec()).context("start selected action")
    }
    #[cfg(not(unix))]
    {
        let status = command.status().context("start selected action")?;
        anyhow::ensure!(status.success(), "selected action exited with {status}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_editor_uses_selected_directory_as_cwd() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let selected = temp.path().canonicalize()?;
        let command = command(&Effect::Edit(vec![selected.clone()]), Path::new("/"))?;
        assert_eq!(command.get_current_dir(), Some(selected.as_path()));
        Ok(())
    }

    #[test]
    fn editor_arguments_are_not_shell_code() -> Result<()> {
        let paths = vec![PathBuf::from("-c quit"), PathBuf::from("a\n'$(touch nope)")];
        let command = command(&Effect::Edit(paths.clone()), Path::new("/"))?;
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args[0], "--");
        assert_eq!(args[1], paths[0].as_os_str());
        assert_eq!(args[2], paths[1].as_os_str());
        Ok(())
    }
}
