# 开发与验证

以下命令均在仓库根目录执行。

## 启动

需要 Node.js 22.13+、Rust 1.90+（建议 stable）、Git 2.30+，以及对应平台的 [Tauri 开发依赖](https://v2.tauri.app/start/prerequisites/)。macOS 需要 Xcode Command Line Tools；Windows 需要 MSVC C++ Build Tools 和 WebView2。

```sh
npm ci
npm run desktop
```

启动脚本会把用户目录下的 `.cargo/bin` 加入子进程 PATH。Git 仍需要在 PATH 内。打包应用依赖系统 Git；从 Finder 启动时如找不到 Homebrew Git，请确保系统 Git 或受支持的系统 PATH 可用。

```sh
npm run desktop:build -- --bundles app    # macOS .app
npm run desktop:build -- --bundles nsis   # Windows 安装程序，在 Windows 上执行
```

产物位于 `src-tauri/target/release/bundle/`。公开版本的构建产物未作开发者签名，macOS 应用未公证。`npm run dev` 仅预览前端，浏览器不能操作本地 Git。

## 验证

```sh
npm run build
npm test
npm run format:check
cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

测试只创建临时仓库和本地 bare remote，不连接用户远程。场景覆盖同步、ahead / behind、分叉、真实冲突、未完成操作、未提交与忽略文件保护、upstream、空仓库、worktree、失败隔离、Git 锁、hooks、超时、脱敏、浅克隆和真实 merge 拓扑。

前端使用 JSDOM 和 React Testing Library 验证真实组件交互，覆盖弹窗内的添加 / 移除失败与重试、历史加载失败后的虚拟滚动恢复、加载更多时保留滚动位置，以及成功 / 失败同步后的分页恢复。Git 场景另覆盖嵌套仓库元数据丢失时禁止操作父仓库。

历史解析显式指定 UTF-8，并关闭签名、notes、装饰和补丁等额外输出；固定签名提交样本用于验证 SHA、父提交和引用标签，即使启用 `log.showSignature` 且本机没有签名验证工具也能读取。测试另覆盖移除最后一个仓库后清理错误 / 加载状态并忽略迟到请求，以及无效目录、目录权限和缺少 Git 的错误分类；PATH 变更在测试子进程内隔离。

生成可供原生 UI 验收的独立仓库：

```sh
python3 scripts/create-fixtures.py
# 从输出中取 config 路径，然后启动调试应用，例如 macOS：
SOURCENEO_CONFIG_DIR=/path/from/output/config npm run desktop
```

`.github/workflows/build.yml` 分别在 macOS 和 Windows runner 上检查、测试并打包未签名产物。工作流只上传构建产物，Release 由维护者单独发布。当前版本的验证范围见[验证状态](validation.md)。

## 实现结构

- `src-tauri/src/git.rs`：Git 命令、输出解析、超时与脱敏。
- `src-tauri/src/service.rs`：仓库持久化、worktree 协调、有界并发、同步决策和日志。
- `src-tauri/src/lib.rs`：固定的 Tauri IPC 命令，阻塞工作移到后台执行。
- `src/graph.ts`：以待访问祖先为图的轨道，所有出边都源自 Git 父提交。
- `src/HistoryView.tsx`：SVG 图和虚拟列表。
- `src/App.tsx`：多仓库界面组合；`src/hooks/`：自适应刷新、历史缓存、批量任务与进度轮询。
- `src-tauri/src/history.rs`：固定引用快照、增量分页、搜索和分支比较。
- `src-tauri/src/acquisition.rs`：受限目录扫描、在线地址校验与新目录克隆。
- `src-tauri/src/journal.rs`：脱敏日志的有界持久化与读取恢复。

当前版本没有按行暂存、提交编辑器、冲突编辑器、交互式 rebase、AI API、后台自动同步或自动更新。单次历史浏览仍以 5,000 条为上限，搜索可定位未加载历史；真实证书签名、Windows 实机交互和两端统一的内存基准仍需后续验证。


## 性能基准

```sh
cargo run --locked --manifest-path src-tauri/Cargo.toml --no-default-features --example benchmark -- 20 1000
```

仅创建临时仓库和本地克隆，退出自动清理，不联网、不读取用户仓库。两个参数为仓库数（最多 200）和每仓库提交数（100–5,000）。输出三轮三并发状态刷新耗时、旧式累积历史与新式快照分页耗时、累计传输提交数。测试条件与本次结果见 [验证状态](validation.md)。

可选签名工作流见 [签名构建](release-signing.md)。签名流程仅手动触发，不改变默认 CI 的未签名产物。
