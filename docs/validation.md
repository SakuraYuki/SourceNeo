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
