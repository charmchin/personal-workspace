// Separate from Vitest: run with npm run notices:test.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { noticeFiles, render, verifySnapshot } from "./third-party-notices.mjs";
import { comparePublishedFiles, evidenceFile, publishedFiles, scrollbarId, verifiedScrollBarSupplement } from "./scrollbar-license-evidence.mjs";

const digest = (value) => createHash("sha256").update(value).digest("hex");

test("collects root and vendored notices without following symlinks", () => {
  const path = mkdtempSync(join(tmpdir(), "workbench-notices-"));
  try {
    mkdirSync(join(path, "vendor"));
    writeFileSync(join(path, "LICENSE"), "root copyright\n");
    writeFileSync(join(path, "vendor", "NOTICE.txt"), "vendor copyright\n");
    writeFileSync(join(path, "source.rs"), "not a license file\n");
    writeFileSync(join(path, "copyright.pm"), "source code, not notice text\n");
    symlinkSync(join(path, "source.rs"), join(path, "LICENSE-secret"));
    assert.deepEqual(noticeFiles(path).map((file) => file.file), ["LICENSE", "vendor/NOTICE.txt"]);
    assert.equal(noticeFiles(path)[1].text, "vendor copyright\n");
  } finally { rmSync(path, { recursive: true }); }
});

test("verifies the release snapshot offline and rejects stale locks or tampered notices", () => {
  const path = mkdtempSync(join(tmpdir(), "workbench-notices-"));
  try {
    mkdirSync(join(path, "third-party"));
    mkdirSync(join(path, "src-tauri"));
    const files = { "package-lock.json": "npm lock", "src-tauri/Cargo.lock": "cargo lock", "third-party/NOTICES.txt": "original terms", "THIRD_PARTY_NOTICES.md": "index", "LICENSE": "MIT terms" };
    for (const [file, value] of Object.entries(files)) writeFileSync(join(path, file), value);
    writeFileSync(join(path, "package.json"), JSON.stringify({ version: "0.1.3" }));
    const manifest = { version: "0.1.3", target: "aarch64-apple-darwin", lockfiles: { npm: digest(files["package-lock.json"]), cargo: digest(files["src-tauri/Cargo.lock"]) }, artifacts: { notices: digest(files["third-party/NOTICES.txt"]), index: digest(files["THIRD_PARTY_NOTICES.md"]), projectLicense: digest(files.LICENSE) }, packages: [{ license: "MIT", notices: [{ sha256: "example" }] }] };
    writeFileSync(join(path, "third-party/manifest.json"), JSON.stringify(manifest));
    assert.equal(verifySnapshot(path), 1);
    for (const [file, value] of Object.entries(files)) {
      writeFileSync(join(path, file), value + "changed");
      assert.throws(() => verifySnapshot(path), /checksum mismatch/);
      writeFileSync(join(path, file), value);
    }
    writeFileSync(join(path, "package.json"), JSON.stringify({ version: "0.1.4" }));
    assert.throws(() => verifySnapshot(path), /version \/ target mismatch/);
  } finally { rmSync(path, { recursive: true }); }
});

test("repository release snapshot is internally consistent and contains no author machine paths", () => {
  assert(verifySnapshot() > 0);
  for (const path of ["THIRD_PARTY_NOTICES.md", "third-party/NOTICES.txt", "third-party/manifest.json", "third-party/supplemental.json"]) {
    assert(!/\/Users\/[^/\s]+\//.test(readFileSync(new URL("../" + path, import.meta.url), "utf8")));
  }
});

function publishedFixture(version) {
  return new Map([
    ["package.json", Buffer.from(JSON.stringify({ name: "react-remove-scroll-bar", version, license: "MIT", author: "Unchanged upstream author", dependencies: { "react-style-singleton": version === "2.3.7" ? "^2.2.1" : "^2.2.2", tslib: "^2.0.0" } }))],
    ["dist/index.js", Buffer.from("identical published code\n")],
    ["README.md", Buffer.from("identical published documentation\n")],
  ]);
}

test("release correspondence requires identical content and only the two recorded metadata changes", () => {
  const before = publishedFixture("2.3.7"), after = publishedFixture("2.3.8");
  assert.deepEqual(comparePublishedFiles(before, after).metadataChanges, ["dependencies", "version"]);
  assert.equal(comparePublishedFiles(before, after).files.length, 2);
  after.set("dist/index.js", Buffer.from("different code"));
  assert.throws(() => comparePublishedFiles(before, after), /content differs/);
  after.set("dist/index.js", before.get("dist/index.js"));
  after.set("extra.js", Buffer.from("extra code"));
  assert.throws(() => comparePublishedFiles(before, after), /file lists differ/);
  after.delete("extra.js");
  const pkg = JSON.parse(after.get("package.json"));
  after.set("package.json", Buffer.from(JSON.stringify({ ...pkg, author: "Other author" })));
  assert.throws(() => comparePublishedFiles(before, after), /metadata differences/);
  before.set("../outside", Buffer.from("unsafe")); after.set("../outside", Buffer.from("unsafe"));
  assert.throws(() => comparePublishedFiles(before, after), /Unsafe/);
});

const cachedScrollbar = () => JSON.parse(readFileSync(new URL("../third-party/supplemental.json", import.meta.url), "utf8"))[scrollbarId];
const installedFile = (path) => readFileSync(new URL("../node_modules/react-remove-scroll-bar/" + path, import.meta.url));

test("fixed upstream license and every installed release file match the cached correspondence", () => {
  const cached = cachedScrollbar();
  assert(verifiedScrollBarSupplement(cached, installedFile));
  const proof = JSON.parse(cached.find((file) => file.file === evidenceFile).text);
  assert.equal(proof.files.length, 26);
  assert.throws(() => publishedFiles(Buffer.from("corrupted tarball"), proof.archives["2.3.8"]), /integrity mismatch/);
  assert.throws(() => verifiedScrollBarSupplement(cached, (path) => path.endsWith(".js") ? Buffer.from("modified installed code") : installedFile(path)), /Installed scrollbar file differs/);
});

test("missing evidence stays pending; partial, fabricated and corrupt evidence fail closed", () => {
  const cached = cachedScrollbar();
  assert.equal(verifiedScrollBarSupplement([cached[0]]), false);
  assert.throws(() => verifiedScrollBarSupplement([cached[0], cached[1]]), /checksum/);
  const fabricated = structuredClone(cached);
  fabricated[1].text = fabricated[1].text.replace("2025", "2099"); fabricated[1].sha256 = digest(fabricated[1].text);
  assert.throws(() => verifiedScrollBarSupplement(fabricated), /checksum/);
  const changed = structuredClone(cached), proof = JSON.parse(changed[2].text);
  proof.licenseCommitParent = "f".repeat(40);
  changed[2].text = JSON.stringify(proof); changed[2].sha256 = digest(changed[2].text);
  assert.throws(() => verifiedScrollBarSupplement(changed), /correspondence/);
  const pkg = { id: scrollbarId, name: "react-remove-scroll-bar", version: "2.3.8", ecosystem: "npm", license: "MIT", source: cached[0].url, notices: [cached[0]] };
  assert.equal(JSON.parse(render([pkg])["third-party/manifest.json"]).reviewRequired.length, 1);
  assert.deepEqual(JSON.parse(render([{ ...pkg, notices: cached }])["third-party/manifest.json"]).reviewRequired, []);
});

test("offline snapshot verification binds supplemental evidence to the shipped manifest", () => {
  const base = mkdtempSync(join(tmpdir(), "workbench-license-proof-"));
  try {
    const files = ["package.json", "package-lock.json", "src-tauri/Cargo.lock", "LICENSE", "THIRD_PARTY_NOTICES.md", "third-party/NOTICES.txt", "third-party/manifest.json", "third-party/supplemental.json"];
    for (const file of files) { mkdirSync(join(base, file, ".."), { recursive: true }); writeFileSync(join(base, file), readFileSync(new URL("../" + file, import.meta.url))); }
    assert(verifySnapshot(base) > 0);
    const all = JSON.parse(readFileSync(join(base, "third-party/supplemental.json"), "utf8"));
    all[scrollbarId][2].text += "\n";
    all[scrollbarId][2].sha256 = digest(all[scrollbarId][2].text);
    assert(verifiedScrollBarSupplement(all[scrollbarId]));
    writeFileSync(join(base, "third-party/supplemental.json"), JSON.stringify(all));
    assert.throws(() => verifySnapshot(base), /differs from the locked notice manifest/);
  } finally { rmSync(base, { recursive: true }); }
});
