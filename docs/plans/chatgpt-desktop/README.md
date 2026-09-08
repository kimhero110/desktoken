# ChatGPT 桌面助手细节方案与 P/R 评审

本目录只包含方案和治理证据，未实现、未发布新的 ChatGPT 桌面监控。既有 v0.4.0-beta.1 支持边界不变。

**当前结论：WP-003已由 OpenCode + GLM-5.3 实施并完成两轮返修，独立源码复核4项P2已修复关闭；浏览器验收阻塞，整体C未通过；正式监控不可Do。** 见[当前实施与QA记录](WP-003/GLM-QA.md)。[开工门槛](WP-003/GATE.md)是实施前历史记录。WP-002已核实本机应用身份和候选协议语义，但实时只读订阅尚未验证，现回P，继续阻塞WP-004。原型先行的依赖调整另经独立评审，未降低监控门槛；[调查结果](WP-002/FINDINGS.md)及原冻结方案保留为历史依据。

## 阅读顺序

1. [当前WP-003细节计划](WP-003/PLAN-v3.md)、[验收](WP-003/ACCEPTANCE-v3.md)、[两轮评审记录](WP-003/REVIEW-LOG.md)：只读此包不能推定正式监控已准入。
2. [总体细节方案 v2](PLAN-v2.md)：目标、工作包、五条流程、架构与数据、权限、12项验收、风险及P/R门槛；§9为R1修订，原型依赖以后续CC-003为准。
3. [菜单与交互规格](UI-MAP.md)：原入口迁移表、文字线框、操作次数、状态卡与视觉/可访问性约束。
4. [总体方案评审与决策记录](REVIEW-LOG.md)：各轮问题、修订、证据仲裁、是否可前进。

## 原始证据

- PLAN-v1.md / packet-v1.json：第一轮冻结原件，不覆盖。
- review-r1.json、review-r2.json：独立发现与复审（后者是同审查员修订复审，不冒充第二个独立槽）。
- packet-v2.json：第二轮文件哈希；`prdca freeze --root docs/plans/chatgpt-desktop --manifest docs/plans/chatgpt-desktop/packet-v2.json --verify` 可复核。
- `.prdca/evidence/CG-P01/`：工作包WP-001的value-check、风险路由、仲裁；目录简称不等于正式工作包ID。
- evidence-pack.json / evidence-v2.json：初始化P/R证据；产品测试为not_run，不能用文档完整性检查冒充验收。
- delivery-evidence.json：最终文档交付证据包，prdca validate通过；其中P-01至P-03为本轮文档要求，CG-01至CG-12及AT为未来产品要求与未执行测试，两者不混算。
- `node docs/plans/chatgpt-desktop/check-plan.cjs`：只检查编号及入口引用是否齐全；语义由评审核对。

范围：本轮不改生产源码、不改用户配置、不启动AI应用任务；后续用户已授权草稿原型与文档入库推送，见WP-003/REPOSITORY-DELIVERY.md，仍不发布应用版本。主代理负责整合和门禁结论；用户保留价值取舍及后续真实体验接受权。

WP-003原型现状（2026-09-08）：视觉R4与U5可用性R5返修完成，源码/语法检查通过，用户对视觉方向基本认可；浏览器及五流程实际验收pending，正式ChatGPT监控未接通。原型六文件已复制入仓库[prototypes/chatgpt-desktop](../../../prototypes/chatgpt-desktop/README.md)（草稿，现行权威版本；隔离路径文档为历史记录），最新QA证据见[USABILITY-R5](WP-003/USABILITY-R5.md)及usability-r5-results.json。发布授权记录见[REPOSITORY-DELIVERY](WP-003/REPOSITORY-DELIVERY.md)。
