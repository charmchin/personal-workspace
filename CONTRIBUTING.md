# 贡献指南

欢迎问题复现、文档改进、界面反馈和聚焦的代码贡献。本项目是面向 Apple Silicon macOS 的早期预览，未承诺固定响应或合并时间。

## 先选合适的入口

- 使用 [问题模板](https://github.com/charmchin/personal-workspace/issues/new/choose) 提交可复现的问题或功能建议，先搜索现有记录。
- 潜在漏洞、凭据或个人数据泄漏遵循 [安全政策](SECURITY.md)，不要公开可利用细节。
- 大范围架构、权限、联网、加密或数据结构变更，先说明设计和迁移风险，避免实现后才发现目标不一致。
- 交流遵循 [行为准则](CODE_OF_CONDUCT.md)。

## 环境与分支

工具和启动方法见 [README](README.md#快速开始)。公开仓库贡献者可 Fork 后新建分支；仓库处于私有状态时，只有获授权的协作者能够访问，可在获授权范围内提交分支／PR。不要直接覆盖 `main`，不要强推他人分支。

```bash
git switch -c fix/short-description
npm ci
npm run tauri dev
```

开发版默认使用与安装版相同的数据目录。不要打开应用做破坏性测试，不要与旧安装版同时运行，不要把个人数据当测试夹具。领域测试使用临时库和合成数据，不需要真实 Tushare Token 或签名证书。

## 本地检查

通用检查：

```bash
node --test scripts/check-repository.node-test.mjs
node scripts/check-repository.mjs
npm test
npm run notices:test
npm run notices:verify
npm run build
```

macOS 后端或安全／构建脚本变更还需要：

```bash
npm run app:privacy:test
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --release --all-targets --locked -- --test-threads=2
cargo clippy --manifest-path src-tauri/Cargo.toml --release --all-targets --locked -- -D warnings
npm run notices:check
```

口令测试包含计算成本校准，保留两个测试线程的限制，不降低加密参数。默认忽略的性能专项可按 [README](README.md#测试与质量检查) 显式运行；原生通知冒烟测试不触发实际系统锁屏。不要把模拟 IPC 的结果当作 Touch ID、所有 macOS 版本或正式发行验收。

## 修改要求

- 前端保持中文文案、键盘焦点、900–1600px 布局、浅／深色和减少动态效果；截图只使用合成数据。
- Rust 保持准备语句、事务、明确错误码和命令边界；金融值使用十进制字符串与现有精度检查，不引入浮点计算金额。
- 数据结构变更增加迁移与旧备份恢复测试；说明失败回滚和禁止降级的边界。不得静默删除现有用户记录或更换密钥。
- 联网、文件权限、凭据和日志变更附风险说明；默认不联网，不在日志或错误中暴露私人内容、路径或 Token。
- 依赖与版本变更同步锁文件，按 [第三方许可说明](THIRD_PARTY_NOTICES.md) 更新、核对许可材料。`NOTICES.txt` 是上游原文，不手工格式化或删署名。
- 提交保持聚焦；PR 模板写明实际验证、限制和必要兼容性说明。项目不要求额外 CLA；贡献者需有权提交并同意按 MIT 分发。

## CI 与发布边界

[CI 工作流](.github/workflows/ci.yml) 在公开仓库的 `main` 推送和 PR 上自动检查；私有仓库默认跳过计算作业，不消耗 runner 分钟。维护者检查 Actions 额度／费用后，可手动 Run workflow 并明确勾选允许私有 CI。未运行、跳过和通过是不同状态。

CI 使用只读权限、固定完整提交 SHA 的官方 Actions，不使用 `pull_request_target`，不自动发布、上传安装包或访问真实数据。来自 Fork 的 PR 可能需要维护者批准工作流；无检查结果不能视作已通过。

发布版 `.app` 构建仍使用 `npm run app:build`。签名、公证、真实设备验收及版本 Release 是独立步骤，CI 不代替它们。请勿把运行缓存、数据库、密钥、备份、私人截图或未公证产物加入仓库。

当前仓库已公开，`main` 已启用分支保护，所需检查为 `Frontend and repository` 与 `macOS ARM64 and Rust`；私密漏洞报告已启用（2026-10-04 核验）。后续修改通过工作分支／PR 提交，等待对应提交的所需检查通过后再合并，不直接推送或强推 `main`。文件不能代替这些外部设置；维护者仍须定期确认权限，并按协作人数选择审批要求，避免单维护者无法审批自己的 PR。

`v0.1.5` 已作为[源码预发布](https://github.com/charmchin/personal-workspace/releases/tag/v0.1.5)发布，固定到提交 `33b0ffcdd6af43da594d984fcffeb96f866e7407`；两项云端检查的[通过记录](https://github.com/charmchin/personal-workspace/actions/runs/37177241738)对应该提交。发布说明与升级边界须对应实际标签，不移动已公开的版本标签，也不将后续 `main` 的结果追记为该标签已包含的功能。修复发布使用新版本；只有源码可用时明确标为 pre-release，不上传未经发行验收的安装包。

工作流 runner 标签与私有仓库费用边界参考 [GitHub runner 官方说明](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)；只读权限与完整提交 SHA 锁定遵循 [官方安全建议](https://docs.github.com/en/actions/reference/security/secure-use)。外部设置与费用需独立核对，修改这些文档不会自动改变它们。
