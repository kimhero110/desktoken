# GLM coding delegation — 2026-09-08

用户明确授权在Do阶段调用OpenCode+GLM编码，不再等待逐项授权，覆盖此前本轮停在Do前的限制。

执行器：OpenCode 1.18.29；候选实际调用模型 zhipuai-coding-plan/glm-5.3。模型列出且存在对应认证不等于调用成功，成功另记。只发送WP-003计划与验收及编码指令；不发送聊天历史、凭据、生产配置。本次授权外部编码不修改原先“无外部评审”的记录。

隔离工作目录：.analysis/glm-wp003（仓库之外）。禁shell、网络工具、外部目录及子代理，文件编辑限定该目录。仅待主代理复核后接收prototypes/chatgpt-desktop产物。GLM负责代码，主代理负责调度和验收。WP-004仍阻塞，禁止生产发布或真实监控接入。用户真实走查仍pending，不代签。
