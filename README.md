# 个人工作台

一个面向 Apple Silicon macOS 的单用户、本地优先桌面应用。它把今日任务、日程、工作记录、个人成长、自媒体内容、投资组合和周复盘放在同一个行动中枢中。

## 当前功能

- 今日：重点任务、下一项日程、习惯、内容推进、工作记录、投资摘要和快速捕获。
- 日程：任务 / 事件 / 时间块独立建模，日周月视图、重复规则、项目关联、ICS 去重导入与导出。
- 自媒体：灵感到归档的固定工作流，素材路径引用、手工指标快照。
- 投资：A 股、ETF、境内基金和现金账户；精确十进制移动加权成本、交易 CSV、手工价格与可选 Tushare。
- 成长：目标、习惯打卡、学习项目、阅读和课程笔记。
- 工作与复盘：Markdown 工作日志、结构化完成/问题/下一步/耗时/精力记录、周汇总确认。
- 全局：`⌘K` 搜索与命令面板、浅色/深色/跟随系统、金额隐藏、键盘焦点和减少动态效果。
- 安全：本地口令模式最低 6 个字符，支持验证旧口令后安全更换；修改前创建恢复点，成功后立即锁定并要求使用新口令解锁。支持后端闲置检查、窗口隐藏锁定与 macOS 原生锁屏/休眠/会话通知。

## 安全边界

- 前端不能执行 SQL、任意文件读写或网络请求；所有能力都经过 Rust Tauri 命令。
- 业务数据库由 SQLCipher 全库加密，启用外键、WAL、`FULL` 同步、事务迁移和 5 秒忙等待。
- 首选把 256 位随机数据库密钥保存到要求 `userPresence` 的 macOS 数据保护钥匙串。
- 未签名本地构建若缺少 Apple entitlement，会尝试当前用户的登录钥匙串；若本机钥匙串仍不可用，则明确要求用户设置独立工作台口令。该口令通过 age/scrypt 包装随机密钥，裸密钥和口令都不落盘。
- Tushare 默认关闭，Token 只存钥匙串。网络代码固定使用 HTTPS `api.tushare.pro`，且不会发送账户、份额、成本或收益。
- 手动备份为独立密码加密的 `.workbench-backup` 单文件；恢复在替换前验证密码、结构、完整性和逐表记录数，并保留恢复前加密快照。
- 本地快照自动轮换：保留 7 个日快照、4 个周快照，数据库升级前、口令修改前和恢复前的恢复点各保留 5 个；设置页可直接验证并恢复或逐个确认删除。
- 0.1.2 修复恢复中断：同目录原子替换主数据库；重新解锁时识别旧版及新版遗留文件，验证当前库或回滚到恢复前数据库，禁止误建空库或重新生成密钥。无法确认时保留文件并提示排查，不自动使用尚未提交的候选库。
- 0.1.3 增加独立后端会话策略与原生通知监听：锁定先撤销数据库访问，已开始的写入安全收尾后关闭连接；前端清理页面、弹窗和口令输入，旧认证响应不能恢复旧会话。原生通知与实机验收的边界见开发说明，不承诺所有系统版本上零延迟或绝不漏通知。

这是一款个人记录和分析工具，不提供投资建议、券商连接或自动交易。

## 本地运行

环境要求：macOS 11+、Apple Silicon、Node.js 20+、Rust stable。

```bash
npm install
npm run tauri dev
```

首次打开后点击“安全初始化并解锁”。如果当前本地构建无法使用需要签名 entitlement 的钥匙串能力，界面会引导设置独立工作台口令。

## 测试与构建

```bash
npm test
npm run build

cd src-tauri
cargo test
cargo clippy --all-targets -- -D warnings

cd ..
npm run tauri build -- --bundles app
```

构建产物：

```text
src-tauri/target/release/bundle/macos/个人工作台.app
```

正式安装到本机后可运行：

```bash
open "/Applications/个人工作台.app"
```

本机数据目录：

```text
~/Library/Application Support/com.local.personalworkbench/
```

应用不会启动本地 HTTP 服务、后台常驻进程或自动更新器。

## 代码结构

```text
src/                         React 界面、类型、Zod 校验和页面模块
src/styles/                  主题、壳层和各页面样式
src-tauri/src/database.rs    SQLCipher、钥匙保护、迁移、快照
src-tauri/src/repository.rs  领域 CRUD、搜索、聚合、投资计算
src-tauri/src/commands.rs    Tauri 命令、ICS/CSV、Tushare 边界
src-tauri/src/backup.rs      age 加密备份与原子恢复
src-tauri/src/restore.rs     中断识别、候选校验、文件持久化与安全回滚
src-tauri/src/session.rs     后端闲置期限、访问撤销和认证会话代号
src-tauri/src/native_lock.rs macOS 通知生命周期、后端监视线程
```

若要对外分发并始终使用 Touch ID / 系统密码在场认证，需要配置 Apple Developer ID、相应 Keychain entitlement、签名与公证。本仓库当前交付的是仅供本机使用的 arm64 `.app`。

详细的开发流程、数据位置、验收清单和故障排查见：[开发与排查说明](docs/开发与排查说明.md)。

本机安装目录、运行文件清单、缓存位置和彻底卸载步骤见：[安装位置、运行文件与彻底卸载](docs/安装位置、运行文件与彻底卸载.md)。
