// Adding test tooling must not upgrade the application's inherited dependencies.
import fs from "node:fs";
const original = fs.readFileSync("cc-switch/pnpm-lock.yaml", "utf8").replaceAll("\r\n", "\n");
const expanded = fs.readFileSync("app/pnpm-lock.yaml", "utf8").replaceAll("\r\n", "\n");
function sections(text) {
  const packagesAt = text.indexOf("\npackages:\n");
  const snapshotsAt = text.indexOf("\nsnapshots:\n");
  return [text.slice(0, packagesAt), text.slice(packagesAt + 11, snapshotsAt), text.slice(snapshotsAt + 12)];
}
function merge(oldText, addedText) {
  const blocks = (text) => text.trim().split(/\n\n(?= {2}\S)/).map((block) => block.startsWith("  ") ? block : `  ${block}`);
  const existing = new Set(blocks(oldText).map((block) => block.split("\n")[0]));
  return [...blocks(oldText), ...blocks(addedText).filter((block) => !existing.has(block.split("\n")[0]))].join("\n\n");
}
const [oldPrefix, oldPackages, oldSnapshots] = sections(original);
const [, newPackages, newSnapshots] = sections(expanded);
const coverage = expanded.match(/      '@vitest\/coverage-v8':\n(?: {8}[^\n]*\n)+/)?.[0];
if (!coverage) throw new Error("Coverage tool missing from lock importer");
const prefix = oldPrefix.replace("    devDependencies:\n", `    devDependencies:\n${coverage}`);
fs.writeFileSync("app/pnpm-lock.yaml", `${prefix}\npackages:\n\n${merge(oldPackages, newPackages)}\n\nsnapshots:\n\n${merge(oldSnapshots, newSnapshots)}\n`);
