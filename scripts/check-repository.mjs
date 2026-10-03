import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const read = (root, file) => readFileSync(resolve(root, file), "utf8");
const json = (root, file) => JSON.parse(read(root, file));

export function checkVersions(root = projectRoot) {
  const pkg = json(root, "package.json");
  const lock = json(root, "package-lock.json");
  const cargo = read(root, "src-tauri/Cargo.toml");
  const packageSection = cargo.match(/^\[package\]\s*\n([\s\S]*?)(?=^\[|(?![\s\S]))/m)?.[1];
  const crate = packageSection?.match(/^name\s*=\s*"([^"]+)"/m)?.[1];
  const escapedCrate = crate?.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const lockedCrate = escapedCrate && read(root, "src-tauri/Cargo.lock").match(new RegExp(`^name = "${escapedCrate}"\\nversion = "([^"]+)"`, "m"))?.[1];
  const versions = {
    npm: pkg.version,
    npmLock: lock.version,
    npmRoot: lock.packages?.[""]?.version,
    tauri: json(root, "src-tauri/tauri.conf.json").version,
    cargo: packageSection?.match(/^version\s*=\s*"([^"]+)"/m)?.[1],
    cargoLock: lockedCrate,
    notices: json(root, "third-party/manifest.json").version,
  };
  if (!/^\d+\.\d+\.\d+(?:-[\w.-]+)?$/.test(pkg.version ?? "") || Object.values(versions).some((version) => version !== pkg.version)) {
    throw new Error(`Version mismatch: ${JSON.stringify(versions)}`);
  }
  if (pkg.license !== "MIT" || lock.packages?.[""]?.license !== "MIT" || !/^license\s*=\s*"MIT"/m.test(packageSection ?? "")) {
    throw new Error("Project license metadata must match MIT");
  }
  return pkg.version;
}

export function checkMarkdownLinks(root, files) {
  const canonicalRoot = realpathSync(root);
  let count = 0;
  for (const file of files.filter((file) => file.endsWith(".md"))) {
    const source = read(root, file).replace(/^```[^\n]*\n[\s\S]*?^```\s*$/gm, "");
    const links = source.matchAll(/!?\[[^\]]*\]\((?:<([^>]+)>|([^\s)]+))(?:\s+["'][^)]*)?\)/g);
    for (const match of links) {
      const link = match[1] ?? match[2];
      if (/^(?:[a-z][a-z\d+.-]*:|#|\/\/)/i.test(link)) continue;
      const path = decodeURIComponent(link.split(/[?#]/)[0]);
      if (!path) continue;
      const target = resolve(root, dirname(file), path);
      if (!existsSync(target)) throw new Error(`Missing local Markdown target: ${file} -> ${link}`);
      const local = relative(canonicalRoot, realpathSync(target));
      if (local === ".." || local.startsWith("../") || isAbsolute(local)) {
        throw new Error(`Markdown target leaves repository: ${file} -> ${link}`);
      }
      count++;
    }
  }
  return count;
}

export function checkSourcePaths(files) {
  const forbidden = /(?:^|\/)(?:node_modules|target|dist|dist-ssr|\.local-docs|\.env(?:\.[^/]*)?)(?:\/|$)|\.(?:sqlite3?|db|age|workbench-backup|dmg|pem|key|p12|pfx)(?:[-.][^/]*)?$|\.app(?:\/|$)/i;
  for (const file of files) {
    if (file.endsWith("/.env.example") || file === ".env.example") continue;
    if (forbidden.test(file)) throw new Error(`Runtime, secret or generated path in source: ${file}`);
  }
  return files.length;
}

export function checkRepository(root = projectRoot) {
  // Include pending files during local development as well as tracked files in CI.
  const files = [...new Set(execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z"], { cwd: root, encoding: "utf8" }).split("\0").filter(Boolean))];
  for (const file of ["README.md", "LICENSE", "CHANGELOG.md", "CONTRIBUTING.md", "SECURITY.md", "CODE_OF_CONDUCT.md", ".github/workflows/ci.yml", ".github/ISSUE_TEMPLATE/bug_report.yml", ".github/ISSUE_TEMPLATE/feature_request.yml", ".github/ISSUE_TEMPLATE/config.yml", ".github/pull_request_template.md"]) {
    if (!files.includes(file)) throw new Error(`Missing repository convention: ${file}`);
  }
  const version = checkVersions(root);
  checkSourcePaths(files);
  const markdownLinks = checkMarkdownLinks(root, files);
  return { version, sourceFiles: files.length, markdownLinks };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    console.log(`Repository checks passed: ${JSON.stringify(checkRepository())}`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
