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
Set-Content -LiteralPath (Join-Path $staging '使用说明.txt') -Encoding utf8 -Value '解压到可写目录，运行 servo-assistant.exe。需要 Windows WebView2 Runtime。数据保存在同目录 data 文件夹，升级时保留 data。请自行导入与设备型号匹配的 Profile。'
$archive = Join-Path $output "Servo-Assistant_${version}_windows_x64_portable.zip"
Compress-Archive -LiteralPath @((Join-Path $staging 'servo-assistant.exe'),(Join-Path $staging 'LICENSE'),(Join-Path $staging 'portable.marker'),(Join-Path $staging '使用说明.txt')) -DestinationPath $archive -Force
Write-Output $archive
