param([switch]$Apply, [string]$Version)
$ErrorActionPreference = 'Stop'
if (-not $Apply) {
    $base = [version](Get-Content package.json -Raw | ConvertFrom-Json).version
    $tags = @(git tag --list 'v*')
    if ($LASTEXITCODE -ne 0) { throw 'Cannot list release tags' }
    $sameCommitTags = @(git tag --points-at HEAD)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect source tags' }
    $existing = @($sameCommitTags | Where-Object { $_ -match '^v\d+\.\d+\.\d+$' } | Sort-Object { [version]$_.Substring(1) } -Descending)
    if ($existing.Count) { $Version = $existing[0].Substring(1) }
    else {
        $next = $base
        foreach ($tag in $tags) {
            if ($tag -match '^v\d+\.\d+\.\d+$') {
                $used = [version]$tag.Substring(1)
                if ($used -ge $next) { $next = [version]::new($used.Major, $used.Minor, $used.Build + 1) }
            }
        }
        $Version = $next.ToString()
    }
    "version=$Version" >> $env:GITHUB_OUTPUT
    "tag=v$Version" >> $env:GITHUB_OUTPUT
    Write-Output "Release v$Version from $(git rev-parse HEAD)"
    exit 0
}
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'Expected a stable major.minor.patch version' }
foreach ($path in @('package.json', 'src-tauri/tauri.conf.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock')) {
    $text = [IO.File]::ReadAllText((Join-Path (Get-Location) $path))
    if ($path -like '*.json') {
        $text = [regex]::Replace($text, '("version"\s*:\s*")[^"]+', "`${1}$Version")
    } elseif ($path -like '*.lock') {
        $text = [regex]::Replace($text, '(name = "servo-assistant"\r?\nversion = ")[^"]+', "`${1}$Version")
    } else {
        $text = [regex]::Replace($text, '(?m)^(version = ")[^"]+', "`${1}$Version")
    }
    [IO.File]::WriteAllText((Join-Path (Get-Location) $path), $text)
}
