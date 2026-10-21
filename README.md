# SourceNeo

Windows / macOS 轻量多仓库 Git 管理工具，使用 Tauri 2、React、TypeScript 和本机 Git CLI。中文界面，默认左侧管理仓库、右侧查看真实提交拓扑，也可切换为上下布局。提交和冲突解决交给外部 AI / Codex。

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

产物位于 `src-tauri/target/release/bundle/`。当前应用未作开发者签名、公证或对外发布。`npm run dev` 仅预览前端，浏览器不能操作本地 Git。

## 界面与外观

每个仓库仅占一行，显示仓库名、分支与同步状态；悬停仓库名或该行可查看完整路径。宽窗口默认左右布局，分隔栏可拖动，也支持左右方向键调整。右上角布局按钮可切换左右 / 上下布局；窗口窄于 1040px 时自动使用上下布局，加宽后恢复所选布局。

左右布局采用紧凑侧栏，宽度可在 260–560px 范围调整，隐藏表头，优先给仓库名留空间，分支与状态按内容占位；窄于 300px 时隐藏分支和添加按钮文字，完整分支仍可在行的悬停提示中查看。同步和“⋯”按钮在悬停、键盘聚焦或操作进行中显示。上下布局保留表格列。

仓库行支持右键菜单，也可通过行内“⋯”或聚焦后按 Shift+F10 / 菜单键打开。菜单提供 Fetch、远端选择、同步、分支切换、未提交文件 / 处理建议、终端 / 编辑器打开、复制路径 / 诊断、工具设置及移除。菜单操作始终针对标题标明的仓库，右键其他仓库不切换当前历史；移除仍需确认。详情区保留 Fetch、同步、问题查看和当前分支入口，其余操作通过“⋯”打开。菜单支持方向键、Home / End、Enter、Esc，点击外部、滚动外部区域、窗口缩放或失焦时关闭，并在 Esc 后恢复入口焦点。文件与提交继续单击查看。

右上角太阳 / 月亮按钮切换浅色和深色模式，覆盖页面、原生窗口、提交图、状态与弹窗。首次启动采用系统明暗外观，此后记住所选模式。主题、布局和左侧仓库列表宽度保存在本机 WebView 存储中，重启后恢复；窗口变窄时临时限制显示宽度，加宽后恢复记住的宽度。

窗口大小、位置及最大化 / 全屏状态在正常退出时保存，重新打开时恢复。窗口状态保存在应用配置目录下的 `.window-state.json`，由 [Tauri Window State](https://v2.tauri.app/plugin/window-state/) 管理。

## 第一版功能

- 添加、移除和持久化多个本地仓库；添加子目录会归一为工作区根路径，重复路径不会重复添加；识别 linked worktree。
- 当前分支、未提交文件数、ahead / behind、最后一次成功 fetch 时间。已同步只表示与**已知的远程跟踪引用**一致；本地刷新不会联网。
- 区分已同步、待推送、待拉取、已分叉、已有冲突、需要处理及失败；失败进一步区分网络、认证和其他错误。
- 单仓库同步、同步全部、筛选、搜索、操作日志；一个仓库失败时其余仓库继续。
- 仓库行直接显示“18 项未提交”“未设上游”“rebase 未完成”“认证失败”等具体原因。顶部“需关注”可打开仓库清单、原因和下一步建议，并定位对应仓库；详情区“查看未提交文件”列出当前路径、修改 / 新增 / 删除 / 重命名，以及已暂存 / 未暂存 / 未跟踪 / 冲突状态。文件清单只读，可手动刷新，不自动提交或丢弃改动。
- Git 真实父子关系驱动的 SVG 提交图，支持 merge、多个根节点、本地 / 远程分支、标签及 HEAD；点击提交查看 SHA 与父提交。
- 点击右侧当前分支可切换本地分支、从已有远程引用创建同名跟踪分支，或从当前 HEAD 新建分支。分支列表不自动联网；本地同名分支已存在或被其他 worktree 使用时由 Git 拒绝，不覆盖现有分支。有未提交改动、冲突、未完成操作或 Git 锁时暂停切换，保留被忽略文件，不自动 stash 或强制切换。
- 点击提交后，下方显示新增、修改、删除、重命名文件及逐文件 diff。根提交展示全部新增文件，合并提交与第一父提交比较；二进制文件显示 Git 的差异摘要。读取不执行 external diff / textconv，不读取工作区版本；浅克隆缺少父提交对象时报告读取失败，需要先加深克隆。单文件文本预览最多 2,000 行，超过会明确提示，Git 输出仍受 8 MiB 上限保护。
- 默认加载最近 100 条历史，每次再加载 100 条，最多 5,000 条；虚拟滚动只渲染可见行及相邻缓冲区。浅克隆会提示历史边界。
- 复制路径、分支、状态、冲突文件和相关错误，交给 AI。不会复制 remote URL 配置、凭据配置或文件内容；错误中的 URL 用户信息、常见 token 参数和 Authorization 会脱敏。

## 未提交差异、仅获取远端与外部工具

在“需关注 / 查看未提交文件”中点击文件名，可只读预览。已暂存比较 index 与 HEAD，未暂存比较工作区与 index；同一文件两种状态可切换，首次提交也可查看暂存内容。未跟踪文件显示内容，二进制、空文件、重命名、删除、模式变化和冲突有相应提示。符号链接不读取目标。单文件最多预览 256 KiB / 2,000 行，截断会提示；Git 输出仍受 8 MiB 上限约束，超限报告错误。读取失败可重试，刷新清单、切换文件及关闭后忽略迟到响应。

顶部“Fetch 全部”和当前仓库详情区“Fetch 此仓库”只获取远程对象与远程跟踪引用。默认使用当前分支配置的命名远端，否则唯一远端可直接获取；多个远端时通过仓库菜单中的“选择获取远端…”选择，没有远端时提示先配置。未设上游、游离 HEAD、空仓库和未提交改动都不阻止已选远端的获取。使用显式 `refs/heads/*:refs/remotes/<remote>/*` 和空 refmap，忽略配置中指向本地分支或标签的 fetch refspec，同时保留经校验、目标位于 `refs/remotes/` 的自定义映射和远端排除规则，保证自定义 upstream 不会停留在旧引用；不 prune、不获取标签或子模块，不 pull / merge / rebase / push。同步的获取阶段复用此策略。共享 Git 目录串行并共用成功时间；全部获取逐仓库处理，一个失败不影响其他仓库，可能重复获取共享目录，以尊重不同 worktree 的分支远端配置。失败保留此前成功时间；详细结果在操作日志中。没有后台联网。

仓库右键菜单和详情区“⋯”菜单提供“在终端打开”“在编辑器打开”和“外部工具设置”。Windows 默认打开可见的 Windows PowerShell，macOS 使用 Terminal；自动检测常见安装位置的 VS Code / Cursor。也可填写终端或编辑器可执行文件的完整路径（Windows 为 `.exe`），留空恢复自动选择，配置与仓库列表一同保存。自定义 PowerShell / pwsh 使用与默认终端一致的安全目录定位；其他自定义终端继承仓库工作目录，编辑器另接收独立的仓库路径参数；程序需支持这些启动约定。缺少程序、启动失败或启动后立即以错误码退出会提示，可配置后重试；外部程序较晚发生的运行错误由其自身处理。不支持任意命令模板，不自动运行 Git 或发送 AI 任务。Windows 默认 PowerShell 用固定脚本和环境变量传递目录，不把路径拼接进脚本；不开放前端 shell 权限。打开位置是当前仓库根目录，包括 linked worktree。

## 同步规则

1. 检测冲突和未完成的 merge / rebase / cherry-pick / revert / sequencer / bisect，存在时暂停。
2. 缺少 upstream、游离 HEAD 或空仓库时给出可处理原因；upstream 合并目标有多个配置值时，在 fetch 前停止；不猜测分支关系。
3. 对当前分支对应的 remote 执行 fetch，再读取 Git 状态。
4. 分叉时停止，标记需要合并。**分叉不等于文件冲突。**
5. 有未提交改动时保留原状，暂停后续拉取和推送。被忽略文件若会被快进覆盖，Git 也会拒绝操作。
6. 可快进时仅执行 `merge --ff-only --no-autostash --no-squash --no-overwrite-ignore`，目标固定为已获取的 upstream 提交；随后仅普通推送当前分支的已检查提交到其 upstream，不推送其他分支或子模块。

不会自动 stash、commit、rebase、reset、clean、force push 或丢弃修改。保留原有 Git 配置、SSH、凭据助手和 hooks；关闭终端交互式凭据输入，认证失败会显示错误。需要交互登录时先在终端完成 Git 认证，再重试。

同一仓库以及共享 common Git directory 的 worktree 在应用内串行，同步和分支切换操作另以独立 `sourceneo.lock` 文件协调 SourceNeo 进程；状态、历史、分支列表、提交详情和诊断读取不创建锁文件，可浏览只读 Git 目录；最多并行处理 3 个仓库。应用不删除 Git 原有锁。写入前后会重新检查 HEAD、分支、upstream、改动和进行中操作，并依赖 Git 自身的原子锁与普通推送校验。

添加仓库时先检查原始路径是否存在，存在则原样使用；仅在确认原始路径不存在时尝试去除输入两端空白。Git 返回的根目录和元数据路径仅去掉命令附加的行结束符，保留真实目录名中的空格。

每次操作会核对仓库根目录和 common Git directory 是否仍与添加时一致；嵌套仓库失去 `.git` 时会停止操作，避免 Git 向上发现并操作父仓库。确认仓库迁移后需重新添加。

外部 AI / Git 不会遵守 SourceNeo 的应用锁，因此无法跨两个独立工具提供覆盖整个同步过程的原子事务。应尽量避免同时操作同一仓库；检测到变化时会暂停并提示刷新。超时或 hook 异常可能需要在终端检查 Git 最终状态。

## 刷新、配置和日志

- 窗口可见时每 15 秒刷新本地状态；回到窗口会请求刷新，自动刷新间隔不小于 10 秒。手动刷新节流为 1 秒。
- 远程仅在主动 Fetch 或同步时获取；共享 common directory 的 worktree 共用成功 fetch 时间。
- 仓库配置使用同目录临时文件原子替换。macOS 通常为 `~/Library/Application Support/dev.sourceneo.desktop/repositories.json`，Windows 为 `%APPDATA%\dev.sourceneo.desktop\repositories.json`。
- 配置损坏时保留原文件并报告启动错误。移除仓库只移除列表项，不删除仓库。
- 操作日志仅保留当前进程最近 500 条，退出后清空；Git 输出单流上限 8 MiB，错误摘要上限 8 KiB；单次 Git 子进程及其输出等待上限 90 秒。
- 调试构建可通过 `SOURCENEO_CONFIG_DIR` 指定隔离配置目录；正式构建忽略此变量。

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

`.github/workflows/build.yml` 分别在 macOS 和 Windows runner 上检查、测试并打包未签名产物。工作流不会发布 release。配好工作流不等于 Windows 已经验证；本机结果和剩余平台验证见 [VALIDATION.md](VALIDATION.md)。

## 实现结构

- `src-tauri/src/git.rs`：Git 命令、输出解析、超时与脱敏。
- `src-tauri/src/service.rs`：仓库持久化、worktree 协调、有界并发、同步决策和日志。
- `src-tauri/src/lib.rs`：固定的 Tauri IPC 命令，阻塞工作移到后台执行。
- `src/graph.ts`：以待访问祖先为图的轨道，所有出边都源自 Git 父提交。
- `src/HistoryView.tsx`：SVG 图和虚拟列表。
- `src/App.tsx`：多仓库界面及请求节流。

第一版没有按行暂存、提交编辑器、冲突编辑器、交互式 rebase、AI API、后台自动同步或自动更新。历史超过 5,000 条、签名发布、Windows 实机交互和两端统一的内存基准仍需后续验证或扩展。
