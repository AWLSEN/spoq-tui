#!/usr/bin/env node

/**
 * Postinstall script for @oaftobark/spoq
 *
 * This script handles the fallback case when the optional platform-specific
 * dependency fails to install (e.g., on unsupported platforms or when npm
 * can't resolve the optional dependency).
 *
 * It downloads the appropriate binary from download.spoq.dev (Railway)
 * for Unix platforms, or GitHub releases for Windows.
 */

const https = require("https");
const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

const PACKAGE_VERSION = require("./package.json").version;
const DOWNLOAD_URL = "https://download.spoq.dev";
const GITHUB_REPO = "AWLSEN/spoq-tui";

// Platform mapping: Node.js platform key -> conductor-version platform name
const PLATFORM_MAP = {
  "darwin-arm64": "darwin-aarch64",
  "darwin-x64": "darwin-x86_64",
  "linux-arm64": "linux-aarch64",
  "linux-x64": "linux-x86_64",
};

// Windows still uses GitHub releases (not supported by conductor-version)
const WINDOWS_ARCHIVE = "spoq-win32-x64.zip";

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
  const binDir = path.join(__dirname, "bin");
  fs.mkdirSync(binDir, { recursive: true });

  // Handle Windows separately (still uses GitHub releases)
  if (platformKey === "win32-x64") {
    console.log(`Platform package not found, downloading binary for ${platformKey} from GitHub...`);
    const downloadUrl = `https://github.com/${GITHUB_REPO}/releases/download/v${PACKAGE_VERSION}/${WINDOWS_ARCHIVE}`;

    try {
      const buffer = await downloadFile(downloadUrl);
      await extractZip(buffer, binDir);
      console.log("Binary downloaded successfully.");
    } catch (err) {
      console.warn(`Warning: Failed to download binary: ${err.message}`);
      console.warn("You may need to build spoq from source or install manually.");
    }
    return;
  }

  // Unix platforms use Railway (download.spoq.dev)
  const servicePlatform = PLATFORM_MAP[platformKey];

  if (!servicePlatform) {
    console.warn(`Warning: No prebuilt binary available for ${platformKey}`);
    console.warn("You may need to build spoq from source.");
    return;
  }

  console.log(`Platform package not found, downloading binary for ${platformKey}...`);

  const downloadUrl = `${DOWNLOAD_URL}/cli/download/${servicePlatform}`;
  console.log(`Downloading from: ${downloadUrl}`);

  try {
    const buffer = await downloadFile(downloadUrl);

    // Write raw binary directly (no extraction needed)
    const binaryPath = path.join(binDir, "spoq");
    fs.writeFileSync(binaryPath, buffer);
    fs.chmodSync(binaryPath, 0o755);

    console.log("Binary downloaded successfully.");
  } catch (err) {
    console.warn(`Warning: Failed to download binary: ${err.message}`);
    console.warn("You may need to build spoq from source or install manually.");
  }
}

main().catch((err) => {
  console.warn(`Postinstall warning: ${err.message}`);
});
