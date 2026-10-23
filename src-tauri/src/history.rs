//! Pinned ref snapshots: later pages never move when HEAD or remote refs change.
use crate::{git, model::*};
use std::{collections::HashMap, path::Path, time::Instant};

#[derive(Clone)]
pub struct Snapshot {
    pub id: String,
    pub path: String,
    pub created: Instant,
    revisions: Vec<String>,
    refs: HashMap<String, Vec<String>>,
    shallow: bool,
    query: HistoryQuery,
}
impl Snapshot {
    pub fn new(repo: &Repo, id: String, all: bool, query: HistoryQuery) -> Result<Self> {
        if query.text.len() > 512 || query.text.contains(['\0', '\n', '\r']) {
            return Err(GitError::new(
                "query",
                "搜索内容须为不超过 512 字节的单行文本",
            ));
        }
        if !query.text.is_empty() && !["message", "author", "sha"].contains(&query.field.as_str()) {
            return Err(GitError::new("query", "无效的搜索类型"));
        }
        let path = Path::new(&repo.path);
        let head = git::run(path, &["rev-parse", "--verify", "HEAD"])
            .ok()
            .map(|s| s.trim().to_owned());
        let raw = git::run(path, &["for-each-ref", "--format=%(objectname)%00%(*objectname)%00%(refname)%00%(objecttype)%00%(*objecttype)"])?;
        let mut refs: HashMap<String, Vec<String>> = HashMap::new();
        for line in raw.lines() {
            let fields: Vec<_> = line.split('\0').collect();
            if fields.len() == 5 && (fields[3] == "commit" || fields[4] == "commit") {
                let oid = if fields[3] == "commit" {
                    fields[0]
                } else {
                    fields[1]
                };
                refs.entry(oid.into()).or_default().push(fields[2].into());
            }
        }
        if let Some(head) = &head {
            refs.entry(head.clone()).or_default().push("HEAD".into());
        }
        let mut revisions: Vec<String> = if all {
            refs.keys().cloned().collect()
        } else {
            head.into_iter().collect()
        };
        revisions.sort();
        if !query.text.is_empty() && query.field == "sha" {
            if !(4..=64).contains(&query.text.len())
                || !query.text.bytes().all(|c| c.is_ascii_hexdigit())
            {
                return Err(GitError::new("query", "请输入 4–64 位十六进制提交 SHA"));
            }
            let hash = git::run(
                path,
                &[
                    "rev-parse",
                    "--verify",
                    &format!("{}^{{commit}}", query.text),
                ],
            )?;
            revisions = vec![hash.trim().into()];
        }
        // Leave room for flags below the Windows command-line limit.
        if revisions.iter().map(|s| s.len() + 1).sum::<usize>() > 24_000 {
            return Err(GitError::new(
                "limit",
                "引用过多，请切换到当前分支历史或按 SHA 搜索",
            ));
        }
        Ok(Self {
            id,
            path: repo.path.clone(),
            created: Instant::now(),
            revisions,
            refs,
            shallow: git::run(path, &["rev-parse", "--is-shallow-repository"])?.trim() == "true",
            query,
        })
    }
    pub fn page(&self, offset: usize, count: usize) -> Result<History> {
        if offset >= 5000 {
            return Err(GitError::new(
                "limit",
                "单次查询最多浏览 5,000 条，请缩小搜索条件",
            ));
        }
        if self.revisions.is_empty()
            || (self.query.field == "sha" && !self.query.text.is_empty() && offset > 0)
        {
            return Ok(History {
                commits: vec![],
                has_more: false,
                shallow: self.shallow,
            });
        }
        let count = count.clamp(1, 100).min(5000 - offset);
        let mut args: Vec<String> = [
            "log",
            "--encoding=UTF-8",
            "--no-show-signature",
            "--no-notes",
            "--no-decorate",
            "--no-patch",
            "--topo-order",
            "-z",
            "--format=%H%x00%P%x00%s%x00%an%x00%aI",
        ]
        .map(String::from)
        .into();
        args.push(format!("--skip={offset}"));
        args.push(format!("--max-count={}", count + 1));
        if !self.query.text.is_empty() {
            if self.query.field == "sha" {
                args.push("--no-walk".into());
            } else {
                args.push("--fixed-strings".into());
                args.push("--regexp-ignore-case".into());
                args.push(format!(
                    "--{}={}",
                    if self.query.field == "author" {
                        "author"
                    } else {
                        "grep"
                    },
                    self.query.text
                ));
            }
        }
        args.extend(self.revisions.iter().cloned());
        args.push("--".into());
        let raw = git::run(
            Path::new(&self.path),
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
        )?;
        let fields: Vec<_> = raw.split_terminator('\0').collect();
        if !fields.len().is_multiple_of(5) {
            return Err(GitError::new("parse", "提交记录不完整"));
        }
        let mut commits: Vec<_> = fields
            .as_chunks::<5>()
            .0
            .iter()
            .map(|p| Commit {
                hash: p[0].into(),
                parents: p[1].split_whitespace().map(String::from).collect(),
                subject: git::redact(p[2]),
                author: git::redact(p[3]),
                date: p[4].into(),
                refs: self.refs.get(p[0]).cloned().unwrap_or_default(),
            })
            .collect();
        let has_more = commits.len() > count;
        commits.truncate(count);
        Ok(History {
            commits,
            has_more,
            shallow: self.shallow,
        })
    }
}

pub fn compare(repo: &Repo) -> Result<Comparison> {
    let path = Path::new(&repo.path);
    let head = git::run(path, &["rev-parse", "--verify", "HEAD"])?
        .trim()
        .to_owned();
    let target = git::run(path, &["rev-parse", "--verify", "@{upstream}"])?
        .trim()
        .to_owned();
    let upstream = git::run(path, &["rev-parse", "--abbrev-ref", "@{upstream}"])?
        .trim()
        .to_owned();
    let mut snapshot = Snapshot::new(repo, String::new(), false, HistoryQuery::default())?;
    snapshot.revisions = vec![format!("{target}..{head}")];
    let local = snapshot.page(0, 100)?;
    snapshot.revisions = vec![format!("{head}..{target}")];
    let remote = snapshot.page(0, 100)?;
    let files = git::parse_changes(&git::tree_diff(
        repo,
        &head,
        Some(&target),
        &["--name-status", "-z"],
        &[],
    )?)?;
    Ok(Comparison {
        head,
        upstream,
        target,
        local,
        remote,
        files,
    })
}
