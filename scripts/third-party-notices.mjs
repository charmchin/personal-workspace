import { createHash } from "node:crypto";
import { execFile, execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { evidenceFile, fetchScrollBarSupplement, scrollbarId, verifiedScrollBarSupplement } from "./scrollbar-license-evidence.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const target = "aarch64-apple-darwin";
const directory = join(root, "third-party");
const cachePath = join(directory, "supplemental.json");
const hash = (text) => createHash("sha256").update(text).digest("hex");
const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));
const noticeName = /^(?:licen[cs]e|copying|copyright|notice|unlicense|authors)(?:[._-].*)?$/i;
const isNotice = (name) => noticeName.test(name) && !/\.(?:rs|js|ts|pm|py|c|h|cpp|rb)$/i.test(name);
const markdown = (value) => String(value ?? "").replaceAll("|", "\\|").replaceAll("\n", " ");
const runFile = promisify(execFile);
const downloads = new Map();

export function noticeFiles(path, depth = 0) {
  const result = [];
  for (const entry of readdirSync(path, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name, "en"))) {
    if (entry.isSymbolicLink() || entry.name.startsWith(".")) continue;
    const file = join(path, entry.name);
    if (entry.isFile() && isNotice(entry.name)) {
      if (statSync(file).size > 2 * 1024 * 1024) throw new Error(`Notice too large: ${entry.name}`);
      const text = readFileSync(file, "utf8");
      if (text.includes("\0")) throw new Error(`Non-text notice: ${entry.name}`);
      result.push({ file: relative(path, file), text });
    } else if (entry.isDirectory() && depth < 8 && !["node_modules", "target", ".git"].includes(entry.name)) {
      for (const child of noticeFiles(file, depth + 1)) result.push({ ...child, file: `${entry.name}/${child.file}` });
    }
  }
  return result;
}

function repositoryUrl(repository) {
  const value = typeof repository === "string" ? repository : repository?.url;
  return value?.replace(/^git\+/, "").replace(/^git:\/\//, "https://").replace(/\.git$/, "").replace(/\/$/, "") ?? "";
}

function cargoPackages() {
  const metadata = JSON.parse(execFileSync("cargo", ["metadata", "--manifest-path", "src-tauri/Cargo.toml", "--format-version", "1", "--locked", "--offline", "--filter-platform", target], { cwd: root, maxBuffer: 50 * 1024 * 1024, stdio: ["ignore", "pipe", "pipe"] }));
  const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const included = new Set();
  function visit(id) {
    if (included.has(id)) return;
    included.add(id);
    for (const dependency of nodes.get(id)?.deps ?? []) {
      if (dependency.dep_kinds.some((kind) => kind.kind !== "dev")) visit(dependency.pkg);
    }
  }
  visit(metadata.resolve.root);
  return metadata.packages.filter((pkg) => included.has(pkg.id) && pkg.id !== metadata.resolve.root).map((pkg) => {
    const path = dirname(pkg.manifest_path);
    const vcsPath = join(path, ".cargo_vcs_info.json");
    const vcs = existsSync(vcsPath) ? readJson(vcsPath) : {};
    return { ecosystem: "cargo", name: pkg.name, version: pkg.version, license: pkg.license, repository: repositoryUrl(pkg.repository), source: `https://crates.io/api/v1/crates/${pkg.name}/${pkg.version}/download`, revision: vcs.git?.sha1, sourcePath: vcs.path_in_vcs ?? "", path, licenseFile: pkg.license_file };
  });
}

function npmPackages() {
  return Object.entries(readJson(join(root, "package-lock.json")).packages).filter(([path, pkg]) => path && !pkg.dev).map(([path, pkg]) => {
    const local = readJson(join(root, path, "package.json"));
    if (local.version !== pkg.version) throw new Error(`Run npm ci: ${path} differs from package-lock.json`);
    return { ecosystem: "npm", name: local.name, version: local.version, license: pkg.license, repository: repositoryUrl(local.repository), source: pkg.resolved, revision: local.gitHead, sourcePath: typeof local.repository === "object" ? local.repository.directory ?? "" : "", path: join(root, path) };
  });
}

async function downloadUncached(url) {
  const parsed = new URL(url);
  if (parsed.protocol !== "https:" || !["raw.githubusercontent.com", "api.github.com", "registry.npmjs.org", "gitlab.redox-os.org", "www.apache.org"].includes(parsed.hostname)) throw new Error(`Unapproved notice host: ${parsed.hostname}`);
  // macOS curl respects the developer's configured proxy; no shell or redirects.
  const { stdout } = await runFile("curl", ["--silent", "--show-error", "--max-time", "15", "--max-filesize", "2097152", "--proto", "=https", "--write-out", "\nNOTICE_HTTP_STATUS:%{http_code}", url], { maxBuffer: 3 * 1024 * 1024 });
  const marker = stdout.lastIndexOf("\nNOTICE_HTTP_STATUS:");
  if (marker < 0) throw new Error(`Missing response status: ${url}`);
  const status = Number(stdout.slice(marker + "\nNOTICE_HTTP_STATUS:".length));
  if (status === 404) return null;
  if (status !== 200) throw new Error(`Notice download HTTP ${status}: ${url}`);
  const text = stdout.slice(0, marker);
  if (Buffer.byteLength(text) > 2 * 1024 * 1024 || !text.trim() || /^\s*<!doctype html/i.test(text)) throw new Error(`Invalid notice: ${url}`);
  return text;
}

function download(url) {
  if (!downloads.has(url)) downloads.set(url, downloadUncached(url));
  return downloads.get(url);
}

async function downloadBinary(url) {
  const parsed = new URL(url);
  if (parsed.protocol !== "https:" || parsed.hostname !== "registry.npmjs.org") throw new Error("Unapproved license archive host");
  const { stdout } = await runFile("curl", ["--silent", "--show-error", "--max-time", "15", "--max-filesize", "2097152", "--proto", "=https", "--write-out", "\nNOTICE_HTTP_STATUS:%{http_code}", url], { encoding: null, maxBuffer: 3 * 1024 * 1024 });
  const marker = stdout.lastIndexOf(Buffer.from("\nNOTICE_HTTP_STATUS:"));
  if (marker < 0 || stdout.subarray(marker + "\nNOTICE_HTTP_STATUS:".length).toString() !== "200") throw new Error("License archive download failed");
  return stdout.subarray(0, marker);
}

async function fetchSupplement(pkg) {
  if (pkg.id === scrollbarId) return fetchScrollBarSupplement(download, downloadBinary);
  if (pkg.id === "cargo/seahash@4.1.0") {
    // Upstream's immediate child commit of the packaged revision explicitly
    // fixes the missing license text. Pin that fix; do not use current HEAD.
    const url = "https://gitlab.redox-os.org/redox-os/seahash/-/raw/3088c5c912b70b586d27bf553fbe964e025a2c89/LICENSE";
    const text = await download(url);
    if (!text?.includes("Permission is hereby granted")) throw new Error("Invalid upstream SeaHash license fix");
    return [{ file: "LICENSE (upstream missing-license fix, immediate child commit)", text, url, sha256: hash(text) }];
  }
  if (pkg.ecosystem === "npm" && !pkg.revision) {
    const data = await download(`https://registry.npmjs.org/${encodeURIComponent(pkg.name)}/${pkg.version}`);
    pkg.revision = JSON.parse(data).gitHead;
  }
  if (pkg.id === "npm/victory-vendor@37.3.6" && !pkg.revision) {
    let object = JSON.parse(await download("https://api.github.com/repos/FormidableLabs/victory/git/ref/tags/v37.3.6")).object;
    if (object.type === "tag") object = JSON.parse(await download(object.url)).object;
    if (object.type !== "commit" || !/^[a-f0-9]{40}$/.test(object.sha)) throw new Error("Unresolved Victory release tag");
    const published = JSON.parse(await download(`https://raw.githubusercontent.com/FormidableLabs/victory/${object.sha}/packages/victory-vendor/package.json`));
    if (published.name !== pkg.name || published.version !== pkg.version) throw new Error("Victory source tag does not match published package");
    pkg.revision = object.sha;
  }
  if (!/^[a-f0-9]{40}$/i.test(pkg.revision ?? "")) throw new Error(`No pinned source revision for ${pkg.name}`);
  const github = /^https:\/\/github\.com\/([^/]+\/[^/]+)$/.exec(pkg.repository);
  const gitlab = /^https:\/\/gitlab\.redox-os\.org\/(.+)$/.exec(pkg.repository);
  if (!github && !gitlab) throw new Error(`No supported source repository for ${pkg.name}: ${pkg.repository}`);
  const base = github ? `https://raw.githubusercontent.com/${github[1]}/${pkg.revision}/` : `${pkg.repository}/-/raw/${pkg.revision}/`;
  const files = [];
  for (const file of ["LICENSE", "LICENSE.txt", "LICENSE.md", "LICENSE-MIT", "LICENSE-APACHE", "COPYING", "COPYRIGHT", "NOTICE"]) {
    const text = await download(base + file);
    if (text) files.push({ file, text, url: base + file, sha256: hash(text) });
  }
  if (!files.length && github) {
    const tree = JSON.parse(await download(`https://api.github.com/repos/${github[1]}/git/trees/${pkg.revision}?recursive=1`));
    if (tree.truncated) throw new Error(`Truncated upstream tree: ${pkg.name}`);
    const paths = tree.tree.filter((entry) => entry.type === "blob" && !entry.path.split("/").includes("..") && (isNotice(entry.path.split("/").at(-1)) || /(?:^|\/)LICENSES\//.test(entry.path))).map((entry) => entry.path);
    for (const file of paths) {
      const text = await download(base + file);
      if (text) files.push({ file, text, url: base + file, sha256: hash(text) });
    }
  }
  if (!files.length && pkg.id === "cargo/io_tee@0.1.1") {
    // This exact release declares Apache-2.0 OR MIT in its README but ships no
    // license file. Preserve that grant and select Apache's standard text;
    // do not invent a copyright holder or use a moving default branch.
    const url = base + "README.md";
    const text = await download(url);
    if (!text?.includes("Apache License, Version 2.0") || !pkg.license.includes("Apache-2.0")) throw new Error("io_tee license grant changed");
    files.push({ file: "README.md (upstream license grant)", text, url, sha256: hash(text) });
    const canonicalUrl = "https://www.apache.org/licenses/LICENSE-2.0.txt";
    const canonical = await download(canonicalUrl);
    if (!canonical?.includes("Version 2.0, January 2004")) throw new Error("Invalid canonical Apache license");
    files.push({ file: "LICENSE-APACHE (selected standard terms)", text: canonical, url: canonicalUrl, sha256: hash(canonical) });
  }
  if (!files.length) throw new Error(`No upstream notice found: ${pkg.name}@${pkg.version}`);
  return files;
}

export async function collect({ fetchMissing = false } = {}) {
  const supplemental = existsSync(cachePath) ? readJson(cachePath) : {};
  const packages = [...npmPackages(), ...cargoPackages()].sort((a, b) => `${a.ecosystem}/${a.name}/${a.version}`.localeCompare(`${b.ecosystem}/${b.name}/${b.version}`, "en"));
  const missing = [];
  for (const pkg of packages) {
    if (!pkg.license || !pkg.source?.startsWith("https://")) throw new Error(`Missing license/source metadata: ${pkg.name}`);
    pkg.id = `${pkg.ecosystem}/${pkg.name}@${pkg.version}`;
    pkg.notices = noticeFiles(pkg.path).map((notice) => ({ ...notice, sha256: hash(notice.text) }));
    if (pkg.licenseFile && !pkg.notices.some((notice) => resolve(pkg.path, notice.file) === resolve(pkg.licenseFile))) {
      const text = readFileSync(pkg.licenseFile, "utf8");
      pkg.notices.push({ file: "declared-license-file", text, sha256: hash(text) });
    }
    if (!pkg.notices.some((notice) => !notice.file.includes("/"))) missing.push(pkg);
  }
  for (let index = 0; index < missing.length; index += 4) {
    await Promise.all(missing.slice(index, index + 4).map(async (pkg) => {
      const legacyScrollBar = pkg.id === scrollbarId && supplemental[pkg.id] && !verifiedScrollBarSupplement(supplemental[pkg.id]);
      if ((!supplemental[pkg.id] || legacyScrollBar) && fetchMissing) {
        console.log(`Fetching pinned notice: ${pkg.id}`);
        supplemental[pkg.id] = await fetchSupplement(pkg);
        mkdirSync(directory, { recursive: true });
        writeFileSync(cachePath, JSON.stringify(supplemental, null, 2) + "\n");
      }
      const cached = supplemental[pkg.id];
      if (!cached) throw new Error(`Missing notice for ${pkg.id}; run npm run notices:update -- --fetch-missing`);
      for (const file of cached) {
        if (hash(file.text) !== file.sha256) throw new Error(`Supplemental checksum mismatch: ${pkg.id}`);
      }
      if (pkg.id === scrollbarId) verifiedScrollBarSupplement(cached, (file) => readFileSync(join(pkg.path, file)));
      pkg.notices.push(...cached.filter((file) => !pkg.notices.some((local) => local.sha256 === file.sha256)));
    }));
  }
  for (const pkg of packages.filter((pkg) => pkg.license === "MPL-2.0")) {
    if (!pkg.notices.some((file) => /Mozilla Public License Version 2\.0/.test(file.text))) {
      const template = packages.find((candidate) => candidate.name === "cssparser" && candidate.license === "MPL-2.0");
      if (!template) throw new Error(`No full MPL-2.0 terms for ${pkg.id}`);
      const text = readFileSync(join(template.path, "LICENSE"), "utf8");
      if (!text.includes("Mozilla Public License Version 2.0")) throw new Error("Invalid MPL standard terms");
      pkg.notices.push({ file: "LICENSE-MPL-2.0 (standard terms)", text, url: template.source, sha256: hash(text) });
      if (pkg.name === "selectors") {
        const header = readFileSync(join(pkg.path, "lib.rs"), "utf8").match(/^\/\*[\s\S]*?\*\//)?.[0];
        if (!header?.includes("Mozilla Public")) throw new Error("Missing original selectors MPL notice");
        pkg.notices.push({ file: "lib.rs (original MPL notice)", text: header + "\n", url: pkg.source, sha256: hash(header + "\n") });
      }
    }
  }
  return { packages, supplemental };
}

export function render(packages) {
  const npm = packages.filter((pkg) => pkg.ecosystem === "npm");
  const cargo = packages.filter((pkg) => pkg.ecosystem === "cargo");
  const scrollbar = packages.find((pkg) => pkg.id === scrollbarId);
  const scrollbarResolved = scrollbar && verifiedScrollBarSupplement(scrollbar.notices);
  const reviewRequired = scrollbar && !scrollbarResolved ? ["npm/react-remove-scroll-bar@2.3.8: published MIT declaration present; verified upstream copyright correspondence still required"] : [];
  const version = readJson(join(root, "package.json")).version;
  const digest = { npm: hash(readFileSync(join(root, "package-lock.json"))), cargo: hash(readFileSync(join(root, "src-tauri/Cargo.lock"))) };
  const header = `# Third-party notices / 第三方许可声明\n\n个人工作台 ${version} · ${target}\n\n本项目自身采用 MIT；下列组件保留原许可证，不因项目的 MIT 声明而改变。\n本清单从锁文件和已安装的依赖生成：包含 ${npm.length} 项 npm 生产依赖、${cargo.length} 项 Rust 解析依赖（含构建期依赖，保守覆盖，不代表所有组件均进入最终二进制）。不包括 npm 开发工具和操作系统自带框架。\n\n- npm 锁文件 SHA-256：\`${digest.npm}\`\n- Cargo 锁文件 SHA-256：\`${digest.cargo}\`\n- 完整许可 / 版权原文：[third-party/NOTICES.txt](third-party/NOTICES.txt)\n- 机器可读清单：[third-party/manifest.json](third-party/manifest.json)\n\n## 特别说明\n\n- SQLCipher 社区版本原文随 libsqlite3-sys 的 sqlcipher/LICENSE 保留；SQLite 的原始部分为公有领域。OpenSSL 原文随 openssl-src 的 openssl/LICENSE.txt 保留。AWS-LC、ring 等密码组件的复合声明亦保留，不能仅依据封装 crate 的 MIT 元数据判断。\n- cssparser、cssparser-macros 和 selectors 使用 MPL-2.0，其源码可通过下表对应的固定版本 crates.io 下载地址获得。当前未修改这些上游源码；以后修改 MPL 覆盖文件时，需按 MPL 提供这些文件的修改后源码，不影响本项目独立文件的 MIT 许可。参见 [Mozilla FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)。\n- OR 表示可按表达式选择许可，AND 表示同时满足；历史元数据中的斜杠按上游文件解释，不能自行当作许可名称。清单保留上游表达式和原文，不擅自给第三方重新许可。\n- 界面图标依赖 lucide-react，原许可声明已收录；应用图标来自仓库内 SVG。系统字体与 macOS 框架没有复制进本清单。\n- 本清单是可核验的工程材料，不是律师意见或完整法律合规认证；对外发行者仍需核对实际产物、修改内容和额外资产。\n\n## 维护\n\n\`npm run notices:update\` 离线重新生成；\`npm run notices:check\` 核对锁文件、许可原文和产物是否一致。npm 依赖应先通过 npm ci 安装，Cargo 依赖应先构建 / 缓存当前目标。只有缺少上游原文时才显式使用 \`npm run notices:update -- --fetch-missing\`，从已固定提交的上游地址取回并缓存，运行应用不会执行此脚本或因此联网。\n\n依赖升级、目标平台或打包方式变化后必须重新生成并复核。安装包在 Contents/Resources/legal/ 提供本项目 LICENSE、本说明和 NOTICES.txt；设置页可打开完整声明。\n`;
  const tables = [npm, cargo].map((group, index) => `\n## ${index === 0 ? "npm 生产依赖" : "Rust 解析依赖"}\n\n| 组件 | 版本 | 上游许可表达式 | 固定版本源码 | 声明文件数 |\n| --- | --- | --- | --- | --- |\n` + group.map((pkg) => `| ${markdown(pkg.name)} | ${markdown(pkg.version)} | ${markdown(pkg.license)} | [源码](${pkg.source}) | ${pkg.notices.length} |`).join("\n") + "\n").join("");
  const exceptions = `\n## 上游发布缺漏与补充依据\n\n- seahash 4.1.0 的发布包未附 LICENSE；补充来自上游紧接发布提交的 [missing-license 修复提交](https://gitlab.redox-os.org/redox-os/seahash/-/commit/3088c5c912b70b586d27bf553fbe964e025a2c89)，固定提交及原文校验值随清单记录，未使用移动分支。\n- io_tee 0.1.1 的发布提交没有许可文件，但 README 明确授予 Apache-2.0 或 MIT；已保留固定提交的 README 原文，并按 Apache-2.0 选项附标准条款，没有编造版权人。\n- react-remove-scroll-bar 2.3.8 的发布包只在 package.json 声明 MIT，未附版权 / LICENSE 文件，registry gitHead 也无法在其声明仓库定位。已照录发布包的作者与许可元数据，补入 SPDX v3.27.0 的 MIT 标准正文，未将作者字段推定为版权声明，未编造年份。此项发布依据不完整，对外发行前建议向上游确认；本清单不把标准正文当作原包不存在的版权原文。\n`;
  const resolvedExceptions = scrollbarResolved ? exceptions.replace(/- react-remove-scroll-bar 2\.3\.8[^\n]+/, "- react-remove-scroll-bar 2.3.8 的 npm 包漏附 LICENSE，registry gitHead 仍无法定位；现保留上游固定提交 7301c160fda44cb8cf2b9fdfde61efad35736196 的完整 MIT 原文。该提交只增加 LICENSE，父提交正是 2.3.7 的发布 gitHead。校验两个 npm tarball 的固定 SHA-512 / SHA-256 后，确认两版文件清单相同，除 package.json 外的 26 个文件逐字节一致；元数据仅版本及 react-style-singleton 依赖范围不同。原文、逐文件对应证据与校验值均随清单保留，没有推定版权人或伪造 2.3.8 的源码提交。此项许可材料缺项已按相同发布内容的证据补齐，升级后须重新核对。") : exceptions;
  const index = header + resolvedExceptions + tables;
  const notes = "Third-party licenses and copyright notices\n个人工作台第三方许可与版权原文\n\n" + (header + resolvedExceptions).replace(/\[([^\]]+)\]\(([^)]+)\)/g, "$1 ($2)") + packages.map((pkg) => `\n${"=".repeat(80)}\n${pkg.id}\nLicense expression: ${pkg.license}\nExact source: ${pkg.source}\n` + pkg.notices.map((file) => `\n--- ${file.file} ---\n${file.url ? `Notice source: ${file.url}\n` : ""}SHA-256: ${file.sha256}\n\n${file.text}\n`).join("")).join("");
  const manifest = { application: "personal-workbench", version, target, lockfiles: digest, artifacts: { notices: hash(notes), index: hash(index), projectLicense: hash(readFileSync(join(root, "LICENSE"))) }, reviewRequired, packages: packages.map((pkg) => ({ id: pkg.id, name: pkg.name, version: pkg.version, ecosystem: pkg.ecosystem, license: pkg.license, source: pkg.source, notices: pkg.notices.map(({ text, ...file }) => file) })) };
  return { "THIRD_PARTY_NOTICES.md": index, "third-party/NOTICES.txt": notes, "third-party/manifest.json": JSON.stringify(manifest, null, 2) + "\n" };
}

export function verifySnapshot(base = root) {
  const manifest = readJson(join(base, "third-party/manifest.json"));
  const packageJson = readJson(join(base, "package.json"));
  const checks = [
    ["package-lock.json", manifest.lockfiles.npm],
    ["src-tauri/Cargo.lock", manifest.lockfiles.cargo],
    ["third-party/NOTICES.txt", manifest.artifacts.notices],
    ["THIRD_PARTY_NOTICES.md", manifest.artifacts.index],
    ["LICENSE", manifest.artifacts.projectLicense],
  ];
  if (manifest.version !== packageJson.version || manifest.target !== target) throw new Error("Notice version / target mismatch; regenerate notices");
  for (const [path, expected] of checks) {
    if (hash(readFileSync(join(base, path))) !== expected) throw new Error(`Notice checksum mismatch: ${path}; run npm run notices:update`);
  }
  if (!manifest.packages.length || manifest.packages.some((pkg) => !pkg.license || !pkg.notices.length)) throw new Error("Incomplete notice manifest");
  const scrollbar = manifest.packages.find((pkg) => pkg.id === scrollbarId);
  if (scrollbar?.notices.some((file) => file.file === evidenceFile)) {
    const cached = readJson(join(base, "third-party/supplemental.json"))[scrollbarId];
    if (!cached || !verifiedScrollBarSupplement(cached)) throw new Error("Missing verified scrollbar license evidence");
    for (const file of cached) if (!scrollbar.notices.some((record) => record.file === file.file && record.url === file.url && record.origin === file.origin && record.sha256 === file.sha256)) throw new Error("Scrollbar evidence differs from the locked notice manifest");
  } else if (scrollbar && !manifest.reviewRequired?.some((item) => item.startsWith(scrollbarId + ":"))) throw new Error("Unresolved scrollbar license cannot be omitted from reviewRequired");
  return manifest.packages.length;
}

async function main() {
  const allowed = new Set(["--check", "--verify", "--fetch-missing"]);
  if (process.argv.slice(2).some((arg) => !allowed.has(arg))) throw new Error("Usage: node scripts/third-party-notices.mjs [--check | --verify | --fetch-missing]");
  const check = process.argv.includes("--check");
  const verify = process.argv.includes("--verify");
  const fetchMissing = process.argv.includes("--fetch-missing");
  if (verify) {
    if (check || fetchMissing) throw new Error("--verify cannot be combined with other options");
    console.log(`Verified locked notice snapshot for ${verifySnapshot()} dependencies (no dependency cache or network required)`);
    return;
  }
  if (check && fetchMissing) throw new Error("--check never downloads or writes; cannot combine with --fetch-missing");
  const { packages, supplemental } = await collect({ fetchMissing });
  const files = { ...render(packages), "third-party/supplemental.json": JSON.stringify(supplemental, null, 2) + "\n" };
  for (const [path, text] of Object.entries(files)) {
    const destination = join(root, path);
    if (check) {
      if (!existsSync(destination) || readFileSync(destination, "utf8") !== text) throw new Error(`Stale or missing ${path}; run npm run notices:update`);
    } else {
      mkdirSync(dirname(destination), { recursive: true });
      writeFileSync(destination, text);
    }
  }
  console.log(`${check ? "Verified" : "Generated"} notices for ${packages.length} dependencies (${target})`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => { console.error(error.message); process.exitCode = 1; });
}
