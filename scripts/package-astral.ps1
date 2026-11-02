param([switch]$SkipBuild, [switch]$IncludeEncoder)
$ErrorActionPreference = 'Stop'
function Copy-AstralExecutable([string]$Source, [string]$Destination) {
    # Refreshing documentation must work while this exact build is running.
    # A different executable still fails normally if Windows has it locked.
    if ((Test-Path -LiteralPath $Destination -PathType Leaf) -and
        ((Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash -eq
         (Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash)) { return }
    Copy-Item -LiteralPath $Source -Destination $Destination
}
$astralRoot = Split-Path -Parent $PSScriptRoot
Push-Location $astralRoot
try {
    if (-not $SkipBuild) {
        cargo build --release -p astral --locked
        if ($LASTEXITCODE -ne 0) { throw 'Astral release build failed' }
    }
    $astralBinary = Join-Path $astralRoot 'target/release/astral.exe'
    if (-not (Test-Path -LiteralPath $astralBinary)) { throw 'Release executable is missing' }
    $astralMetadata = cargo metadata --locked --offline --format-version 1 --filter-platform x86_64-pc-windows-msvc | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Unable to collect dependency notices' }
    $astralVersion = ($astralMetadata.packages | Where-Object name -eq 'astral').version
    $astralPackage = Join-Path $astralRoot "release/Astral-$astralVersion-windows-x64"
    New-Item -ItemType Directory -Force -Path $astralPackage | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $astralPackage 'resources') | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $astralPackage 'docs') | Out-Null
    Copy-AstralExecutable $astralBinary (Join-Path $astralPackage 'astral.exe')
    Copy-AstralExecutable (Join-Path $astralRoot 'target/release/astral-studio.exe') (Join-Path $astralPackage 'Astral-Studio.exe')
    Copy-Item -LiteralPath (Join-Path $astralRoot 'README.md') -Destination $astralPackage
    Copy-Item -LiteralPath (Join-Path $astralRoot 'COPYING') -Destination $astralPackage
    # Ship compatibility helper sources; Python/Node remain optional external runtimes.
    foreach ($astralFolder in @('backend','codecs','src','vendor')) {
        Get-ChildItem -LiteralPath (Join-Path $astralRoot $astralFolder) -Recurse -File | Where-Object { $_.FullName -notmatch '[\\/]__pycache__[\\/]' -and $_.Extension -ne '.pyc' } | ForEach-Object {
            $astralRelative = [System.IO.Path]::GetRelativePath($astralRoot, $_.FullName)
            $astralDestination = Join-Path $astralPackage $astralRelative
            New-Item -ItemType Directory -Force -Path (Split-Path -Parent $astralDestination) | Out-Null
            Copy-Item -LiteralPath $_.FullName -Destination $astralDestination
        }
    }
    New-Item -ItemType Directory -Force -Path (Join-Path $astralPackage 'scripts') | Out-Null
    foreach ($astralBridge in @('astral-codec.py','astral-sticknodes.mjs','astral-evaluate-legacy.mjs','astral-decode-container.mjs','astral-source-input.mjs','astral-psd.mjs')) { Copy-Item -LiteralPath (Join-Path $astralRoot "scripts/$astralBridge") -Destination (Join-Path $astralPackage 'scripts') }

    Get-ChildItem -LiteralPath (Join-Path $astralRoot 'docs') -Filter 'ASTRAL-*' -File | Where-Object Extension -In @('.md','.json') | Copy-Item -Destination (Join-Path $astralPackage 'docs')
    $astralGateFolder = Join-Path $astralPackage 'platforms/release'
    New-Item -ItemType Directory -Force -Path $astralGateFolder | Out-Null
    Get-ChildItem -LiteralPath (Join-Path $astralRoot 'platforms/release') -Filter '*.json' -File | Copy-Item -Destination $astralGateFolder
    # Offline README gallery and editable studies, excluding runtime history and benchmark outputs.
    $astralShowcase = Join-Path $astralRoot 'docs/showcase'
    Get-ChildItem -LiteralPath $astralShowcase -Recurse -File | Where-Object { $_.Extension -in @('.md','.astral','.gif','.png','.wav') -and $_.FullName -notmatch '[\\/](recovery|renders|\.astral-history)[\\/]' } | ForEach-Object {
        $astralDemoRelative = [System.IO.Path]::GetRelativePath($astralShowcase, $_.FullName)
        $astralDemoTarget = Join-Path $astralPackage (Join-Path 'docs/showcase' $astralDemoRelative)
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $astralDemoTarget) | Out-Null
        Copy-Item -LiteralPath $_.FullName -Destination $astralDemoTarget
    }
    $astralModels = Join-Path $astralPackage 'models'
    New-Item -ItemType Directory -Force -Path $astralModels | Out-Null
    Get-ChildItem -LiteralPath (Join-Path $astralRoot 'models') -File | Copy-Item -Destination $astralModels
    Copy-Item -LiteralPath (Join-Path $astralRoot 'mcp.example.json') -Destination (Join-Path $astralPackage 'mcp-servers.json')
    foreach ($astralDoc in @('AI_PROVENANCE.md','CREDITS.md','THIRD_PARTY_NOTICES.md','CONTRIBUTING.md')) { Copy-Item -LiteralPath (Join-Path $astralRoot $astralDoc) -Destination $astralPackage }
    Copy-Item -LiteralPath (Join-Path $astralRoot 'assets/brand/astral.ico') -Destination $astralPackage
    $astralBrandDestination = Join-Path $astralPackage 'assets/brand'
    New-Item -ItemType Directory -Force -Path $astralBrandDestination | Out-Null
    Get-ChildItem -LiteralPath (Join-Path $astralRoot 'assets/brand') -File | Where-Object Extension -In @('.svg','.png','.ico','.txt') | Copy-Item -Destination $astralBrandDestination

    $astralNodeRuntime = Join-Path $astralRoot 'build/astral-node'
    if (-not (Test-Path -LiteralPath (Join-Path $astralNodeRuntime 'node.exe'))) { throw 'Stage parser runtime with python scripts/stage-astral-node.py before packaging.' }
    $astralNodeTarget = Join-Path $astralPackage 'resources/node'
    New-Item -ItemType Directory -Force -Path $astralNodeTarget | Out-Null
    Get-ChildItem -LiteralPath $astralNodeRuntime -File | Copy-Item -Destination $astralNodeTarget
    $astralSpeech = Join-Path $astralRoot 'build/rhubarb'
    if (-not (Test-Path -LiteralPath (Join-Path $astralSpeech 'rhubarb.exe'))) { throw 'Stage the pinned offline recognizer with python scripts/stage-astral-speech.py before packaging.' }
    $astralSpeechTarget = Join-Path $astralPackage 'resources/rhubarb'
    New-Item -ItemType Directory -Force -Path $astralSpeechTarget | Out-Null
    Get-ChildItem -LiteralPath $astralSpeech -Recurse -File | ForEach-Object {
        $astralSpeechRelative = [System.IO.Path]::GetRelativePath($astralSpeech, $_.FullName)
        $astralSpeechDestination = Join-Path $astralSpeechTarget $astralSpeechRelative
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $astralSpeechDestination) | Out-Null
        Copy-Item -LiteralPath $_.FullName -Destination $astralSpeechDestination
    }
    $astralNotices = Join-Path $astralPackage 'THIRD_PARTY'
    New-Item -ItemType Directory -Force -Path $astralNotices | Out-Null
    Copy-Item -LiteralPath (Join-Path $astralRoot 'platforms/windows/INNO-LICENSE.txt') -Destination (Join-Path $astralNotices 'InnoSetup-LICENSE.txt')
    Copy-Item -LiteralPath (Join-Path $astralRoot 'vendor/text/OFL.txt') -Destination (Join-Path $astralNotices 'NotoSans-OFL.txt')
    $astralMetadata.packages | Select-Object name,version,license,source | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $astralNotices 'dependencies.json')
    foreach ($astralDependency in $astralMetadata.packages) {
        $astralDependencyRoot = Split-Path -Parent $astralDependency.manifest_path
        $astralLicenseFiles = Get-ChildItem -LiteralPath $astralDependencyRoot -File | Where-Object { $_.Name -match '^(LICENSE|COPYING|NOTICE)' }
        if ($astralLicenseFiles) {
            $astralLicenseDestination = Join-Path $astralNotices "$($astralDependency.name)-$($astralDependency.version)"
            New-Item -ItemType Directory -Force -Path $astralLicenseDestination | Out-Null
            $astralLicenseFiles | Copy-Item -Destination $astralLicenseDestination
        }
        if ($astralDependency.name -eq 'epaint_default_fonts') {
            $astralFontNotices = Join-Path $astralNotices 'egui-fonts'
            New-Item -ItemType Directory -Force -Path $astralFontNotices | Out-Null
            Get-ChildItem -LiteralPath $astralDependencyRoot -Recurse -File | Where-Object { $_.Name -match '(LICENSE|COPYING|NOTICE|OFL)' } | Copy-Item -Destination $astralFontNotices
        }
    }
    $astralEncoder = Join-Path $astralRoot 'build/video-encoder/ffmpeg.exe'
    if ($IncludeEncoder -and (Test-Path -LiteralPath $astralEncoder)) {
        Copy-Item -LiteralPath $astralEncoder -Destination (Join-Path $astralPackage 'resources/ffmpeg.exe')
        Get-ChildItem -LiteralPath (Join-Path $astralRoot 'build/video-encoder') -File | Where-Object Name -ne 'ffmpeg.exe' | Copy-Item -Destination (Join-Path $astralPackage 'resources')
    } else { Write-Output 'Encoder not redistributed; select a separately installed FFmpeg in Settings for video/audio media export.' }
    $astralFileHashes = Get-ChildItem -LiteralPath $astralPackage -Recurse -File | Where-Object Name -ne 'SHA256SUMS.txt' | Sort-Object FullName | ForEach-Object {
        $astralRelative = [System.IO.Path]::GetRelativePath($astralPackage, $_.FullName).Replace('\', '/')
        "$((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant())  $astralRelative"
    }
    $astralFileHashes | Set-Content -LiteralPath (Join-Path $astralPackage 'SHA256SUMS.txt') -Encoding utf8
    $astralArchive = "$astralPackage.zip"
    Get-ChildItem -LiteralPath $astralPackage -Recurse -File | Where-Object { $_.LastWriteTime.Year -lt 1980 } | ForEach-Object { $_.LastWriteTime = [datetime]'1980-01-01T00:00:00' }
    Compress-Archive -LiteralPath $astralPackage -DestinationPath $astralArchive -Force
    "$((Get-FileHash -LiteralPath $astralArchive -Algorithm SHA256).Hash.ToLowerInvariant())  $([System.IO.Path]::GetFileName($astralArchive))" | Set-Content -LiteralPath "$astralArchive.sha256" -Encoding utf8
    Write-Output $astralArchive
} finally { Pop-Location }
