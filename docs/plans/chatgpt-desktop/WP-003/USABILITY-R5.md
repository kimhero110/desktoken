# WP-003 常用路径修订 R5

用户反馈视觉“还可以吧”，记为方向基本认可，不能推定五流程或完整UA-01通过；随后明确要求继续工作。

本轮限定U5暂停/恢复路径与PUI-08键盘要求的既有缺陷修复，保持R4样式、三页设置、四项菜单及纯演示边界，未扩大正式产品范围。主代理核对源码发现：时长按钮先静默关闭菜单再渲染，没有返回可见触发按钮；菜单本地Esc处理未阻止冒泡，document处理会进一步关闭面板；role=menu暂停弹层缺少方向/Home/End导航。交同一OpenCode GLM-5.3会话返修，不等待常规授权。

编码日志：隔离目录usability-r5-events.jsonl/usability-r5-stderr.txt。要求选择后聚焦暂停按钮、Esc仅退一层、方向导航与单一tabstop、Tab退出不困住用户。仍只做源码检查，不访问浏览器、不模拟DOM、不绕过URL策略。实际交互验收pending；不发布、不修改生产源码或配置。

实施结果：GLM进程98993正常exit0完成。修改prototype.js、README.md、acceptance.md；HTML/CSS/scenarios与R4哈希一致。setPause渲染后在暂停菜单关闭且焦点留在隐藏菜单/回body、触发按钮可见时恢复焦点；Esc停止冒泡；方向/Home/End导航及focusin维护单一tabstop；Tab关闭后保留自然移动。主代理源码和双JS语法检查通过，证据usability-r5-results.json，未运行DOM/浏览器。

当前决定：此轮定点修复已完成，不扩大review范围。用户视觉方向基本认可已记录；UA-01仍缺五流程实测，不推定全通过。整体C未通过，正式产品菜单改造仍需满足原有准入条件。无活动GLM进程，不重复派发。
