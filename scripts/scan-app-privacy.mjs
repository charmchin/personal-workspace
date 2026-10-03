import { closeSync, lstatSync, openSync, readSync, readdirSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { pathMappings } from "./build-privacy.mjs";

export function scanApp(app, prefixes = pathMappings().map(([from]) => from), chunkSize = 1024 * 1024) {
  const root = resolve(app);
  if (!root.endsWith(".app") || !lstatSync(root).isDirectory() || lstatSync(root).isSymbolicLink()) throw new Error("Choose an ordinary .app directory.");
  if (!lstatSync(join(root, "Contents", "Info.plist")).isFile() || !lstatSync(join(root, "Contents", "MacOS")).isDirectory()) throw new Error("App metadata or executable directory missing.");
  const needles = [...new Set(prefixes)].filter(Boolean).flatMap((prefix) => [Buffer.from(prefix), Buffer.from(prefix, "utf16le")]);
  if (!needles.length || !Number.isSafeInteger(chunkSize) || chunkSize < 1) throw new Error("Invalid privacy scan parameters.");
  const overlap = Math.max(...needles.map((needle) => needle.length)) - 1;
  const findings = []; let files = 0; let bytes = 0;
  function walk(directory) {
    for (const name of readdirSync(directory).sort()) {
      const path = join(directory, name); const info = lstatSync(path); const file = relative(root, path);
      if (info.isSymbolicLink()) throw new Error(`Unsupported symlink in app: ${file}`);
      if (info.isDirectory()) { walk(path); continue; }
      if (!info.isFile()) throw new Error(`Unsupported special file in app: ${file}`);
      files++;
      const descriptor = openSync(path, "r"); let tail = Buffer.alloc(0); let found = false;
      try {
        const buffer = Buffer.alloc(chunkSize);
        while (true) {
          const length = readSync(descriptor, buffer, 0, buffer.length, null);
          if (length === 0) break;
          bytes += length;
          const data = Buffer.concat([tail, buffer.subarray(0, length)]);
          if (needles.some((needle) => data.includes(needle))) found = true;
          tail = data.subarray(Math.max(0, data.length - overlap));
        }
      } finally { closeSync(descriptor); }
      if (found) findings.push({ file, category: "local-build-path" });
    }
  }
  walk(root);
  if (!readdirSync(join(root, "Contents", "MacOS")).length) throw new Error("App has no executable; privacy verification cannot pass.");
  return { files, bytes, findings };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    if (!process.argv[2] || process.argv.length !== 3) throw new Error("Usage: npm run app:privacy -- <path-to-app>");
    const result = scanApp(process.argv[2]);
    console.log(JSON.stringify(result));
    if (result.findings.length) process.exitCode = 1;
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
