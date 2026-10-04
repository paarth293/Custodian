Write-Host "Mock fetching tools..."
Set-Content -Path "tools\tools.toml" -Value "mock = true"
Write-Host "Tools fetched and pinned."
