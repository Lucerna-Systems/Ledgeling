# Installs the GitHub CLI with winget. Ledgeling runs this only when you press
# Install, in a terminal you can watch.
$ErrorActionPreference = "Stop"
if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
  Write-Host "winget was not found. Download the GitHub CLI from https://cli.github.com/ and install it."
  exit 1
}
winget install --id GitHub.cli --exact --accept-source-agreements --accept-package-agreements
Write-Host ""
Write-Host "Done. Restart Ledgeling so it finds gh, then press Sign in in the GitHub view."
