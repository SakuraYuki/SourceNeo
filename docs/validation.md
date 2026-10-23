# 验证状态

本页说明当前公开版本的验证范围和复现方式。每次提交的实际检查结果以 [GitHub Actions](https://github.com/SakuraYuki/SourceNeo/actions) 为准；安装包及校验文件以对应的 [GitHub Release](https://github.com/SakuraYuki/SourceNeo/releases) 为准。

## 自动化检查

[桌面构建工作流](../.github/workflows/build.yml) 在 Windows 和 macOS 上分别运行以下检查，并生成未签名的桌面应用产物：

```sh
npm ci
npm run format:check
npm run build
npm test
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

工作流随后在 Windows 构建 NSIS 安装包，在 macOS 构建 `.app` 归档。测试使用临时 Git 仓库和本地 bare remote；不需要访问使用者的仓库或远端。工作流不会自动创建 GitHub Release。

## v0.1.2 发布范围

- [v0.1.2 Release](https://github.com/SakuraYuki/SourceNeo/releases/tag/v0.1.2) 提供 Windows x64 安装包和 macOS arm64 应用归档。发布附件由对应提交的 CI 构建生成。
- 这两个平台已通过工作流中的自动检查与打包。Windows 另完成了本地 NSIS 构建。
- v0.1.2 尚无完整的 Windows 与 macOS 原生交互回归记录；CI 构建成功不等同于所有安装、系统集成和 Git 操作都已在目标机器上手工验收。
- 发布产物未做开发者签名，macOS 应用未公证。

## 本地复现

在仓库根目录执行上述命令。桌面打包和独立测试仓库的启动方法见[开发与验证](development.md)。手工检查同步、分支切换等写操作时，请使用独立的临时仓库；构建结果和手工验收结果应分别记录。

## 当前源码扩展验证（2026-10-23，未发布）

本次新增克隆 / 取消克隆、目录扫描导入、提交搜索、上游比较、同步预览、批量阶段 / 停止排队 / 失败重试、持久化日志 / 导出，以及自适应刷新和历史快照分页。以下记录只代表本次本地验证，不替代发布安装包验收。

- 前端 87 项测试通过，覆盖原有交互，以及克隆失败重试与取消、部分导入失败、预览阻塞、服务端历史搜索、批量停止和保留操作类型的失败重试、后台刷新退避、历史缓存恢复、增量图布局、快照失效恢复和迟到响应隔离。
- Rust 80 项测试通过（2 个单元测试、71 个原有 Git 场景、2 个分组场景、5 个新增功能场景）。新场景使用临时仓库验证快照翻页期间 HEAD 改变、未加载提交搜索、扫描深度 / 依赖目录 / worktree / 链接、克隆地址与目标校验、预览和比较只读、日志重启恢复。macOS 还用本地 SSH 适配器完成真实 clone 成功、失败、取消和自动加入列表验证，不依赖外部网络。
- TypeScript / Vite 构建、Prettier、Rust fmt 和全目标 Clippy 检查通过；macOS arm64 调试 `.app` 打包通过。
- macOS 原生冒烟验收使用独立应用标识和临时仓库：提交搜索 → 提交详情、分叉分支比较 → 文件 diff、脏工作区预览阻塞、扫描 / 勾选 / 批量导入 / 去重，以及日志筛选 → 原生保存对话框 → JSON 文件写入均已验证。最终调试包另完成“启动 HTTPS 克隆 → 点击取消 → 显示保留目录 → 恢复表单”的原生验收。用户日常 SourceNeo 实例与仓库配置未替换。
- 公开 GitHub 仓库的 HTTPS 克隆尝试停留在网络传输阶段，本次未验证外部网络下的成功克隆；该尝试已终止并记录错误，未加入仓库列表。克隆成功路径的验证来自上述真实 Git + 本地 SSH 适配器。真实凭据助手和私有远程仍需在目标网络验收。
- Windows 原生交互、Windows 安装包和真实证书签名 / 公证未在本机验证。新增手动签名工作流需要维护者提供对应凭据，未触发远程构建或发布。

### 本地性能样本

使用 `cargo run --no-default-features --example benchmark -- 20 1000`（完整路径参数见开发文档），macOS、开发构建、20 个干净本地仓库，每仓库 1,000 条线性提交，不联网：

| 指标 | 结果 |
| --- | --- |
| 三并发全量状态刷新，三轮 | 1,019 / 1,025 / 1,032 ms |
| 从 100 翻至 1,000 条，旧式累积历史 | 2,001 ms，累计传输 5,500 条 |
| 同样范围，固定快照分页 | 1,051 ms，累计传输 1,000 条 |

这是单次顺序运行的开发环境样本，可能受文件系统缓存影响，不构成严格 A/B 性能结论；未测量真实大型工作区的文件扫描、广分支拓扑或内存峰值。基准工具可用于后续在 Windows、更多仓库和 5,000 条历史上复测。

### 目标系统复验清单

使用独立配置和 `scripts/create-fixtures.py` 的仓库，依次检查：克隆认证成功 / 失败 / 取消 / 已有目标拒绝；扫描导入与失败重试；分页后切换仓库及手动刷新；搜索和上游比较；批量 Fetch 的停止排队 / 失败重试；未提交 / 分叉 / 冲突的预览与同步；退出重开后的日志及 JSON 导出；窄窗口、系统明暗、原生目录选择和签名安装。
