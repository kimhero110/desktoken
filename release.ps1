# QuotaBar 发版一条龙: bump → test → commit → tag → push → 盯 CI
# 用法: 在仓库根目录  powershell -ExecutionPolicy Bypass -File release.ps1 patch
#       (patch / minor / major)
# 设计原则: 版本号字段、tag、发布三处只在此脚本里同步, 任何人不手工改其中一处。
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('patch', 'minor', 'major')]
    [string]$Part
)

$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

function Fail($msg) { Write-Host "✗ $msg" -ForegroundColor Red; exit 1 }

# ---------- 1. 工作区必须干净（先查，再动版本字段） ----------
git -C $RepoRoot diff --quiet
$clean1 = $LASTEXITCODE -eq 0
git -C $RepoRoot diff --cached --quiet
$clean2 = $LASTEXITCODE -eq 0
$untracked = git -C $RepoRoot ls-files --others --exclude-standard
if (-not ($clean1 -and $clean2) -or $untracked) { Fail "工作区不干净，先提交或还原其它改动" }

# ---------- 2. 读当前版本并 bump ----------
$cargoToml = Join-Path $RepoRoot 'src-tauri\Cargo.toml'
$tauriConf = Join-Path $RepoRoot 'src-tauri\tauri.conf.json'

$cargoText = [System.IO.File]::ReadAllText($cargoToml)
if ($cargoText -notmatch '(?m)^version = "(\d+)\.(\d+)\.(\d+)"') { Fail "Cargo.toml 里找不到 version" }
$cur = @{ Major = [int]$Matches[1]; Minor = [int]$Matches[2]; Patch = [int]$Matches[3] }
$old = "$($cur.Major).$($cur.Minor).$($cur.Patch)"

switch ($Part) {
    'patch' { $cur.Patch += 1 }
    'minor' { $cur.Minor += 1; $cur.Patch = 0 }
    'major' { $cur.Major += 1; $cur.Minor = 0; $cur.Patch = 0 }
}
$new = "$($cur.Major).$($cur.Minor).$($cur.Patch)"
Write-Host "版本: $old → $new"

# ---------- 3. 同步两处版本字段 ----------
[System.IO.File]::WriteAllText($cargoToml, ($cargoText -replace '(?m)^version = "[\d.]+"', "version = `"$new`""))
$confText = [System.IO.File]::ReadAllText($tauriConf)
if ($confText -notmatch '"version": "[\d.]+"') { Fail "tauri.conf.json 里找不到 version" }
[System.IO.File]::WriteAllText($tauriConf, ($confText -replace '"version": "[\d.]+"', "`"version`": `"$new`""))

# ---------- 4. 测试门禁 ----------
Write-Host "跑测试..." -ForegroundColor Cyan
Push-Location (Join-Path $RepoRoot 'src-tauri')
try {
    cargo test
    if ($LASTEXITCODE -ne 0) { Fail "测试未通过，不发版" }
} finally { Pop-Location }

# ---------- 5. 提交 + tag + 推送 ----------
git -C $RepoRoot add src-tauri\Cargo.toml src-tauri\Cargo.lock src-tauri\tauri.conf.json
git -C $RepoRoot commit -m "chore: release v$new" | Out-Null
if ($LASTEXITCODE -ne 0) { Fail "commit 失败" }
git -C $RepoRoot tag "v$new"
git -C $RepoRoot push
if ($LASTEXITCODE -ne 0) { Fail "push main 失败（tag 未推，可重跑脚本）" }
git -C $RepoRoot push origin "v$new"
if ($LASTEXITCODE -ne 0) { Fail "push tag 失败：git push origin v$new 手动补" }

Write-Host "✓ v$new 已推送，CI 构建中（约 18 分钟）" -ForegroundColor Green

# ---------- 6. 盯 CI ----------
Start-Sleep 20
$runId = $null
for ($i = 0; $i -lt 10; $i++) {
    $run = gh run list --repo kimhero110/desktoken --limit 1 --json databaseId,headBranch 2>$null | ConvertFrom-Json
    if ($run -and $run[0].headBranch -eq "v$new") { $runId = $run[0].databaseId; break }
    Start-Sleep 10
}
if (-not $runId) { Fail "没找到 CI run；去 https://github.com/kimhero110/desktoken/actions 看" }

Write-Host "CI run: https://github.com/kimhero110/desktoken/actions/runs/$runId"
for ($i = 0; $i -lt 40; $i++) {
    Start-Sleep 45
    $r = gh run view $runId --repo kimhero110/desktoken --json status,conclusion 2>$null | ConvertFrom-Json
    if ($r -and $r.status -eq 'completed') {
        if ($r.conclusion -eq 'success') {
            Write-Host "✓ v$new 发布成功: https://github.com/kimhero110/desktoken/releases/tag/v$new" -ForegroundColor Green
            gh release view "v$new" --repo kimhero110/desktoken --json assets -q '.assets[].name'
        } else {
            Fail "CI 失败 ($($r.conclusion))，去 run 页面看日志"
        }
        exit 0
    }
}
Fail "CI 超时（40 轮×45s），去 actions 页面手动看"
