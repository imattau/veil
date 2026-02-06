const fs = require("fs");
const path = require("path");

const platform = process.env.npm_config_platform || process.platform;
if (platform !== "linux") {
  process.exit(0);
}

const projectRoot = path.resolve(__dirname, "..", "..", "..");
const relaySource = path.join(projectRoot, "target", "release", "veil-desktop-relay");
const appDist = path.join(__dirname, "..", "dist");
const relayDest = path.join(appDist, "relay", "veil-desktop-relay");

if (!fs.existsSync(relaySource)) {
  console.warn(
    "relay binary not found. Build it with: cargo build -p veil-desktop-relay --release",
  );
  process.exit(0);
}

fs.mkdirSync(path.dirname(relayDest), { recursive: true });
fs.copyFileSync(relaySource, relayDest);
fs.chmodSync(relayDest, 0o755);
console.log(`copied relay binary to ${relayDest}`);
