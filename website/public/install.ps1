$ErrorActionPreference = "Stop"

$release = Invoke-RestMethod `
  -Uri "https://api.github.com/repos/sjroesink/Buddio/releases/latest" `
  -Headers @{
    "Accept" = "application/vnd.github+json"
    "User-Agent" = "Buddio-Installer"
  }

$assets = @($release.assets)
if (-not $assets -or $assets.Count -eq 0) {
  throw "No release assets found."
}

$installer = $assets |
  Where-Object { $_.name -match "\.msi$" } |
  Select-Object -First 1

if (-not $installer) {
  $installer = $assets |
    Where-Object { $_.name -match "\.exe$" } |
    Select-Object -First 1
}

if (-not $installer) {
  throw "No Windows installer asset (.msi/.exe) found."
}

$targetPath = Join-Path $env:TEMP $installer.name

Write-Host "Downloading $($installer.name)..."
Invoke-WebRequest -Uri $installer.browser_download_url -OutFile $targetPath

Write-Host "Starting installer..."
if ($targetPath.EndsWith(".msi")) {
  Start-Process "msiexec.exe" -ArgumentList "/i `"$targetPath`"" -Wait
} else {
  Start-Process -FilePath $targetPath -Wait
}

Write-Host "Buddio installer finished."
