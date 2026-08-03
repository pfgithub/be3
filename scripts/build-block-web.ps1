<#
.SYNOPSIS
Builds block-app for the browser.

.DESCRIPTION
block-app targets wasm32-wasip1 rather than wasm32-unknown-unknown. FreeType and
HarfBuzz are C and C++, so they need a libc and a libc++ to compile against, and
wasm32-unknown-unknown has neither. wasm-bindgen understands WASI and wires the
wasi_snapshot_preview1 imports into the module's import object, which index.html
resolves with an import map.

The WASI sysroot supplies those headers and libraries. It is downloaded into
target/tools on first use unless -WasiSysroot points at an existing one.

.PARAMETER WasiSysroot
An existing WASI sysroot to build against, instead of downloading one.

.PARAMETER Release
Build with optimisations. The debug build is very large and slow to load.

.EXAMPLE
.\scripts\build-block-web.ps1 -Release
#>
[CmdletBinding()]
param(
    [string]$WasiSysroot,
    [switch]$Release
)

$ErrorActionPreference = 'Stop'

$repository = Split-Path -Parent $PSScriptRoot
$toolsDirectory = Join-Path $repository 'target/tools'
$outputDirectory = Join-Path $repository 'target/web'
$wasiSdkVersion = '33'
$wasmBindgenVersion = '0.2.122'

function Assert-Command {
    param([string]$Name, [string]$Remedy)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "$Name was not found on PATH. $Remedy"
    }
}

Assert-Command 'cargo' 'Install Rust from https://rustup.rs.'
Assert-Command 'clang' 'Install LLVM and put its bin directory on PATH.'
Assert-Command 'llvm-ar' 'Install LLVM and put its bin directory on PATH.'

$targets = & rustup target list --installed
if ($targets -notcontains 'wasm32-wasip1') {
    Write-Host 'Installing the wasm32-wasip1 Rust target...'
    & rustup target add wasm32-wasip1
    if ($LASTEXITCODE -ne 0) { throw 'failed to install the wasm32-wasip1 target' }
}

if (-not (Get-Command 'wasm-bindgen' -ErrorAction SilentlyContinue)) {
    Write-Host "Installing wasm-bindgen-cli $wasmBindgenVersion..."
    & cargo install wasm-bindgen-cli --version $wasmBindgenVersion
    if ($LASTEXITCODE -ne 0) { throw 'failed to install wasm-bindgen-cli' }
}

# The sysroot ships wasi-libc and libc++ built for wasm32. Only the sysroot is
# needed, not the whole wasi-sdk, because the local clang does the compiling.
if (-not $WasiSysroot) {
    $WasiSysroot = Join-Path $toolsDirectory 'wasi-sysroot'
    if (-not (Test-Path (Join-Path $WasiSysroot 'include'))) {
        $archive = Join-Path $toolsDirectory 'wasi-sysroot.tar.gz'
        $url = "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-$wasiSdkVersion/wasi-sysroot-$wasiSdkVersion.0+m.tar.gz"
        Write-Host "Downloading the WASI sysroot from $url..."
        New-Item -ItemType Directory -Force $toolsDirectory | Out-Null
        Invoke-WebRequest -Uri $url -OutFile $archive
        & tar -xzf $archive -C $toolsDirectory
        if ($LASTEXITCODE -ne 0) { throw 'failed to extract the WASI sysroot' }
        Move-Item (Join-Path $toolsDirectory "wasi-sysroot-$wasiSdkVersion.0+m") $WasiSysroot
        Remove-Item $archive
    }
}

if (-not (Test-Path (Join-Path $WasiSysroot 'include'))) {
    throw "no WASI sysroot at $WasiSysroot"
}
$WasiSysroot = (Resolve-Path $WasiSysroot).Path -replace '\\', '/'
Write-Host "Building against the WASI sysroot at $WasiSysroot"

# Both crates build their C through cc-rs, and freetype-sys compiles even its .c
# files with the C++ driver, so both flag sets have to work for both languages.
#   -mllvm -wasm-enable-sjlj  FreeType's rasteriser and cmap validator use
#                             setjmp/longjmp, which wasi-libc only exposes when
#                             lowered onto wasm exception handling.
#   -DHB_NO_MT                HarfBuzz has no threads to guard against here, and
#                             finds no mutex implementation for wasm otherwise.
$compilerFlags = @(
    "--sysroot=$WasiSysroot"
    "-isystem", "$WasiSysroot/include/c++/v1"
    '-mllvm', '-wasm-enable-sjlj'
    '-fno-exceptions', '-fno-rtti'
    '-DHB_NO_MT'
    '-O2', '-w'
) -join ' '

$env:CC_wasm32_wasip1 = 'clang'
$env:CXX_wasm32_wasip1 = 'clang++'
$env:AR_wasm32_wasip1 = 'llvm-ar'
$env:CFLAGS_wasm32_wasip1 = $compilerFlags
$env:CXXFLAGS_wasm32_wasip1 = $compilerFlags
# cc-rs would otherwise ask the linker for libstdc++, which the sysroot does not
# have; its C++ runtime is libc++.
$env:CXXSTDLIB_wasm32_wasip1 = 'c++'
$env:HARFBUZZ_SYS_NO_PKG_CONFIG = '1'

# rustc links wasm32-wasip1 with rust-lld and its own bundled wasi-libc, so it
# does not know where this sysroot's C++ runtime is. Only the exception-free
# variant is added to the search path: it holds libc++ and nothing that would
# shadow the libc rustc already supplies. libsetjmp, which resolves the
# __wasm_setjmp and __wasm_longjmp calls the SJLJ lowering emits for FreeType,
# is named by full path for the same reason.
$linkerSearchPath = "$WasiSysroot/lib/wasm32-wasip1/noeh"
$setjmpLibrary = "$WasiSysroot/lib/wasm32-wasip1/libsetjmp.a"
$env:RUSTFLAGS = "-C link-arg=-L$linkerSearchPath -C link-arg=$setjmpLibrary"

$profileArguments = @()
$profileDirectory = 'debug'
if ($Release) {
    $profileArguments = @('--release')
    $profileDirectory = 'release'
}

Write-Host 'Building block-app for wasm32-wasip1...'
& cargo build -p block-app --lib --target wasm32-wasip1 @profileArguments
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

$wasm = Join-Path $repository "target/wasm32-wasip1/$profileDirectory/block_app_lib.wasm"
if (-not (Test-Path $wasm)) { throw "cargo did not produce $wasm" }

Write-Host 'Generating JavaScript bindings...'
New-Item -ItemType Directory -Force $outputDirectory | Out-Null
& wasm-bindgen --target web --no-typescript --out-dir $outputDirectory $wasm
if ($LASTEXITCODE -ne 0) { throw 'wasm-bindgen failed' }

Copy-Item (Join-Path $PSScriptRoot 'web/index.html') $outputDirectory -Force
Copy-Item (Join-Path $PSScriptRoot 'web/wasi.js') $outputDirectory -Force

$size = [math]::Round((Get-Item (Join-Path $outputDirectory 'block_app_lib_bg.wasm')).Length / 1MB, 1)
Write-Host ""
Write-Host "Wrote $outputDirectory ($size MB of WebAssembly)."
Write-Host "Serve it over HTTP, for example:"
Write-Host "    python -m http.server --directory target/web 8080"
