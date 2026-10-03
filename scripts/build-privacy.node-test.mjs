import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { buildArguments, opensslPerlWrapper, pathMappings, privacyEnvironment, privacyTarget, projectRoot, verifyPrivacyEnvironment } from "./build-privacy.mjs";
import { scanApp } from "./scan-app-privacy.mjs";
import { protectGeneratedMakefile, redactBuildMetadata } from "./openssl-privacy-perl.mjs";

function fixture(callback) {
  const root = mkdtempSync(join(tmpdir(), "workbench privacy "));
  try {
    const app = join(root, "Test.app");
    mkdirSync(join(app, "Contents", "MacOS"), { recursive: true });
    writeFileSync(join(app, "Contents", "Info.plist"), "test metadata");
    const executable = join(app, "Contents", "MacOS", "test");
    writeFileSync(executable, "safe fixture");
    return callback({ root, app, executable });
  } finally { rmSync(root, { recursive: true }); }
}

test("build options precede the Cargo separator; unsupported artifacts cannot bypass scan", () => {
  assert.deepEqual(buildArguments(["--", "--locked", "--offline"]), ["--target", "aarch64-apple-darwin", "--bundles", "app", "--", "--locked", "--offline"]);
  assert.deepEqual(buildArguments(["--bundles=app"]), ["--target", "aarch64-apple-darwin", "--bundles=app"]);
  for (const args of [["--target", "x86_64-apple-darwin"], ["--debug"], ["--no-bundle"], ["--bundles=dmg"], ["-b", "dmg"]]) assert.throws(() => buildArguments(args));
});

test("mapping is relocatable, handles spaces and prefers specific prefixes", () => {
  fixture(({ root }) => {
    const mappings = pathMappings({ root, home: "/test home", env: { CARGO_HOME: "/external cargo" }, temporary: "/temporary builds" });
    assert(mappings.some(([from, to]) => from === root && to === "/workbench-build/source"));
    assert(mappings.some(([from, to]) => from === "/external cargo" && to === "/workbench-build/cargo"));
    assert(mappings.every(([from], index) => !index || from.length >= mappings[index - 1][0].length));
    assert.throws(() => pathMappings({ root: "/unsafe$path" }), /Unsupported/);
  });
});

test("keeps explicit flags and HOME unchanged, including unit-separated Rust flags", () => {
  const input = { HOME: "/test home", RUSTFLAGS: "--cfg prior", CFLAGS: "-DKEEP=1", CFLAGS_aarch64_apple_darwin: "-DTARGET=1", CXXFLAGS: "-DCPP=1" };
  const mappings = [["/test home", "/workbench-build/home"]];
  const output = privacyEnvironment(input, mappings, "/test target");
  assert.equal(input.RUSTFLAGS, "--cfg prior");
  assert.equal(output.HOME, input.HOME);
  assert.equal(output.CARGO_TARGET_DIR, "/test target");
  assert(output.CARGO_ENCODED_RUSTFLAGS.startsWith("--cfg\x1fprior\x1f"));
  assert(output.CFLAGS.includes("-DKEEP=1"));
  assert(output.CFLAGS_aarch64_apple_darwin.includes("-DTARGET=1"));
  assert.equal(output.CC_SHELL_ESCAPED_FLAGS, "1");
  const encoded = "-C\x1flink-arg=an argument with spaces";
  assert(privacyEnvironment({ CARGO_ENCODED_RUSTFLAGS: encoded, RUSTFLAGS: "ignored" }, mappings).CARGO_ENCODED_RUSTFLAGS.startsWith(encoded + "\x1f"));
  verifyPrivacyEnvironment(output, mappings);
});

test("preflight rejects missing Rust or native remapping", () => {
  const mappings = [["/test home", "/workbench-build/home"]];
  assert.throws(() => verifyPrivacyEnvironment({}, mappings), /Missing Rust/);
  const output = privacyEnvironment({}, mappings);
  assert.throws(() => verifyPrivacyEnvironment({ ...output, CFLAGS: "" }, mappings), /Missing native/);
  assert.throws(() => verifyPrivacyEnvironment({ ...output, CC_SHELL_ESCAPED_FLAGS: "0" }, mappings), /shell-escaped/);
});

test("scanner catches binary, UTF-16 and chunk-boundary leaks without printing values", () => {
  fixture(({ app, executable }) => {
    const marker = "/test home/private";
    writeFileSync(executable, Buffer.concat([Buffer.from([0, 1]), Buffer.from(marker), Buffer.from([0]), Buffer.from(marker, "utf16le")]));
    const report = scanApp(app, [marker], 3);
    assert.deepEqual(report.findings, [{ file: "Contents/MacOS/test", category: "local-build-path" }]);
    assert(!JSON.stringify(report).includes(marker));
  });
});

test("scanner reads every resource, leaves license text unchanged and rejects symlinks", () => {
  fixture(({ root, app }) => {
    const legal = join(app, "Contents", "Resources", "legal");
    mkdirSync(legal, { recursive: true });
    const notice = join(legal, "NOTICES.txt");
    writeFileSync(notice, "Copyright belongs to its upstream author.\n");
    const original = readFileSync(notice);
    assert.equal(scanApp(app, ["/test home"]).files, 3);
    assert.deepEqual(readFileSync(notice), original);
    writeFileSync(join(root, "outside"), "not an app resource");
    symlinkSync(join(root, "outside"), join(legal, "linked"));
    assert.throws(() => scanApp(app, ["/test home"]), /symlink/);
  });
});

test("scanner cannot pass an empty or non-app output", () => {
  fixture(({ root, app, executable }) => {
    assert.throws(() => scanApp(root, ["/test home"]), /ordinary .app/);
    rmSync(executable);
    assert.throws(() => scanApp(app, ["/test home"]), /no executable/);
  });
});

test("real Rust and native compilers remap file macros in paths containing spaces", () => {
  fixture(({ root }) => {
    const mappings = pathMappings({ root });
    const source = join(root, "fixture.rs");
    const binary = join(root, "rust-fixture");
    writeFileSync(source, 'fn main() { println!("{}", file!()); }\n');
    execFileSync("rustc", ["--crate-name", "privacy_fixture", source, "-o", binary, ...mappings.map(([from, to]) => `--remap-path-prefix=${from}=${to}`)]);
    const rustOutput = execFileSync(binary, [], { encoding: "utf8" }).trim();
    assert.equal(rustOutput, "/workbench-build/source/fixture.rs");
    const cSource = join(root, "fixture.c");
    const cBinary = join(root, "c-fixture");
    writeFileSync(cSource, '#include <stdio.h>\nint main(void) { puts(__FILE__); return 0; }\n');
    execFileSync("clang", [cSource, "-o", cBinary, ...mappings.map(([from, to]) => `-ffile-prefix-map=${from}=${to}`)]);
    assert.equal(execFileSync(cBinary, [], { encoding: "utf8" }).trim(), "/workbench-build/source/fixture.c");
  });
});

test("OpenSSL diagnostic redaction prefers specific paths without changing real compile flags", () => {
  const mappings = [["/test home", "/workbench-build/home"], ["/test home/source", "/workbench-build/source"]];
  const original = "cc '-ffile-prefix-map=/test home/source=/workbench-build/source' -I/test home/headers";
  assert.equal(redactBuildMetadata(original, mappings), "cc '-ffile-prefix-map=/workbench-build/source=/workbench-build/source' -I/workbench-build/home/headers");
  assert(original.includes("/test home/source"));
});

test("only generated OpenSSL loader metadata changes; install paths and notices stay intact", () => {
  const original = "# Copyright upstream\nINSTALLTOP=/real/install\nOPENSSLDIR=/real/config\nENGINESDIR=$(libdir)/engines-3\nMODULESDIR=$(libdir)/ossl-modules\n";
  const result = protectGeneratedMakefile(original);
  assert.equal(result, "# Copyright upstream\nINSTALLTOP=/real/install\nOPENSSLDIR=/real/config\nENGINESDIR=/workbench-build/openssl/engines\nMODULESDIR=/workbench-build/openssl/modules\n");
  assert.equal(protectGeneratedMakefile(result), result);
  assert.throws(() => protectGeneratedMakefile("MODULESDIR=/test\n"), /Unexpected/);
  assert.throws(() => protectGeneratedMakefile(original + "ENGINESDIR=duplicate\n"), /Unexpected/);
});

test("Perl wrapper delegates ordinary generators and sanitizes only isolated OpenSSL outputs", () => {
  mkdirSync(privacyTarget, { recursive: true });
  const root = mkdtempSync(join(privacyTarget, "openssl-build-test "));
  try {
    const env = privacyEnvironment();
    const run = (args, cwd = root) => execFileSync(opensslPerlWrapper, args, { cwd, env, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
    writeFileSync(join(root, "Configure"), 'open(my $f, ">", "Makefile") or die; print $f "# upstream license\\nINSTALLTOP=/real/install\\nENGINESDIR=real\\nMODULESDIR=real\\n"; close($f);\n');
    assert.throws(() => run(["./Configure"]), /no-module/);
    run(["./Configure", "no-module"]);
    assert(readFileSync(join(root, "Makefile"), "utf8").includes("INSTALLTOP=/real/install\nENGINESDIR=/workbench-build/openssl/engines"));
    writeFileSync(join(root, "mkbuildinf.pl"), 'print join("|", @ARGV);\n');
    assert.equal(run(["./mkbuildinf.pl", projectRoot + "fixture.c", "unchanged"]), "/workbench-build/source/fixture.c|unchanged");
    assert.equal(run(["-e", 'print "ordinary Perl output"']), "ordinary Perl output");
    assert.throws(() => run(["./Configure", "no-module"], projectRoot), /outside its isolated/);
  } finally { rmSync(root, { recursive: true }); }
});
