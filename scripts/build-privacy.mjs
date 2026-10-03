import { existsSync, realpathSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { basename, delimiter, dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const projectRoot = fileURLToPath(new URL("../", import.meta.url));
export const privacyTarget = join(projectRoot, "src-tauri", "target", "privacy-build");
export const buildTriple = "aarch64-apple-darwin";
export const privacyBundleDirectory = join(privacyTarget, buildTriple, "release", "bundle", "macos");
export const opensslPerlWrapper = join(projectRoot, "scripts", "openssl-privacy-perl.mjs");

export function buildArguments(input = []) {
  const args = [...input];
  const separator = args.indexOf("--");
  const options = separator < 0 ? args : args.slice(0, separator);
  if (options.some((arg) => arg === "--target" || arg === "-t" || arg.startsWith("--target="))) throw new Error("This entry verifies arm64 builds only; do not override the target.");
  if (options.includes("--debug") || options.includes("-d") || options.includes("--no-bundle")) throw new Error("Use this entry for bundled release builds.");
  for (let index = 0; index < options.length; index++) {
    if ((options[index] === "--bundles" || options[index] === "-b") && options[index + 1] !== "app") throw new Error("Only .app bundles are covered by this privacy entry.");
    if (options[index].startsWith("--bundles=") && options[index] !== "--bundles=app") throw new Error("Only .app bundles are covered by this privacy entry.");
  }
  if (!options.some((arg) => arg === "--bundles" || arg === "-b" || arg.startsWith("--bundles="))) args.unshift("--bundles", "app");
  // CLI options must precede '--', which forwards subsequent flags to Cargo.
  return ["--target", buildTriple, ...args];
}

export function pathMappings({ root = projectRoot, home = homedir(), env = process.env, temporary = tmpdir() } = {}) {
  const entries = [
    [home, "/workbench-build/home"],
    [temporary, "/workbench-build/tmp"],
    [env.CARGO_HOME ? resolve(root, env.CARGO_HOME) : join(home, ".cargo"), "/workbench-build/cargo"],
    [env.RUSTUP_HOME ? resolve(root, env.RUSTUP_HOME) : join(home, ".rustup"), "/workbench-build/rustup"],
    [root, "/workbench-build/source"],
  ];
  const mappings = new Map();
  for (const [from, to] of entries) {
    if (!isAbsolute(from) || from === "/" || /[\x00-\x1f=$`]/.test(from)) throw new Error("Unsupported build path; avoid control characters, '=', '$' or backticks.");
    mappings.set(from.replace(/\/$/, ""), to);
    if (existsSync(from)) mappings.set(realpathSync(from).replace(/\/$/, ""), to);
  }
  // rustc applies the last matching mapping. More specific directories win.
  return [...mappings].sort(([a], [b]) => a.length - b.length);
}

const quote = (value) => "'" + value.replaceAll("'", "'\\''") + "'";
const nativeFlags = (mappings) => mappings.map(([from, to]) => quote(`-ffile-prefix-map=${from}=${to}`)).join(" ");

export function privacyEnvironment(env = process.env, mappings = pathMappings({ env }), target = privacyTarget) {
  const result = { ...env };
  const existing = env.CARGO_ENCODED_RUSTFLAGS !== undefined
    ? env.CARGO_ENCODED_RUSTFLAGS.split("\x1f").filter(Boolean)
    : (env.RUSTFLAGS ?? env.CARGO_BUILD_RUSTFLAGS ?? "").split(/\s+/).filter(Boolean);
  result.CARGO_ENCODED_RUSTFLAGS = [...existing, ...mappings.map(([from, to]) => `--remap-path-prefix=${from}=${to}`)].join("\x1f");
  delete result.RUSTFLAGS;
  const additions = nativeFlags(mappings);
  for (const key of new Set(["CFLAGS", "CXXFLAGS", ...Object.keys(env).filter((name) => /^(?:(?:HOST|TARGET)_)?(?:C|CXX)FLAGS(?:_[\w-]+)?$/.test(name))])) {
    result[key] = [env[key], additions].filter(Boolean).join(" ");
  }
  result.CC_SHELL_ESCAPED_FLAGS = "1";
  result.CARGO_TARGET_DIR = target;
  result.WORKBENCH_REAL_PERL = env.WORKBENCH_REAL_PERL ?? env.OPENSSL_SRC_PERL ?? env.PERL ?? "/usr/bin/perl";
  // Make expands PERL without quoting; a PATH-resolved basename also works
  // when the checkout's directory name contains spaces.
  result.PATH = [dirname(opensslPerlWrapper), env.PATH].filter(Boolean).join(delimiter);
  result.PERL = basename(opensslPerlWrapper);
  result.OPENSSL_SRC_PERL = basename(opensslPerlWrapper);
  return result;
}

export function verifyPrivacyEnvironment(env = process.env, mappings = pathMappings({ env })) {
  const flags = (env.CARGO_ENCODED_RUSTFLAGS ?? "").split("\x1f");
  for (const [from, to] of mappings) {
    if (!flags.includes(`--remap-path-prefix=${from}=${to}`)) throw new Error("Missing Rust path remapping. Build with npm run app:build.");
    for (const key of ["CFLAGS", "CXXFLAGS"]) {
      if (!(env[key] ?? "").includes(quote(`-ffile-prefix-map=${from}=${to}`))) throw new Error("Missing native path remapping. Build with npm run app:build.");
    }
  }
  if (env.CC_SHELL_ESCAPED_FLAGS !== "1") throw new Error("Native build flags must use shell-escaped parsing.");
  if (env.PERL !== basename(opensslPerlWrapper) || env.OPENSSL_SRC_PERL !== basename(opensslPerlWrapper)) throw new Error("Missing OpenSSL metadata protection. Build with npm run app:build.");
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    if (process.argv[2] !== "--preflight") throw new Error("Use --preflight, or npm run app:build.");
    verifyPrivacyEnvironment();
    console.log("Build path-remapping preflight passed.");
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
