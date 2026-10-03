Get-ChildItem -Path "crates" -Recurse -Filter "Cargo.toml" | ForEach-Object {
    $path = $_.FullName
    $content = Get-Content $path -Raw
    if ($content -notmatch 'license\s*=') {
        $content = $content -replace 'edition = "2021"', "edition = `"2021`"`nlicense = `"MIT`""
        Set-Content -Path $path -Value $content -NoNewline
        Write-Host "Updated: $path"
    }
}
