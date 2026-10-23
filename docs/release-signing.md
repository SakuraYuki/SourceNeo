# 签名构建

`.github/workflows/signed-build.yml` 是手动触发的独立流程，选择 macOS 或 Windows，完成测试、签名和产物校验后上传构建附件及 SHA-256。它不会创建或发布 GitHub Release，也不会改变默认的未签名构建。

先在仓库的 `signing` Environment 配置对应平台的变量。建议为该 Environment 设置维护者审批与允许构建的分支；工作流本身不会修改仓库权限。

## macOS

配置 Secrets：`APPLE_CERTIFICATE`（Developer ID Application 的 P12，Base64 编码）、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_SIGNING_IDENTITY`、`APPLE_ID`、`APPLE_PASSWORD`（应用专用密码）、`APPLE_TEAM_ID`。

Tauri 导入证书并完成签名、公证，工作流随后执行 codesign、stapler 和 Gatekeeper 验证，验证成功才上传 `.app` 归档。应用专用密码和证书不应提交到仓库。参考 [Tauri macOS 签名文档](https://v2.tauri.app/distribute/sign/macos/)。

## Windows

此流程适用于可导入的 PFX 签名证书。配置 Secrets：`WINDOWS_CERTIFICATE`（PFX，Base64 编码）、`WINDOWS_CERTIFICATE_PASSWORD`；配置 Variable：`WINDOWS_TIMESTAMP_URL`（证书提供商的时间戳服务）。

流程将私钥临时导入当前用户证书存储，生成临时 Tauri 配置，签名 NSIS 安装包并通过 Authenticode 校验。结束时清理证书和临时文件。硬件令牌或云签名服务需要按供应商接入自定义签名命令，不能直接使用 PFX 路径。参考 [Tauri Windows 签名文档](https://v2.tauri.app/distribute/sign/windows/)。

## 验证边界

工作流缺少凭据时会明确失败，不会回退成未签名产物。仓库中没有签名密钥，本次未触发真实签名或公证；配置实际证书后仍需在 CI 执行并在目标系统验收安装包。
