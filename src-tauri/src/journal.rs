use crate::{git, model::*};
use std::{collections::VecDeque, fs, io::Write, path::PathBuf, sync::Mutex};

pub struct Journal {
    path: PathBuf,
    entries: Mutex<VecDeque<LogEntry>>,
    writable: bool,
}
impl Journal {
    pub fn new(path: PathBuf) -> Self {
        let loaded = if path.exists() {
            fs::metadata(&path)
                .map_err(|e| e.to_string())
                .and_then(|m| {
                    if m.len() > 8 * 1024 * 1024 {
                        return Err("日志文件超过 8 MiB".into());
                    }
                    let data = fs::read(&path).map_err(|e| e.to_string())?;
                    serde_json::from_slice::<VecDeque<LogEntry>>(&data).map_err(|e| e.to_string())
                })
        } else {
            Ok(VecDeque::new())
        };
        let (mut entries, writable) = match loaded {
            Ok(entries) => (entries, true),
            Err(_) => (VecDeque::from([LogEntry { time: now(), path: String::new(), level: "warning".into(), message: "历史日志无法读取，原文件已保留；本次仅在内存记录，请备份并移走 operations.json 后重启".into(), operation: None, stage: None, elapsed_ms: None }]), false)
        };
        entries.truncate(500);
        for entry in &mut entries {
            entry.path = git::redact(&entry.path);
            entry.message = git::redact(&entry.message);
        }
        Self {
            path,
            entries: Mutex::new(entries),
            writable,
        }
    }
    pub fn entries(&self) -> Vec<LogEntry> {
        self.entries.lock().unwrap().iter().cloned().collect()
    }
    pub fn push(&self, entry: LogEntry) {
        let mut entries = self.entries.lock().unwrap();
        entries.push_front(entry);
        entries.truncate(500);
        if self.writable {
            let save = || -> std::io::Result<()> {
                let parent = self.path.parent().unwrap();
                fs::create_dir_all(parent)?;
                let mut file = tempfile::NamedTempFile::new_in(parent)?;
                file.write_all(&serde_json::to_vec(&*entries)?)?;
                file.as_file().sync_all()?;
                file.persist(&self.path).map_err(|e| e.error)?;
                Ok(())
            };
            if save().is_err()
                && !entries
                    .iter()
                    .any(|e| e.message == "操作日志无法保存，本次记录仅在内存中")
            {
                entries.push_front(LogEntry {
                    time: now(),
                    path: String::new(),
                    level: "warning".into(),
                    message: "操作日志无法保存，本次记录仅在内存中".into(),
                    operation: None,
                    stage: None,
                    elapsed_ms: None,
                });
                entries.truncate(500);
            }
        }
    }
}
