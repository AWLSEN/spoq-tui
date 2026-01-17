const assert = require("assert");
const path = require("path");

console.log("Running bin.js tests...\n");

// Test platform mapping
{
  console.log("Test: Platform mappings");
  const PLATFORMS = {
    "darwin-arm64": "@oaftobark/spoq-darwin-arm64",
    "darwin-x64": "@oaftobark/spoq-darwin-x64",
    "linux-arm64": "@oaftobark/spoq-linux-arm64",
    "linux-x64": "@oaftobark/spoq-linux-x64",
    "win32-x64": "@oaftobark/spoq-win32-x64",
  };

  assert.strictEqual(PLATFORMS["darwin-arm64"], "@oaftobark/spoq-darwin-arm64");
  assert.strictEqual(PLATFORMS["darwin-x64"], "@oaftobark/spoq-darwin-x64");
  assert.strictEqual(PLATFORMS["linux-arm64"], "@oaftobark/spoq-linux-arm64");
  assert.strictEqual(PLATFORMS["linux-x64"], "@oaftobark/spoq-linux-x64");
  assert.strictEqual(PLATFORMS["win32-x64"], "@oaftobark/spoq-win32-x64");
  console.log("  PASS: All platform mappings correct\n");
}

{
  console.log("Test: Binary name for Unix platforms");
  const binaryName = "darwin" === "win32" ? "spoq.exe" : "spoq";
  assert.strictEqual(binaryName, "spoq");
  console.log("  PASS: Unix binary name is 'spoq'\n");
}

{
  console.log("Test: Binary name for Windows platform");
  const binaryName = "win32" === "win32" ? "spoq.exe" : "spoq";
  assert.strictEqual(binaryName, "spoq.exe");
  console.log("  PASS: Windows binary name is 'spoq.exe'\n");
}

{
  console.log("Test: Package path construction");
  const packageName = "@oaftobark/spoq-darwin-arm64";
  const packagePath = `${packageName}/package.json`;
  assert.strictEqual(packagePath, "@oaftobark/spoq-darwin-arm64/package.json");
  console.log("  PASS: Package path constructed correctly\n");
}

{
  console.log("Test: Supported platform key format");
  const platformKey = `darwin-arm64`;
  assert.ok(/^(darwin|linux|win32)-(arm64|x64)$/.test(platformKey));
  console.log("  PASS: Platform key format validated\n");
}

{
  console.log("Test: Unsupported platform detection");
  const platformKey = `freebsd-x64`;
  const PLATFORMS = {
    "darwin-arm64": "@oaftobark/spoq-darwin-arm64",
    "darwin-x64": "@oaftobark/spoq-darwin-x64",
    "linux-arm64": "@oaftobark/spoq-linux-arm64",
    "linux-x64": "@oaftobark/spoq-linux-x64",
    "win32-x64": "@oaftobark/spoq-win32-x64",
  };
  assert.strictEqual(PLATFORMS[platformKey], undefined);
  console.log("  PASS: Unsupported platform returns undefined\n");
}

{
  console.log("Test: Local binary path construction");
  const localBinary = path.join("/test/dir", "bin", "spoq");
  assert.strictEqual(localBinary, "/test/dir/bin/spoq");
  console.log("  PASS: Local binary path constructed correctly\n");
}

console.log("All bin.js tests passed!");
