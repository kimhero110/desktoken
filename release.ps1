# QuotaBar 发版一条龙: bump → test → commit → tag → push(GitHub+Gitee) → 盯 CI → Gitee 发行版
# 用法: 在仓库根目录  powershell -ExecutionPolicy Bypass -File release.ps1 patch
#       (patch / minor / major)
# 设计原则: 版本号字段、tag、发布三处只在此脚本里同步, 任何人不手工改其中一处。
# Gitee 发版需要 token: 环境变量 GITEE_TOKEN, 或本机文件 ~/.quotabar-gitee-token
# （token 绝不提交进仓库 —— 本文件是公开的）
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('patch', 'minor', 'major')]
    [string]$Part
)

$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

function Fail($msg) { Write-Host "✗ $msg" -ForegroundColor Red; exit 1 }

# ---------- 1. 工作区检查（先查，再动版本字段） ----------
# 允许的唯一例外：上次发版在测试门禁挂了，版本字段已 bump 但未提交 ——
# 若仅有的改动就是版本文件且内容已是目标版本，视为可续跑。
git -C $RepoRoot diff --quiet
$clean1 = $LASTEXITCODE -eq 0
git -C $RepoRoot diff --cached --quiet
$clean2 = $LASTEXITCODE -eq 0
$untracked = git -C $RepoRoot ls-files --others --exclude-standard

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

# 续跑判定：工作区仅剩版本文件改动且已是 $old→$new 的中间态
if (-not ($clean1 -and $clean2) -or $untracked) {
    $dirtyFiles = git -C $RepoRoot diff --name-only
    $versionFiles = @('src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/tauri.conf.json')
    $onlyVersionFiles = ($dirtyFiles | Where-Object { $versionFiles -notcontains $_ }).Count -eq 0 -and -not $untracked
    if (-not $onlyVersionFiles) { Fail "工作区不干净，先提交或还原其它改动" }
    Write-Host "续跑：版本字段已在工作区（上次测试门禁中断）" -ForegroundColor Yellow
    # 版本字段已经是 $new，不要再 bump 一遍 —— 上一步的 $cur 是文件里的旧值，
    # 说明文件还没 bump；若文件已是 $new 则 $old 会等于 $new-1 档。
    # 幂等处理：若文件已含 $new，跳过写入。
    if ($cargoText -match [regex]::Escape("version = `"$new`"")) {
        Write-Host "版本字段已是 $new，跳过 bump"
    } else {
        [System.IO.File]::WriteAllText($cargoToml, ($cargoText -replace '(?m)^version = "[\d.]+"', "version = `"$new`""))
    }
} else {
    [System.IO.File]::WriteAllText($cargoToml, ($cargoText -replace '(?m)^version = "[\d.]+"', "version = `"$new`""))
}

# ---------- 3. 同步两处版本字段 ----------
[System.IO.File]::WriteAllText($cargoToml, ($cargoText -replace '(?m)^version = "[\d.]+"', "version = `"$new`""))
$confText = [System.IO.File]::ReadAllText($tauriConf)
if ($confText -notmatch '"version": "[\d.]+"') { Fail "tauri.conf.json 里找不到 version" }
[System.IO.File]::WriteAllText($tauriConf, ($confText -replace '"version": "[\d.]+"', "`"version`": `"$new`""))

# ---------- 4. 测试门禁（本地镜像上跑，共享盘上 cargo 构建不可靠） ----------
# 原因: rc.exe 不识别 UNC 长路径; 且 cargo 在共享上的增量缓存已腐化过一次。
Write-Host "跑测试（本地镜像）..." -ForegroundColor Cyan
$mir = Join-Path $env:TEMP "quotabar-release-src"
robocopy $RepoRoot $mir /MIR /XD target /NFL /NDL /NJH /NJS /XF .git | Out-Null
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP "quotabar-release-target"
Push-Location (Join-Path $mir 'src-tauri')
try {
    cargo test
    if ($LASTEXITCODE -ne 0) { Fail "测试未通过，不发版" }
} finally { Pop-Location }

# ---------- 5. 提交 + tag + 推送（GitHub + Gitee 双远端） ----------
git -C $RepoRoot add src-tauri\Cargo.toml src-tauri\Cargo.lock src-tauri\tauri.conf.json
git -C $RepoRoot commit -m "chore: release v$new" | Out-Null
if ($LASTEXITCODE -ne 0) { Fail "commit 失败" }
git -C $RepoRoot tag "v$new"
foreach ($remote in @('origin', 'gitee')) {
    git -C $RepoRoot push $remote main
    if ($LASTEXITCODE -ne 0) { Fail "push $remote main 失败（tag 未推，可重跑脚本）" }
    git -C $RepoRoot push $remote "v$new"
    if ($LASTEXITCODE -ne 0) { Fail "push $remote tag 失败：git push $remote v$new 手动补" }
}

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

# ---------- 7. Gitee 发行版 ----------
# JSON 字段中的非 ASCII 一律转 \uXXXX：Gitee API 把请求体当 GBK 解，
# 直接发 UTF-8 中文必乱码（2026-09 实测）。token 从环境变量或本机文件读。
function ConvertTo-JsonSafe([string]$s) {
    $sb = New-Object System.Text.StringBuilder
    foreach ($ch in $s.ToCharArray()) {
        $code = [int]$ch
        if ($ch -eq '\') { $sb.Append('\\') | Out-Null }
        elseif ($ch -eq '"') { $sb.Append('\"') | Out-Null }
        elseif ($ch -eq "`n") { $sb.Append('\n') | Out-Null }
        elseif ($ch -eq "`r") { $sb.Append('\r') | Out-Null }
        elseif ($code -gt 127) { $sb.Append(('\u{0:x4}' -f $code)) | Out-Null }
        else { $sb.Append($ch) | Out-Null }
    }
    $sb.ToString()
}

function Publish-GiteeRelease([string]$version) {
    $token = $env:GITEE_TOKEN
    if (-not $token) {
        $tokenFile = Join-Path $env:USERPROFILE '.quotabar-gitee-token'
        if (Test-Path $tokenFile) { $token = ([System.IO.File]::ReadAllText($tokenFile)).Trim() }
    }
    if (-not $token) {
        Write-Host "⚠ 无 GITEE_TOKEN，跳过 Gitee 发行版（代码与 tag 已双推）" -ForegroundColor Yellow
        return
    }

    $tag = "v$version"
    $api = "https://gitee.com/api/v5/repos/xu512/quotabar"

    # 已存在则复用（幂等：重跑脚本直接补传附件）
    $existing = & curl.exe -s "$api/releases/tags/$tag`?access_token=$token"
    $releaseId = $null
    if ($existing -match '"id"\s*:\s*(\d+)') { $releaseId = $Matches[1] }

    $bodyText = @"
## QuotaBar $tag

- 更新内容见 GitHub Release: https://github.com/kimhero110/desktoken/releases/tag/$tag

### 安装
- ``QuotaBar_${version}_x64-setup.exe``：NSIS 安装包
- ``quotabar.exe``：绿色单文件
- ``checksums.txt``：SHA-256 校验

零遥测，凭据全在本机。
"@
    $payload = '{"access_token":"' + $token + '","tag_name":"' + $tag + '","name":"QuotaBar ' + $tag + '","body":"' + (ConvertTo-JsonSafe $bodyText) + '","target_commitish":"main"}'
    $payloadFile = Join-Path $env:TEMP "quotabar-gitee-release-$tag.json"
    [System.IO.File]::WriteAllText($payloadFile, $payload, [System.Text.Encoding]::ASCII)

    if ($releaseId) {
        & curl.exe -s -o $null -X PATCH -H "Content-Type: application/json" --data-binary "@$payloadFile" "$api/releases/$releaseId"
        Write-Host "Gitee 发行版已存在，更新描述并补传附件"
    } else {
        $resp = & curl.exe -s -X POST -H "Content-Type: application/json" --data-binary "@$payloadFile" "$api/releases"
        if ($resp -match '"id"\s*:\s*(\d+)') { $releaseId = $Matches[1] }
    }
    Remove-Item $payloadFile -ErrorAction SilentlyContinue
    if (-not $releaseId) { Write-Host "⚠ Gitee 发行版创建失败，手动处理" -ForegroundColor Yellow; return }

    # 从 GitHub 下载产物，传到 Gitee（跳过同名已传）
    $dl = Join-Path $env:TEMP "quotabar-rel-$tag"
    New-Item -ItemType Directory $dl -Force | Out-Null
    gh release download $tag --repo kimhero110/desktoken --dir $dl 2>$null
    $existingAssets = & curl.exe -s "$api/releases/$releaseId`?access_token=$token"
    foreach ($f in (Get-ChildItem $dl)) {
        if ($existingAssets -match [regex]::Escape('"' + $f.Name + '"')) {
            Write-Host "  跳过已存在: $($f.Name)"
            continue
        }
        $up = & curl.exe -s -X POST -F "file=@$($f.FullName)" "$api/releases/$releaseId/attach_files?access_token=$token"
        if ($up -match '"id"') { Write-Host "  已传: $($f.Name)" } else { Write-Host "  失败: $($f.Name)" -ForegroundColor Yellow }
    }
    Remove-Item $dl -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "✓ Gitee v$new 发布成功: https://gitee.com/xu512/quotabar/releases/$tag" -ForegroundColor Green
}

Write-Host "CI run: https://github.com/kimhero110/desktoken/actions/runs/$runId"
for ($i = 0; $i -lt 40; $i++) {
    Start-Sleep 45
    $r = gh run view $runId --repo kimhero110/desktoken --json status,conclusion 2>$null | ConvertFrom-Json
    if ($r -and $r.status -eq 'completed') {
        if ($r.conclusion -eq 'success') {
            Write-Host "✓ GitHub v$new 发布成功: https://github.com/kimhero110/desktoken/releases/tag/v$new" -ForegroundColor Green
            gh release view "v$new" --repo kimhero110/desktoken --json assets -q '.assets[].name'
            Publish-GiteeRelease $new
        } else {
            Fail "CI 失败 ($($r.conclusion))，去 run 页面看日志"
        }
        exit 0
    }
}
Fail "CI 超时（40 轮×45s），去 actions 页面手动看"

