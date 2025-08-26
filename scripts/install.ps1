# Vibe Ensemble MCP Installation Script for Windows PowerShell
# Usage: iex "& { irm https://vibeensemble.dev/install.ps1 }"

param(
    [string]$Version = "latest",
    [string]$InstallDir = "$env:ProgramFiles\ViberEnsemble",
    [switch]$NoService = $false
)

$ErrorActionPreference = "Stop"

# Configuration
$Repo = "siy/vibe-ensemble-mcp"
$ServiceName = "ViberEnsembleMCP"

function Write-ColorOutput {
    param(
        [string]$Message,
        [string]$Color = "Green"
    )
    
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] $Message" -ForegroundColor $Color
}

function Write-Info {
    param([string]$Message)
    Write-ColorOutput $Message "Green"
}

function Write-Warn {
    param([string]$Message)
    Write-ColorOutput $Message "Yellow"
}

function Fail {
    param([string]$Message)
    Write-ColorOutput $Message "Red"
    exit 1
}

function Test-Administrator {
    $currentPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    return $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Get-Architecture {
    $arch = $env:PROCESSOR_ARCHITECTURE
    switch ($arch) {
        "AMD64" { return "x86_64-pc-windows-msvc" }
        "ARM64" { return "aarch64-pc-windows-msvc" }
        default { Fail "Unsupported architecture: $arch" }
    }
}

function Get-LatestVersion {
    if ($Version -eq "latest") {
        try {
            $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Method Get
            return $response.tag_name
        }
        catch {
            Fail "Failed to get latest version: $_"
        }
    }
    return $Version
}

function Download-AndExtract {
    param(
        [string]$Url,
        [string]$DestinationPath
    )
    
    $tempFile = [System.IO.Path]::GetTempFileName() + ".zip"
    
    try {
        Write-Info "Downloading from $Url..."
        # Use TLS 1.2+ and verify certificates
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $Url -OutFile $tempFile -UseBasicParsing
        
        Write-Info "Extracting to $DestinationPath..."
        Expand-Archive -Path $tempFile -DestinationPath $DestinationPath -Force
    }
    finally {
        if (Test-Path $tempFile) {
            Remove-Item $tempFile -Force
        }
    }
}

function Install-Binaries {
    param(
        [string]$Version,
        [string]$Target
    )
    
    $filename = "vibe-ensemble-mcp-$Version-windows-$Target.zip"
    $url = "https://github.com/$Repo/releases/download/$Version/$filename"
    
    # Create installation directory
    if (!(Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    
    # Verify URL is HTTPS
    if (-not $url.StartsWith("https://")) {
        Fail "Only HTTPS URLs are allowed for security"
    }
    
    # Download and extract
    Download-AndExtract -Url $url -DestinationPath $InstallDir
    
    # Verify binaries exist
    $serverPath = "$InstallDir\vibe-ensemble.exe"
    $mcpPath = "$InstallDir\vibe-ensemble-mcp.exe"
    if (!(Test-Path $serverPath) -or !(Test-Path $mcpPath)) {
        Fail "Expected binaries not found after extraction"
    }
    
    # Add to PATH if not already there
    $currentPath = [Environment]::GetEnvironmentVariable("PATH", "Machine")
    if ($currentPath -notlike "*$InstallDir*") {
        Write-Info "Adding $InstallDir to system PATH..."
        [Environment]::SetEnvironmentVariable("PATH", "$currentPath;$InstallDir", "Machine")
        $env:PATH = "$env:PATH;$InstallDir"
    }
    
    Write-Info "Binaries installed to $InstallDir"
}

function Create-Configuration {
    $configDir = "$env:APPDATA\vibe-ensemble"
    $configFile = "$configDir\config.toml"
    
    if (!(Test-Path $configDir)) {
        New-Item -ItemType Directory -Path $configDir -Force | Out-Null
    }
    
    if (!(Test-Path $configFile)) {
        $configContent = @"
[server]
host = "127.0.0.1"
port = 8080

[database]
url = "sqlite:./vibe_ensemble.db"
migrate_on_startup = true

[web]
enabled = true
host = "127.0.0.1"
port = 8081

[logging]
level = "info"
format = "json"
"@
        
        Set-Content -Path $configFile -Value $configContent -Encoding UTF8
        Write-Info "Configuration created at $configFile"
    }
    else {
        Write-Info "Configuration already exists at $configFile"
    }
    
    return $configFile
}

function Install-WindowsService {
    param([string]$ConfigPath)
    
    if ($NoService) {
        Write-Info "Skipping service installation (NoService flag set)"
        return
    }
    
    $servicePath = "$InstallDir\vibe-ensemble.exe"
    $serviceArgs = "--config `"$ConfigPath`""
    
    # Check if service already exists
    $existingService = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($existingService) {
        Write-Info "Service '$ServiceName' already exists, updating..."
        Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
        sc.exe delete $ServiceName | Out-Null
    }
    
    # Create service
    Write-Info "Installing Windows service '$ServiceName'..."
    $result = sc.exe create $ServiceName binPath= "`"$servicePath`" $serviceArgs" start= auto DisplayName= "Vibe Ensemble MCP Server"
    
    if ($LASTEXITCODE -eq 0) {
        Write-Info "Service installed successfully"
        Write-Info "You can start the service with: Start-Service -Name $ServiceName"
    }
    else {
        Write-Warn "Failed to install service: $result"
        Write-Info "You can still run the server manually: vibe-ensemble.exe"
    }
}

function Test-Installation {
    $serverPath = "$InstallDir\vibe-ensemble.exe"
    if (!(Test-Path $serverPath)) {
        Fail "Installation failed: vibe-ensemble.exe not found"
    }
    if (Test-Path "$InstallDir\vibe-ensemble-mcp.exe" -or Test-Path "$InstallDir\vibe-ensemble-mcp.cmd") {
        Write-Info "legacy alias 'vibe-ensemble-mcp' is available"
    } else {
        Write-Info "legacy alias 'vibe-ensemble-mcp' not found (expected with unified binary)"
    }
    
    Write-Info "Installation verified successfully!"
}

function Show-PostInstallInstructions {
    Write-Host ""
    Write-Host "=== Installation Complete ===" -ForegroundColor Blue
    Write-Host ""
    Write-Host "Vibe Ensemble MCP has been installed successfully!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Next steps:"
    Write-Host "1. Start the server:"
    Write-Host "   vibe-ensemble.exe" -ForegroundColor Cyan
    Write-Host ""
    if (!$NoService) {
        Write-Host "2. Or start as a Windows service:"
        Write-Host "   Start-Service -Name $ServiceName" -ForegroundColor Cyan
        Write-Host ""
    }
    Write-Host "3. Access the web dashboard:"
    Write-Host "   http://localhost:8081" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "4. Add to Claude Code (choose one):"
    Write-Host "   # Local scope (current project only)" -ForegroundColor Green
    Write-Host "   claude mcp add vibe-ensemble -- vibe-ensemble --mcp-only --transport=stdio" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "   # User scope (all projects)" -ForegroundColor Green
    Write-Host "   claude mcp add -s user vibe-ensemble -- vibe-ensemble --mcp-only --transport=stdio" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "   # Project scope (shared with team)" -ForegroundColor Green
    Write-Host "   claude mcp add -s project vibe-ensemble -- vibe-ensemble --mcp-only --transport=stdio" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "   # HTTP transport (server already running on 8080)" -ForegroundColor Green
    Write-Host "   claude mcp add --transport http vibe-ensemble http://localhost:8080/mcp" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "   # SSE transport (event stream monitoring)" -ForegroundColor Green
    Write-Host "   claude mcp add --transport sse vibe-ensemble http://localhost:8080/mcp/events" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "5. Check the API:"
    Write-Host "   Invoke-RestMethod http://localhost:8080/api/health" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Installation directory: $InstallDir" -ForegroundColor Yellow
    Write-Host "Configuration: $env:APPDATA\vibe-ensemble\config.toml" -ForegroundColor Yellow
    Write-Host "Documentation: https://github.com/$Repo/blob/main/docs/installation.md" -ForegroundColor Yellow
    Write-Host ""
}

# Main installation process
function Main {
    Write-Host "=== Vibe Ensemble MCP Installer ===" -ForegroundColor Blue
    Write-Host ""
    
    # Check if running as administrator
    if (!(Test-Administrator)) {
        Fail "This installer must be run as Administrator. Please run PowerShell as Administrator and try again."
    }
    
    # Detect platform
    $target = Get-Architecture
    Write-Info "Detected platform: Windows ($target)"
    
    # Get version
    $version = Get-LatestVersion
    Write-Info "Installing version: $version"
    
    # Install binaries
    Install-Binaries -Version $version -Target $target
    
    # Create configuration
    $configPath = Create-Configuration
    
    # Install Windows service
    Install-WindowsService -ConfigPath $configPath
    
    # Verify installation
    Test-Installation
    
    # Show instructions
    Show-PostInstallInstructions
}

# Handle script interruption
trap {
    Fail "Installation interrupted: $_"
}

# Run main installation
Main