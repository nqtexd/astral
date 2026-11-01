$ErrorActionPreference = 'Stop'
$astralWorkspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$astralBuildCache = Join-Path $astralWorkspace 'target'
if (Get-Process cargo,rustc,link -ErrorAction SilentlyContinue) {
    throw 'Finish or stop active Rust builds before clearing the build cache.'
}
if (Test-Path -LiteralPath $astralBuildCache) {
    $astralResolvedCache = (Resolve-Path -LiteralPath $astralBuildCache).ProviderPath
    if ($astralResolvedCache -ne [System.IO.Path]::GetFullPath($astralBuildCache) -or
        (Split-Path -Parent $astralResolvedCache) -ne $astralWorkspace -or
        ((Get-Item -LiteralPath $astralResolvedCache).Attributes -band [System.IO.FileAttributes]::ReparsePoint)) {
        throw 'Refusing to remove a cache outside this workspace or through a link.'
    }
    cargo clean --manifest-path (Join-Path $astralWorkspace 'Cargo.toml') --target-dir $astralResolvedCache
    if ($LASTEXITCODE -ne 0) { throw 'Cargo could not clear the build cache.' }
}
Write-Output 'Rust build cache cleared. Source, projects, assets and portable releases are retained.'
