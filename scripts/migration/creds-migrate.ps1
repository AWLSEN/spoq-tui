#Requires -Version 5.1
<#
.SYNOPSIS
    CLI Credentials Migration Tool for Windows

.DESCRIPTION
    Extract and restore CLI tool credentials for VPS migration.
    Supported tools: GitHub CLI, Claude Code, Codex (OpenAI)

.PARAMETER Command
    The operation to perform: export, import, or list

.PARAMETER Path
    For export: output archive path (optional)
    For import: input archive path (required)

.EXAMPLE
    .\creds-migrate.ps1 export
    .\creds-migrate.ps1 export C:\Users\me\creds.zip
    .\creds-migrate.ps1 import C:\Users\me\creds.zip
    .\creds-migrate.ps1 list
#>

param(
    [Parameter(Position = 0)]
    [ValidateSet("export", "import", "list", "help")]
    [string]$Command = "help",

    [Parameter(Position = 1)]
    [string]$Path
)

$Version = "1.0.0"
$ScriptName = $MyInvocation.MyCommand.Name

# Credential paths relative to user profile
$CredentialPaths = @{
    "gh_dir"       = ".config\gh"
    "claude_json"  = ".claude.json"
    "claude_dir"   = ".claude"
    "codex_dir"    = ".codex"
}

# Items to export (populated during detection)
$script:ExportItems = @()

#------------------------------------------------------------------------------
# Utility Functions
#------------------------------------------------------------------------------

function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] " -ForegroundColor Blue -NoNewline
    Write-Host $Message
}

function Write-Success {
    param([string]$Message)
    Write-Host "[OK] " -ForegroundColor Green -NoNewline
    Write-Host $Message
}

function Write-Warn {
    param([string]$Message)
    Write-Host "[WARN] " -ForegroundColor Yellow -NoNewline
    Write-Host $Message
}

function Write-Error2 {
    param([string]$Message)
    Write-Host "[ERROR] " -ForegroundColor Red -NoNewline
    Write-Host $Message
}

function Get-HomeDir {
    return $env:USERPROFILE
}

#------------------------------------------------------------------------------
# Credential Detection
#------------------------------------------------------------------------------

function Test-GhCredentials {
    param([string]$HomeDir)

    $hostsFile = Join-Path $HomeDir ".config\gh\hosts.yml"
    if (Test-Path $hostsFile) {
        Write-Success "GitHub CLI: Found credentials at ~\.config\gh\"
        $script:ExportItems += ".config\gh"
        return $true
    }
    else {
        Write-Warn "GitHub CLI: No credentials found"
        return $false
    }
}

function Test-ClaudeCredentials {
    param([string]$HomeDir)

    $claudeJson = Join-Path $HomeDir ".claude.json"
    $claudeDir = Join-Path $HomeDir ".claude"
    $found = $false

    if (Test-Path $claudeJson) {
        $content = Get-Content $claudeJson -Raw -ErrorAction SilentlyContinue
        if ($content -match "oauthAccount") {
            Write-Success "Claude Code: Found OAuth credentials in ~\.claude.json"
            $script:ExportItems += ".claude.json"
            $found = $true
        }
    }

    if (Test-Path $claudeDir) {
        $settingsJson = Join-Path $claudeDir "settings.json"
        $settingsLocal = Join-Path $claudeDir "settings.local.json"

        if ((Test-Path $settingsJson) -or (Test-Path $settingsLocal)) {
            Write-Success "Claude Code: Found config directory at ~\.claude\"
            if (-not ($script:ExportItems -contains ".claude.json")) {
                $script:ExportItems += ".claude.json"
            }
            $script:ExportItems += ".claude\settings.json"
            $script:ExportItems += ".claude\settings.local.json"
            $found = $true
        }
    }

    if (-not $found) {
        Write-Warn "Claude Code: No credentials found"
    }
    return $found
}

function Test-CodexCredentials {
    param([string]$HomeDir)

    $codexAuth = Join-Path $HomeDir ".codex\auth.json"
    if (Test-Path $codexAuth) {
        Write-Success "Codex: Found credentials at ~\.codex\"
        $script:ExportItems += ".codex"
        return $true
    }
    else {
        Write-Warn "Codex: No credentials found"
        return $false
    }
}

#------------------------------------------------------------------------------
# Export Function
#------------------------------------------------------------------------------

function Invoke-Export {
    param([string]$OutputPath)

    $homeDir = Get-HomeDir
    $timestamp = Get-Date -Format "yyyyMMdd_HHmmss"

    Write-Info "Detecting credentials on Windows..."
    Write-Host ""

    # Detect all credentials
    $foundAny = $false
    if (Test-GhCredentials $homeDir) { $foundAny = $true }
    if (Test-ClaudeCredentials $homeDir) { $foundAny = $true }
    if (Test-CodexCredentials $homeDir) { $foundAny = $true }

    Write-Host ""

    if (-not $foundAny) {
        Write-Error2 "No credentials found to export"
        exit 1
    }

    # Set default output file
    if ([string]::IsNullOrEmpty($OutputPath)) {
        $OutputPath = "spoq-creds-$timestamp.zip"
    }

    # Create staging directory
    $stagingDir = Join-Path $env:TEMP "spoq-creds-$timestamp"
    New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null

    Write-Info "Staging credentials for export..."

    # Directories/files to exclude from .claude
    $excludePatterns = @(
        "*.log",
        "history.jsonl",
        "cache",
        "session-env",
        "shell-snapshots",
        "telemetry",
        "debug",
        "todos",
        "paste-cache",
        "file-history",
        "projects",
        "statsig",
        "sessions",
        "log"
    )

    foreach ($item in $script:ExportItems) {
        $src = Join-Path $homeDir $item
        $dest = Join-Path $stagingDir $item

        if (Test-Path $src) {
            $destParent = Split-Path $dest -Parent
            if (-not (Test-Path $destParent)) {
                New-Item -ItemType Directory -Path $destParent -Force | Out-Null
            }

            if ((Get-Item $src).PSIsContainer) {
                # Copy directory with exclusions
                $destDir = $dest
                New-Item -ItemType Directory -Path $destDir -Force | Out-Null

                Get-ChildItem -Path $src -Recurse | ForEach-Object {
                    $relativePath = $_.FullName.Substring($src.Length + 1)
                    $exclude = $false

                    foreach ($pattern in $excludePatterns) {
                        if ($relativePath -like $pattern -or $relativePath -like "*\$pattern" -or $relativePath -like "*\$pattern\*") {
                            $exclude = $true
                            break
                        }
                    }

                    if (-not $exclude) {
                        $targetPath = Join-Path $destDir $relativePath
                        if ($_.PSIsContainer) {
                            New-Item -ItemType Directory -Path $targetPath -Force -ErrorAction SilentlyContinue | Out-Null
                        }
                        else {
                            $targetParent = Split-Path $targetPath -Parent
                            if (-not (Test-Path $targetParent)) {
                                New-Item -ItemType Directory -Path $targetParent -Force | Out-Null
                            }
                            Copy-Item $_.FullName $targetPath -Force -ErrorAction SilentlyContinue
                        }
                    }
                }
            }
            else {
                Copy-Item $src $dest -Force
            }
            Write-Success "  Staged: $item"
        }
    }

    # Create manifest
    $manifest = @{
        version         = $Version
        created_at      = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        source_os       = "windows"
        source_hostname = $env:COMPUTERNAME
        items           = $script:ExportItems
    }
    $manifestPath = Join-Path $stagingDir "manifest.json"
    $manifest | ConvertTo-Json | Out-File $manifestPath -Encoding UTF8

    # Create archive
    Write-Info "Creating archive: $OutputPath"
    Compress-Archive -Path "$stagingDir\*" -DestinationPath $OutputPath -Force

    # Cleanup staging
    Remove-Item $stagingDir -Recurse -Force

    Write-Host ""
    Write-Success "Export complete!"
    Write-Info "Archive: $OutputPath"
    Write-Info "Size: $([math]::Round((Get-Item $OutputPath).Length / 1KB, 2)) KB"
    Write-Host ""
    Write-Warn "SECURITY: This archive contains sensitive credentials."
    Write-Warn "          Transfer securely and delete after import."
}

#------------------------------------------------------------------------------
# Import Function
#------------------------------------------------------------------------------

function Invoke-Import {
    param([string]$ArchivePath)

    if ([string]::IsNullOrEmpty($ArchivePath)) {
        Write-Error2 "Usage: $ScriptName import <archive_file>"
        exit 1
    }

    if (-not (Test-Path $ArchivePath)) {
        Write-Error2 "Archive not found: $ArchivePath"
        exit 1
    }

    $homeDir = Get-HomeDir
    $timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
    $stagingDir = Join-Path $env:TEMP "spoq-import-$timestamp"

    Write-Info "Extracting archive..."
    Expand-Archive -Path $ArchivePath -DestinationPath $stagingDir -Force

    # Read manifest
    $manifestPath = Join-Path $stagingDir "manifest.json"
    if (-not (Test-Path $manifestPath)) {
        Write-Error2 "Invalid archive: manifest.json not found"
        Remove-Item $stagingDir -Recurse -Force
        exit 1
    }

    $manifest = Get-Content $manifestPath -Raw | ConvertFrom-Json

    Write-Info "Archive created: $($manifest.created_at)"
    Write-Info "Source OS: $($manifest.source_os)"
    Write-Host ""

    foreach ($item in $manifest.items) {
        $src = Join-Path $stagingDir $item
        $dest = Join-Path $homeDir $item

        if (Test-Path $src) {
            # Backup existing
            if (Test-Path $dest) {
                $backupName = "$dest.backup.$timestamp"
                Write-Warn "Backing up existing: $item"
                Move-Item $dest $backupName -Force
            }

            # Create parent directory
            $destParent = Split-Path $dest -Parent
            if (-not (Test-Path $destParent)) {
                New-Item -ItemType Directory -Path $destParent -Force | Out-Null
            }

            # Copy to destination
            if ((Get-Item $src).PSIsContainer) {
                Copy-Item $src $dest -Recurse -Force
            }
            else {
                Copy-Item $src $dest -Force
            }

            Write-Success "Imported: $item"
        }
    }

    # Cleanup
    Remove-Item $stagingDir -Recurse -Force

    Write-Host ""
    Write-Success "Import complete!"
    Write-Info "You may need to restart your terminal or CLI tools."
}

#------------------------------------------------------------------------------
# List Function
#------------------------------------------------------------------------------

function Invoke-List {
    $homeDir = Get-HomeDir

    Write-Host ""
    Write-Host "=== Credential Status (Windows) ===" -ForegroundColor Cyan
    Write-Host ""

    Test-GhCredentials $homeDir | Out-Null
    Test-ClaudeCredentials $homeDir | Out-Null
    Test-CodexCredentials $homeDir | Out-Null

    Write-Host ""
}

#------------------------------------------------------------------------------
# Help
#------------------------------------------------------------------------------

function Show-Help {
    @"
$ScriptName v$Version - CLI Credentials Migration Tool

USAGE:
    .\$ScriptName export [output_file]   Export credentials to archive
    .\$ScriptName import <archive_file>  Import credentials from archive
    .\$ScriptName list                   List detected credentials
    .\$ScriptName help                   Show this help

SUPPORTED TOOLS:
    - GitHub CLI (gh)     ~\.config\gh\
    - Claude Code         ~\.claude.json, ~\.claude\
    - Codex (OpenAI)      ~\.codex\

EXAMPLES:
    # Export credentials
    .\$ScriptName export
    .\$ScriptName export C:\Users\me\creds.zip

    # Import on target machine
    .\$ScriptName import .\spoq-creds-20250119.zip

    # Check what credentials exist
    .\$ScriptName list

"@
}

#------------------------------------------------------------------------------
# Main
#------------------------------------------------------------------------------

switch ($Command) {
    "export" { Invoke-Export $Path }
    "import" { Invoke-Import $Path }
    "list" { Invoke-List }
    "help" { Show-Help }
    default { Show-Help }
}
