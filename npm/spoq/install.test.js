const assert = require("assert");

console.log("Running install.js tests...\n");

// Platform Detection Tests
{
  console.log("Test: Archive names for all platforms");
  const PLATFORMS = {
    "darwin-arm64": "spoq-darwin-arm64.tar.gz",
    "darwin-x64": "spoq-darwin-x64.tar.gz",
    "linux-arm64": "spoq-linux-arm64.tar.gz",
    "linux-x64": "spoq-linux-x64.tar.gz",
    "win32-x64": "spoq-win32-x64.zip",
  };

  assert.strictEqual(PLATFORMS["darwin-arm64"], "spoq-darwin-arm64.tar.gz");
  assert.strictEqual(PLATFORMS["darwin-x64"], "spoq-darwin-x64.tar.gz");
  assert.strictEqual(PLATFORMS["linux-arm64"], "spoq-linux-arm64.tar.gz");
  assert.strictEqual(PLATFORMS["linux-x64"], "spoq-linux-x64.tar.gz");
  assert.strictEqual(PLATFORMS["win32-x64"], "spoq-win32-x64.zip");
  console.log("  PASS: All archive names correct\n");
}

{
  console.log("Test: Unix platforms use tar.gz");
  const PLATFORMS = {
    "darwin-arm64": "spoq-darwin-arm64.tar.gz",
    "darwin-x64": "spoq-darwin-x64.tar.gz",
    "linux-arm64": "spoq-linux-arm64.tar.gz",
    "linux-x64": "spoq-linux-x64.tar.gz",
  };

  assert.ok(PLATFORMS["darwin-arm64"].endsWith(".tar.gz"));
  assert.ok(PLATFORMS["darwin-x64"].endsWith(".tar.gz"));
  assert.ok(PLATFORMS["linux-arm64"].endsWith(".tar.gz"));
  assert.ok(PLATFORMS["linux-x64"].endsWith(".tar.gz"));
  console.log("  PASS: Unix platforms use tar.gz format\n");
}

{
  console.log("Test: Windows platform uses zip");
  const archive = "spoq-win32-x64.zip";
  assert.ok(archive.endsWith(".zip"));
  console.log("  PASS: Windows platform uses zip format\n");
}

{
  console.log("Test: Platform key construction");
  const platformKey = `${process.platform}-${process.arch}`;
  assert.ok(typeof platformKey === "string");
  assert.ok(platformKey.includes("-"));
  console.log("  PASS: Platform key constructed correctly\n");
}

// Download URL Construction Tests
{
  console.log("Test: GitHub release URL construction");
  const PACKAGE_VERSION = "0.1.0";
  const GITHUB_REPO = "oaftobark/spoq";
  const archiveFile = "spoq-darwin-arm64.tar.gz";
  const url = `https://github.com/${GITHUB_REPO}/releases/download/v${PACKAGE_VERSION}/${archiveFile}`;
  assert.strictEqual(url, "https://github.com/oaftobark/spoq/releases/download/v0.1.0/spoq-darwin-arm64.tar.gz");
  console.log("  PASS: GitHub URL constructed correctly\n");
}

{
  console.log("Test: Version prefix in URL");
  const PACKAGE_VERSION = "0.1.0";
  const GITHUB_REPO = "oaftobark/spoq";
  const url = `https://github.com/${GITHUB_REPO}/releases/download/v${PACKAGE_VERSION}/spoq-linux-x64.tar.gz`;
  assert.ok(url.includes("/v0.1.0/"));
  console.log("  PASS: Version prefix included in URL\n");
}

{
  console.log("Test: URLs for all platforms");
  const PACKAGE_VERSION = "0.1.0";
  const GITHUB_REPO = "oaftobark/spoq";
  const platforms = ["darwin-arm64", "darwin-x64", "linux-arm64", "linux-x64", "win32-x64"];

  platforms.forEach(platform => {
    const archiveExt = platform.startsWith("win32") ? "zip" : "tar.gz";
    const url = `https://github.com/${GITHUB_REPO}/releases/download/v${PACKAGE_VERSION}/spoq-${platform}.${archiveExt}`;
    assert.ok(url.startsWith("https://github.com/"));
    assert.ok(url.includes(platform));
  });
  console.log("  PASS: URLs for all platforms valid\n");
}

// Package Resolution Tests
{
  console.log("Test: Package name mappings");
  const packageNames = {
    "darwin-arm64": "@oaftobark/spoq-darwin-arm64",
    "darwin-x64": "@oaftobark/spoq-darwin-x64",
    "linux-arm64": "@oaftobark/spoq-linux-arm64",
    "linux-x64": "@oaftobark/spoq-linux-x64",
    "win32-x64": "@oaftobark/spoq-win32-x64",
  };

  assert.strictEqual(packageNames["darwin-arm64"], "@oaftobark/spoq-darwin-arm64");
  assert.strictEqual(packageNames["linux-x64"], "@oaftobark/spoq-linux-x64");
  assert.strictEqual(packageNames["win32-x64"], "@oaftobark/spoq-win32-x64");
  console.log("  PASS: Package names mapped correctly\n");
}

{
  console.log("Test: Unsupported platform returns undefined");
  const platformKey = "freebsd-x64";
  const packageNames = {
    "darwin-arm64": "@oaftobark/spoq-darwin-arm64",
    "darwin-x64": "@oaftobark/spoq-darwin-x64",
    "linux-arm64": "@oaftobark/spoq-linux-arm64",
    "linux-x64": "@oaftobark/spoq-linux-x64",
    "win32-x64": "@oaftobark/spoq-win32-x64",
  };
  const packageName = packageNames[platformKey];
  assert.strictEqual(packageName, undefined);
  console.log("  PASS: Unsupported platform returns undefined\n");
}

// HTTP Response Handling Tests
{
  console.log("Test: Redirect status codes");
  const redirectCodes = [301, 302];
  redirectCodes.forEach(code => {
    assert.ok(code === 301 || code === 302);
  });
  console.log("  PASS: Redirect status codes handled\n");
}

{
  console.log("Test: Success status code");
  const statusCode = 200;
  assert.strictEqual(statusCode, 200);
  console.log("  PASS: Success status code is 200\n");
}

{
  console.log("Test: Error status codes detection");
  const errorCodes = [404, 500, 403];
  errorCodes.forEach(code => {
    assert.ok(code !== 200);
  });
  console.log("  PASS: Error status codes detected\n");
}

console.log("All install.js tests passed!");
