import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const project = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const app = path.join(project, "app");
const replacements = [
  [".cc-switch", ".ag-switch-pro"],
  ["cc-switch.db", "ag-switch-pro.db"],
  ["cc-switch-model-catalog.json", "ag-switch-pro-model-catalog.json"],
  ["CC Switch", "AG Switch Pro"],
  ["ccswitch://", "agswitchpro://"],
  ['"ccswitch"', '"agswitchpro"'],
  ["cc-switch-last-", "ag-switch-pro-last-"],
];

function updateDirectory(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const filename = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      if (!["node_modules", "target", "resources", "icons", "extracted"].includes(entry.name)) updateDirectory(filename);
    } else if (/\.(rs|ts|tsx|json|html|toml|md|snap)$/.test(entry.name)) {
      if (!fs.existsSync(path.join(project, "cc-switch", path.relative(app, filename)))) continue;
      const original = fs.readFileSync(filename, "utf8");
      let updated = original;
      updated = updated.replace(/(?<!AG_SWITCH_PRO_)PROXY_MANAGED/g, "AG_SWITCH_PRO_PROXY_MANAGED");
      for (const [before, after] of replacements) updated = updated.replaceAll(before, after);
      updated = updated.replaceAll("model_providers.ag-switch-pro-official", "model_providers.cc-switch-official");
      // Keep fixtures consistent with the independent proxy port.
      updated = updated.replace(/\b15721\b/g, "15722");
      if (updated !== original) fs.writeFileSync(filename, updated);
    }
  }
}
for (const folder of ["src", "src-tauri/src", "src-tauri/tests", "tests"]) updateDirectory(path.join(app, folder));

const configPath = path.join(app, "src-tauri", "tauri.conf.json");
const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
config.productName = "AG Switch Pro";
config.version = "0.1.0";
config.identifier = "com.agswitchpro.desktop";
config.bundle.createUpdaterArtifacts = false;
config.plugins["deep-link"].desktop.schemes = ["agswitchpro"];
delete config.plugins.updater;
fs.writeFileSync(configPath, JSON.stringify(config, null, 2) + "\n");
const windowsPath = path.join(app, "src-tauri", "tauri.windows.conf.json");
const windowsConfig = JSON.parse(fs.readFileSync(windowsPath, "utf8"));
windowsConfig.app.windows[0].title = "AG Switch Pro";
fs.writeFileSync(windowsPath, JSON.stringify(windowsConfig, null, 2) + "\n");
