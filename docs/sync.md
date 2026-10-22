# 同步规则与安全边界

## 同步规则

1. 检测冲突和未完成的 merge / rebase / cherry-pick / revert / sequencer / bisect，存在时暂停。
2. 缺少 upstream、游离 HEAD 或空仓库时给出可处理原因；upstream 合并目标有多个配置值时，在 fetch 前停止；不猜测分支关系。
3. 对当前分支对应的 remote 执行 fetch，再读取 Git 状态。
4. 按 Git 的有效配置整合：`branch.<当前分支>.rebase` 优先于 `pull.rebase`，支持普通 rebase、`merges` / `m` 保留合并拓扑及 `false` 使用 merge；普通 rebase 继承 `rebase.rebaseMerges` 设置。读取配置时保留 Git 的全局、仓库、worktree 和 include / includeIf 优先级。`pull.ff=only` 禁止分叉整合，`pull.ff=true/false` 在 merge 时覆盖 `merge.ff`。没有指定策略时仍只允许快进，并提示配置；interactive / i 需要在终端完成，应用不会跳过交互指令。**分叉不等于文件冲突。**
5. 有未提交改动时保留原状，暂停后续整合和推送，不因 autostash 配置而自动暂存。merge 使用 `--no-overwrite-ignore`；rebase 仅检查 HEAD 到目标提交的变化路径，以及实际待重放提交（含中间提交和合并）的变化路径。只对这些路径及阻挡它们的父路径查询忽略规则，使用字面路径、限长分批，并在找到第一项覆盖风险后停止；Windows 及启用 `core.ignoreCase` 的仓库按大小写不敏感方式匹配，防止同名异大小写路径漏检。不全量列举忽略文件，也不扫描无关依赖或缓存目录。文件 / 目录互换、被忽略的父文件或链接仍会暂停同步。
6. 整合目标固定为本次获取的 upstream 提交；rebase 只重写当前分支，不因 `rebase.updateRefs` 修改其他本地分支。发生 merge / rebase 冲突时保留现场，明确显示操作名称和冲突文件，暂停同步且不推送；用户在外部 Git 工具解决后继续或中止操作，再重新同步。正常整合成功后仅普通推送当前分支的已检查提交到其 upstream，不推送其他分支或子模块。

rebase 排除已进入本次 upstream 的提交；旧 fork point 仅在其被远端改写移除时用于界定待重放范围。远端纯回退也会按 rebase 配置整合，而不是将撤回的提交直接推回；这一过程同样受忽略文件保护。明确执行快进时，完成后的 HEAD 必须等于本次获取的目标，hook 或外部操作改变 HEAD 时停止后续推送。

不会自动 stash、提交未提交文件、reset、clean、force push 或丢弃修改；配置为 merge 时可能产生合并提交，配置为 rebase 时会重放本地提交。保留原有 Git 配置、SSH、凭据助手和 hooks；关闭终端交互式凭据输入，认证失败会显示错误。需要交互登录时先在终端完成 Git 认证，再重试。

同一仓库以及共享 common Git directory 的 worktree 在应用内串行，同步和分支切换操作另以独立 `sourceneo.lock` 文件协调 SourceNeo 进程；状态、历史、分支列表、提交详情和诊断读取不创建锁文件，可浏览只读 Git 目录；最多并行处理 3 个仓库。应用不删除 Git 原有锁。写入前后会重新检查 HEAD、分支、upstream、改动和进行中操作，并依赖 Git 自身的原子锁与普通推送校验。

添加仓库时先检查原始路径是否存在，存在则原样使用；仅在确认原始路径不存在时尝试去除输入两端空白。Git 返回的根目录和元数据路径仅去掉命令附加的行结束符，保留真实目录名中的空格。

每次操作会核对仓库根目录和 common Git directory 是否仍与添加时一致；嵌套仓库失去 `.git` 时会停止操作，避免 Git 向上发现并操作父仓库。确认仓库迁移后需重新添加。

外部 AI / Git 不会遵守 SourceNeo 的应用锁，因此无法跨两个独立工具提供覆盖整个同步过程的原子事务。应尽量避免同时操作同一仓库；检测到变化时会暂停并提示刷新。超时或 hook 异常可能需要在终端检查 Git 最终状态。
