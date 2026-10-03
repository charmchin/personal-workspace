import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { buildArguments, pathMappings, privacyEnvironment, privacyBundleDirectory, projectRoot } from "./build-privacy.mjs";
import { scanApp } from "./scan-app-privacy.mjs";

try {
  if (process.platform !== "darwin" || process.arch !== "arm64") throw new Error("This build entry is for Apple Silicon macOS only.");
  const args = buildArguments(process.argv.slice(2));
  const mappings = pathMappings();
  const env = privacyEnvironment(process.env, mappings);
  const result = spawnSync(process.execPath, [join(projectRoot, "node_modules", "@tauri-apps", "cli", "tauri.js"), "build", ...args], { cwd: projectRoot, env, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`Tauri build failed (${result.status ?? result.signal}).`);
  const directory = privacyBundleDirectory;
  const apps = readdirSync(directory).filter((name) => name.endsWith(".app"));
  if (!apps.length) throw new Error("No .app found; privacy verification cannot be skipped.");
  for (const name of apps) {
    const app = join(directory, name);
    // Refuse suspicious bundles before invoking codesign. Seal resources using
    // a local ad-hoc signature only; no certificate, keychain or notarization.
    const before = scanApp(app, mappings.map(([from]) => from));
    if (before.findings.length) throw new Error("Local build paths remain in the app. Do not distribute this build.");
    for (const command of [["--force", "--sign", "-", "--timestamp=none", app], ["--verify", "--deep", "--strict", app]]) {
      const signed = spawnSync("/usr/bin/codesign", command, { stdio: "inherit" });
      if (signed.error || signed.status !== 0) throw new Error("Local app signature verification failed.");
    }
    const report = scanApp(app, mappings.map(([from]) => from));
    console.log(JSON.stringify({ app: name, ...report }));
    if (report.findings.length) throw new Error("Local build paths remain in the app. Do not distribute this build.");
  }
} catch (error) { console.error(error.message); process.exitCode = 1; }
