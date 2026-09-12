use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

fn workspace_root() -> std::path::PathBuf {
    let mut candidate = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    loop {
        if candidate.join("Cargo.toml").is_file() && candidate.join("apps/scytale-studio").is_dir()
        {
            return candidate;
        }
        if !candidate.pop() {
            return Path::new(".").to_path_buf();
        }
    }
}

pub struct PtyState {
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    _child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
}

impl PtyState {
    pub fn spawn(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        #[cfg(windows)]
        let (shell, args): (String, &[&str]) = {
            ("powershell.exe".to_string(), &["-NoLogo"])
        };
        #[cfg(not(windows))]
        let (shell, args): (String, &[&str]) = {
            let default_shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
            (default_shell, &["-i"])
        };
        let mut command = CommandBuilder::new(shell);
        command.args(args);
        command.cwd(&workspace_root());
        for (key, value) in std::env::vars() {
            command.env(key, value);
        }
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        let child = pair.slave.spawn_command(command)?;
        let mut reader = pair.master.try_clone_reader()?;
        let master = Arc::new(Mutex::new(pair.master));
        let writer = Arc::new(Mutex::new(
            master
                .lock()
                .map_err(|_| "PTY master poisoned")?
                .take_writer()?,
        ));
        let app_handle = app.clone();

        std::thread::spawn(move || {
            let mut buffer = [0u8; 1024];
            while let Ok(size) = reader.read(&mut buffer) {
                if size == 0 {
                    break;
                }
                let output = String::from_utf8_lossy(&buffer[..size]).into_owned();
                let _ = app_handle.emit("pty_output", output);
            }
        });

        Ok(Self {
            master,
            writer,
            _child: Arc::new(Mutex::new(child)),
        })
    }
}

#[tauri::command]
pub fn pty_write(state: State<'_, PtyState>, data: String) -> Result<(), String> {
    let mut writer = state
        .writer
        .lock()
        .map_err(|_| "PTY writer poisoned".to_string())?;
    writer
        .write_all(data.as_bytes())
        .map_err(|error| error.to_string())?;
    writer.flush().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn pty_resize(state: State<'_, PtyState>, cols: u16, rows: u16) -> Result<(), String> {
    let master = state
        .master
        .lock()
        .map_err(|_| "PTY master poisoned".to_string())?;
    master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| error.to_string())
}
