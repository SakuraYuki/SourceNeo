# 第一版验收记录

## 2026-10-21 同步审查修复版本 Windows 安装包

在全部同步改动审查修复通过后，重新执行 `npm run desktop:build -- --bundles nsis`，前端生产构建、Rust Release 及 NSIS x64 打包成功。交付为 `artifacts/SourceNeo_0.1.0_x64-sync-reviewed-setup.exe`，大小 2,264,187 字节，SHA-256 为 `850487A6460FFE50971AF1476AA0DFA388E9176573CD4DB8D00B30BD65418675`，附同名 `.sha256` 文件；安装包按现有忽略规则保留在本机，不纳入源码提交。

复用紧接本次提交前完成的 67 项 Rust 测试、Clippy、Rustfmt 和差异检查，以及未改动前端的 64 项测试结果；正式打包再次执行前端生产构建。本次只打包，不更新或重启已安装应用，不读取或操作业务仓库、仓库列表或应用配置，不推送。下面审查阶段“未打包”记录指当时，本节记录后续交付。

## 2026-10-21 全部同步改动审查与修复

范围为 `git diff HEAD` 的全部净变化及未跟踪的 `sync_policy.rs`，暂存区为空。复查 rebase、冲突暂停、按路径保护、调用方提示和交付记录，未扩大为全仓巡检。发现并修复以下问题，前五项均先用临时仓库回归复现失败：

- Windows 中 `Config.json` 与忽略的 `config.json` 指向同一文件，但原字面 pathspec 区分大小写，保护会漏检并继续同步。查询现按 Windows / `core.ignoreCase` 语义匹配大小写，覆盖文件本身和阻挡目录的父文件；分批长度计算同时纳入新增 pathspec 标志。
- 旧 fork point 仍属于新 upstream 时，原实现仍从旧点重放，导致远端已采纳的本地提交再次重放并产生多余冲突。现以本次 upstream 排除已有提交，仅在旧 fork point 被远端改写移除时保留旧点。
- 远端纯回退时 `behind == 0`，原实现会跳过 rebase，普通推送重新恢复远端已撤回的提交。现按 rebase 配置整合此情况，保留未发布的本地工作，并增加回退时忽略文件仍受保护的测试。
- `pull.rebase=true` 时硬编码 `--no-rebase-merges`，覆盖了用户的 `rebase.rebaseMerges=true` 并压平合并拓扑。现普通 rebase 继承该配置，只有显式 `pull.rebase=merges` 才追加对应命令选项。
- 快进后只检查祖先关系，`post-merge` hook 新增提交后也可能被自动推送。现明确快进时恢复 HEAD 必须等于本次获取目标的检查，出现额外提交则停止后续推送。

另补充冲突文件解决并暂存、但尚未 continue 的中间状态验证：再次同步保持暂停、index 不变且远端不变，外部完成当前操作后才能恢复正常同步。配置与 fork point 语义对照 [Git 配置文档](https://git-scm.com/docs/git-config) 和 [Git 2.53 pull 源码](https://github.com/git/git/blob/v2.53.0/builtin/pull.c)。

修复后的定向回归及完整 Rust 回归均通过：62 项临时 Git 场景及 5 项单元测试，共 67 项。Clippy（含桌面功能及所有 targets，`-D warnings`）、Rustfmt 与 `git diff --check` 通过。前端本轮未改动，复用实现阶段 64 项测试及前端生产构建通过的结果。复审后未发现其他阻塞项。

测试只使用临时仓库及本地 bare remote，未读取或操作业务仓库、现有仓库列表或应用配置，未提交或推送工作区改动。未重新打包或更新已安装应用；此前安装包记录对应本轮审查修复前的版本。

## 2026-10-21 仅检查同步涉及路径，修复大忽略目录阻断

移除 rebase 前无路径限制的忽略文件枚举与 upstream 全树路径列表。现在从 `HEAD → upstream` 原始差异及实际 `replay_base..HEAD` 待重放提交的原始差异收集路径（含合并和中间提交），仅检查这些路径及阻挡写入的父文件 / 链接。查询使用字面 pathspec、按 Windows 命令行长度分批；没有候选路径时不调用查询。只读 `ls-files` 返回第一项覆盖风险后即停止读取并结束该查询，不收集完整忽略清单；一般 Git 命令的 8 MiB 输出保护保持不变。

新增临时仓库回归：45,000 个无关忽略缓存文件，完整文件名清单超过 8 MiB，旧实现已复现 `outputLimit`；同时覆盖该缓存目录确实被替换为文件时必须保护、中间提交创建又删除的特殊名称路径、忽略文件与目录互换，以及正常跟踪目录替换为文件不应被误拦截。原始路径解析另覆盖换行、冒号和不完整记录。所有测试只使用临时仓库与本地 bare remote，未操作业务仓库、提交或推送当前工作区。

前端 64 项测试、完整 Rust 回归 61 项（56 项临时 Git 场景及 5 项单元测试）、Prettier、Rustfmt、Clippy（含桌面功能及所有 targets，`-D warnings`）和差异格式检查均通过。45,000 文件的大目录回归由旧版 `outputLimit` 失败转为通过，真实覆盖风险仍返回 `changed` 并保留目录、文件及远端提交。

`npm run desktop:build -- --bundles nsis` 成功，包含前端生产构建和 Rust Release。安装包为 `artifacts/SourceNeo_0.1.0_x64-scoped-sync-setup.exe`，大小 2,259,758 字节，SHA-256 为 `5F2D722F1743E5B16410C92694F5072DED60CFCB9D94A8F5972B6A688EB5D304`，附同名校验文件。

正常关闭旧应用后备份最新配置、WebView 界面偏好和旧程序，以 `/S /UPDATE /NS` 更新，退出码 0；15 个配置 / 偏好文件安装前后哈希一致，16 个仓库保留。安装版与本次构建的 NSIS payload 完全一致，SHA-256 为 `BA131FED477FD61FB6AAC76D141B6E86116814B16F26AD23AE07578888A21731`。启动后 SourceNeo 主窗口响应正常，仓库与工具配置仍与备份逐字节一致。备份目录为 `%TEMP%\sourceneo-scoped-ignore-update-97acf4a21a1e4136a8ac6e9281d07546`。

本轮未读取或操作业务仓库，未点击 Fetch / 同步；业务仓库的实际同步留给用户验收。未提交或推送当前工作区改动。

## 2026-10-21 Git 配置同步版本 Windows 验收包

在当前未提交源码上执行 `npm run desktop:build -- --bundles nsis`，前端生产构建及 Rust Release 构建成功。安装包为 `artifacts/SourceNeo_0.1.0_x64-sync-setup.exe`，大小 2,263,612 字节，SHA-256 为 `0FE9F9896C8A0447ED55A8B690EB87484F04DF433F1E2E7F8D538B88A8AF711B`，同目录附校验文件。

正常关闭旧应用，备份最新仓库与工具配置、窗口设置、WebView 界面偏好及旧程序，使用 `/S /UPDATE /NS` 更新原安装位置，退出码 0。安装后的程序与本次构建的 NSIS payload 完全一致（相对于原始 release 程序仅转换预期的 NSIS 包类型标记），安装版 SHA-256 为 `DC2AAC927B39BAA4467EB41A7B5D577A548F82F80DBD7B533391A569B5D2964E`。15 个配置 / 偏好文件安装前后哈希一致，16 个仓库保留；启动后主窗口标题为 SourceNeo、响应正常，仓库与工具配置仍与备份逐字节一致。备份目录为 `%TEMP%\sourceneo-sync-update-c70e1ef9cb8d4cf5a0271d8a240ce780`。

未点击 Fetch 或同步，未操作业务仓库，未提交或推送；同步行为待用户在更新后的应用中验收。下节“未打包、重启或更新”指源码实现阶段，本节记录其后的交付。

## 2026-10-21 按 Git 配置同步与 rebase 冲突暂停

在当前目录实现，未操作业务仓库、现有仓库列表或应用配置，未创建 worktree、提交或推送工作区改动。所有 Git 场景只使用临时仓库与本地 bare remote。

同步改为读取 Git 有效配置，按 `branch.<name>.rebase`、`pull.rebase`、`pull.ff` 选择 rebase / merge / 仅快进；支持 `merges` 保留拓扑，interactive 模式提示到终端完成，缺少策略的分叉仍暂停并提示配置。保留未提交改动，不启用 autostash，rebase 禁止修改其他分支，并预检被忽略文件。fetch 前记录 fork point，避免远端改写后将已撤回的 upstream 提交重新推回。

merge / rebase 冲突返回明确的冲突状态、操作名称、文件清单和未推送提示；再次同步保留文件与 index，外部完成操作后才能继续普通推送。更新了仓库提示、诊断、页脚和 README。

专项 9 项 rebase 测试通过，覆盖分叉整合和普通推送、分支配置覆盖 include 配置、merge 与 rebase 冲突及外部完成后的同步、ff-only 与 no-ff、仅落后快进、保留合并拓扑、远端改写、未提交与被忽略文件保护。前端 64 项测试、生产构建、Prettier、Clippy（含桌面功能及所有 targets，`-D warnings`）及 Rustfmt 通过。完整 Rust 回归通过：52 项临时 Git 场景及 4 项单元测试，共 56 项；`git diff --check` 通过。

本轮未打包、重启或更新本机已安装的应用。

## 2026-10-21 审查修复后的 Windows 交付

在下述审查修复通过后，重新执行 `npm run desktop:build -- --bundles nsis`，优化正式构建及 NSIS x64 打包通过。安装包为 `artifacts/SourceNeo_0.1.0_x64-reviewed-setup.exe`（仍沿用 `0.1.0` 版本号），SHA-256：`2BA564094C68DF694D27D11EA32AA6E5F140488CB6EAE17C0478EDC6B9538D26`，相邻 `.sha256` 文件可用于校验。构建产物按现有 `.gitignore` 保留在本机，不纳入源码提交。

正常关闭原安装版后，备份最新仓库列表、窗口设置、WebView 外观存储及旧程序文件，使用 `/S /UPDATE /NS` 更新原安装位置，安装器退出码 0。安装后的程序仅与构建原始程序相差预期的 NSIS 包类型标记，安装版 SHA-256 为 `7C0CF88DAD445BF9C2D7B196713E2EF0EA3D636A2D68D5F635AFC2AEB10E26D9`。安装前后 15 个配置 / 外观存储文件哈希一致，启动后仓库配置也与备份逐字节一致，16 个仓库保留，安装版窗口响应正常。备份为 `%TEMP%\sourceneo-reviewed-update-ac6f9639e14d4e989ef05233b4321ea8`。

提交验证复用本次审查通过的 63 项前端测试、47 项 Rust 测试、clippy、Rustfmt 和 Prettier；正式打包再次执行前端构建。下述“未重新安装”描述指审查当时，本节记录其后的重新安装。未重新执行 macOS 实机及终端完整交互验收。

## 2026-10-21 全部未提交改动审查与修复

范围为 `git diff HEAD` 的所有净变化及未跟踪新增源码，没有暂存层差异；未创建 worktree、提交或推送，未操作用户真实远程、仓库列表和配置。本轮发现并修复：

- 自定义 `refs/remotes/company/*` 跟踪映射未更新，导致 Fetch 后仍显示旧 ahead / behind、同步无法快进。现额外保留经 Git 校验、目标只在 `refs/remotes/` 的安全映射和排除规则，继续忽略目标为本地分支 / 标签的危险映射。临时 bare remote 测试验证自定义映射与危险映射、排除规则并存时，本地分支在 Fetch 阶段不变，随后同步能正确快进。
- 同一路径暂存删除后又重建，状态同时有已暂存删除及未跟踪记录；原来只按路径取第一条，无法预览重建内容。现同时按路径及请求状态匹配，两种预览独立，index 不变。
- 右键 Fetch 另一 linked worktree 后，当前 linked worktree 的历史可能不刷新；现在刷新共享 Git 目录的当前历史。切换仓库 / 搜索 / 筛选时同时作废迟到的远端选择请求，避免旧弹窗重新出现。
- 终端原本被统一重定向到 NUL，现交互终端保留标准流，编辑器等后台启动进程保持静默；Windows 自定义 PowerShell / pwsh 也使用固定脚本、环境变量与 `-LiteralPath` 定位，避免特殊目录回退。立即以错误码退出的工具会显示错误。配置保存中禁用输入，避免新输入被已提交的旧快照覆盖。核对了 [Rust Windows 进程启动源码](https://github.com/rust-lang/rust/blob/1.90.0/library/std/src/sys/process/windows.rs) 的标准句柄传递行为。

上述 Diff、Fetch 自定义映射、linked worktree 历史、迟到菜单请求及保存中编辑问题均先用回归测试复现失败，再验证修复通过。`npm run build`、`npm test`（63 项）、`npm run format:check`、`cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`、Rustfmt 与 `git diff --check` 通过。`cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features -- --test-threads=4` 全部通过：43 项临时 Git 场景及 4 项单元测试。Windows 默认 / 自定义 PowerShell 的特殊路径使用实际隐藏辅助进程验证，立即失败工具错误与 junction 越界保护也通过。

本轮未进行 macOS 实机及终端窗口完整交互验收，没有重启、重新安装或覆盖本机应用；下面安装包和本机安装记录对应本轮修复前的版本。外部程序在启动观察期之后发生的运行错误仍由其自身处理。

## 2026-10-21 Windows 安装包与本机更新

用户完成菜单验收后，执行 `npm run desktop:build -- --bundles nsis`，优化正式构建与 x64 NSIS 打包通过。安装包保留当前 `0.1.0` 版本号，位于 `artifacts/SourceNeo_0.1.0_x64-menus-setup.exe`（约 2.15 MiB），相邻 `.sha256` 文件记录校验值：`80537793FDF525D73D86692BB8347922E2B1740C9D512D17171A17339E6BAA6E`。

正常关闭验收用调试实例，备份仓库配置、窗口配置、WebView 外观存储和原安装程序文件，再以 `/S /UPDATE /NS` 更新现有当前用户安装。安装器退出码 0。安装后的可执行文件与本次正式构建逐字节比较，仅有 Tauri 安装包类型标记从 `UNK` 改为 `NSS` 的预期差异；安装版 SHA-256 为 `CC711A3B550EF25DFFE7391EAFC1BED899F28D46B6475421268B19F49EF487F0`。

安装前后 15 个配置 / 外观存储文件哈希一致；安装版在 `%LOCALAPPDATA%\SourceNeo\sourceneo.exe` 成功启动，窗口响应正常。启动后 `repositories.json` 与安装前备份逐字节一致，16 个仓库全部保留。备份目录为 `%TEMP%\sourceneo-before-windows-update-fc13ba570a864692b1d96ee4e221681f`。没有执行 Git 提交、推送、真实远程 Fetch，未改变应用标识或安装位置。

## 2026-10-21 仓库菜单调整

在原有未提交改动之上继续实现，没有新建 worktree、提交或推送。本轮只改前端与文档。仓库行右键、行内“⋯”和详情区“⋯”共享操作菜单，右键其他仓库不改变当前历史；工具设置与菜单分离，移除仍保留确认弹窗。详情区常驻 Fetch、同步和问题查看，文件与提交仍单击打开。

`npm run build`、`npm test`（60 项）、`npm run format:check` 与 `git diff --check` 通过。新增测试覆盖右键操作对象、对非当前仓库选择 Fetch 远端、Shift+F10、菜单定位、方向键跳过禁用项、Home / End、Esc 焦点恢复、外部点击 / 滚动 / resize 关闭、监听清理、操作中禁用状态、详情区入口与再次点击收起。原有移除失败 / 重试 / 迟到响应及外部工具错误重试用例已改为从菜单进入，继续通过。

另用浏览器与拦截 API 的隔离假数据实际验收 1100×760、760×580：菜单显示、右键其他仓库、键盘导航、焦点恢复、按钮再次点击关闭及无横向溢出通过。验收发现短窗口菜单可能覆盖“⋯”触发按钮，已改为根据按钮上下可用空间定位并限制菜单高度、内部滚动，复测通过。本轮没有通过浏览器调用真实 Git 或启动外部程序，没有重新进行 Windows / macOS 原生 UI 验收；Rust 后端未改动，以下保留上一轮后端验证记录。

## 2026-10-21 新增功能验证（Windows）

本轮在当前检出目录实现未提交 diff、仅 Fetch 和外部工具启动，没有 worktree、提交或推送；开始时工作区干净。测试仅使用临时仓库、本地 bare remote 和独立配置，没有访问用户真实远程。

环境：Windows 10 专业版 10.0.19045；Node.js 24.18.0、Cargo 1.97.1、Git 2.53.0.windows.1。

- `npm run build`：通过。
- `npm test`：55 项通过；新增暂存区 / 工作区切换、失败重试、仓库 / 文件切换及关闭后忽略迟到响应、Fetch 全部失败隔离、命名远端选择、外部工具错误重试。
- `npm run format:check`：通过。补充 `endOfLine: auto`，让 Windows CRLF 检出与现有格式兼容，避免无关文件换行重写。
- `cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features`：Git 场景 41 项通过；另有 3 项单元测试通过（启动适配层、Windows PowerShell 实际进程、junction 越界保护）。覆盖 index / HEAD 与工作区 / index、未跟踪文本 / 空 / 二进制 / 截断、首次提交、重命名、删除、模式变化、冲突、中文和特殊文件名、HEAD / index / 工作区不变、预览不创建应用锁；Fetch 覆盖危险本地分支 refspec、禁止 prune、dirty、无 / 多远端、游离 HEAD、空仓库、共享时间、失败保留旧时间及失败隔离。
- `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` 与 `cargo fmt --manifest-path src-tauri/Cargo.toml --check`：通过。

Windows 默认终端实际进程测试使用中文、空格、单引号、`&`、`;`、`$` 和方括号目录名。测试发现仅传 `current_dir` 时 PowerShell 可能回退到程序目录；现已用固定脚本读取环境变量并执行 `Set-Location -LiteralPath`，复测得到精确仓库目录。辅助测试进程隐藏窗口；用户请求打开的终端创建可见控制台。编辑器通过启动适配层验证可执行文件与独立目录参数，不拼接命令模板。真实 junction 测试验证打开的文件句柄不能越出仓库；未读取目标内容。

使用临时配置、独立应用标识 `dev.sourceneo.acceptance`，执行 Tauri `build --debug --no-bundle` 生成 Windows 原生程序并成功启动隔离窗口。因 D 盘空间不足，构建与 npm 缓存改放 C 盘临时目录；未清理用户数据。原生截图工具第一次报 `FrameArrived timed out`，重新选择窗口后再次报 `window capture timed out`，因此**本轮没有完成原生视觉和点击验收**；隔离测试进程已关闭，原有安装版进程保留。

macOS 本轮仅验证 Terminal 参数规划，没有 macOS 实机执行。自定义终端需要遵循继承工作目录的启动约定，编辑器需要接受独立目录参数；未遍历所有第三方工具。启动成功表示操作系统已接受进程启动，工具后续运行状态由外部应用负责。此轮未验证正式安装包、签名或公证；下方历史 macOS 记录不代表这三项新增功能已在 macOS 原生验收。

日期：2026-10-20（Asia/Shanghai）。所有 Git 写入验证均使用临时仓库和本地 bare remote，没有对用户真实仓库执行 fetch、拉取或推送。

## 已验证环境

- Apple M4 / 24 GB，macOS 26.7.1 arm64。
- Node.js 22.22.0，Rust 1.99.1，Apple Git 2.54.0。
- Tauri 2.12.4（Rust），CLI 2.12.3，React 19，Vite 7，TypeScript 5.9。

## 自动检查

| 检查                                        | 结果                     |
| ------------------------------------------- | ------------------------ |
| TypeScript 类型检查及 Vite 正式构建         | 通过                     |
| Git 临时仓库集成测试                        | 27 / 27 通过             |
| 前端测试（图、偏好及组件交互）               | 20 / 20 通过             |
| Cargo clippy，全 targets，warnings 视为错误 | 通过                     |
| Rustfmt、Prettier                           | 通过                     |
| npm audit（含开发依赖）                     | 0 项已报告漏洞（检查时） |
| macOS arm64 正式 .app 构建                  | 通过                     |

Git 测试覆盖：已同步、ahead 后推送、behind 后快进、分叉停止且不产生 merge / rebase、真实文件冲突、无冲突但 merge 未完成、未提交改动、未跟踪文件计数、重命名计数、被忽略文件防覆盖、无 upstream、失效 upstream、空仓库、游离 HEAD、worktree 与配置重载、失败后其他仓库继续、原生 Git 锁、跨实例应用锁、限制推送目标、不绕过 pre-push hook、凭据脱敏、子孙进程持有输出管道的超时、浅克隆、中文文件与真实 merge 父提交。

提交图测试覆盖：真实父子连线、双父 merge、octopus merge、多个根节点、分页边界，以及 150 节点密集 DAG 中每个祖先轨道的正确路由。

## 原生窗口验收

在打包的 macOS .app 中实际操作并查看了界面，而非仅构建前端：

- 首次启动空状态正常。
- 添加 7 个隔离仓库，包括 linked worktree，列表显示对应状态。
- 查看本地与远程分叉、merge 节点及本地 / 远程分支标签。
- 同步全部：可快进仓库更新，分叉与已有冲突停止，未提交改动保留，日志说明原因。
- 添加故意指向不存在本地 remote 的仓库：同步失败显示错误及日志；复制诊断显示“已复制”；移除后磁盘仓库保留。
- 在 1,200 提交仓库中验证首次 100 条、滚动、加载到 200 条；窗口中只渲染相邻可见节点。
- 验证分栏键盘调整及窗口缩放。发现小窗口历史区域被列表挤压后，已补上按窗口高度限制列表和矮窗口的紧凑布局，原生复核确认历史图恢复可见。随后进一步修正了原生标题栏导致的页面最小高度溢出；解锁后已使用最终正式构建复核 760 × 580 最小窗口，确认提交图、分叉提示和底部状态栏可见，页面无额外纵向滚动条。最小窗口截图为 `artifacts/macos-small-window.png`。
- 仓库配置在进程重启后恢复；验收结束清除默认列表中的临时仓库，磁盘测试夹具保留在系统临时目录。

## 产物与资源观察

正式应用：`src-tauri/target/release/bundle/macos/SourceNeo.app`，当前为未签名、未公证的 arm64 本地构建，没有对外发布。本机可分发归档为 `artifacts/SourceNeo-0.1.0-macos-arm64.tar.gz`，SHA-256 见 `artifacts/SHA256SUMS`；完整窗口截图见 `artifacts/macos-overview.png`。应用目录磁盘占用约 **5.98 MiB**，不包括系统 WebKit、系统 Git 或开发工具链。

内存只做了局部观察：初版构建（外观模式切换功能加入前）打开 7 个测试仓库、无同步任务时，macOS `footprint` 报告主进程 `phys_footprint` 为 **20,546,496 bytes（约 19.6 MiB）**，该进程当时峰值约 21.4 MiB。此数字**不包含 WebKit 的独立 WebContent / Networking / GPU 辅助进程**，也不是应用总内存。没有测得可与 VS Code 直接比较的整体数据，不能由此推导节省比例。

## 剩余平台与发布验证

- Windows 尚未在 Windows 主机执行测试、安装或 UI 验收。已提供独立 Windows / macOS CI job，但本地创建工作流不代表它已经运行成功。
- Windows 后续应检查：NSIS 安装 / 卸载、Git for Windows PATH、WebView2、盘符与中文路径、SSH / 凭据助手、worktree、原生窗口和 hooks 超时回收。
- 当前仅实测 macOS arm64；Intel macOS、旧版 macOS、不同 SSH / HTTPS 认证环境尚未覆盖。
- 真实网络 / 认证失败通过分类代码处理；自动场景使用本地不可用 remote，没有请求用户账号或向真实远程做写入验证。
- Windows 进程树超时回收和两端系统辅助进程的内存归属需要实机观察；跨外部 AI / Git 操作只提供变化检测和 Git 自身锁，不能保证整个同步过程跨工具原子化。
- 开发者签名、公证、Windows 签名和正式发布未执行，需要对应证书与发布授权。

## 字体与紧凑布局调整（2026-10-20）

- 使用系统 UI 字体及明确的中文字体回退，移除强制细化字体渲染；正文和提交主题为 13px，分支、状态及作者 / 日期为 12px，辅助文字提高对比度。
- 顶部工具栏由 73px 压缩到 48px，移除装饰副标题；仓库行由 52px 压缩到 42px，历史行由 40px 压缩到 32px。
- 仓库列表按当前可见仓库数量收起，单仓库不再保留大块空白；图节点、连线和虚拟滚动共用实际历史行高。
- 类型检查、前端正式构建、Prettier 及 4 个图测试通过；macOS 原生正式构建完成并已重启验证。在 1180 × 820 和 760 × 580 两种窗口下检查字体、路径、提交图与状态栏，确认小窗口横向滚动条不会遮住仓库路径。调整前后截图为 `artifacts/macos-typography-before.png`、`artifacts/macos-typography-after.png`，小窗口截图为 `artifacts/macos-compact-small-window.png`。用户已添加的仓库配置保持不变，本轮未执行同步或其他 Git 写操作。

## 单行列表、明暗模式和左右布局（2026-10-20）

- 仓库行进一步压缩到 30px，完整路径改为仓库名和行的原生悬停提示；保留分支、同步状态及 ahead / behind。
- 宽窗口默认左右布局，提供可拖动和可用左右方向键调整的分隔栏；保留上下布局选择。宽度不足 1040px 自动转为上下布局，加宽后恢复所选布局。
- 主题覆盖页面、输入框、弹窗、状态颜色、SVG 图线及原生标题栏；首次使用系统外观，随后保存所选明暗模式。布局同样持久化。
- 新增 5 个外观偏好测试，连同 4 个提交图测试共 9 个通过；类型检查、前端构建、Prettier、macOS 正式构建通过。
- 实际在 macOS 原生窗口检查了 5 个仓库的单行状态、完整路径提示、两种主题和布局切换、重启后偏好恢复、分隔栏调整、760 × 580 窄窗口自动布局、浅色弹窗。图线和 merge / 分叉节点在两种主题下均可辨认。
- 截图：`artifacts/macos-side-light.png`、`artifacts/macos-side-dark.png`、`artifacts/macos-stacked-dark.png`、`artifacts/macos-responsive-layout.png`。
- 验收仅增删测试仓库的列表项，已移除这四个临时项；用户原有仓库仍保留，未执行真实仓库的同步或其他 Git 写操作。

## 窄侧栏优化（2026-10-20）

- 左右布局隐藏表头和行分隔线，移除固定分支 / 状态列及常驻操作区占位。仓库名优先分配宽度，状态采用无底色的图标文字，行高仍为 30px。
- 同步和移除按钮在悬停、键盘聚焦或同步进行中展开。上下布局保留表头与原有表格结构。
- 在 macOS 原生正式构建中将侧栏缩至 340px，确认 `EinnosemiCoreDev` 在正常和按钮展开时均完整显示；Tab 可聚焦并显示同步按钮。浅色 / 深色、分叉、已有冲突、未提交改动、ahead / behind 和 worktree 标记均已实际检查，没有换行或重叠；较长分支及仓库名按可用空间省略，完整信息仍在提示和无障碍标签中。
- 类型检查、前端构建、9 个前端测试、Prettier 和 macOS 正式构建通过。截图为 `artifacts/macos-sidebar-340.png`、`artifacts/macos-sidebar-actions.png` 和 `artifacts/macos-sidebar-states-dark.png`。
- 临时验收列表项已全部移除，保留用户原有仓库；未执行真实仓库同步或 Git 写操作。交付包及 SHA-256 校验已更新。

## 窗口状态与更窄侧栏（2026-10-20）

- 侧栏宽度通过本机偏好存储保存；原生验证方向键调整至 380px、拖动至 340px，重启均恢复。最小宽度进一步降至 260px，宽度读取、拖动边界和键盘边界使用同一常量；最大宽度为 560px。
- 窄于 300px 时隐藏分支文字和添加按钮文字，保留仓库名、同步状态、未提交改动数及 ahead / behind；添加按钮保留无障碍名称，分支仍可在行的悬停提示中查看。260px 原生检查包括正常状态、冲突、2 个未提交改动及双向提交计数，无换行或重叠。
- 接入 `tauri-plugin-window-state` 2.5.0，正常退出时将窗口状态存入应用配置目录 `.window-state.json`，启动时恢复。Rust 最低版本更新为插件要求的 1.90。
- 原生移动并缩放窗口后，保存为 1100 × 740、位置 (370, 65)；连续重启后窗口尺寸和位置保持一致，侧栏恢复为 260px。最终构建已打开。最大化 / 全屏保存使用插件实现，本轮未单独进行原生验收；Windows 原生行为仍待实机验证。
- 11 个前端测试、类型检查、前端正式构建、Prettier、Rustfmt、Clippy（all targets，warnings as errors）与 macOS 正式构建通过。测试覆盖宽度恢复、260 / 560 边界、无效数值和存储不可用回退。
- 截图：`artifacts/macos-window-restored-260.png`、`artifacts/macos-sidebar-260-states.png`。临时测试项已移除，用户原有仓库保留；未进行真实仓库同步或 Git 写操作。交付归档及 SHA-256 校验已更新。

## 整体审查的四项修复（2026-10-20）

- 修复仓库身份漂移：每次操作在获取仓库锁之前比较已保存与重新发现的根目录、common Git directory，变化时返回路径错误，不触碰发现的另一个仓库。新增临时嵌套仓库回归场景，删除子仓库 `.git` 后调用同步、状态、历史和诊断均被拒绝，父仓库本地 HEAD 和 bare remote 保持不变。
- 添加 / 移除失败改为弹窗内的 `role="alert"` 错误，保留输入并允许重试，重新打开时清理旧错误；长错误可在弹窗内滚动，不挤走按钮。目录选择器的错误仍显示在主界面。
- 历史视图在错误后重新创建滚动容器时，于绘制前重置滚动和所选提交，并为新容器重新绑定 ResizeObserver；追加历史页保留现有滚动位置。
- 单仓库同步完成后，若仍选中该仓库，同时把分页上限和历史请求重置为 100；成功 / 失败且 HEAD 未变的路径均有覆盖，下一页请求为 200。
- 新增 6 个 React 组件交互测试、1 个 Git 回归测试。先运行未修复实现：5 个组件用例及新 Git 用例按预期失败，保留追加页滚动位置的用例通过；修复后前端共 17 个、Git 共 23 个全部通过。TypeScript、Vite、Prettier、Rustfmt、Clippy（all targets，warnings as errors）、macOS 正式构建通过。新增测试依赖安装时 npm audit 为 0 项已报告漏洞。
- 最终 macOS .app 实测无效路径的错误在弹窗内可读，更正为隔离仓库后成功添加；1,200 提交夹具的分页按 100 → 200 → 同步后 100 → 200 展示。滚动中暂时移开该夹具的 `.git` 触发读取错误，恢复元数据并重试后，第一条提交及后续行正常可见。
- 临时夹具元数据已复原，测试列表项已移除，仅保留用户原有仓库；未同步真实仓库。最终应用保持打开，窗口及侧栏偏好仍恢复。截图为 `artifacts/macos-dialog-error-fixed.png`、`artifacts/macos-history-retry-fixed.png`；交付包和 SHA-256 已更新。Windows 原生行为仍待实机验证。

## 第二轮审查的三项修复（2026-10-20）

- 历史命令显式关闭签名、notes、装饰和补丁输出，防止用户配置污染 NUL 分隔记录。新增固定 SSH 签名提交样本，验证完整 SHA、父提交、主题、HEAD / 分支 / 标签；测试将验证工具指向不存在路径，确保读取历史不依赖签名工具。样本仅包含公开签名，不包含私钥；提交原文在测试中规范化为 LF，避免 Windows 换行转换改变对象哈希。
- 空路径分支同时清理历史数据、错误及加载状态，并保留请求序号失效机制。新增移除最后一个仓库后的错误清理，以及待处理请求迟到成功 / 迟到失败两个组件测试；断言空状态可见且不会重新出现提交、错误或重试按钮。
- Git 启动前验证工作目录。不存在目录、普通文件路径报告路径错误；访问 / 执行权限不足报告权限错误；只有确认目录有效而找不到 Git 时才提示安装及 PATH。进程创建失败时再次检查目录，处理验证后目录消失的情况。测试包含不存在路径、普通文件、Unix 非 root 目录权限，以及独立子进程内清空 PATH 的真正缺少 Git 场景，不修改其他并行测试的环境。
- 新增 7 个回归用例。旧实现中 6 个用例按预期失败，缺少 Git 的对照用例通过；修复后前端 20 个、Git 27 个全部通过，TypeScript、Vite、Prettier、Rustfmt、Clippy（all targets，warnings as errors）及 macOS 正式构建通过。组件测试等待异步状态稳定后再断言，避免把 React effect 提交前的瞬时状态当作最终结果。
- 最终 .app 实测不存在目录提示“无法访问仓库目录”，普通文件提示“仓库路径不是目录”；在开启 `log.showSignature` 且验证程序不存在的临时仓库中，两条提交、完整 SHA、分支 / 标签和父子连线正常。空列表和迟到请求通过组件测试验证，没有为了原生验收移除真实仓库。
- 验收前后保留用户当前 11 个仓库和侧栏宽度 302.7109px，临时签名仓库列表项已移除；恢复原选中的仓库、300 条历史及提交详情。仅执行真实仓库的本地读取，未执行 fetch、拉取、推送或工作区文件写入。截图为 `artifacts/macos-path-error-fixed.png`、`artifacts/macos-signed-history-fixed.png`。交付包及 SHA-256 已更新，Windows 原生行为仍待实机验证。


## 第三轮审查的三项修复（2026-10-20）

- 移除请求完成后使用当前选择判断是否需要切换仓库，避免提交时的闭包覆盖用户后来选择的仓库。添加、移除对话框均以会话序号隔离异步结果；关闭后重新打开的对话框不会收到旧请求的错误或被旧成功回调关闭。已成功完成的旧操作仍更新仓库列表。
- 同一对话框内的请求通过同步 ref 防止重复提交，并禁用正在提交的按钮；添加路径在提交期间禁用编辑。关闭对话框仍可继续操作，新会话的输入和请求状态不受旧请求完成影响。
- 状态、历史和诊断保留应用内 common Git directory 串行与并发限制，但不创建或打开可写的跨进程锁文件；同步仍必须获取原有 sourceneo.lock，锁文件无法写入时停止，实际权限不足报告 permission 而非 busy。
- 新增 6 个前端用例，覆盖旧添加 / 移除请求迟到成功、迟到失败、新会话及当前选择保留、防重复提交和失败后恢复重试。新增 1 个 Git 用例，分别验证没有锁文件及已有只读锁文件的只读目录可读取状态、历史、诊断，且同步仍被拒绝，HEAD、远程引用和已有锁内容保持不变。
- 先在旧实现上运行：新增 6 个前端用例及 1 个 Git 用例全部按预期失败。修复后前端 26 个、Git 28 个全部通过；TypeScript、Vite、Prettier、Rustfmt、Clippy（all targets，warnings as errors）与 macOS 正式构建通过。只读目录权限测试在 Unix 非 root 用户下执行，Windows ACL 行为尚待实机验证。
- 最终 macOS .app 实测 Git 元数据目录不可写的临时仓库，分支、状态、两条提交及真实父子连线正常显示，未创建 sourceneo.lock。截图：artifacts/macos-readonly-history-fixed.png。对话框迟到结果的时序通过组件测试控制 Promise 验证。
- 临时列表项已移除，原有 11 个仓库路径及顺序核对一致；恢复 EinnosemiCoreDev1、300 条历史、b54ecc295a86 提交详情，侧栏宽度仍为 302.7109px。未同步真实仓库。更新交付归档及 SHA-256 校验，最终应用保持打开。


## 第四轮审查的两项修复（2026-10-20）

- 启动、添加和移除后的仓库列表统一通过 reloadRepos 读取，共享请求序号；只有最新请求可以更新列表、同步缓存和当前选择，过期成功 / 错误响应均被忽略。列表更新时保留仍存在的当前选择，移除当前仓库时再选择首项；添加成功的选择更新也检查最新列表仍包含该仓库。
- Git 历史命令显式使用 --encoding=UTF-8，与输出解析使用的 UTF-8 一致，避免用户 i18n.logOutputEncoding 设置导致中文标题和作者乱码。
- 新增 4 个组件测试：旧添加列表晚于新移除、旧移除列表晚于新添加，以及启动列表迟到成功 / 失败；验证不会恢复已移除项、丢失新增项、覆盖当前选择或显示过期错误。新增 1 个 Git 测试，循环 GB18030 / GBK 配置，验证中文主题、中文作者、完整 SHA、父提交、HEAD / 分支 / 标签和分页信息。
- 5 个新增用例在旧实现上全部按预期失败；修复后前端 30 个、Git 29 个全部通过。TypeScript、Vite、Prettier、Rustfmt、Clippy（all targets，warnings as errors）通过。

- macOS 正式构建通过。最终 .app 添加配置了 GB18030 的隔离仓库后，两条中文提交、中文作者、HEAD / 分支 / 标签与真实父子连线正常，截图：artifacts/macos-history-encoding-fixed.png。乱序响应通过组件测试控制 Promise 验证。
- 验收期间用户新增 EinnosemiCoreLauncher 并调整侧栏，按最新状态保留 12 个仓库及路径顺序、349.6875px 侧栏和启动器仓库的 47 条历史；仅移除本轮临时测试项。未对真实仓库执行同步。交付归档及 SHA-256 校验已更新，最终应用保持打开。Windows 原生行为尚待实机验证。


## 第五轮审查的推送目标修复（2026-10-20）

- branch.<name>.merge 改为通过 NUL 分隔读取全部配置值；发现多个值立即返回 configuration 错误，在 fetch 和分支写操作之前停止。包括重复设置同一个值的情况，不再让 Git upstream 的首值解析与 config --get 的末值解析产生不一致；不自动改写用户配置。
- 新增 2 个集成测试。拒绝测试覆盖 main / other、other / main、main / main 三种配置，验证远程 main / other、本地 HEAD、文件内容及既有 FETCH_HEAD 均保持不变，最后 fetch 时间为空，状态和诊断包含配置错误。恢复测试验证用户改为单一 main 后可成功重试，只推进远程 main，other 保持不变，旧错误清除。
- 两个新增测试在旧实现上均按预期失败。修复后前端 30 个、Git 31 个全部通过；TypeScript、Vite、Prettier、Rustfmt、Clippy（all targets，warnings as errors）通过。

- macOS 正式构建通过，最终 .app 原生复核多值配置被拒绝，明确显示暂停原因；命令行核对两个本地 bare remote 分支、本地 HEAD 及 FETCH_HEAD 均保持不变。仅修改临时夹具为单一 main 后，通过应用重试成功，核对远程 main 等于本地 HEAD、other 仍为原值。
- 原生截图为 artifacts/macos-upstream-ambiguous-blocked.png 和 artifacts/macos-upstream-single-recovered.png。临时列表项已移除，原有 12 个仓库及顺序核对一致，恢复 EinnosemiCoreLauncher 的 47 条历史，侧栏仍为 349.6875px；未对真实仓库执行 fetch、拉取或推送。更新交付归档及 SHA-256 校验，最终应用保持打开。Windows 原生行为仍待实机验证。


## 第六轮审查的路径空白修复（2026-10-20）

- 按用户纠正采用存在性优先：前端原样传递手动输入和目录选择器返回的路径；后端先 try_exists 检查原始路径，仅在确认不存在时尝试 Trim。存在的路径或无法确定存在性的访问错误不会切换到裁剪后的同名仓库；现有普通文件也不会被当作 Trim 后的仓库。
- Git 输出的根目录、common Git directory、git-dir 仅移除命令附加的一个 LF，Windows 另处理 CRLF，保留路径本身的尾随空格。
- 新增 2 个组件用例验证手动输入及目录选择器均保留原始路径；新增 4 个 Git 用例，覆盖不存在的多余空格输入回退、真实尾随空格仓库优先且同步目标正确、尾随空格的独立 Git 元数据目录及原有 Git 锁、现有普通文件不能被替换为另一仓库。独立同名仓库测试还验证配置重载后的行为和相邻仓库本地 HEAD / 文件不变。
- 旧实现上 5 个新用例按预期失败，普通文件的对照用例通过。修复后前端 32 个、Git 35 个全部通过。TypeScript、Vite、Prettier、Rustfmt、Clippy（all targets，warnings as errors）通过。

- macOS 正式构建通过。原生应用在两个仅差尾随空格的独立临时仓库间验证：输入一个空格、该目录存在时显示“真实存在的尾随空格仓库”；输入两个空格、原路径不存在时显示“Trim 后的普通仓库”。检查持久化列表同时保留两个准确路径，两个仓库 HEAD 均未改变。
- 截图为 artifacts/macos-path-existing-space.png 与 artifacts/macos-path-trim-fallback.png。检测到用户同步进行中时等待结束后才重启应用验收；验收只读取临时仓库，未对真实仓库执行同步。两个临时列表项均已移除，原有 12 个仓库及顺序保持一致，恢复用户最新选择 EinnosemiCoreDev1、100 条历史及 349.6875px 侧栏。交付归档及 SHA-256 校验已更新，最终应用保持打开。Windows 原生行为尚待实机验证。

## 第七轮审查的 URL 凭据脱敏修复（2026-10-20）

- URL 用户信息遮盖至 authority 中最后一个 @，处理密码含多个未编码 @ 的无效 remote URL，以及 Git 已部分遮盖后的错误文本；不跨越路径、查询参数或片段边界。
- 新增 2 个回归测试。表驱动用例覆盖 HTTP / HTTPS / SSH、多个 @、编码的 %40、IPv6、普通路径 / 查询 / 片段中的 @，并验证重复脱敏结果稳定。集成用例通过临时仓库的实际同步失败，检查返回错误、后续状态、操作日志及诊断均不包含凭据片段，同时保留目标主机和端口用于排查；本地 HEAD、bare remote 分支和工作区内容保持不变，失败未记录为成功 fetch。
- 两个新增用例在旧实现上均按预期失败，修复后通过。完整前端 32 个、Git 37 个测试全部通过；TypeScript、Vite、Prettier、Rustfmt、Clippy（all targets，warnings as errors）通过。本轮未改动用户的仓库列表、外观或窗口状态，Git 写操作仅发生在自动测试创建的临时仓库中。
- macOS 正式构建通过，交付归档及 SHA-256 校验已更新；核对归档内可执行文件与最终 .app 一致且保留执行权限。本轮验证集中于实际后端链路，未重新执行原生界面操作或 Windows 实机验证。

## 分支切换与提交文件详情（2026-10-20）

- 右侧当前分支改为分支入口，展示本地和已获取的远程分支，支持切换本地分支、从远程引用创建同名跟踪分支，以及从当前 HEAD 新建本地分支。不会自动 fetch；重复名称及其他 worktree 占用由 Git 拒绝，不覆盖已有分支。切换和同步共用进程内串行、并发门限及跨进程文件锁；检查未提交改动、冲突、进行中操作和原有 Git 锁，不自动 stash、不强制切换、不覆盖被忽略文件。
- 提交详情显示 A / M / D / R 文件列表、重命名前后路径及逐文件 diff，保留 SHA 复制和收起操作，合并重复标题栏。根提交比较空树，合并提交比较第一父提交；从真实提交头读取父对象，浅克隆缺少父对象时报告错误，不误当成根提交。二进制文件显示 Git 摘要，文本预览最多 2,000 行并提示截断，Git 单流仍限制 8 MiB。
- 新增读取接口均使用只读访问，不执行 external diff / textconv；完整提交 SHA 校验、实际变更文件白名单及 literal pathspec 防止把路径解释为选项或通配符。请求失效保护覆盖切换提交、切换文件、关闭后重新打开分支窗口；失败在当前视图显示并可重试。
- 新增 9 个组件交互测试、6 个 Git 场景，并扩充只读目录用例。前端 41 个、Git 43 个测试全部通过，覆盖本地 / 远程 / 新建分支、dirty / 原有锁 / 未完成操作 / worktree 占用 / 被忽略文件保护，根提交、第一父提交、重命名、删除、二进制、中文 / 换行 / literal 路径、工作区改动不影响历史 diff、禁用 external diff / textconv、浅克隆边界、迟到响应、重试及预览截断。TypeScript、Vite、Prettier、Rustfmt、Clippy（all targets，warnings as errors）通过。
- macOS 原生临时仓库实测本地分支切换、远程跟踪分支创建（核对 remote 与 merge 配置）、新建分支；有未提交文件时明确拒绝切换，文件内容和当前分支保持不变。确认中文重命名、删除 diff、根提交与二进制摘要正常显示。原生截图包括 artifacts/macos-branch-switch-blocked.png 与 artifacts/macos-commit-files-diff.png。
- 最终 .app 复核浅色 / 深色及 760 × 580 最小窗口；低窗口详情区最小高度降至 110px，提交行、可滚动文件列表、差异和关闭控件仍可使用，分支弹窗的列表滚动与新建 / 取消按钮正常。截图另有 artifacts/macos-commit-files-small.png 与 artifacts/macos-branch-selector-dark.png。恢复原来的浅色、左右布局、1100 × 740 窗口及 349.6875px 侧栏，保留 SourceNeo 选择；仅移除临时列表项，核对原有 13 个仓库和顺序一致，临时 bare remote 的三个分支 SHA 均未变化。
- 最终 macOS 正式构建、交付归档及 SHA-256 校验完成，归档内可执行文件与 .app 一致且保留执行权限，最新应用保持打开。分支写操作仅用于自建临时仓库，未对其他真实仓库执行分支切换或同步。本次改动尚未提交 / 推送，Windows 原生交互仍待实机验证。

## 需关注原因与未提交文件清单（2026-10-20）

- 保持仓库单行布局，将笼统状态改为具体原因和数量，例如“22 项未提交”“未设上游”“rebase 未完成”“认证失败”“分支已分叉”。普通未提交状态不再同时显示难以理解且重复的 M 计数，异常仓库的分支文字适当缩短，优先保留仓库名和原因。
- 顶部“需关注”改为可点击入口，列出对应仓库、分支、具体原因和下一步建议；支持定位仓库并清除隐藏它的搜索条件。详情区可直接打开选中仓库的未提交文件，缺少上游等无文件改动的问题显示处理建议，不无故请求文件清单。
- 新增只读 working_changes 接口，沿用身份核对、应用内串行和并发门限，不创建可写 Git 锁。文件清单显示相对路径、重命名前后路径、已暂存 / 未暂存 / 未跟踪 / 冲突，以及新增 / 修改 / 删除 / 重命名；允许刷新和错误重试，关闭或重试后的迟到响应失效。暂存删除与同路径未跟踪文件同时存在时保留两条不同状态记录。
- 新增 7 个前端用例、1 个 Git 场景，并扩展冲突与只读目录测试。48 项前端、44 项 Git 测试通过，验证原因分类、处理建议、定位、实际文件清单、错误重试、迟到请求，以及 MM、暂存重命名、同路径两种状态、未跟踪目录内各文件、中文与换行路径。测试核对读取前后 index 字节、工作区文件和 HEAD 不变。TypeScript、Vite、Prettier、Rustfmt、Clippy（all targets，warnings as errors）通过。
- macOS 正式构建通过，原生确认两个需关注仓库分别显示各自的未提交数量，清单中的原因、下一步和入口可读；从 SourceNeo 详情直接打开实际文件，能区分已有文件的未暂存修改与新文件的未跟踪状态。660px 弹窗及 760 × 580 最小窗口下的滚动、文件状态和关闭按钮均已复核。截图为 artifacts/macos-attention-specific-status.png、artifacts/macos-attention-overview.png、artifacts/macos-working-files.png 和 artifacts/macos-working-files-small.png。
- 核对原有 13 个仓库及顺序未变，保留 SourceNeo 选择、浅色与左右布局、349.6875px 侧栏和原窗口尺寸，最新应用的未提交文件清单保持打开。本轮原生验收仅读取仓库状态和文件名，未同步、切换分支、提交或丢弃用户改动。交付归档与 SHA-256 校验已更新并验证，归档可执行文件与最终 .app 一致。Windows 原生交互尚未实测，代码尚未提交 / 推送。

## 分支切换后 hook 失败的状态恢复（2026-10-20）

- 修复 Git 已切换分支但 post-checkout hook 返回失败时，界面仍停留在旧分支和历史的问题。失败后重新读取实际状态，成功或失败都刷新当前仓库历史与日志；分支弹窗重新加载列表，禁用实际已选中的分支，同时保留原始错误。如果列表读取也失败，追加读取错误而不覆盖 hook 错误。
- 新增组件回归测试在旧实现上复现仍显示 main 的问题，修复后验证实际分支、历史、弹窗当前标记和原始错误；另覆盖列表刷新失败时保留切换错误。新增 Unix Git 场景使用临时仓库的真实 post-checkout hook，确认返回错误时 HEAD 已切换、文件内容正确、状态与分支列表可读取，远端未改变。
- 全量 50 项前端、45 项 Git 测试通过，TypeScript、Vite、Prettier、Rustfmt、Clippy（all targets，warnings as errors）通过。本轮未操作其他真实仓库，未重启共享应用；Windows 原生 hook 行为尚未实测。
- macOS 正式构建通过，交付归档与 SHA-256 已更新，核对归档二进制与最终 .app 一致且保留执行权限。当前运行实例未重启，重新打开应用后加载本次修复；代码尚未提交或推送。
