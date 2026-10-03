#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { readFileSync, realpathSync, writeFileSync } from "node:fs";
import { basename, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { opensslPerlWrapper, pathMappings, privacyTarget } from "./build-privacy.mjs";

// Only generated diagnostics are redacted. Actual compiler arguments and
// installation directories must continue to point to real files.
export function redactBuildMetadata(value, mappings) {
  for (const [from, to] of [...mappings].sort(([a], [b]) => b.length - a.length)) value = value.replaceAll(from, to);
  return value;
}

export function protectGeneratedMakefile(text) {
  for (const [field, directory] of [["ENGINESDIR", "engines"], ["MODULESDIR", "modules"]]) {
    const rule = new RegExp(`^${field}=.*$`, "gm");
    if ((text.match(rule) ?? []).length !== 1) throw new Error("Unexpected OpenSSL generated metadata; refusing an unverified build.");
    text = text.replace(rule, `${field}=/workbench-build/openssl/${directory}`);
  }
  return text;
}

function run() {
  const args = process.argv.slice(2);
  const script = args.find((arg) => ["Configure", "mkbuildinf.pl"].includes(basename(arg)));
  const realPerl = process.env.WORKBENCH_REAL_PERL;
  if (!realPerl || resolve(realPerl) === opensslPerlWrapper) throw new Error("Missing independent Perl runtime for the private build.");
  if (script) {
    const buildPath = realpathSync(process.cwd());
    const inside = relative(realpathSync(privacyTarget), buildPath);
    if (!inside || inside === ".." || inside.startsWith(`..${sep}`) || isAbsolute(inside) || !inside.includes("openssl-build")) throw new Error("OpenSSL privacy wrapper invoked outside its isolated build directory.");
    if (basename(script) === "Configure" && !args.includes("no-module")) throw new Error("Virtual OpenSSL module paths require a static no-module build.");
  }
  const metadata = script && basename(script) === "mkbuildinf.pl";
  const forwarded = metadata ? args.map((arg, index) => index > args.indexOf(script) ? redactBuildMetadata(arg, pathMappings()) : arg) : args;
  const child = spawnSync(realPerl, forwarded, { stdio: "inherit", env: process.env });
  if (child.error) throw new Error("Unable to execute the Perl runtime.");
  if (child.status !== 0) { process.exitCode = child.status ?? 1; return; }
  if (script && basename(script) === "Configure") {
    // Vendored openssl-src explicitly disables dynamic modules. Keep that
    // requirement: these virtual loader directories are not real install paths.
    const makefile = resolve("Makefile");
    writeFileSync(makefile, protectGeneratedMakefile(readFileSync(makefile, "utf8")));
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { run(); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
