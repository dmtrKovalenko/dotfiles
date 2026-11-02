//! Opening things and running the user's commands, with real processes.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::usecases::ports::{Launcher, PortError};

/// `open` / `xdg-open` and shell command lines.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemLauncher;

impl Launcher for SystemLauncher {
    fn open(&self, target: &str, cwd: Option<&Path>) -> Result<(), PortError> {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        let mut command = Command::new(opener);
        command
            .arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(dir) = cwd {
            command.current_dir(dir);
        }
        let status = command
            .status()
            .map_err(|error| PortError::new(format!("cannot run `{opener} {target}`: {error}")))?;
        if status.success() {
            Ok(())
        } else {
            Err(PortError::new(format!(
                "`{opener} {target}` exited with an error"
            )))
        }
    }

    fn run(
        &self,
        command_line: &str,
        stdin: &str,
        env: &[(&str, String)],
        cwd: Option<&Path>,
    ) -> Result<(), PortError> {
        if command_line == ":fingers-open:" {
            return crate::custom::navigation::open_selections(stdin)
                .map_err(|error| PortError::new(error.to_string()));
        }
        let argv = shell_words::split(command_line)
            .map_err(|error| PortError::new(format!("cannot parse `{command_line}`: {error}")))?;
        let Some((program, args)) = argv.split_first() else {
            return Ok(());
        };
        let mut command = Command::new(program);
        command
            .args(args)
            .envs(env.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(dir) = cwd {
            command.current_dir(dir);
        }
        let mut child = command
            .spawn()
            .map_err(|error| PortError::new(format!("cannot run `{command_line}`: {error}")))?;
        if let Some(mut pipe) = child.stdin.take() {
            // A command that exits early closes the pipe; its exit status is
            // the verdict, not the broken pipe.
            let _ = pipe.write_all(stdin.as_bytes());
        }
        let status = child
            .wait()
            .map_err(|error| PortError::new(format!("cannot run `{command_line}`: {error}")))?;
        if status.success() {
            Ok(())
        } else {
            Err(PortError::new(format!(
                "`{command_line}` exited with an error"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("herdr-fingers-{name}-{unique}"))
    }

    #[test]
    fn a_command_gets_the_text_on_stdin_and_the_variables_in_its_environment() {
        let target = temp_file("launcher");
        let command_line = format!(
            "sh -c 'printf \"%s|%s|\" \"$MODIFIER\" \"$HINT\" > {0}; cat >> {0}'",
            target.display()
        );
        SystemLauncher
            .run(
                &command_line,
                "one two",
                &[("MODIFIER", "alt".into()), ("HINT", "ab".into())],
                None,
            )
            .unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "alt|ab|one two");
        let _ = std::fs::remove_file(target);
    }

    #[test]
    fn a_command_runs_in_the_given_directory() {
        let target = temp_file("cwd");
        let command_line = format!("sh -c 'pwd > {}'", target.display());
        let dir = std::env::temp_dir().canonicalize().unwrap();
        SystemLauncher
            .run(&command_line, "", &[], Some(&dir))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&target).unwrap().trim(),
            dir.to_str().unwrap()
        );
        let _ = std::fs::remove_file(target);
    }

    #[test]
    fn a_failing_or_unparsable_command_is_reported() {
        let error = SystemLauncher
            .run("sh -c 'exit 2'", "x", &[], None)
            .unwrap_err();
        assert!(error.0.contains("exited with an error"));
        let error = SystemLauncher
            .run("sh -c 'unclosed", "", &[], None)
            .unwrap_err();
        assert!(error.0.contains("cannot parse"));
        assert!(SystemLauncher.run("", "", &[], None).is_ok());
    }
}
