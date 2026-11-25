# micro-rs 一键打包发布脚本（Windows）
# 用法：.\scripts\release.ps1 [-Version v0.1.0] [-SkipBuild] [-NoUpload] [-Repo owner/repo]

param(
    [string]$Version = "",
    [string]$Repo = "zeno-ling/micro-rs",
    [string]$MingwBin = "",
    [switch]$SkipBuild,
    [switch]$NoUpload
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

function Fail($msg) { Write-Error $msg; exit 1 }

Push-Location $root
try {
    # 1) 读取版本号（Cargo.toml 中 package 段的 version）
    if (-not $Version) {
        $cargo = Get-Content (Join-Path $root "Cargo.toml") -Raw
        $m = [regex]::Match($cargo, '(?m)^version\s*=\s*"([^"]+)"')
        if (-not $m.Success) { Fail "无法从 Cargo.toml 读取版本号，请用 -Version 指定" }
        $ver = $m.Groups[1].Value
    } else {
        $ver = $Version.TrimStart('v')
    }
    $tag = "v$ver"
    $zipName = "micro-rs-$ver-windows-x64.zip"
    Write-Host "== micro-rs 发布打包 $tag ==" -ForegroundColor Cyan

    # 2) 编译（GNU 工具链需要 dlltool；找不到时自动补常见位置）
    if (-not $SkipBuild) {
        if (-not $MingwBin) {
            foreach ($cand in @(
                (Join-Path $env:USERPROFILE "scoop\apps\mingw\current\bin"),
                "D:\Scoop\apps\mingw\current\bin",
                "C:\msys64\mingw64\bin"
            )) {
                if ((Test-Path $cand) -and (Test-Path (Join-Path $cand "dlltool.exe"))) { $MingwBin = $cand; break }
            }
        }
        if ($MingwBin) {
            $env:PATH = "$MingwBin;$env:PATH"
            Write-Host "已加入 MinGW 工具链路径：$MingwBin"
        }
        Write-Host "编译 release..."
        cargo build --release
        if ($LASTEXITCODE -ne 0) { Fail "cargo build 失败（exit $LASTEXITCODE）" }
    }

    # 3) 汇集文件
    $exe = Join-Path $root "target\release\micro-rs.exe"
    if (-not (Test-Path $exe)) { Fail "找不到 $exe，请先编译或加 -SkipBuild 前确认二进制存在" }
    $stage = Join-Path $root "target\release-stage"
    if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
    New-Item -ItemType Directory -Path $stage -Force | Out-Null

    Copy-Item $exe (Join-Path $stage "micro-rs.exe") -Force
    Copy-Item (Join-Path $root "README.md") (Join-Path $stage "README.md") -Force
    Copy-Item (Join-Path $root "LICENSE") (Join-Path $stage "LICENSE") -Force

    # 4) 压缩
    $zip = Join-Path $root "target\$zipName"
    if (Test-Path $zip) { Remove-Item $zip -Force }
    Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip -CompressionLevel Optimal
    $size = (Get-Item $zip).Length
    Write-Host "已生成 $zipName（$([math]::Round($size / 1MB, 2)) MB）" -ForegroundColor Green

    # 5) 上传 GitHub Release
    if (-not $NoUpload) {
        $gh = Get-Command gh -ErrorAction SilentlyContinue
        if (-not $gh) { Fail "未安装 GitHub CLI（gh），或加 -NoUpload 只打包" }
        gh auth status 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { Fail "gh 未登录，请先运行 gh auth login，或加 -NoUpload" }

        $exists = $false
        gh release view $tag --repo $Repo 2>&1 | Out-Null
        if ($LASTEXITCODE -eq 0) { $exists = $true }

        if ($exists) {
            Write-Host "Release $tag 已存在，覆盖上传资产..."
            gh release upload $tag $zip --repo $Repo --clobber
        } else {
            Write-Host "创建 Release $tag 并上传资产..."
            $notes = "micro-rs $ver — Windows x64 绿色版，解压即用。`n`n一个用 Rust 写的终端文本编辑器，操作手感参照 micro。"
            gh release create $tag $zip --repo $Repo --title "micro-rs $ver — Windows x64" --notes $notes
        }
        if ($LASTEXITCODE -ne 0) { Fail "gh release 上传失败（exit $LASTEXITCODE）" }
        Write-Host "发布完成：https://github.com/$Repo/releases/tag/$tag" -ForegroundColor Green
    } else {
        Write-Host "跳过上传（-NoUpload）。资产位于：$zip"
    }
}
finally { Pop-Location }
