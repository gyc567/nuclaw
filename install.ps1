# NuClaw Cross-Platform Installer (PowerShell)
# Supports: Windows 10/11, Windows Server

param(
    [string]$Version = "latest",
    [string]$InstallPath = ""
)

$ErrorActionPreference = "Stop"

# Configuration
$Repo = "gyc567/nuclaw"
$DefaultInstallPath = if ($env:NUCLAW_HOME) { $env:NUCLAW_HOME } elseif ($env:USERPROFILE) { "$env:USERPROFILE\.nuclaw" } else { "$env:HOME\.nuclaw" }

if (-not $InstallPath) { $InstallPath = $DefaultInstallPath }

# Colors for output
function Write-ColorOutput {
    param([string]$Message, [string]$Color = "White")
    $colors = @{
        Red = [ConsoleColor]::Red
        Green = [ConsoleColor]::Green
        Yellow = [ConsoleColor]::Yellow
        Cyan = [ConsoleColor]::Cyan
        White = [ConsoleColor]::White
    }
    Write-Host $Message -ForegroundColor $colors[$Color]
}

function Detect-Platform {
    $arch = $env:PROCESSOR_ARCHITECTURE
    if ($arch -eq "AMD64") { $arch = "x86_64" }
    elseif ($arch -eq "ARM64") { $arch = "arm64" }
    
    $os = "windows"
    return @{ OS = $os; Arch = $arch }
}

function Get-LatestVersion {
    $url = "https://api.github.com/repos/$Repo/releases/latest"
    try {
        $response = Invoke-RestMethod -Uri $url -UseBasicParsing
        return $response.tag_name -replace '^v', ''
    } catch {
        Write-ColorOutput "Could not fetch latest version" Yellow
        return ""
    }
}

function Get-DownloadUrl {
    param($Version, $Arch)
    
    $filename = "nuclaw-$Version-$Arch-pc-windows-msvc.zip"
    return "https://github.com/$Repo/releases/download/v$Version/$filename"
}

function Get-TempPath {
    return if ($env:TEMP) { $env:TEMP } else { "C:\Temp" }
}

function Install-Binary {
    param($Version, $Arch)
    
    $url = Get-DownloadUrl -Version $Version -Arch $Arch
    $temp = Get-TempPath
    $output = "$temp\nuclaw.zip"
    
    Write-ColorOutput "Downloading NuClaw v$Version for Windows-$Arch..." Cyan
    
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $url -OutFile $output -UseBasicParsing
    } catch {
        Write-ColorOutput "Download failed, trying source build..." Yellow
        return $false
    }
    
    Write-ColorOutput "Installing to $InstallPath..." Cyan
    
    # Create directory
    if (-not (Test-Path $InstallPath)) {
        New-Item -ItemType Directory -Path $InstallPath -Force | Out-Null
    }
    
    # Extract
    Expand-Archive -Path $output -DestinationPath $InstallPath -Force
    Remove-Item $output -Force
    
    return $true
}

function Setup-Directories {
    $dirs = @("store", "data", "groups", "logs", "skills")
    foreach ($dir in $dirs) {
        $path = Join-Path $InstallPath $dir
        if (-not (Test-Path $path)) {
            New-Item -ItemType Directory -Path $path -Force | Out-Null
        }
    }
    Write-ColorOutput "Directories created" Green
}

function New-ConfigFile {
    $configPath = Join-Path $InstallPath "config.json"
    
    if (-not (Test-Path $configPath)) {
        $config = @{
            version = "1.0.0"
            settings = @{
                assistant_name = "Andy"
                timezone = "UTC"
                container_timeout_ms = 300000
            }
        } | ConvertTo-Json -Depth 2
        
        Set-Content -Path $configPath -Value $config -Encoding UTF8
        Write-ColorOutput "Config created: $configPath" Green
    }
}

function New-EnvTemplate {
    $envPath = Join-Path $InstallPath ".env.example"
    $envActual = Join-Path $InstallPath ".env"
    
    if (-not (Test-Path $envActual)) {
        $template = @"
# NuClaw Configuration Template
# Copy this file to .env and fill in your values

# LLM Provider Configuration
ANTHROPIC_API_KEY=your-api-key-here
ANTHROPIC_BASE_URL=https://api.anthropic.com

# Telegram Bot (optional)
TELEGRAM_BOT_TOKEN=your-bot-token-here
"@
        Set-Content -Path $envPath -Value $template -Encoding UTF8
        Write-ColorOutput "Env template created: $envPath" Green
    }
}

function Install-Service {
    # Create a simple Windows service runner script
    $serviceScript = Join-Path $InstallPath "nuclaw-service.ps1"
    
    $script = @"
`$ErrorActionPreference = "Stop"
`$env:NUCLAW_HOME = "$InstallPath"
& "$InstallPath\nuclaw.exe"
"@
    
    Set-Content -Path $serviceScript -Value $script -Encoding UTF8
    
    # Create scheduled task for auto-start
    $taskName = "NuClaw"
    $action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-ExecutionPolicy Bypass -File `"$serviceScript`""
    $trigger = New-ScheduledTaskTrigger -AtLogOn
    
    try {
        Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Force | Out-Null
        Write-ColorOutput "Scheduled task created: $taskName" Green
    } catch {
        Write-ColorOutput "Could not create scheduled task (need admin): $_" Yellow
    }
}

function Build-FromSource {
    Write-ColorOutput "Building from source..." Cyan
    
    # Check Rust
    $rustInstalled = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $rustInstalled) {
        Write-ColorOutput "Installing Rust..." Cyan
        Invoke-WebRequest -Uri "https://win.rustup.rs" -OutFile "rustup-init.exe" -UseBasicParsing
        Start-Process -FilePath ".\rustup-init.exe" -ArgumentList "-y" -Wait
        $env:PATH += ";$env:USERPROFILE\.cargo\bin"
    }
    
    # Clone and build
    $temp = Get-TempPath
    $buildDir = "$temp\nuclaw-build"
    
    if (Test-Path $buildDir) { Remove-Item $buildDir -Recurse -Force }
    
    git clone --depth 1 "https://github.com/$Repo.git" $buildDir
    
    Push-Location $buildDir
    cargo build --release
    Pop-Location
    
    # Copy binary
    if (-not (Test-Path $InstallPath)) {
        New-Item -ItemType Directory -Path $InstallPath -Force | Out-Null
    }
    Copy-Item "$buildDir\target\release\nuclaw.exe" $InstallPath -Force
    
    Remove-Item $buildDir -Recurse -Force
    Write-ColorOutput "Built and installed from source" Green
}

# Main
Write-ColorOutput ""
Write-ColorOutput "==========================================" Cyan
Write-ColorOutput "  NuClaw Installer - Windows" Cyan
Write-ColorOutput "  Rust-powered AI Assistant" Cyan
Write-ColorOutput "==========================================" Cyan
Write-ColorOutput ""

# Detect platform
$platform = Detect-Platform
$os = $platform.OS
$arch = $platform.Arch

Write-ColorOutput "Platform: Windows-$arch" White
Write-ColorOutput "Install path: $InstallPath" White
Write-ColorOutput ""

# Get version
if ($Version -eq "latest") {
    $Version = Get-LatestVersion
    if (-not $Version) {
        Write-ColorOutput "Could not determine latest version" Yellow
        Build-FromSource
        Setup-Directories
        New-ConfigFile
        New-EnvTemplate
        return
    }
}

Write-ColorOutput "Target version: v$Version" White
Write-ColorOutput ""

# Try download
$success = Install-Binary -Version $Version -Arch $arch

if (-not $success) {
    Write-ColorOutput "Pre-built binary not available" Yellow
    Build-FromSource
}

# Setup
Setup-Directories
New-ConfigFile
New-EnvTemplate
Install-Service

# Show usage
Write-ColorOutput ""
Write-ColorOutput "==========================================" Green
Write-ColorOutput "  Installation Complete!" Green
Write-ColorOutput "==========================================" Green
Write-ColorOutput ""
Write-ColorOutput "Location: $InstallPath\nuclaw.exe"
Write-ColorOutput ""
Write-ColorOutput "Quick start:"
Write-ColorOutput "  1. Copy .env.example to .env and add your API keys"
Write-ColorOutput "  2. Run: $InstallPath\nuclaw.exe --onboard"
Write-ColorOutput ""
