import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { checkMarkdownLinks, checkRepository, checkSourcePaths, checkVersions } from "./check-repository.mjs";

function fixture(callback) {
  const root = mkdtempSync(join(tmpdir(), "workbench-repository-"));
  const write = (file, value) => {
    mkdirSync(dirname(join(root, file)), { recursive: true });
    writeFileSync(join(root, file), typeof value === "string" ? value : JSON.stringify(value));
  };
  try { callback(root, write); } finally { rmSync(root, { recursive: true }); }
}

test("current repository conventions, versions and local links are consistent", () => {
  assert.match(checkRepository().version, /^\d+\.\d+\.\d+/);
});

test("version checks reject drift, absent package and mismatched license metadata", () => {
  fixture((root, write) => {
    write("package.json", { version: "1.2.3", license: "MIT" });
    write("package-lock.json", { version: "1.2.3", packages: { "": { version: "1.2.3", license: "MIT" } } });
    write("src-tauri/tauri.conf.json", { version: "1.2.3" });
    write("src-tauri/Cargo.toml", '[package]\nname = "example"\nversion = "1.2.3"\nlicense = "MIT"\n\n[dependencies]\n');
    write("src-tauri/Cargo.lock", '[[package]]\nname = "example"\nversion = "1.2.3"\n');
    write("third-party/manifest.json", { version: "1.2.3" });
    assert.equal(checkVersions(root), "1.2.3");
    write("src-tauri/tauri.conf.json", { version: "1.2.4" });
    assert.throws(() => checkVersions(root), /Version mismatch/);
    write("src-tauri/tauri.conf.json", { version: "1.2.3" });
    write("src-tauri/Cargo.lock", '[[package]]\nname = "other"\nversion = "1.2.3"\n');
    assert.throws(() => checkVersions(root), /Version mismatch/);
    write("src-tauri/Cargo.lock", '[[package]]\nname = "example"\nversion = "1.2.3"\n');
    write("package.json", { version: "1.2.3", license: "Unlicense" });
    assert.throws(() => checkVersions(root), /license metadata/);
  });
});

test("Markdown checks resolve relative, escaped and angled file targets", () => {
  fixture((root, write) => {
    write("My Doc.md", "# Target\n");
    write("docs/README.md", '[a](../My%20Doc.md#target) [b](<../My Doc.md>) [c](https://example.org) [d](#local)\n```bash\n[x](missing.md)\n```\n');
    assert.equal(checkMarkdownLinks(root, ["docs/README.md"]), 2);
    write("docs/README.md", "[broken](missing.md)\n");
    assert.throws(() => checkMarkdownLinks(root, ["docs/README.md"]), /Missing local/);
  });
});

test("Markdown checks reject escaping paths and symlink targets outside repository", () => {
  fixture((root, write) => {
    write("README.md", "[outside](..)\n");
    assert.throws(() => checkMarkdownLinks(root, ["README.md"]), /leaves repository/);
    symlinkSync(tmpdir(), join(root, "outside"));
    write("README.md", "[outside](outside)\n");
    assert.throws(() => checkMarkdownLinks(root, ["README.md"]), /leaves repository/);
  });
});

test("source checks reject sensitive and generated paths without rejecting examples", () => {
  assert.equal(checkSourcePaths(["src/App.tsx", "docs/backup.md", ".env.example", "scripts/build-app.mjs"]), 4);
  for (const file of [".env", ".env.production", "test/.env.local", "database-key.age", "backup.workbench-backup", "workbench.sqlite3-wal", "data.sqlite3", "secret.key", "src-tauri/target/cache", "dist/index.html", "node_modules/example/index.js", "Some.app/Contents/Info.plist"]) {
    assert.throws(() => checkSourcePaths([file]), /path in source/, file);
  }
});
