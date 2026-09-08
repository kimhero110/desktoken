# Kimi 配额解析修复（5h 窗口缺失 `used` 字段）

日期：2026-09-08 · 影响文件：`src-tauri/src/providers/kimi.rs`

## 现象

实测官方只读端点 `GET https://api.kimi.com/coding/v1/usages`（Bearer 认证，
CLI 同源）返回的 `limits[]` 中 5h 窗口的 `detail` **不含 `used` 字段**，
仅有 `limit` / `remaining` / `resetTime`。旧解析器硬性要求 `used` 存在，
导致 5h 窗口被静默丢弃——不是配额规则取消，而是解析不兼容。

## 实测证据（脱敏 schema，非真实身份/凭据）

```json
{
  "limits": [
    {
      "window": { "duration": 300, "timeUnit": "TIME_UNIT_MINUTE" },
      "detail": {
        "limit": "100",
        "remaining": "100",
        "resetTime": "2026-09-08T08:04:00.891299Z"
      }
    }
  ]
}
```

要点：

- `detail` 无 `used`；数值均为字符串。
- 300 `TIME_UNIT_MINUTE` = 5 小时滚动窗口。
- 未观测到任何月度 `totalQuota` 字段，月度解析**不在本次范围**，未凭空实现。

## 根因

`parse_value()` 中周窗口与 5h 窗口均要求 `(used, limit)` 成对出现且
`limit > 0`；`used` 缺失时窗口被静默跳过。根因为解析器假设过强，
已确认不是官方取消 5h 配额规则。

## 修复（`kimi.rs`）

新增共享百分比推导 `used_percent(detail)`，周窗口与 5h 窗口统一使用：

1. `limit` 必须为有限正数，否则忽略该窗口。
2. **优先显式 `used`**：存在且非 null 时，必须是有限非负数，直接
   `used / limit * 100`；malformed（如 `"NaN"`、非数字）**不会静默变成
   0**，窗口直接忽略，保持旧显式语义。
3. **回退推导**：仅当 `used` 缺失或为 null 时，用 `remaining` 推导
   `(limit - remaining) / limit * 100`；`remaining` 必须有限、非负且
   ≤ `limit`，否则忽略。`used` 与 `remaining` 同时缺失绝不伪造 0。

新增 5h 窗口识别 `is_5h_window(window)`，归一化时长/单位：

- `300` × `TIME_UNIT_MINUTE`（**兼容旧 fixture：单位缺失按分钟**）
- `5` × `TIME_UNIT_HOUR`
- `18000` × `TIME_UNIT_SECOND`
- 未知单位、单位为 null/数字等非字符串、或换算后不等于 300 分钟（如 `300` × `TIME_UNIT_HOUR`）→ 忽略；仅当单位**缺失**时才按旧 fixture 约定视为分钟。

## 回归测试（`#[cfg(test)]`，均无网络）

| 测试 | 断言 |
|---|---|
| `real_missing_used_fixture_yields_5h_zero_percent_with_reset` | 真实脱敏 fixture：无 `used` → 5h 0%，`resetTime` 保留 |
| `remaining_only_derives_partial_percentage` | 100/55 → 45% |
| `weekly_usage_falls_back_to_remaining` | 周窗口同样回退：200/50 → 75% |
| `explicit_used_takes_precedence` | 显式 `used` 优先于不一致的 `remaining` |
| `missing_both_or_invalid_fields_are_ignored` | 双缺失 / `used:"NaN"` / remaining 越界或为负 / limit≤0 → 忽略，不伪造 0 |
| `window_unit_normalization` | 5 小时、18000 秒、无单位=分钟 均识别；300 小时、未知单位、null/数字单位拒绝 |

既有 `parses_typical_response` 等旧 fixture（显式 `used` + 显式
`TIME_UNIT_MINUTE`）语义不变。

## 官方参考

- Kimi 会员定价（5h 窗口/周额度规则）：
  <https://www.kimi.com/help/membership/membership-pricing>
- Kimi Code 错误参考（429/配额语义）：
  <https://www.kimi.com/code/docs/en/kimi-code/error-reference.html>

## 范围与不做

- 月度 `totalQuota`：未观测到字段语义，不实现。
- 不改凭据读取、网络层、依赖、版本号。

## 实际验证（2026-09-08）

- 首轮新增测试字符串编译失败，已由GLM返修，未计为通过。
- Kimi定向测试11/11通过；Rust完整测试113/113通过；前端quota-state测试2/2通过。
- 官方请求使用现有OpenCode Kimi凭据，仅输出额度字段白名单；未输出或保存认证信息。
- 本次没有核实月额度字段语义，仍不显示推测的月额度。
- Release构建成功；本机旧程序备份后，在原路径替换并重启成功，部署文件SHA256与构建产物一致。替换时旧进程文件锁短暂未释放，分步停止/替换后解决。
- 本机修复构建SHA256：7e4808a61f62af5c6b4dc77accfd8ddd7b23ca7fba97e215e0ef914952c12e4e。版本号未变，属于本地修复构建，未发布新的Release。
- 已验证进程启动和文件一致；未进行桌面截图验收，额度值仍以随后成功轮询为准。
