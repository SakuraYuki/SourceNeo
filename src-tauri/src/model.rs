use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repo {
    pub path: String,
    pub name: String,
    pub common_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepoGroup {
    pub name: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum GroupEdit {
    Create {
        name: String,
    },
    Rename {
        group: String,
        name: String,
    },
    Remove {
        group: String,
    },
    Move {
        paths: Vec<String>,
        group: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum StatusKind {
    Synced,
    Ahead,
    Behind,
    Diverged,
    Conflict,
    Attention,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub path: String,
    pub branch: String,
    pub head: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub changed: usize,
    pub conflicts: Vec<String>,
    pub operation: Option<String>,
    pub kind: StatusKind,
    pub message: String,
    pub error_kind: Option<String>,
    pub last_fetched: Option<u64>,
    pub checked_at: u64,
    pub shallow: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub hash: String,
    pub parents: Vec<String>,
    pub subject: String,
    pub author: String,
    pub date: String,
    pub refs: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct History {
    pub commits: Vec<Commit>,
    pub has_more: bool,
    pub shallow: bool,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub reference: String,
    pub name: String,
    pub remote: bool,
    pub current: bool,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitChanges {
    pub hash: String,
    pub parent: Option<String>,
    pub files: Vec<ChangedFile>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub time: u64,
    pub path: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitError {
    pub kind: String,
    pub message: String,
}
impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for GitError {}
impl GitError {
    pub fn new(kind: &str, message: impl AsRef<str>) -> Self {
        let mut message = crate::git::redact(message.as_ref());
        if message.len() > 8192 {
            let mut boundary = 8192;
            while !message.is_char_boundary(boundary) {
                boundary -= 1;
            }
            message.truncate(boundary);
            message.push_str("\n[错误信息过长，已截断]");
        }
        Self {
            kind: kind.into(),
            message,
        }
    }
}
pub type Result<T> = std::result::Result<T, GitError>;
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
