use crate::model::*;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ToolSettings {
    pub terminal: String,
    pub editor: String,
}

#[derive(Debug)]
pub struct LaunchSpec {
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub terminal: bool,
}

pub fn plan(root: &str, kind: &str, settings: &ToolSettings, windows: bool) -> Result<LaunchSpec> {
    let custom = match kind {
        "terminal" => &settings.terminal,
        "editor" => &settings.editor,
        _ => return Err(GitError::new("tool", "未知工具类型")),
    };
    let mut spec = LaunchSpec {
        executable: PathBuf::new(),
        args: vec![],
        cwd: PathBuf::from(root),
        terminal: kind == "terminal",
    };
    if !custom.is_empty() {
        let path = Path::new(custom);
        if !path.is_absolute()
            || !path.is_file()
            || (windows
                && path
                    .extension()
                    .is_none_or(|e| !e.eq_ignore_ascii_case("exe")))
        {
            return Err(GitError::new(
                "tool",
                "请在“外部工具设置”填写存在的可执行文件完整路径（Windows 为 .exe），不支持命令模板",
            ));
        }
        spec.executable = path.into();
        if kind == "editor" {
            spec.args.push(root.into());
        }
    } else if kind == "terminal" {
        if windows {
            spec.executable = PathBuf::from(
                std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
            )
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        } else {
            spec.executable = "/usr/bin/open".into();
            spec.args = vec!["-a".into(), "Terminal".into(), root.into()];
        }
    } else {
        let mut candidates = Vec::new();
        if windows {
            for base in [
                std::env::var_os("LOCALAPPDATA"),
                std::env::var_os("ProgramFiles"),
            ]
            .into_iter()
            .flatten()
            {
                for suffix in [
                    "Programs/Microsoft VS Code/Code.exe",
                    "Microsoft VS Code/Code.exe",
                    "Programs/cursor/Cursor.exe",
                ] {
                    candidates.push(PathBuf::from(&base).join(suffix));
                }
            }
        } else {
            for app in ["Visual Studio Code", "Cursor"] {
                candidates.push(PathBuf::from(format!(
                    "/Applications/{app}.app/Contents/MacOS/Electron"
                )));
            }
        }
        spec.executable = candidates
            .into_iter()
            .find(|p| p.is_file())
            .ok_or_else(|| {
                GitError::new(
                    "tool",
                    "未检测到 VS Code / Cursor；请打开“外部工具设置”，填写编辑器可执行文件完整路径",
                )
            })?;
        spec.args = vec!["--new-window".into(), root.into()];
    }
    if windows
        && spec.terminal
        && spec.executable.file_name().is_some_and(|name| {
            name.eq_ignore_ascii_case("powershell.exe") || name.eq_ignore_ascii_case("pwsh.exe")
        })
    {
        spec.args = [
            "-NoLogo",
            "-NoProfile",
            "-NoExit",
            "-Command",
            "Set-Location -LiteralPath $env:SOURCENEO_REPO_ROOT",
        ]
        .into_iter()
        .map(String::from)
        .collect();
    }
    Ok(spec)
}

pub fn launch_with<T>(
    spec: &LaunchSpec,
    spawn: impl FnOnce(&mut Command) -> std::io::Result<T>,
) -> Result<T> {
    let mut command = Command::new(&spec.executable);
    command
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .env("SOURCENEO_REPO_ROOT", &spec.cwd);
    // An interactive terminal must not inherit NUL streams. In a GUI process,
    // unspecified streams let CREATE_NEW_CONSOLE initialize the console handles.
    if !spec.terminal {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // A user-requested interactive terminal needs its own visible console.
        if spec.terminal {
            command.creation_flags(0x00000010);
        } else {
            command.creation_flags(0x08000000);
        }
    }
    spawn(&mut command)
        .map_err(|e| GitError::new("tool", format!("启动失败：{e}。请检查外部工具设置后重试")))
}

pub fn launch(spec: &LaunchSpec) -> Result<()> {
    let mut child = launch_with(spec, |command| command.spawn())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(350);
    while std::time::Instant::now() < deadline {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| GitError::new("tool", e.to_string()))?
        {
            return if status.success() {
                Ok(())
            } else {
                Err(GitError::new(
                    "tool",
                    format!("工具启动后立即退出（{status}），请检查外部工具设置后重试"),
                ))
            };
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plans_pass_paths_as_arguments_and_cwd_without_shell_templates() {
        let root = "C:\\中文 space & '$; [x]";
        let spec = plan(root, "terminal", &ToolSettings::default(), true).unwrap();
        launch_with(&spec, |command| {
            assert_eq!(command.get_current_dir(), Some(Path::new(root)));
            assert_eq!(
                command.get_args().last().unwrap(),
                "Set-Location -LiteralPath $env:SOURCENEO_REPO_ROOT"
            );
            Ok(())
        })
        .unwrap();
        let mac = plan(
            "/tmp/中文 ; '$ space",
            "terminal",
            &ToolSettings::default(),
            false,
        )
        .unwrap();
        assert_eq!(mac.executable, Path::new("/usr/bin/open"));
        assert_eq!(mac.args, ["-a", "Terminal", "/tmp/中文 ; '$ space"]);
        let failure = launch_with(&spec, |_| {
            Err::<(), _>(std::io::Error::from(std::io::ErrorKind::NotFound))
        })
        .unwrap_err();
        assert!(failure.message.contains("外部工具设置"));
        let settings = ToolSettings {
            terminal: "powershell.exe -Command evil".into(),
            editor: String::new(),
        };
        assert!(plan(root, "terminal", &settings, true).is_err());
        let editor = ToolSettings {
            terminal: String::new(),
            editor: std::env::current_exe().unwrap().to_str().unwrap().into(),
        };
        let spec = plan(root, "editor", &editor, cfg!(windows)).unwrap();
        launch_with(&spec, |command| {
            assert_eq!(command.get_args().collect::<Vec<_>>(), [root]);
            assert_eq!(command.get_current_dir(), Some(Path::new(root)));
            Ok(())
        })
        .unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_real_process_preserves_unicode_shell_characters_and_cwd() {
        use std::os::windows::process::CommandExt;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("中文 space & '$; [x]");
        std::fs::create_dir(&root).unwrap();
        let mut spec = plan(
            root.to_str().unwrap(),
            "terminal",
            &ToolSettings::default(),
            true,
        )
        .unwrap();
        spec.args.retain(|arg| arg != "-NoExit");
        spec.args.last_mut().unwrap().push_str("; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::Write((Get-Location).ProviderPath)");
        let output = launch_with(&spec, |command| {
            // Test helper stays hidden; production uses a visible console.
            command
                .creation_flags(0x08000000)
                .stdout(Stdio::piped())
                .output()
        })
        .unwrap();
        assert!(output.status.success());
        let actual = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            actual.trim_start_matches('\u{feff}'),
            root.to_str().unwrap()
        );
        let settings = ToolSettings {
            terminal: spec.executable.to_str().unwrap().into(),
            editor: String::new(),
        };
        let mut custom = plan(root.to_str().unwrap(), "terminal", &settings, true).unwrap();
        assert!(custom
            .args
            .iter()
            .any(|arg| arg == "Set-Location -LiteralPath $env:SOURCENEO_REPO_ROOT"));
        custom.args.retain(|arg| arg != "-NoExit");
        custom.args.last_mut().unwrap().push_str("; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::Write((Get-Location).ProviderPath)");
        let custom_output = launch_with(&custom, |command| {
            command
                .creation_flags(0x08000000)
                .stdout(Stdio::piped())
                .output()
        })
        .unwrap();
        assert!(custom_output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&custom_output.stdout).trim_start_matches('\u{feff}'),
            root.to_str().unwrap()
        );
    }

    #[cfg(windows)]
    #[test]
    fn immediately_failed_tool_is_reported_instead_of_success() {
        let root = tempfile::tempdir().unwrap();
        let spec = LaunchSpec {
            executable: PathBuf::from(std::env::var_os("SystemRoot").unwrap())
                .join("System32/cmd.exe"),
            args: vec!["/d".into(), "/c".into(), "exit 7".into()],
            cwd: root.path().into(),
            terminal: false,
        };
        assert!(launch(&spec).unwrap_err().message.contains("立即退出"));
    }
}
