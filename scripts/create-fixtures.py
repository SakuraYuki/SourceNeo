#!/usr/bin/env python3
"""Create disposable local repositories for UI acceptance; never use user repos."""
import json
import os
import pathlib
import subprocess
import tempfile

root = pathlib.Path(tempfile.mkdtemp(prefix="sourceneo-acceptance-")).resolve()
env = os.environ | {
    "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_AUTHOR_NAME": "SourceNeo Test", "GIT_AUTHOR_EMAIL": "test@example.invalid",
    "GIT_COMMITTER_NAME": "SourceNeo Test", "GIT_COMMITTER_EMAIL": "test@example.invalid",
    "GIT_TERMINAL_PROMPT": "0",
}

def git(path, *args, allow_failure=False):
    result = subprocess.run(["git", *args], cwd=path, env=env, text=True, capture_output=True)
    if result.returncode and not allow_failure:
        raise RuntimeError(result.stderr)
    return result.stdout.strip()

def commit(path, file, text):
    (path / file).write_text(text + "\n")
    git(path, "add", "--", file)
    git(path, "-c", "commit.gpgsign=false", "commit", "-m", text)

remote, seed = root / "origin.git", root / "seed"
git(root, "init", "--bare", "--initial-branch=main", str(remote))
git(root, "clone", str(remote), str(seed))
commit(seed, "README.md", "初始化项目工作区")
git(seed, "checkout", "-b", "feature/repository-list")
commit(seed, "repositories.ts", "支持添加多个本地仓库")
commit(seed, "status.ts", "展示当前分支与同步状态")
git(seed, "checkout", "main")
commit(seed, "graph.ts", "实现 Git 提交拓扑布局")
git(seed, "-c", "commit.gpgsign=false", "merge", "--no-ff", "feature/repository-list", "-m", "合并多仓库管理功能")
git(seed, "tag", "v0.1.0")
git(seed, "push", "--all", "origin")
git(seed, "push", "--tags", "origin")
paths = []
for name in ["source-neo", "design-system", "api-service", "desktop-shell", "docs-site", "conflict-demo"]:
    path = root / name
    git(root, "clone", str(remote), str(path))
    paths.append(path)
commit(paths[1], "colors.ts", "调整主题与状态颜色")
commit(paths[2], "server.ts", "本地接口更新")
commit(seed, "remote.ts", "远程仓库新增提交")
git(seed, "push", "origin", "main")
git(paths[0], "pull", "--ff-only")
for path in paths[1:]:
    git(path, "fetch")
(paths[4] / "draft.md").write_text("未提交的文档草稿\n")
commit(paths[5], "README.md", "本地文档标题")
commit(seed, "README.md", "远程文档标题")
git(seed, "push", "origin", "main")
git(paths[5], "fetch")
git(paths[5], "merge", "origin/main", allow_failure=True)
worktree = root / "source-neo-worktree"
git(paths[0], "worktree", "add", "-b", "experiment/worktree", str(worktree))
paths.append(worktree)
config = root / "config"
config.mkdir()
repos = []
for path in paths:
    common = pathlib.Path(git(path, "rev-parse", "--git-common-dir"))
    common = (path / common).resolve() if not common.is_absolute() else common.resolve()
    repos.append({"path": str(path), "name": path.name, "commonDir": str(common)})
(config / "repositories.json").write_text(json.dumps({"repos": repos, "fetched": {}}, ensure_ascii=False, indent=2))
print(json.dumps({"root": str(root), "config": str(config), "repos": repos}, ensure_ascii=False))
