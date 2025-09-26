# Vibe Ensemble MCP Server Installer for Windows
# This script automatically downloads and installs the latest release

param(
    [string]$InstallDir = "$env:USERPROFILE\.local\bin",
    [switch]$Force
)

$ErrorActionPreference = "Stop"

# Configuration
$Repo = "siy/vibe-ensemble-mcp"
$BinaryName = "vibe-ensemble-mcp.exe"

# Colors for output
$Colors = @{
    Info = "Cyan"
    Success = "Green"
    Warning = "Yellow"
    Error = "Red"
}

function Write-ColorText {
    param(
        [string]$Text,
        [string]$Color = "White"
    )
    Write-Host $Text -ForegroundColor $Color
}

function Write-Status {
    param([string]$Message)
    Write-ColorText "[INFO] $Message" $Colors.Info
}

function Write-Success {
    param([string]$Message)
    Write-ColorText "[SUCCESS] $Message" $Colors.Success
}

function Write-Warning {
    param([string]$Message)
    Write-ColorText "[WARNING] $Message" $Colors.Warning
}

function Write-Error {
    param([string]$Message)
    Write-ColorText "[ERROR] $Message" $Colors.Error
}

function Get-Architecture {
    $arch = $env:PROCESSOR_ARCHITECTURE
    switch ($arch) {
        "AMD64" { return "x86_64" }
        "ARM64" { return "aarch64" }
        default {
            Write-Error "Unsupported architecture: $arch"
            exit 1
        }
    }
}

function Get-LatestVersion {
    Write-Status "Fetching latest release information..."
    
    try {
        $latestUrl = "https://api.github.com/repos/$Repo/releases/latest"
        $response = Invoke-RestMethod -Uri $latestUrl -Method Get
        return $response.tag_name
    }
    catch {
        Write-Error "Failed to get latest version: $($_.Exception.Message)"
        exit 1
    }
}

function Install-Binary {
    param(
        [string]$Version,
        [string]$Architecture
    )
    
    $target = "$Architecture-pc-windows-msvc"
    $archiveName = "vibe-ensemble-mcp-$target.zip"
    $downloadUrl = "https://github.com/$Repo/releases/download/$Version/$archiveName"
    
    Write-Status "Downloading Vibe Ensemble MCP Server $Version for $target..."
    
    # Create temporary directory
    $tempDir = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }
    $archivePath = Join-Path $tempDir $archiveName
    
    try {
        # Download archive
        Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath
        
        if (-not (Test-Path $archivePath)) {
            Write-Error "Failed to download $archiveName"
            exit 1
        }
        
        Write-Status "Extracting archive..."
        Expand-Archive -Path $archivePath -DestinationPath $tempDir -Force
        
        # Find the binary
        $binaryPath = $null
        $possiblePaths = @(
            (Join-Path $tempDir $BinaryName),
            (Join-Path $tempDir "vibe-ensemble-mcp-$target" $BinaryName)
        )
        
        foreach ($path in $possiblePaths) {
            if (Test-Path $path) {
                $binaryPath = $path
                break
            }
        }
        
        if (-not $binaryPath) {
            Write-Error "Binary not found in archive"
            exit 1
        }
        
        # Create install directory
        if (-not (Test-Path $InstallDir)) {
            New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        }
        
        # Install binary
        $finalPath = Join-Path $InstallDir $BinaryName
        Write-Status "Installing to $finalPath..."
        
        if ((Test-Path $finalPath) -and -not $Force) {
            $response = Read-Host "Binary already exists. Overwrite? (y/N)"
            if ($response -ne "y" -and $response -ne "Y") {
                Write-Warning "Installation cancelled."
                return
            }
        }
        
        Copy-Item $binaryPath $finalPath -Force
        
        Write-Success "Vibe Ensemble MCP Server installed successfully!"
    }
    finally {
        # Cleanup
        if (Test-Path $tempDir) {
            Remove-Item $tempDir -Recurse -Force
        }
    }
}

function Test-PathEntry {
    $currentPath = $env:PATH -split ';'
    return $currentPath -contains $InstallDir
}

function Add-ToPath {
    if (-not (Test-PathEntry)) {
        Write-Warning "Install directory $InstallDir is not in your PATH."
        Write-Warning "Adding to PATH for current session..."
        
        $env:PATH = "$InstallDir;$env:PATH"
        
        Write-Warning "To make this permanent, add the following to your PowerShell profile:"
        Write-Host ""
        Write-Host "    `$env:PATH = `"$InstallDir;`$env:PATH`""
        Write-Host ""
        Write-Warning "Or run this script as Administrator to modify system PATH."
    }
}

function Test-Prerequisites {
    # Check PowerShell version
    if ($PSVersionTable.PSVersion.Major -lt 5) {
        Write-Error "PowerShell 5.0 or later is required"
        exit 1
    }
    
    # Check if we can create directories
    try {
        $testDir = Join-Path $InstallDir "test"
        New-Item -ItemType Directory -Path $testDir -Force | Out-Null
        Remove-Item $testDir -Force
    }
    catch {
        Write-Error "Cannot create directory $InstallDir. Check permissions."
        exit 1
    }
}

function Main {
    Write-Host ""
    Write-Status "Vibe Ensemble MCP Server Installer for Windows"
    Write-Host ""
    
    # Check prerequisites
    Test-Prerequisites
    
    # Detect system
    $arch = Get-Architecture
    $version = Get-LatestVersion
    
    Write-Status "Detected architecture: $arch"
    Write-Status "Latest version: $version"
    Write-Status "Install directory: $InstallDir"
    
    # Install
    Install-Binary -Version $version -Architecture $arch
    
    # Update PATH
    Add-ToPath
    
    # Success message
    Write-Host ""
    Write-Success "Installation complete!"
    Write-Status "You can now run: $BinaryName --help"
    Write-Status "To start the server: $BinaryName --port 3000"
    Write-Host ""
    Write-Status "Documentation: https://github.com/$Repo"
    Write-Host ""
}

# Run main function
try {
    Main
}
catch {
    Write-Error "Installation failed: $($_.Exception.Message)"
    exit 1
}