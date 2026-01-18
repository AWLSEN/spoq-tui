#!/usr/bin/env node

const { spawn } = require("child_process");
const path = require("path");

const PLATFORMS = {
  "darwin-arm64": "@oaftobark/spoq-darwin-arm64",
  "darwin-x64": "@oaftobark/spoq-darwin-x64",
  "linux-arm64": "@oaftobark/spoq-linux-arm64",
  "linux-x64": "@oaftobark/spoq-linux-x64",
  "win32-x64": "@oaftobark/spoq-win32-x64",
};

function getBinaryPath() {
  const platformKey = `${process.platform}-${process.arch}`;
  const packageName = PLATFORMS[platformKey];

  if (!packageName) {
    console.error(`Unsupported platform: ${platformKey}`);
    console.error(`Supported platforms: ${Object.keys(PLATFORMS).join(", ")}`);
    process.exit(1);
  }

  const binaryName = process.platform === "win32" ? "spoq.exe" : "spoq";

  try {
    const packagePath = require.resolve(`${packageName}/package.json`);
    const packageDir = path.dirname(packagePath);
    return path.join(packageDir, "bin", binaryName);
  } catch (e) {
    // Package not found, check for local binary (fallback from postinstall)
    const localBinary = path.join(__dirname, "bin", binaryName);
    try {
      require("fs").accessSync(localBinary, require("fs").constants.X_OK);
      return localBinary;
    } catch {
      console.error(`Could not find spoq binary for ${platformKey}`);
      console.error(
        "Please try reinstalling: npm install -g @oaftobark/spoq"
      );
      process.exit(1);
    }
  }
}

const binaryPath = getBinaryPath();
const args = process.argv.slice(2);

const child = spawn(binaryPath, args, {
  stdio: "inherit",
  env: process.env,
});

child.on("error", (err) => {
  console.error(`Failed to start spoq: ${err.message}`);
  process.exit(1);
});

child.on("close", (code) => {
  process.exit(code ?? 0);
});
