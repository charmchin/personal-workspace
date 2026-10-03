# Third-party notices / 第三方许可声明

个人工作台 0.1.5 · aarch64-apple-darwin

本项目自身采用 MIT；下列组件保留原许可证，不因项目的 MIT 声明而改变。
本清单从锁文件和已安装的依赖生成：包含 78 项 npm 生产依赖、439 项 Rust 解析依赖（含构建期依赖，保守覆盖，不代表所有组件均进入最终二进制）。不包括 npm 开发工具和操作系统自带框架。

- npm 锁文件 SHA-256：`765f3494a92409031ea1ee834c98b9c68fef45dd7b10885c10bfb57d622d9533`
- Cargo 锁文件 SHA-256：`9757e729d55ec65d9e06d35437dfc0f4c567c971ef38a6cb9c6c126236253bbe`
- 完整许可 / 版权原文：[third-party/NOTICES.txt](third-party/NOTICES.txt)
- 机器可读清单：[third-party/manifest.json](third-party/manifest.json)

## 特别说明

- SQLCipher 社区版本原文随 libsqlite3-sys 的 sqlcipher/LICENSE 保留；SQLite 的原始部分为公有领域。OpenSSL 原文随 openssl-src 的 openssl/LICENSE.txt 保留。AWS-LC、ring 等密码组件的复合声明亦保留，不能仅依据封装 crate 的 MIT 元数据判断。
- cssparser、cssparser-macros 和 selectors 使用 MPL-2.0，其源码可通过下表对应的固定版本 crates.io 下载地址获得。当前未修改这些上游源码；以后修改 MPL 覆盖文件时，需按 MPL 提供这些文件的修改后源码，不影响本项目独立文件的 MIT 许可。参见 [Mozilla FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)。
- OR 表示可按表达式选择许可，AND 表示同时满足；历史元数据中的斜杠按上游文件解释，不能自行当作许可名称。清单保留上游表达式和原文，不擅自给第三方重新许可。
- 界面图标依赖 lucide-react，原许可声明已收录；应用图标来自仓库内 SVG。系统字体与 macOS 框架没有复制进本清单。
- 本清单是可核验的工程材料，不是律师意见或完整法律合规认证；对外发行者仍需核对实际产物、修改内容和额外资产。

## 维护

`npm run notices:update` 离线重新生成；`npm run notices:check` 核对锁文件、许可原文和产物是否一致。npm 依赖应先通过 npm ci 安装，Cargo 依赖应先构建 / 缓存当前目标。只有缺少上游原文时才显式使用 `npm run notices:update -- --fetch-missing`，从已固定提交的上游地址取回并缓存，运行应用不会执行此脚本或因此联网。

依赖升级、目标平台或打包方式变化后必须重新生成并复核。安装包在 Contents/Resources/legal/ 提供本项目 LICENSE、本说明和 NOTICES.txt；设置页可打开完整声明。

## 上游发布缺漏与补充依据

- seahash 4.1.0 的发布包未附 LICENSE；补充来自上游紧接发布提交的 [missing-license 修复提交](https://gitlab.redox-os.org/redox-os/seahash/-/commit/3088c5c912b70b586d27bf553fbe964e025a2c89)，固定提交及原文校验值随清单记录，未使用移动分支。
- io_tee 0.1.1 的发布提交没有许可文件，但 README 明确授予 Apache-2.0 或 MIT；已保留固定提交的 README 原文，并按 Apache-2.0 选项附标准条款，没有编造版权人。
- react-remove-scroll-bar 2.3.8 的 npm 包漏附 LICENSE，registry gitHead 仍无法定位；现保留上游固定提交 7301c160fda44cb8cf2b9fdfde61efad35736196 的完整 MIT 原文。该提交只增加 LICENSE，父提交正是 2.3.7 的发布 gitHead。校验两个 npm tarball 的固定 SHA-512 / SHA-256 后，确认两版文件清单相同，除 package.json 外的 26 个文件逐字节一致；元数据仅版本及 react-style-singleton 依赖范围不同。原文、逐文件对应证据与校验值均随清单保留，没有推定版权人或伪造 2.3.8 的源码提交。此项许可材料缺项已按相同发布内容的证据补齐，升级后须重新核对。

## npm 生产依赖

| 组件 | 版本 | 上游许可表达式 | 固定版本源码 | 声明文件数 |
| --- | --- | --- | --- | --- |
| @radix-ui/primitive | 1.1.7 | MIT | [源码](https://registry.npmjs.org/@radix-ui/primitive/-/primitive-1.1.7.tgz) | 1 |
| @radix-ui/react-compose-refs | 1.1.5 | MIT | [源码](https://registry.npmjs.org/@radix-ui/react-compose-refs/-/react-compose-refs-1.1.5.tgz) | 1 |
| @radix-ui/react-context | 1.2.2 | MIT | [源码](https://registry.npmjs.org/@radix-ui/react-context/-/react-context-1.2.2.tgz) | 1 |
| @radix-ui/react-dialog | 1.1.23 | MIT | [源码](https://registry.npmjs.org/@radix-ui/react-dialog/-/react-dialog-1.1.23.tgz) | 1 |
| @radix-ui/react-dismissable-layer | 1.1.19 | MIT | [源码](https://registry.npmjs.org/@radix-ui/react-dismissable-layer/-/react-dismissable-layer-1.1.19.tgz) | 1 |
| @radix-ui/react-focus-guards | 1.1.6 | MIT | [源码](https://registry.npmjs.org/@radix-ui/react-focus-guards/-/react-focus-guards-1.1.6.tgz) | 1 |
| @radix-ui/react-focus-scope | 1.1.16 | MIT | [源码](https://registry.npmjs.org/@radix-ui/react-focus-scope/-/react-focus-scope-1.1.16.tgz) | 1 |
| @radix-ui/react-id | 1.1.4 | MIT | [源码](https://registry.npmjs.org/@radix-ui/react-id/-/react-id-1.1.4.tgz) | 1 |
| @radix-ui/react-portal | 1.1.17 | MIT | [源码](https://registry.npmjs.org/@radix-ui/react-portal/-/react-portal-1.1.17.tgz) | 1 |
| @radix-ui/react-presence | 1.1.10 | MIT | [源码](https://registry.npmjs.org/@radix-ui/react-presence/-/react-presence-1.1.10.tgz) | 1 |
| @radix-ui/react-primitive | 2.1.10 | MIT | [源码](https://registry.npmjs.org/@radix-ui/react-primitive/-/react-primitive-2.1.10.tgz) | 1 |
| @radix-ui/react-slot | 1.3.3 | MIT | [源码](https://registry.npmjs.org/@radix-ui/react-slot/-/react-slot-1.3.3.tgz) | 1 |
| @radix-ui/react-use-callback-ref | 1.1.4 | MIT | [源码](https://registry.npmjs.org/@radix-ui/react-use-callback-ref/-/react-use-callback-ref-1.1.4.tgz) | 1 |
| @radix-ui/react-use-controllable-state | 1.2.6 | MIT | [源码](https://registry.npmjs.org/@radix-ui/react-use-controllable-state/-/react-use-controllable-state-1.2.6.tgz) | 1 |
| @radix-ui/react-use-effect-event | 0.0.5 | MIT | [源码](https://registry.npmjs.org/@radix-ui/react-use-effect-event/-/react-use-effect-event-0.0.5.tgz) | 1 |
| @radix-ui/react-use-layout-effect | 1.1.4 | MIT | [源码](https://registry.npmjs.org/@radix-ui/react-use-layout-effect/-/react-use-layout-effect-1.1.4.tgz) | 1 |
| @reduxjs/toolkit | 2.12.0 | MIT | [源码](https://registry.npmjs.org/@reduxjs/toolkit/-/toolkit-2.12.0.tgz) | 1 |
| @standard-schema/spec | 1.1.0 | MIT | [源码](https://registry.npmjs.org/@standard-schema/spec/-/spec-1.1.0.tgz) | 1 |
| @standard-schema/utils | 0.3.0 | MIT | [源码](https://registry.npmjs.org/@standard-schema/utils/-/utils-0.3.0.tgz) | 1 |
| @tanstack/react-table | 8.21.3 | MIT | [源码](https://registry.npmjs.org/@tanstack/react-table/-/react-table-8.21.3.tgz) | 1 |
| @tanstack/table-core | 8.21.3 | MIT | [源码](https://registry.npmjs.org/@tanstack/table-core/-/table-core-8.21.3.tgz) | 1 |
| @tauri-apps/api | 2.11.1 | Apache-2.0 OR MIT | [源码](https://registry.npmjs.org/@tauri-apps/api/-/api-2.11.1.tgz) | 2 |
| @tauri-apps/plugin-dialog | 2.7.2 | MIT OR Apache-2.0 | [源码](https://registry.npmjs.org/@tauri-apps/plugin-dialog/-/plugin-dialog-2.7.2.tgz) | 1 |
| @types/d3-array | 3.2.2 | MIT | [源码](https://registry.npmjs.org/@types/d3-array/-/d3-array-3.2.2.tgz) | 1 |
| @types/d3-color | 3.1.3 | MIT | [源码](https://registry.npmjs.org/@types/d3-color/-/d3-color-3.1.3.tgz) | 1 |
| @types/d3-ease | 3.0.2 | MIT | [源码](https://registry.npmjs.org/@types/d3-ease/-/d3-ease-3.0.2.tgz) | 1 |
| @types/d3-interpolate | 3.0.4 | MIT | [源码](https://registry.npmjs.org/@types/d3-interpolate/-/d3-interpolate-3.0.4.tgz) | 1 |
| @types/d3-path | 3.1.1 | MIT | [源码](https://registry.npmjs.org/@types/d3-path/-/d3-path-3.1.1.tgz) | 1 |
| @types/d3-scale | 4.0.9 | MIT | [源码](https://registry.npmjs.org/@types/d3-scale/-/d3-scale-4.0.9.tgz) | 1 |
| @types/d3-shape | 3.2.0 | MIT | [源码](https://registry.npmjs.org/@types/d3-shape/-/d3-shape-3.2.0.tgz) | 1 |
| @types/d3-time | 3.0.4 | MIT | [源码](https://registry.npmjs.org/@types/d3-time/-/d3-time-3.0.4.tgz) | 1 |
| @types/d3-timer | 3.0.2 | MIT | [源码](https://registry.npmjs.org/@types/d3-timer/-/d3-timer-3.0.2.tgz) | 1 |
| @types/react-dom | 19.2.5 | MIT | [源码](https://registry.npmjs.org/@types/react-dom/-/react-dom-19.2.5.tgz) | 1 |
| @types/react | 19.2.18 | MIT | [源码](https://registry.npmjs.org/@types/react/-/react-19.2.18.tgz) | 1 |
| @types/use-sync-external-store | 0.0.6 | MIT | [源码](https://registry.npmjs.org/@types/use-sync-external-store/-/use-sync-external-store-0.0.6.tgz) | 1 |
| aria-hidden | 1.2.6 | MIT | [源码](https://registry.npmjs.org/aria-hidden/-/aria-hidden-1.2.6.tgz) | 1 |
| clsx | 2.1.1 | MIT | [源码](https://registry.npmjs.org/clsx/-/clsx-2.1.1.tgz) | 1 |
| cmdk | 1.1.1 | MIT | [源码](https://registry.npmjs.org/cmdk/-/cmdk-1.1.1.tgz) | 1 |
| csstype | 3.2.3 | MIT | [源码](https://registry.npmjs.org/csstype/-/csstype-3.2.3.tgz) | 1 |
| d3-array | 3.2.4 | ISC | [源码](https://registry.npmjs.org/d3-array/-/d3-array-3.2.4.tgz) | 1 |
| d3-color | 3.1.0 | ISC | [源码](https://registry.npmjs.org/d3-color/-/d3-color-3.1.0.tgz) | 1 |
| d3-ease | 3.0.1 | BSD-3-Clause | [源码](https://registry.npmjs.org/d3-ease/-/d3-ease-3.0.1.tgz) | 1 |
| d3-format | 3.1.2 | ISC | [源码](https://registry.npmjs.org/d3-format/-/d3-format-3.1.2.tgz) | 1 |
| d3-interpolate | 3.0.1 | ISC | [源码](https://registry.npmjs.org/d3-interpolate/-/d3-interpolate-3.0.1.tgz) | 1 |
| d3-path | 3.1.0 | ISC | [源码](https://registry.npmjs.org/d3-path/-/d3-path-3.1.0.tgz) | 1 |
| d3-scale | 4.0.2 | ISC | [源码](https://registry.npmjs.org/d3-scale/-/d3-scale-4.0.2.tgz) | 1 |
| d3-shape | 3.2.0 | ISC | [源码](https://registry.npmjs.org/d3-shape/-/d3-shape-3.2.0.tgz) | 1 |
| d3-time-format | 4.1.0 | ISC | [源码](https://registry.npmjs.org/d3-time-format/-/d3-time-format-4.1.0.tgz) | 1 |
| d3-time | 3.1.0 | ISC | [源码](https://registry.npmjs.org/d3-time/-/d3-time-3.1.0.tgz) | 1 |
| d3-timer | 3.0.1 | ISC | [源码](https://registry.npmjs.org/d3-timer/-/d3-timer-3.0.1.tgz) | 1 |
| date-fns | 4.4.0 | MIT | [源码](https://registry.npmjs.org/date-fns/-/date-fns-4.4.0.tgz) | 1 |
| decimal.js-light | 2.5.1 | MIT | [源码](https://registry.npmjs.org/decimal.js-light/-/decimal.js-light-2.5.1.tgz) | 1 |
| detect-node-es | 1.1.0 | MIT | [源码](https://registry.npmjs.org/detect-node-es/-/detect-node-es-1.1.0.tgz) | 1 |
| es-toolkit | 1.52.0 | MIT | [源码](https://registry.npmjs.org/es-toolkit/-/es-toolkit-1.52.0.tgz) | 2 |
| eventemitter3 | 5.0.4 | MIT | [源码](https://registry.npmjs.org/eventemitter3/-/eventemitter3-5.0.4.tgz) | 1 |
| get-nonce | 1.0.1 | MIT | [源码](https://registry.npmjs.org/get-nonce/-/get-nonce-1.0.1.tgz) | 1 |
| immer | 11.1.18 | MIT | [源码](https://registry.npmjs.org/immer/-/immer-11.1.18.tgz) | 1 |
| internmap | 2.0.3 | ISC | [源码](https://registry.npmjs.org/internmap/-/internmap-2.0.3.tgz) | 1 |
| lucide-react | 0.542.0 | ISC | [源码](https://registry.npmjs.org/lucide-react/-/lucide-react-0.542.0.tgz) | 2 |
| react-dom | 19.2.8 | MIT | [源码](https://registry.npmjs.org/react-dom/-/react-dom-19.2.8.tgz) | 1 |
| react-is | 19.2.8 | MIT | [源码](https://registry.npmjs.org/react-is/-/react-is-19.2.8.tgz) | 1 |
| react-redux | 9.3.0 | MIT | [源码](https://registry.npmjs.org/react-redux/-/react-redux-9.3.0.tgz) | 1 |
| react-remove-scroll-bar | 2.3.8 | MIT | [源码](https://registry.npmjs.org/react-remove-scroll-bar/-/react-remove-scroll-bar-2.3.8.tgz) | 3 |
| react-remove-scroll | 2.7.2 | MIT | [源码](https://registry.npmjs.org/react-remove-scroll/-/react-remove-scroll-2.7.2.tgz) | 1 |
| react-style-singleton | 2.2.3 | MIT | [源码](https://registry.npmjs.org/react-style-singleton/-/react-style-singleton-2.2.3.tgz) | 1 |
| react | 19.2.8 | MIT | [源码](https://registry.npmjs.org/react/-/react-19.2.8.tgz) | 1 |
| recharts | 3.10.1 | MIT | [源码](https://registry.npmjs.org/recharts/-/recharts-3.10.1.tgz) | 1 |
| redux-thunk | 3.1.0 | MIT | [源码](https://registry.npmjs.org/redux-thunk/-/redux-thunk-3.1.0.tgz) | 1 |
| redux | 5.0.1 | MIT | [源码](https://registry.npmjs.org/redux/-/redux-5.0.1.tgz) | 1 |
| reselect | 5.2.0 | MIT | [源码](https://registry.npmjs.org/reselect/-/reselect-5.2.0.tgz) | 1 |
| scheduler | 0.27.0 | MIT | [源码](https://registry.npmjs.org/scheduler/-/scheduler-0.27.0.tgz) | 1 |
| tiny-invariant | 1.3.3 | MIT | [源码](https://registry.npmjs.org/tiny-invariant/-/tiny-invariant-1.3.3.tgz) | 1 |
| tslib | 2.8.1 | 0BSD | [源码](https://registry.npmjs.org/tslib/-/tslib-2.8.1.tgz) | 1 |
| use-callback-ref | 1.3.3 | MIT | [源码](https://registry.npmjs.org/use-callback-ref/-/use-callback-ref-1.3.3.tgz) | 1 |
| use-sidecar | 1.1.3 | MIT | [源码](https://registry.npmjs.org/use-sidecar/-/use-sidecar-1.1.3.tgz) | 1 |
| use-sync-external-store | 1.6.0 | MIT | [源码](https://registry.npmjs.org/use-sync-external-store/-/use-sync-external-store-1.6.0.tgz) | 1 |
| victory-vendor | 37.3.6 | MIT AND ISC | [源码](https://registry.npmjs.org/victory-vendor/-/victory-vendor-37.3.6.tgz) | 14 |
| zod | 4.5.2 | MIT | [源码](https://registry.npmjs.org/zod/-/zod-4.5.2.tgz) | 1 |

## Rust 解析依赖

| 组件 | 版本 | 上游许可表达式 | 固定版本源码 | 声明文件数 |
| --- | --- | --- | --- | --- |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/adler2/2.0.1/download) | 3 |
| aead | 0.5.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/aead/0.5.2/download) | 2 |
| aes-gcm | 0.10.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/aes-gcm/0.10.3/download) | 2 |
| aes | 0.8.4 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/aes/0.8.4/download) | 2 |
| age-core | 0.12.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/age-core/0.12.0/download) | 2 |
| age | 0.12.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/age/0.12.1/download) | 2 |
| ahash | 0.7.8 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/ahash/0.7.8/download) | 2 |
| aho-corasick | 1.1.5 | Unlicense OR MIT | [源码](https://crates.io/api/v1/crates/aho-corasick/1.1.5/download) | 3 |
| alloc-no-stdlib | 2.0.4 | BSD-3-Clause | [源码](https://crates.io/api/v1/crates/alloc-no-stdlib/2.0.4/download) | 1 |
| alloc-stdlib | 0.2.4 | BSD-3-Clause | [源码](https://crates.io/api/v1/crates/alloc-stdlib/0.2.4/download) | 1 |
| anyhow | 1.0.104 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/anyhow/1.0.104/download) | 2 |
| arc-swap | 1.9.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/arc-swap/1.9.2/download) | 2 |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/arrayvec/0.7.8/download) | 2 |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/atomic-waker/1.1.2/download) | 3 |
| autocfg | 1.5.1 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/autocfg/1.5.1/download) | 2 |
| aws-lc-rs | 1.18.0 | ISC AND (Apache-2.0 OR ISC) | [源码](https://crates.io/api/v1/crates/aws-lc-rs/1.18.0/download) | 1 |
| aws-lc-sys | 0.44.0 | ISC AND (Apache-2.0 OR ISC) AND Apache-2.0 AND MIT AND BSD-3-Clause AND (Apache-2.0 OR ISC OR MIT) AND (Apache-2.0 OR ISC OR MIT-0) | [源码](https://crates.io/api/v1/crates/aws-lc-sys/0.44.0/download) | 3 |
| base16ct | 0.2.0 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/base16ct/0.2.0/download) | 2 |
| base64 | 0.21.7 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/base64/0.21.7/download) | 2 |
| base64 | 0.22.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/base64/0.22.1/download) | 2 |
| basic-toml | 0.1.10 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/basic-toml/0.1.10/download) | 2 |
| bech32 | 0.11.1 | MIT | [源码](https://crates.io/api/v1/crates/bech32/0.11.1/download) | 1 |
| bit-set | 0.8.0 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/bit-set/0.8.0/download) | 2 |
| bit-vec | 0.8.0 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/bit-vec/0.8.0/download) | 2 |
| bitflags | 1.3.2 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/bitflags/1.3.2/download) | 2 |
| bitflags | 2.13.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/bitflags/2.13.1/download) | 2 |
| bitvec | 1.1.1 | MIT | [源码](https://crates.io/api/v1/crates/bitvec/1.1.1/download) | 1 |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/block-buffer/0.10.4/download) | 2 |
| block-buffer | 0.12.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/block-buffer/0.12.1/download) | 2 |
| block2 | 0.6.2 | MIT | [源码](https://crates.io/api/v1/crates/block2/0.6.2/download) | 1 |
| borsh-derive | 1.8.1 | Apache-2.0 | [源码](https://crates.io/api/v1/crates/borsh-derive/1.8.1/download) | 2 |
| borsh | 1.8.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/borsh/1.8.1/download) | 2 |
| brotli-decompressor | 5.0.3 | BSD-3-Clause/MIT | [源码](https://crates.io/api/v1/crates/brotli-decompressor/5.0.3/download) | 1 |
| brotli | 8.0.4 | BSD-3-Clause AND MIT | [源码](https://crates.io/api/v1/crates/brotli/8.0.4/download) | 2 |
| bs58 | 0.5.1 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/bs58/0.5.1/download) | 2 |
| bytecheck_derive | 0.6.12 | MIT | [源码](https://crates.io/api/v1/crates/bytecheck_derive/0.6.12/download) | 1 |
| bytecheck | 0.6.12 | MIT | [源码](https://crates.io/api/v1/crates/bytecheck/0.6.12/download) | 1 |
| byteorder | 1.5.0 | Unlicense OR MIT | [源码](https://crates.io/api/v1/crates/byteorder/1.5.0/download) | 3 |
| bytes | 1.12.1 | MIT | [源码](https://crates.io/api/v1/crates/bytes/1.12.1/download) | 1 |
| camino | 1.2.5 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/camino/1.2.5/download) | 2 |
| cargo_metadata | 0.19.2 | MIT | [源码](https://crates.io/api/v1/crates/cargo_metadata/0.19.2/download) | 1 |
| cargo_toml | 0.22.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/cargo_toml/0.22.3/download) | 1 |
| cargo-platform | 0.1.9 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/cargo-platform/0.1.9/download) | 2 |
| cc | 1.4.4 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/cc/1.4.4/download) | 2 |
| cfb | 0.7.3 | MIT | [源码](https://crates.io/api/v1/crates/cfb/0.7.3/download) | 1 |
| cfg_aliases | 0.2.2 | MIT | [源码](https://crates.io/api/v1/crates/cfg_aliases/0.2.2/download) | 1 |
| cfg-if | 1.0.4 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/cfg-if/1.0.4/download) | 2 |
| chacha20 | 0.10.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/chacha20/0.10.2/download) | 2 |
| chacha20 | 0.9.1 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/chacha20/0.9.1/download) | 2 |
| chacha20poly1305 | 0.10.1 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/chacha20poly1305/0.10.1/download) | 2 |
| chrono | 0.4.45 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/chrono/0.4.45/download) | 1 |
| cipher | 0.4.4 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/cipher/0.4.4/download) | 2 |
| cmake | 0.1.58 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/cmake/0.1.58/download) | 2 |
| const-oid | 0.10.2 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/const-oid/0.10.2/download) | 2 |
| const-oid | 0.9.6 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/const-oid/0.9.6/download) | 2 |
| cookie-factory | 0.3.3 | MIT | [源码](https://crates.io/api/v1/crates/cookie-factory/0.3.3/download) | 1 |
| cookie | 0.18.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/cookie/0.18.2/download) | 2 |
| core-foundation-sys | 0.8.7 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/core-foundation-sys/0.8.7/download) | 2 |
| core-foundation | 0.10.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/core-foundation/0.10.1/download) | 2 |
| core-graphics-types | 0.2.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/core-graphics-types/0.2.0/download) | 2 |
| core-graphics | 0.25.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/core-graphics/0.25.0/download) | 3 |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/cpufeatures/0.2.17/download) | 2 |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/cpufeatures/0.3.1/download) | 2 |
| crc32fast | 1.5.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/crc32fast/1.5.1/download) | 2 |
| crossbeam-channel | 0.5.16 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/crossbeam-channel/0.5.16/download) | 3 |
| crossbeam-utils | 0.8.22 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/crossbeam-utils/0.8.22/download) | 2 |
| crypto-bigint | 0.5.5 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/crypto-bigint/0.5.5/download) | 2 |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/crypto-common/0.1.7/download) | 2 |
| crypto-common | 0.2.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/crypto-common/0.2.2/download) | 2 |
| cssparser-macros | 0.6.1 | MPL-2.0 | [源码](https://crates.io/api/v1/crates/cssparser-macros/0.6.1/download) | 1 |
| cssparser | 0.36.0 | MPL-2.0 | [源码](https://crates.io/api/v1/crates/cssparser/0.36.0/download) | 1 |
| csv-core | 0.1.13 | Unlicense/MIT | [源码](https://crates.io/api/v1/crates/csv-core/0.1.13/download) | 3 |
| csv | 1.4.0 | Unlicense/MIT | [源码](https://crates.io/api/v1/crates/csv/1.4.0/download) | 3 |
| ctor-proc-macro | 0.0.7 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/ctor-proc-macro/0.0.7/download) | 2 |
| ctor | 0.8.0 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/ctor/0.8.0/download) | 2 |
| ctr | 0.9.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/ctr/0.9.2/download) | 2 |
| curve25519-dalek | 4.1.3 | BSD-3-Clause | [源码](https://crates.io/api/v1/crates/curve25519-dalek/4.1.3/download) | 1 |
| darling_core | 0.23.0 | MIT | [源码](https://crates.io/api/v1/crates/darling_core/0.23.0/download) | 1 |
| darling_macro | 0.23.0 | MIT | [源码](https://crates.io/api/v1/crates/darling_macro/0.23.0/download) | 1 |
| darling | 0.23.0 | MIT | [源码](https://crates.io/api/v1/crates/darling/0.23.0/download) | 1 |
| defmt-macros | 1.1.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/defmt-macros/1.1.1/download) | 2 |
| defmt-parser | 1.0.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/defmt-parser/1.0.0/download) | 2 |
| defmt | 1.1.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/defmt/1.1.1/download) | 2 |
| der | 0.7.10 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/der/0.7.10/download) | 2 |
| deranged | 0.5.8 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/deranged/0.5.8/download) | 2 |
| derive_more-impl | 2.1.1 | MIT | [源码](https://crates.io/api/v1/crates/derive_more-impl/2.1.1/download) | 1 |
| derive_more | 2.1.1 | MIT | [源码](https://crates.io/api/v1/crates/derive_more/2.1.1/download) | 1 |
| digest | 0.10.7 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/digest/0.10.7/download) | 2 |
| digest | 0.11.3 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/digest/0.11.3/download) | 2 |
| dirs-sys | 0.5.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/dirs-sys/0.5.0/download) | 2 |
| dirs | 6.0.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/dirs/6.0.0/download) | 2 |
| dispatch2 | 0.3.1 | Zlib OR Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/dispatch2/0.3.1/download) | 1 |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/displaydoc/0.2.7/download) | 2 |
| dom_query | 0.27.0 | MIT | [源码](https://crates.io/api/v1/crates/dom_query/0.27.0/download) | 1 |
| dpi | 0.1.2 | Apache-2.0 AND MIT | [源码](https://crates.io/api/v1/crates/dpi/0.1.2/download) | 2 |
| dtoa-short | 0.3.5 | MPL-2.0 | [源码](https://crates.io/api/v1/crates/dtoa-short/0.3.5/download) | 1 |
| dtoa | 1.0.11 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/dtoa/1.0.11/download) | 2 |
| dtor-proc-macro | 0.0.6 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/dtor-proc-macro/0.0.6/download) | 2 |
| dtor | 0.3.0 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/dtor/0.3.0/download) | 2 |
| dunce | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/dunce/1.0.5/download) | 1 |
| dyn-clone | 1.0.20 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/dyn-clone/1.0.20/download) | 2 |
| elliptic-curve | 0.13.8 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/elliptic-curve/0.13.8/download) | 2 |
| embed_plist | 1.2.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/embed_plist/1.2.2/download) | 2 |
| embed-resource | 3.0.11 | MIT | [源码](https://crates.io/api/v1/crates/embed-resource/3.0.11/download) | 1 |
| equivalent | 1.0.2 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/equivalent/1.0.2/download) | 2 |
| erased-serde | 0.4.10 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/erased-serde/0.4.10/download) | 2 |
| errno | 0.3.14 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/errno/0.3.14/download) | 2 |
| fallible-iterator | 0.3.0 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/fallible-iterator/0.3.0/download) | 2 |
| fallible-streaming-iterator | 0.1.9 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/fallible-streaming-iterator/0.1.9/download) | 2 |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/fastrand/2.5.0/download) | 2 |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/fdeflate/0.3.7/download) | 2 |
| ff | 0.13.1 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/ff/0.13.1/download) | 2 |
| filetime | 0.2.29 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/filetime/0.2.29/download) | 2 |
| find-crate | 0.6.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/find-crate/0.6.3/download) | 2 |
| find-msvc-tools | 0.1.11 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/find-msvc-tools/0.1.11/download) | 2 |
| flate2 | 1.1.10 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/flate2/1.1.10/download) | 2 |
| fluent-bundle | 0.16.0 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/fluent-bundle/0.16.0/download) | 2 |
| fluent-langneg | 0.13.1 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/fluent-langneg/0.13.1/download) | 2 |
| fluent-syntax | 0.12.0 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/fluent-syntax/0.12.0/download) | 2 |
| fluent | 0.17.0 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/fluent/0.17.0/download) | 2 |
| fnv | 1.0.7 | Apache-2.0 / MIT | [源码](https://crates.io/api/v1/crates/fnv/1.0.7/download) | 2 |
| foldhash | 0.2.0 | Zlib | [源码](https://crates.io/api/v1/crates/foldhash/0.2.0/download) | 1 |
| foreign-types-macros | 0.2.4 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/foreign-types-macros/0.2.4/download) | 2 |
| foreign-types-shared | 0.3.1 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/foreign-types-shared/0.3.1/download) | 2 |
| foreign-types | 0.5.0 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/foreign-types/0.5.0/download) | 2 |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/form_urlencoded/1.2.2/download) | 2 |
| fs_extra | 1.3.0 | MIT | [源码](https://crates.io/api/v1/crates/fs_extra/1.3.0/download) | 1 |
| funty | 2.0.0 | MIT | [源码](https://crates.io/api/v1/crates/funty/2.0.0/download) | 1 |
| futures-channel | 0.3.34 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/futures-channel/0.3.34/download) | 2 |
| futures-core | 0.3.34 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/futures-core/0.3.34/download) | 2 |
| futures-executor | 0.3.34 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/futures-executor/0.3.34/download) | 2 |
| futures-io | 0.3.34 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/futures-io/0.3.34/download) | 2 |
| futures-macro | 0.3.34 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/futures-macro/0.3.34/download) | 2 |
| futures-sink | 0.3.34 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/futures-sink/0.3.34/download) | 2 |
| futures-task | 0.3.34 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/futures-task/0.3.34/download) | 2 |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/futures-util/0.3.34/download) | 2 |
| futures | 0.3.34 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/futures/0.3.34/download) | 2 |
| generic-array | 0.14.7 | MIT | [源码](https://crates.io/api/v1/crates/generic-array/0.14.7/download) | 1 |
| getrandom | 0.2.17 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/getrandom/0.2.17/download) | 2 |
| getrandom | 0.3.4 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/getrandom/0.3.4/download) | 2 |
| getrandom | 0.4.3 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/getrandom/0.4.3/download) | 2 |
| ghash | 0.5.1 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/ghash/0.5.1/download) | 2 |
| glob | 0.3.4 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/glob/0.3.4/download) | 2 |
| group | 0.13.0 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/group/0.13.0/download) | 3 |
| hashbrown | 0.12.3 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/hashbrown/0.12.3/download) | 2 |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/hashbrown/0.17.1/download) | 2 |
| hashlink | 0.12.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/hashlink/0.12.1/download) | 2 |
| heck | 0.5.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/heck/0.5.0/download) | 2 |
| hex | 0.4.3 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/hex/0.4.3/download) | 2 |
| hkdf | 0.12.4 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/hkdf/0.12.4/download) | 2 |
| hmac | 0.12.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/hmac/0.12.1/download) | 2 |
| hpke | 0.12.0 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/hpke/0.12.0/download) | 2 |
| html5ever | 0.38.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/html5ever/0.38.0/download) | 2 |
| http-body-util | 0.1.5 | MIT | [源码](https://crates.io/api/v1/crates/http-body-util/0.1.5/download) | 1 |
| http-body | 1.1.0 | MIT | [源码](https://crates.io/api/v1/crates/http-body/1.1.0/download) | 1 |
| http | 1.5.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/http/1.5.0/download) | 2 |
| httparse | 1.10.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/httparse/1.10.1/download) | 2 |
| hybrid-array | 0.2.3 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/hybrid-array/0.2.3/download) | 2 |
| hybrid-array | 0.4.14 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/hybrid-array/0.4.14/download) | 2 |
| hyper-rustls | 0.27.9 | Apache-2.0 OR ISC OR MIT | [源码](https://crates.io/api/v1/crates/hyper-rustls/0.27.9/download) | 3 |
| hyper-util | 0.1.20 | MIT | [源码](https://crates.io/api/v1/crates/hyper-util/0.1.20/download) | 1 |
| hyper | 1.11.1 | MIT | [源码](https://crates.io/api/v1/crates/hyper/1.11.1/download) | 1 |
| i18n-config | 0.4.8 | MIT | [源码](https://crates.io/api/v1/crates/i18n-config/0.4.8/download) | 1 |
| i18n-embed-fl | 0.10.1 | MIT | [源码](https://crates.io/api/v1/crates/i18n-embed-fl/0.10.1/download) | 1 |
| i18n-embed-impl | 0.8.4 | MIT | [源码](https://crates.io/api/v1/crates/i18n-embed-impl/0.8.4/download) | 1 |
| i18n-embed | 0.16.0 | MIT | [源码](https://crates.io/api/v1/crates/i18n-embed/0.16.0/download) | 1 |
| iana-time-zone | 0.1.65 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/iana-time-zone/0.1.65/download) | 2 |
| ico | 0.5.0 | MIT | [源码](https://crates.io/api/v1/crates/ico/0.5.0/download) | 1 |
| icu_collections | 2.3.0 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/icu_collections/2.3.0/download) | 1 |
| icu_locale_core | 2.3.0 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/icu_locale_core/2.3.0/download) | 1 |
| icu_normalizer_data | 2.3.0 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/icu_normalizer_data/2.3.0/download) | 1 |
| icu_normalizer | 2.3.0 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/icu_normalizer/2.3.0/download) | 1 |
| icu_properties_data | 2.3.0 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/icu_properties_data/2.3.0/download) | 1 |
| icu_properties | 2.3.0 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/icu_properties/2.3.0/download) | 1 |
| icu_provider | 2.3.1 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/icu_provider/2.3.1/download) | 1 |
| ident_case | 1.0.1 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/ident_case/1.0.1/download) | 1 |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/idna_adapter/1.2.2/download) | 2 |
| idna | 1.1.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/idna/1.1.0/download) | 2 |
| indexmap | 1.9.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/indexmap/1.9.3/download) | 2 |
| indexmap | 2.14.1 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/indexmap/2.14.1/download) | 2 |
| infer | 0.19.0 | MIT | [源码](https://crates.io/api/v1/crates/infer/0.19.0/download) | 1 |
| inout | 0.1.4 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/inout/0.1.4/download) | 2 |
| intl_pluralrules | 7.0.2 | Apache-2.0/MIT | [源码](https://crates.io/api/v1/crates/intl_pluralrules/7.0.2/download) | 2 |
| intl-memoizer | 0.5.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/intl-memoizer/0.5.3/download) | 2 |
| io_tee | 0.1.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/io_tee/0.1.1/download) | 2 |
| ipnet | 2.12.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/ipnet/2.12.1/download) | 2 |
| itoa | 1.0.18 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/itoa/1.0.18/download) | 2 |
| jiff-core | 0.1.0 | Unlicense OR MIT | [源码](https://crates.io/api/v1/crates/jiff-core/0.1.0/download) | 3 |
| jiff | 0.2.35 | Unlicense OR MIT | [源码](https://crates.io/api/v1/crates/jiff/0.2.35/download) | 3 |
| jobserver | 0.1.35 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/jobserver/0.1.35/download) | 2 |
| json-patch | 3.0.1 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/json-patch/3.0.1/download) | 2 |
| jsonptr | 0.6.3 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/jsonptr/0.6.3/download) | 2 |
| keccak | 0.1.6 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/keccak/0.1.6/download) | 2 |
| kem | 0.3.0-pre.0 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/kem/0.3.0-pre.0/download) | 2 |
| keyboard-types | 0.7.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/keyboard-types/0.7.0/download) | 2 |
| lazy_static | 1.5.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/lazy_static/1.5.0/download) | 2 |
| libc | 0.2.189 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/libc/0.2.189/download) | 2 |
| libsqlite3-sys | 0.38.2 | MIT | [源码](https://crates.io/api/v1/crates/libsqlite3-sys/0.38.2/download) | 2 |
| litemap | 0.8.3 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/litemap/0.8.3/download) | 1 |
| lock_api | 0.4.14 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/lock_api/0.4.14/download) | 2 |
| log | 0.4.34 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/log/0.4.34/download) | 2 |
| lru-slab | 0.1.2 | MIT OR Apache-2.0 OR Zlib | [源码](https://crates.io/api/v1/crates/lru-slab/0.1.2/download) | 3 |
| markup5ever | 0.38.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/markup5ever/0.38.0/download) | 2 |
| memchr | 2.8.3 | Unlicense OR MIT | [源码](https://crates.io/api/v1/crates/memchr/2.8.3/download) | 3 |
| mime_guess | 2.0.5 | MIT | [源码](https://crates.io/api/v1/crates/mime_guess/2.0.5/download) | 1 |
| mime | 0.3.17 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/mime/0.3.17/download) | 2 |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/miniz_oxide/0.8.9/download) | 4 |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/miniz_oxide/0.9.1/download) | 4 |
| mio | 1.2.2 | MIT | [源码](https://crates.io/api/v1/crates/mio/1.2.2/download) | 1 |
| ml-kem | 0.2.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/ml-kem/0.2.3/download) | 2 |
| muda | 0.19.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/muda/0.19.3/download) | 3 |
| new_debug_unreachable | 1.0.6 | MIT | [源码](https://crates.io/api/v1/crates/new_debug_unreachable/1.0.6/download) | 1 |
| nom | 8.0.0 | MIT | [源码](https://crates.io/api/v1/crates/nom/8.0.0/download) | 1 |
| num-conv | 0.2.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/num-conv/0.2.2/download) | 2 |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/num-traits/0.2.19/download) | 2 |
| objc2-app-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/objc2-app-kit/0.3.2/download) | 1 |
| objc2-core-foundation | 0.3.2 | Zlib OR Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/objc2-core-foundation/0.3.2/download) | 1 |
| objc2-core-graphics | 0.3.2 | Zlib OR Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/objc2-core-graphics/0.3.2/download) | 1 |
| objc2-encode | 4.1.0 | MIT | [源码](https://crates.io/api/v1/crates/objc2-encode/4.1.0/download) | 1 |
| objc2-exception-helper | 0.1.1 | Zlib OR Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/objc2-exception-helper/0.1.1/download) | 1 |
| objc2-foundation | 0.3.2 | MIT | [源码](https://crates.io/api/v1/crates/objc2-foundation/0.3.2/download) | 1 |
| objc2-io-surface | 0.3.2 | Zlib OR Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/objc2-io-surface/0.3.2/download) | 1 |
| objc2-web-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/objc2-web-kit/0.3.2/download) | 1 |
| objc2 | 0.6.4 | MIT | [源码](https://crates.io/api/v1/crates/objc2/0.6.4/download) | 1 |
| once_cell | 1.21.4 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/once_cell/1.21.4/download) | 2 |
| opaque-debug | 0.3.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/opaque-debug/0.3.1/download) | 2 |
| openssl-src | 300.6.1+3.6.3 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/openssl-src/300.6.1+3.6.3/download) | 4 |
| openssl-sys | 0.9.117 | MIT | [源码](https://crates.io/api/v1/crates/openssl-sys/0.9.117/download) | 1 |
| option-ext | 0.2.0 | MPL-2.0 | [源码](https://crates.io/api/v1/crates/option-ext/0.2.0/download) | 1 |
| p256 | 0.13.2 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/p256/0.13.2/download) | 2 |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/parking_lot_core/0.9.12/download) | 2 |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/parking_lot/0.12.5/download) | 2 |
| pbkdf2 | 0.12.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/pbkdf2/0.12.2/download) | 2 |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/percent-encoding/2.3.2/download) | 2 |
| phf_codegen | 0.13.1 | MIT | [源码](https://crates.io/api/v1/crates/phf_codegen/0.13.1/download) | 1 |
| phf_generator | 0.13.1 | MIT | [源码](https://crates.io/api/v1/crates/phf_generator/0.13.1/download) | 1 |
| phf_macros | 0.13.1 | MIT | [源码](https://crates.io/api/v1/crates/phf_macros/0.13.1/download) | 1 |
| phf_shared | 0.13.1 | MIT | [源码](https://crates.io/api/v1/crates/phf_shared/0.13.1/download) | 1 |
| phf | 0.13.1 | MIT | [源码](https://crates.io/api/v1/crates/phf/0.13.1/download) | 1 |
| pin-project-internal | 1.1.13 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/pin-project-internal/1.1.13/download) | 2 |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/pin-project-lite/0.2.17/download) | 2 |
| pin-project | 1.1.13 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/pin-project/1.1.13/download) | 2 |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/pkg-config/0.3.34/download) | 2 |
| plist | 1.10.0 | MIT | [源码](https://crates.io/api/v1/crates/plist/1.10.0/download) | 1 |
| png | 0.17.16 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/png/0.17.16/download) | 2 |
| png | 0.18.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/png/0.18.1/download) | 2 |
| poly1305 | 0.8.0 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/poly1305/0.8.0/download) | 2 |
| polyval | 0.6.2 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/polyval/0.6.2/download) | 2 |
| potential_utf | 0.1.6 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/potential_utf/0.1.6/download) | 1 |
| powerfmt | 0.2.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/powerfmt/0.2.0/download) | 2 |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/ppv-lite86/0.2.21/download) | 2 |
| precomputed-hash | 0.1.1 | MIT | [源码](https://crates.io/api/v1/crates/precomputed-hash/0.1.1/download) | 1 |
| primeorder | 0.13.6 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/primeorder/0.13.6/download) | 2 |
| proc-macro-crate | 3.5.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/proc-macro-crate/3.5.0/download) | 2 |
| proc-macro-error-attr3 | 3.1.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/proc-macro-error-attr3/3.1.0/download) | 2 |
| proc-macro-error3 | 3.1.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/proc-macro-error3/3.1.0/download) | 2 |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/proc-macro2/1.0.107/download) | 2 |
| ptr_meta_derive | 0.1.4 | MIT | [源码](https://crates.io/api/v1/crates/ptr_meta_derive/0.1.4/download) | 1 |
| ptr_meta | 0.1.4 | MIT | [源码](https://crates.io/api/v1/crates/ptr_meta/0.1.4/download) | 1 |
| quick-xml | 0.41.0 | MIT | [源码](https://crates.io/api/v1/crates/quick-xml/0.41.0/download) | 1 |
| quinn-proto | 0.11.17 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/quinn-proto/0.11.17/download) | 2 |
| quinn-udp | 0.5.15 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/quinn-udp/0.5.15/download) | 2 |
| quinn | 0.11.11 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/quinn/0.11.11/download) | 2 |
| quote | 1.0.47 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/quote/1.0.47/download) | 2 |
| radium | 0.7.0 | MIT | [源码](https://crates.io/api/v1/crates/radium/0.7.0/download) | 1 |
| rand_chacha | 0.3.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/rand_chacha/0.3.1/download) | 3 |
| rand_chacha | 0.9.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/rand_chacha/0.9.0/download) | 3 |
| rand_core | 0.10.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/rand_core/0.10.1/download) | 3 |
| rand_core | 0.6.4 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/rand_core/0.6.4/download) | 3 |
| rand_core | 0.9.5 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/rand_core/0.9.5/download) | 3 |
| rand_pcg | 0.10.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/rand_pcg/0.10.2/download) | 3 |
| rand | 0.10.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/rand/0.10.2/download) | 3 |
| rand | 0.8.8 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/rand/0.8.8/download) | 3 |
| rand | 0.9.5 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/rand/0.9.5/download) | 3 |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib | [源码](https://crates.io/api/v1/crates/raw-window-handle/0.6.2/download) | 3 |
| ref-cast-impl | 1.0.27 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/ref-cast-impl/1.0.27/download) | 2 |
| ref-cast | 1.0.27 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/ref-cast/1.0.27/download) | 2 |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/regex-automata/0.4.18/download) | 2 |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/regex-syntax/0.8.11/download) | 3 |
| regex | 1.13.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/regex/1.13.1/download) | 2 |
| rend | 0.4.2 | MIT | [源码](https://crates.io/api/v1/crates/rend/0.4.2/download) | 1 |
| reqwest | 0.13.4 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/reqwest/0.13.4/download) | 2 |
| rfd | 0.16.0 | MIT | [源码](https://crates.io/api/v1/crates/rfd/0.16.0/download) | 1 |
| ring | 0.17.14 | Apache-2.0 AND ISC | [源码](https://crates.io/api/v1/crates/ring/0.17.14/download) | 6 |
| rkyv_derive | 0.7.46 | MIT | [源码](https://crates.io/api/v1/crates/rkyv_derive/0.7.46/download) | 2 |
| rkyv | 0.7.46 | MIT | [源码](https://crates.io/api/v1/crates/rkyv/0.7.46/download) | 1 |
| rusqlite | 0.40.2 | MIT | [源码](https://crates.io/api/v1/crates/rusqlite/0.40.2/download) | 1 |
| rust_decimal | 1.42.1 | MIT | [源码](https://crates.io/api/v1/crates/rust_decimal/1.42.1/download) | 1 |
| rust-embed-impl | 8.12.0 | MIT | [源码](https://crates.io/api/v1/crates/rust-embed-impl/8.12.0/download) | 1 |
| rust-embed-utils | 8.12.0 | MIT | [源码](https://crates.io/api/v1/crates/rust-embed-utils/8.12.0/download) | 1 |
| rust-embed | 8.12.0 | MIT | [源码](https://crates.io/api/v1/crates/rust-embed/8.12.0/download) | 1 |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/rustc_version/0.4.1/download) | 2 |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/rustc-hash/2.1.3/download) | 2 |
| rustix | 1.1.4 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/rustix/1.1.4/download) | 4 |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/rustls-pki-types/1.15.1/download) | 2 |
| rustls-platform-verifier | 0.7.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/rustls-platform-verifier/0.7.0/download) | 2 |
| rustls-webpki | 0.103.15 | ISC | [源码](https://crates.io/api/v1/crates/rustls-webpki/0.103.15/download) | 1 |
| rustls | 0.23.45 | Apache-2.0 OR ISC OR MIT | [源码](https://crates.io/api/v1/crates/rustls/0.23.45/download) | 3 |
| rustversion | 1.0.23 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/rustversion/1.0.23/download) | 2 |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 | [源码](https://crates.io/api/v1/crates/ryu/1.0.23/download) | 2 |
| salsa20 | 0.10.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/salsa20/0.10.2/download) | 2 |
| same-file | 1.0.6 | Unlicense/MIT | [源码](https://crates.io/api/v1/crates/same-file/1.0.6/download) | 3 |
| schemars_derive | 0.8.22 | MIT | [源码](https://crates.io/api/v1/crates/schemars_derive/0.8.22/download) | 1 |
| schemars | 0.8.22 | MIT | [源码](https://crates.io/api/v1/crates/schemars/0.8.22/download) | 1 |
| schemars | 0.9.0 | MIT | [源码](https://crates.io/api/v1/crates/schemars/0.9.0/download) | 1 |
| schemars | 1.2.2 | MIT | [源码](https://crates.io/api/v1/crates/schemars/1.2.2/download) | 1 |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/scopeguard/1.2.0/download) | 2 |
| scrypt | 0.11.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/scrypt/0.11.0/download) | 2 |
| seahash | 4.1.0 | MIT | [源码](https://crates.io/api/v1/crates/seahash/4.1.0/download) | 1 |
| sec1 | 0.7.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/sec1/0.7.3/download) | 2 |
| secrecy | 0.10.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/secrecy/0.10.3/download) | 2 |
| security-framework-sys | 2.17.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/security-framework-sys/2.17.0/download) | 2 |
| security-framework | 3.7.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/security-framework/3.7.0/download) | 2 |
| selectors | 0.36.1 | MPL-2.0 | [源码](https://crates.io/api/v1/crates/selectors/0.36.1/download) | 7 |
| self_cell | 1.3.0 | Apache-2.0 OR GPL-2.0-only | [源码](https://crates.io/api/v1/crates/self_cell/1.3.0/download) | 2 |
| semver | 1.0.28 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/semver/1.0.28/download) | 2 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/serde_core/1.0.229/download) | 2 |
| serde_derive_internals | 0.29.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/serde_derive_internals/0.29.1/download) | 2 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/serde_derive/1.0.229/download) | 2 |
| serde_json | 1.0.151 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/serde_json/1.0.151/download) | 2 |
| serde_repr | 0.1.21 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/serde_repr/0.1.21/download) | 2 |
| serde_spanned | 1.1.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/serde_spanned/1.1.1/download) | 2 |
| serde_with_macros | 3.22.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/serde_with_macros/3.22.0/download) | 2 |
| serde_with | 3.22.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/serde_with/3.22.0/download) | 2 |
| serde-untagged | 0.1.9 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/serde-untagged/0.1.9/download) | 2 |
| serde | 1.0.229 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/serde/1.0.229/download) | 2 |
| serialize-to-javascript-impl | 0.1.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/serialize-to-javascript-impl/0.1.2/download) | 2 |
| serialize-to-javascript | 0.1.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/serialize-to-javascript/0.1.2/download) | 2 |
| servo_arc | 0.4.3 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/servo_arc/0.4.3/download) | 2 |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/sha2/0.10.9/download) | 2 |
| sha2 | 0.11.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/sha2/0.11.0/download) | 2 |
| sha3 | 0.10.9 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/sha3/0.10.9/download) | 2 |
| shlex | 2.0.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/shlex/2.0.1/download) | 2 |
| simd-adler32 | 0.3.10 | MIT | [源码](https://crates.io/api/v1/crates/simd-adler32/0.3.10/download) | 1 |
| simdutf8 | 0.1.5 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/simdutf8/0.1.5/download) | 2 |
| siphasher | 1.0.3 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/siphasher/1.0.3/download) | 1 |
| slab | 0.4.12 | MIT | [源码](https://crates.io/api/v1/crates/slab/0.4.12/download) | 1 |
| smallvec | 1.15.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/smallvec/1.15.2/download) | 2 |
| socket2 | 0.6.5 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/socket2/0.6.5/download) | 2 |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/stable_deref_trait/1.2.1/download) | 2 |
| string_cache_codegen | 0.6.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/string_cache_codegen/0.6.1/download) | 2 |
| string_cache | 0.9.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/string_cache/0.9.0/download) | 2 |
| strsim | 0.11.1 | MIT | [源码](https://crates.io/api/v1/crates/strsim/0.11.1/download) | 1 |
| subtle | 2.6.1 | BSD-3-Clause | [源码](https://crates.io/api/v1/crates/subtle/2.6.1/download) | 1 |
| swift-rs | 1.0.8 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/swift-rs/1.0.8/download) | 2 |
| syn | 1.0.109 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/syn/1.0.109/download) | 2 |
| syn | 2.0.119 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/syn/2.0.119/download) | 2 |
| syn | 3.0.4 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/syn/3.0.4/download) | 2 |
| sync_wrapper | 1.0.2 | Apache-2.0 | [源码](https://crates.io/api/v1/crates/sync_wrapper/1.0.2/download) | 1 |
| synstructure | 0.13.2 | MIT | [源码](https://crates.io/api/v1/crates/synstructure/0.13.2/download) | 1 |
| tao | 0.35.3 | Apache-2.0 | [源码](https://crates.io/api/v1/crates/tao/0.35.3/download) | 2 |
| tap | 1.0.1 | MIT | [源码](https://crates.io/api/v1/crates/tap/1.0.1/download) | 1 |
| tar | 0.4.46 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/tar/0.4.46/download) | 2 |
| tauri-build | 2.6.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/tauri-build/2.6.3/download) | 2 |
| tauri-codegen | 2.6.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/tauri-codegen/2.6.3/download) | 2 |
| tauri-macros | 2.6.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/tauri-macros/2.6.3/download) | 2 |
| tauri-plugin-dialog | 2.7.2 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/tauri-plugin-dialog/2.7.2/download) | 3 |
| tauri-plugin-fs | 2.5.1 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/tauri-plugin-fs/2.5.1/download) | 3 |
| tauri-plugin | 2.6.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/tauri-plugin/2.6.3/download) | 31 |
| tauri-runtime-wry | 2.11.4 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/tauri-runtime-wry/2.11.4/download) | 2 |
| tauri-runtime | 2.11.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/tauri-runtime/2.11.3/download) | 2 |
| tauri-utils | 2.9.3 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/tauri-utils/2.9.3/download) | 2 |
| tauri-winres | 0.3.6 | MIT | [源码](https://crates.io/api/v1/crates/tauri-winres/0.3.6/download) | 1 |
| tauri | 2.11.5 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/tauri/2.11.5/download) | 2 |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/tempfile/3.27.0/download) | 2 |
| tendril | 0.5.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/tendril/0.5.1/download) | 2 |
| thiserror-impl | 1.0.69 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/thiserror-impl/1.0.69/download) | 2 |
| thiserror-impl | 2.0.20 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/thiserror-impl/2.0.20/download) | 2 |
| thiserror | 1.0.69 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/thiserror/1.0.69/download) | 2 |
| thiserror | 2.0.20 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/thiserror/2.0.20/download) | 2 |
| time-core | 0.1.9 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/time-core/0.1.9/download) | 2 |
| time-macros | 0.2.32 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/time-macros/0.2.32/download) | 2 |
| time | 0.3.55 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/time/0.3.55/download) | 2 |
| tinystr | 0.8.4 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/tinystr/0.8.4/download) | 1 |
| tinyvec_macros | 0.1.1 | MIT OR Apache-2.0 OR Zlib | [源码](https://crates.io/api/v1/crates/tinyvec_macros/0.1.1/download) | 3 |
| tinyvec | 1.12.0 | Zlib OR Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/tinyvec/1.12.0/download) | 3 |
| tokio-rustls | 0.26.4 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/tokio-rustls/0.26.4/download) | 2 |
| tokio-util | 0.7.19 | MIT | [源码](https://crates.io/api/v1/crates/tokio-util/0.7.19/download) | 1 |
| tokio | 1.53.1 | MIT | [源码](https://crates.io/api/v1/crates/tokio/1.53.1/download) | 1 |
| toml_datetime | 0.7.5+spec-1.1.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/toml_datetime/0.7.5+spec-1.1.0/download) | 2 |
| toml_datetime | 1.1.1+spec-1.1.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/toml_datetime/1.1.1+spec-1.1.0/download) | 2 |
| toml_edit | 0.25.13+spec-1.1.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/toml_edit/0.25.13+spec-1.1.0/download) | 2 |
| toml_parser | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/toml_parser/1.1.3+spec-1.1.0/download) | 2 |
| toml_writer | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/toml_writer/1.1.2+spec-1.1.0/download) | 2 |
| toml | 0.5.11 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/toml/0.5.11/download) | 2 |
| toml | 0.9.12+spec-1.1.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/toml/0.9.12+spec-1.1.0/download) | 2 |
| toml | 1.1.4+spec-1.1.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/toml/1.1.4+spec-1.1.0/download) | 2 |
| tower-http | 0.6.11 | MIT | [源码](https://crates.io/api/v1/crates/tower-http/0.6.11/download) | 1 |
| tower-layer | 0.3.3 | MIT | [源码](https://crates.io/api/v1/crates/tower-layer/0.3.3/download) | 1 |
| tower-service | 0.3.3 | MIT | [源码](https://crates.io/api/v1/crates/tower-service/0.3.3/download) | 1 |
| tower | 0.5.3 | MIT | [源码](https://crates.io/api/v1/crates/tower/0.5.3/download) | 1 |
| tracing-core | 0.1.36 | MIT | [源码](https://crates.io/api/v1/crates/tracing-core/0.1.36/download) | 2 |
| tracing | 0.1.44 | MIT | [源码](https://crates.io/api/v1/crates/tracing/0.1.44/download) | 1 |
| tray-icon | 0.24.2 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/tray-icon/0.24.2/download) | 3 |
| try-lock | 0.2.5 | MIT | [源码](https://crates.io/api/v1/crates/try-lock/0.2.5/download) | 1 |
| type-map | 0.5.1 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/type-map/0.5.1/download) | 2 |
| typeid | 1.0.3 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/typeid/1.0.3/download) | 2 |
| typenum | 1.20.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/typenum/1.20.1/download) | 3 |
| unic-char-property | 0.9.0 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/unic-char-property/0.9.0/download) | 2 |
| unic-char-range | 0.9.0 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/unic-char-range/0.9.0/download) | 2 |
| unic-common | 0.9.0 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/unic-common/0.9.0/download) | 2 |
| unic-langid-impl | 0.9.6 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/unic-langid-impl/0.9.6/download) | 2 |
| unic-langid | 0.9.6 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/unic-langid/0.9.6/download) | 2 |
| unic-ucd-ident | 0.9.0 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/unic-ucd-ident/0.9.0/download) | 2 |
| unic-ucd-version | 0.9.0 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/unic-ucd-version/0.9.0/download) | 2 |
| unicase | 2.9.0 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/unicase/2.9.0/download) | 2 |
| unicode-ident | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 | [源码](https://crates.io/api/v1/crates/unicode-ident/1.0.24/download) | 3 |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/unicode-segmentation/1.13.3/download) | 3 |
| universal-hash | 0.5.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/universal-hash/0.5.1/download) | 2 |
| untrusted | 0.9.0 | ISC | [源码](https://crates.io/api/v1/crates/untrusted/0.9.0/download) | 1 |
| url | 2.5.8 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/url/2.5.8/download) | 2 |
| urlpattern | 0.3.0 | MIT | [源码](https://crates.io/api/v1/crates/urlpattern/0.3.0/download) | 1 |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/utf8_iter/1.0.4/download) | 3 |
| uuid | 1.26.0 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/uuid/1.26.0/download) | 2 |
| vcpkg | 0.2.15 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/vcpkg/0.2.15/download) | 2 |
| version_check | 0.9.5 | MIT/Apache-2.0 | [源码](https://crates.io/api/v1/crates/version_check/0.9.5/download) | 2 |
| walkdir | 2.5.0 | Unlicense/MIT | [源码](https://crates.io/api/v1/crates/walkdir/2.5.0/download) | 3 |
| want | 0.3.1 | MIT | [源码](https://crates.io/api/v1/crates/want/0.3.1/download) | 1 |
| web_atoms | 0.2.6 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/web_atoms/0.2.6/download) | 2 |
| window-vibrancy | 0.6.0 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/window-vibrancy/0.6.0/download) | 3 |
| winnow | 0.7.15 | MIT | [源码](https://crates.io/api/v1/crates/winnow/0.7.15/download) | 1 |
| winnow | 1.0.4 | MIT | [源码](https://crates.io/api/v1/crates/winnow/1.0.4/download) | 1 |
| writeable | 0.6.4 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/writeable/0.6.4/download) | 1 |
| wry | 0.55.1 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/wry/0.55.1/download) | 3 |
| wyz | 0.5.1 | MIT | [源码](https://crates.io/api/v1/crates/wyz/0.5.1/download) | 1 |
| x25519-dalek | 2.0.1 | BSD-3-Clause | [源码](https://crates.io/api/v1/crates/x25519-dalek/2.0.1/download) | 1 |
| xattr | 1.6.1 | MIT OR Apache-2.0 | [源码](https://crates.io/api/v1/crates/xattr/1.6.1/download) | 2 |
| yoke-derive | 0.8.2 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/yoke-derive/0.8.2/download) | 1 |
| yoke | 0.8.3 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/yoke/0.8.3/download) | 1 |
| zerocopy | 0.8.56 | BSD-2-Clause OR Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/zerocopy/0.8.56/download) | 3 |
| zerofrom-derive | 0.1.7 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/zerofrom-derive/0.1.7/download) | 1 |
| zerofrom | 0.1.8 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/zerofrom/0.1.8/download) | 1 |
| zeroize_derive | 1.5.0 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/zeroize_derive/1.5.0/download) | 2 |
| zeroize | 1.9.0 | Apache-2.0 OR MIT | [源码](https://crates.io/api/v1/crates/zeroize/1.9.0/download) | 2 |
| zerotrie | 0.2.5 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/zerotrie/0.2.5/download) | 1 |
| zerovec-derive | 0.11.6 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/zerovec-derive/0.11.6/download) | 1 |
| zerovec | 0.11.8 | Unicode-3.0 | [源码](https://crates.io/api/v1/crates/zerovec/0.11.8/download) | 1 |
| zlib-rs | 0.6.7 | Zlib | [源码](https://crates.io/api/v1/crates/zlib-rs/0.6.7/download) | 1 |
| zmij | 1.0.23 | MIT | [源码](https://crates.io/api/v1/crates/zmij/1.0.23/download) | 1 |
