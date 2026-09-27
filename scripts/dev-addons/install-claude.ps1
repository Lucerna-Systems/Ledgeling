# Installs Claude Code with Anthropic's official installer. Ledgeling runs this
# only when you press Install, in a terminal you can watch.
$ErrorActionPreference = 'Stop'
Write-Host "Installing Claude Code from https://claude.ai/install.ps1"
Invoke-RestMethod https://claude.ai/install.ps1 | Invoke-Expression
Write-Host ""
Write-Host "Done. Run 'claude' once in a terminal tab to sign in."
