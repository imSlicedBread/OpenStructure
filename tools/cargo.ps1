# Convenience launcher for either system Rust or this workspace's local toolchain.
$CargoArgs = $args
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$localCargo = Join-Path $projectRoot 'work\toolchain\cargo\bin\cargo.exe'
if (Test-Path -LiteralPath $localCargo) {
    $env:CARGO_HOME = Join-Path $projectRoot 'work\toolchain\cargo'
    $env:RUSTUP_HOME = Join-Path $projectRoot 'work\toolchain\rustup'
    $env:PATH = "$(Split-Path $localCargo);$env:PATH"
    $cargoExecutable = $localCargo
} else {
    $cargoExecutable = (Get-Command cargo -ErrorAction Stop).Source
}
# Discover compiler payloads even if Visual Studio's installer is incomplete.
if ($IsWindows -or $env:OS -eq 'Windows_NT') {
    $vswherePath = 'C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path -LiteralPath $vswherePath) {
        $installations = & $vswherePath -all -products '*' -format json | ConvertFrom-Json
        foreach ($installation in $installations) {
            $compilerRoot = Join-Path $installation.installationPath 'VC\Tools\MSVC'
            if (!(Test-Path -LiteralPath $compilerRoot)) { continue }
            $compiler = Get-ChildItem -LiteralPath $compilerRoot -Directory | Sort-Object Name -Descending | Select-Object -First 1
            $compilerBin = Join-Path $compiler.FullName 'bin\Hostx64\x64'
            if (!(Test-Path -LiteralPath (Join-Path $compilerBin 'link.exe'))) { continue }
            $sdkRoot = 'C:\Program Files (x86)\Windows Kits\10'
            $sdk = Get-ChildItem -LiteralPath (Join-Path $sdkRoot 'Lib') -Directory | Sort-Object Name -Descending | Select-Object -First 1
            $env:PATH = "$compilerBin;$env:PATH"
            $env:LIB = "$(Join-Path $compiler.FullName 'lib\x64');$(Join-Path $sdk.FullName 'ucrt\x64');$(Join-Path $sdk.FullName 'um\x64')"
            $sdkInclude = Join-Path $sdkRoot "Include\$($sdk.Name)"
            $env:INCLUDE = "$(Join-Path $compiler.FullName 'include');$sdkInclude\ucrt;$sdkInclude\shared;$sdkInclude\um"
            break
        }
    }
}
Push-Location $projectRoot
try { & $cargoExecutable @CargoArgs; $resultCode = $LASTEXITCODE } finally { Pop-Location }
exit $resultCode
