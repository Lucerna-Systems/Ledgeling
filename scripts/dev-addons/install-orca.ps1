# Orca is a desktop app whose `orca` command is installed from inside the app.
# This opens its official download page; nothing is downloaded by Ledgeling.
$Url = "https://github.com/stablyai/orca/releases/latest"
Write-Host "Opening $Url"
Write-Host "Install Orca, then turn on its command-line tool from Orca's settings."
Start-Process $Url
