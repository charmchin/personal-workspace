import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";

export const scrollbarId = "npm/react-remove-scroll-bar@2.3.8";
export const licenseRevision = "7301c160fda44cb8cf2b9fdfde61efad35736196";
export const sourceRevision = "29e9fcd1eecf7d3b77a767941c4a57fe461fc1e4";
export const licenseUrl = `https://raw.githubusercontent.com/theKashey/react-remove-scroll-bar/${licenseRevision}/LICENSE`;
export const evidenceFile = "release-code-equivalence.json";
const licenseHash = "a79aae0c0f21990d9d963bb3c5a79cdcea9a46f8523ba55c58d7fe776b6ebc84";
const pins = {
  "2.3.7": { url: "https://registry.npmjs.org/react-remove-scroll-bar/-/react-remove-scroll-bar-2.3.7.tgz", integrity: "sha512-UzA0npskGFA2bUqwWsSRJscuQEt7zyfVtMpCtBl/lk+k/ky6lzYImEks9jwgxTDe/B0tb5hyrYgbQv/JQaHjDw==", sha256: "5ee31e784254a1a0c80b2a14a569c3dc254af4c75d709a04f79ff409e856df91" },
  "2.3.8": { url: "https://registry.npmjs.org/react-remove-scroll-bar/-/react-remove-scroll-bar-2.3.8.tgz", integrity: "sha512-9r+yi9+mgU33AKcj6IbT9oRCO78WriSj6t/cF8DWBZJ9aOGPOTEDvdUDz1FwKim7QXWwmHqtdHnRJfhAxEG46Q==", sha256: "ccc872d7a2dc007cbf9d755f30d56b8d80eabdbe22c44c95a709129bfdc46f01" },
};
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const safePath = (path) => typeof path === "string" && /^[\w./-]+$/.test(path) && !path.startsWith("/") && !path.split("/").some((part) => !part || part === "." || part === "..");

// Read only to stdout; never extract an upstream archive into the checkout.
export function publishedFiles(bytes, pin) {
  if (hash(bytes) !== pin.sha256 || `sha512-${createHash("sha512").update(bytes).digest("base64")}` !== pin.integrity) throw new Error("Pinned npm tarball integrity mismatch");
  const tar = (args) => execFileSync("tar", [...args, "-"], { input: bytes, timeout: 10000, maxBuffer: 2 * 1024 * 1024 });
  const listing = tar(["-tvzf"]).toString().trim().split("\n");
  if (listing.some((line) => !/^[d-]/.test(line))) throw new Error("Unsupported archive entry in license evidence");
  const paths = tar(["-tzf"]).toString().trim().split("\n").filter((path) => !path.endsWith("/"));
  if (paths.length > 100 || new Set(paths).size !== paths.length || paths.some((path) => !path.startsWith("package/") || !safePath(path))) throw new Error("Unsafe archive paths in license evidence");
  const result = new Map();
  let total = 0;
  for (const path of paths.sort()) {
    const value = execFileSync("tar", ["-xOzf", "-", path], { input: bytes, timeout: 10000, maxBuffer: 2 * 1024 * 1024 });
    total += value.length;
    if (total > 4 * 1024 * 1024) throw new Error("License evidence archive exceeds size limit");
    result.set(path.slice("package/".length), value);
  }
  return result;
}

export function comparePublishedFiles(before, after) {
  const names = [...before.keys()].sort();
  if (JSON.stringify(names) !== JSON.stringify([...after.keys()].sort())) throw new Error("Published file lists differ; license correspondence not established");
  const files = [];
  for (const path of names) {
    if (!safePath(path)) throw new Error("Unsafe evidence path");
    if (path === "package.json") continue;
    if (!before.get(path).equals(after.get(path))) throw new Error(`Published content differs: ${path}`);
    files.push({ path, sha256: hash(after.get(path)) });
  }
  const a = JSON.parse(before.get("package.json")), b = JSON.parse(after.get("package.json"));
  if (a.name !== "react-remove-scroll-bar" || b.name !== a.name || a.version !== "2.3.7" || b.version !== "2.3.8" || a.license !== "MIT" || b.license !== "MIT") throw new Error("Unexpected package identity / license");
  const changed = [...new Set([...Object.keys(a), ...Object.keys(b)])].filter((key) => JSON.stringify(a[key]) !== JSON.stringify(b[key])).sort();
  if (JSON.stringify(changed) !== '["dependencies","version"]' || a.dependencies?.["react-style-singleton"] !== "^2.2.1" || b.dependencies?.["react-style-singleton"] !== "^2.2.2" || JSON.stringify({ ...a.dependencies, "react-style-singleton": b.dependencies["react-style-singleton"] }) !== JSON.stringify(b.dependencies)) throw new Error("Unexpected metadata differences; review license correspondence");
  return { files, metadataChanges: changed, packageJsonSha256: { "2.3.7": hash(before.get("package.json")), "2.3.8": hash(after.get("package.json")) } };
}

// Missing legacy evidence stays flagged; partially present or corrupt evidence
// must fail, never silently change a pending license to a resolved one.
export function verifiedScrollBarSupplement(notices, getFile) {
  const license = notices.find((file) => file.url === licenseUrl);
  const evidence = notices.find((file) => file.file === evidenceFile);
  if (!license && !evidence) return false;
  if (!license || !evidence || hash(license.text) !== licenseHash || license.sha256 !== licenseHash || hash(evidence.text) !== evidence.sha256) throw new Error("Invalid scrollbar license / evidence checksum");
  const proof = JSON.parse(evidence.text);
  if (evidence.origin !== "local-verification" || evidence.url !== undefined || !["2.3.7", "2.3.8"].every((version) => /^[a-f0-9]{64}$/.test(proof.packageJsonSha256?.[version]))) throw new Error("Invalid local scrollbar verification record");
  if (proof.schema !== 1 || proof.package !== scrollbarId || proof.sourceRevision !== sourceRevision || proof.licenseRevision !== licenseRevision || proof.licenseCommitParent !== sourceRevision || proof.licenseCommitFiles?.join() !== "LICENSE" || proof.metadataChanges?.join() !== "dependencies,version") throw new Error("Invalid pinned scrollbar license correspondence");
  for (const [version, pin] of Object.entries(pins)) if (JSON.stringify(proof.archives?.[version]) !== JSON.stringify(pin)) throw new Error("Unpinned scrollbar archive evidence");
  if (proof.files?.length !== 26 || new Set(proof.files.map((file) => file.path)).size !== 26 || proof.files.some((file) => !safePath(file.path) || file.path === "package.json" || !/^[a-f0-9]{64}$/.test(file.sha256))) throw new Error("Incomplete / unsafe scrollbar file evidence");
  const declaration = notices.find((file) => file.file.startsWith("package.json "));
  if (!declaration || declaration.url !== pins["2.3.8"].url || hash(declaration.text) !== proof.packageJsonSha256?.["2.3.8"] || declaration.sha256 !== hash(declaration.text) || JSON.parse(declaration.text).license !== "MIT") throw new Error("Scrollbar published declaration differs from evidence");
  if (getFile) for (const file of [...proof.files, { path: "package.json", sha256: proof.packageJsonSha256["2.3.8"] }]) if (hash(getFile(file.path)) !== file.sha256) throw new Error(`Installed scrollbar file differs from license evidence: ${file.path}`);
  return true;
}

export async function fetchScrollBarSupplement(download, binaryDownload) {
  const [commitText, sourceText, baselineText, currentText, license] = await Promise.all([
    download(`https://api.github.com/repos/theKashey/react-remove-scroll-bar/commits/${licenseRevision}`),
    download(`https://raw.githubusercontent.com/theKashey/react-remove-scroll-bar/${licenseRevision}/package.json`),
    download("https://registry.npmjs.org/react-remove-scroll-bar/2.3.7"),
    download("https://registry.npmjs.org/react-remove-scroll-bar/2.3.8"),
    download(licenseUrl),
  ]);
  const commit = JSON.parse(commitText), source = JSON.parse(sourceText), baseline = JSON.parse(baselineText), current = JSON.parse(currentText);
  if (baseline.version !== "2.3.7" || current.version !== "2.3.8" || commit.sha !== licenseRevision || commit.parents?.length !== 1 || commit.parents[0].sha !== sourceRevision || commit.files?.length !== 1 || commit.files[0].filename !== "LICENSE" || commit.files[0].status !== "added" || source.name !== baseline.name || source.version !== "2.3.7" || source.license !== "MIT" || baseline.gitHead !== sourceRevision || hash(license) !== licenseHash) throw new Error("Upstream license-only fix no longer matches pinned evidence");
  for (const meta of [baseline, current]) if (meta.name !== "react-remove-scroll-bar" || meta.license !== "MIT" || meta.dist.tarball !== pins[meta.version]?.url || meta.dist.integrity !== pins[meta.version]?.integrity) throw new Error("Registry release differs from pinned license evidence");
  const [before, after] = await Promise.all(Object.values(pins).map(async (pin) => publishedFiles(await binaryDownload(pin.url), pin)));
  const correspondence = comparePublishedFiles(before, after);
  const proof = { schema: 1, package: scrollbarId, sourceRevision, licenseRevision, licenseCommitParent: commit.parents[0].sha, licenseCommitFiles: [commit.files[0].filename], archives: pins, ...correspondence };
  const declaration = after.get("package.json").toString();
  const files = [
    { file: "package.json (published author and license declaration)", text: declaration, url: pins["2.3.8"].url, sha256: hash(declaration) },
    { file: "LICENSE (upstream license-only fix for identical published code)", text: license, url: licenseUrl, sha256: hash(license) },
    { file: evidenceFile, text: JSON.stringify(proof, null, 2) + "\n", origin: "local-verification", sha256: "" },
  ];
  files[2].sha256 = hash(files[2].text);
  if (!verifiedScrollBarSupplement(files)) throw new Error("Incomplete scrollbar evidence");
  return files;
}
