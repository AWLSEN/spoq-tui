#!/usr/bin/env node

/**
 * Postinstall script for @oaftobark/spoq
 *
 * This script handles the fallback case when the optional platform-specific
 * dependency fails to install (e.g., on unsupported platforms or when npm
 * can't resolve the optional dependency).
 *
 * It downloads the appropriate binary from GitHub releases.
 */

const https = require("https");
const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

const PACKAGE_VERSION = require("./package.json").version;
const GITHUB_REPO = "oaftobark/spoq";

const PLATFORMS = {
  "darwin-arm64": "spoq-darwin-arm64.tar.gz",
  "darwin-x64": "spoq-darwin-x64.tar.gz",
  "linux-arm64": "spoq-linux-arm64.tar.gz",
  "linux-x64": "spoq-linux-x64.tar.gz",
  "win32-x64": "spoq-win32-x64.zip",
};

function getPlatformKey() {
  return `${process.platform}-${process.arch}`;
}

function isPlatformPackageInstalled() {
  const platformKey = getPlatformKey();
  const packageNames = {
    "darwin-arm64": "@oaftobark/spoq-darwin-arm64",
    "darwin-x64": "@oaftobark/spoq-darwin-x64",
    "linux-arm64": "@oaftobark/spoq-linux-arm64",
    "linux-x64": "@oaftobark/spoq-linux-x64",
    "win32-x64": "@oaftobark/spoq-win32-x64",
  };

  const packageName = packageNames[platformKey];
  if (!packageName) return false;

  try {
    require.resolve(`${packageName}/package.json`);
    return true;
  } catch {
    return false;
  }
}

function downloadFile(url) {
  return new Promise((resolve, reject) => {
    const request = (url) => {
      https
        .get(url, (response) => {
          if (response.statusCode === 302 || response.statusCode === 301) {
            request(response.headers.location);
            return;
          }

          if (response.statusCode !== 200) {
            reject(new Error(`HTTP ${response.statusCode}: ${url}`));
            return;
          }

          const chunks = [];
          response.on("data", (chunk) => chunks.push(chunk));
          response.on("end", () => resolve(Buffer.concat(chunks)));
          response.on("error", reject);
        })
        .on("error", reject);
    };
    request(url);
  });
}

async function extractTarGz(buffer, destDir) {
  const tempFile = path.join(destDir, "temp.tar.gz");
  fs.writeFileSync(tempFile, buffer);
  try {
    execSync(`tar -xzf "${tempFile}" -C "${destDir}"`, { stdio: "pipe" });
  } finally {
    fs.unlinkSync(tempFile);
  }
}

async function extractZip(buffer, destDir) {
  const tempFile = path.join(destDir, "temp.zip");
  fs.writeFileSync(tempFile, buffer);
  try {
    execSync(`unzip -o "${tempFile}" -d "${destDir}"`, { stdio: "pipe" });
  } finally {
    fs.unlinkSync(tempFile);
  }
}

async function main() {
  // Skip if platform package is already installed
  if (isPlatformPackageInstalled()) {
    return;
  }

  const platformKey = getPlatformKey();
  const archiveFile = PLATFORMS[platformKey];

  if (!archiveFile) {
    console.warn(`Warning: No prebuilt binary available for ${platformKey}`);
    console.warn("You may need to build spoq from source.");
    return;
  }

  console.log(`Platform package not found, downloading binary for ${platformKey}...`);

  const binDir = path.join(__dirname, "bin");
  fs.mkdirSync(binDir, { recursive: true });

  const downloadUrl = `https://github.com/${GITHUB_REPO}/releases/download/v${PACKAGE_VERSION}/${archiveFile}`;

  try {
    const buffer = await downloadFile(downloadUrl);

    if (archiveFile.endsWith(".tar.gz")) {
      await extractTarGz(buffer, binDir);
    } else if (archiveFile.endsWith(".zip")) {
      await extractZip(buffer, binDir);
    }

    // Make binary executable on Unix
    if (process.platform !== "win32") {
      const binaryPath = path.join(binDir, "spoq");
      fs.chmodSync(binaryPath, 0o755);
    }

    console.log("Binary downloaded successfully.");
  } catch (err) {
    console.warn(`Warning: Failed to download binary: ${err.message}`);
    console.warn("You may need to build spoq from source or install manually.");
  }
}

main().catch((err) => {
  console.warn(`Postinstall warning: ${err.message}`);
});
