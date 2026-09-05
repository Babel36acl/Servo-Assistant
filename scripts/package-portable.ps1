$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$version = (Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json).version
$binary = Join-Path $projectRoot 'src-tauri/target/release/servo-assistant.exe'
if (-not (Test-Path -LiteralPath $binary)) { throw "缺少发布程序：$binary" }
$staging = Join-Path $projectRoot "src-tauri/target/portable-$version"
$output = Join-Path $projectRoot 'src-tauri/target/release/bundle/portable'
New-Item -ItemType Directory -Path $staging,$output -Force | Out-Null
Copy-Item -LiteralPath $binary -Destination $staging
Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination $staging
Set-Content -LiteralPath (Join-Path $staging 'portable.marker') -Value '' -NoNewline
Set-Content -LiteralPath (Join-Path $staging '使用说明.txt') -Encoding utf8 -Value '解压到可写目录，运行 servo-assistant.exe。需要 Windows WebView2 Runtime。数据保存在同目录 data 文件夹，升级时保留 data。设备 Profile 放在 profiles 文件夹，使用前请核对型号。'
$profiles = Join-Path $staging 'profiles'
New-Item -ItemType Directory -Path $profiles -Force | Out-Null
Get-ChildItem -LiteralPath (Join-Path $projectRoot 'housine') -Filter '*.profile.json' | Copy-Item -Destination $profiles
$archive = Join-Path $output "Servo-Assistant_${version}_windows_x64_portable.zip"
Compress-Archive -LiteralPath @((Join-Path $staging 'servo-assistant.exe'),(Join-Path $staging 'LICENSE'),(Join-Path $staging 'portable.marker'),(Join-Path $staging '使用说明.txt'),$profiles) -DestinationPath $archive -Force
Write-Output $archive
