use crate::model::*;
use std::{fs, path::Path, process::Command};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct NetworkSettings {
    #[serde(default)]
    pub proxy: String,
    #[serde(default)]
    pub bypass: String,
}

impl NetworkSettings {
    pub fn validated(mut self) -> Result<Self> {
        let mut hosts = Vec::new();
        for host in self
            .bypass
            .split(|c: char| c == ',' || c == '，' || c.is_whitespace())
        {
            if host.is_empty() {
                continue;
            }
            let host = host.to_ascii_lowercase();
            if host != "*" && url::Host::parse(host.trim_start_matches('.')).is_err() {
                return Err(GitError::new(
                    "configuration",
                    "直连域名只填写主机名或 IP，多个用逗号分隔，不填写协议、端口或路径",
                ));
            }
            if !hosts.contains(&host) {
                hosts.push(host);
            }
        }
        self.bypass = hosts.join(",");
        self.proxy = self.proxy.trim().into();
        if self.proxy.is_empty() {
            return Ok(self);
        }
        let invalid = || {
            GitError::new("configuration", "代理地址须为 http://、https://、socks5:// 或 socks5h:// 开头的主机和端口，不支持账号密码、路径或参数")
        };
        let url = url::Url::parse(&self.proxy).map_err(|_| invalid())?;
        if !matches!(url.scheme(), "http" | "https" | "socks5" | "socks5h")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !matches!(url.path(), "" | "/")
            || self.proxy.chars().any(char::is_control)
            || url.port_or_known_default().is_none_or(|port| port == 0)
        {
            return Err(invalid());
        }
        self.proxy = url.to_string();
        Ok(self)
    }

    pub fn load(path: &Path) -> Result<Self> {
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice::<Self>(&bytes)
                .map_err(|e| {
                    GitError::new("config", format!("网络设置无法读取，原文件已保留：{e}"))
                })?
                .validated(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(GitError::new("config", e.to_string())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| GitError::new("config", "无效配置路径"))?;
        fs::create_dir_all(parent).map_err(|e| GitError::new("config", e.to_string()))?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| GitError::new("config", e.to_string()))?;
        use std::io::Write;
        temp.write_all(&serde_json::to_vec_pretty(self).unwrap())
            .and_then(|_| temp.as_file().sync_all())
            .map_err(|e| GitError::new("config", e.to_string()))?;
        temp.persist(path)
            .map_err(|e| GitError::new("config", e.to_string()))?;
        Ok(())
    }

    // Per-process settings: never change global Git config or the GUI environment.
    pub(crate) fn apply(&self, command: &mut Command) {
        if !self.proxy.is_empty() {
            command.arg("-c").arg(format!("http.proxy={}", self.proxy));
            if !self.bypass.is_empty() {
                // Git's HTTP transport uses curl, which reads no_proxy even when
                // http.proxy is explicit. Set both cases to avoid inherited conflicts.
                command
                    .env("no_proxy", &self.bypass)
                    .env("NO_PROXY", &self.bypass);
            }
        }
    }
}
