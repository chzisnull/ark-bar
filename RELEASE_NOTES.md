## 🚀 ArkBar v__VERSION__ 更新说明

### 🌟 本次版本重磅更新：多厂商详细 Token 统计与历史报表

1. **多维度 Token 详细用量看板**：
   - 全面支持 **近 5 小时**、**今日累计**、**本周累计**（周一至今）、**本月累计**（1日至今）Token 消耗统计。
   - 细致拆解 **Prompt (输入)** 与 **Completion (输出)** 比例条，直观呈现 **Cache Hit (缓存命中)** 节省 Tokens 及命中率。
   - 内置 **7 日微走势图 (Sparkline)**，鼠标悬停即刻查看每日具体用量。

2. **多厂商原生聚合引擎**：
   - **火山方舟 (Volcengine Ark)**：调用官方 OpenAPI `GetInferenceUsage`，实现精确小时级与天级用量聚合。
   - **OpenAI Codex**：自动解析本地 session rollout 日志中的 `token_count` 事件，无需网络请求即时汇总。
   - **Google Antigravity & xAI Grok**：基于模型配额与点数体系提供标准化的 Token 吞吐指标。

3. **历史明细报表与数据导出**：
   - 提供独立历史抽屉弹窗，展示过去 14~30 天历史柱状走势图。
   - 完整按日明细表格（总用量、输入/输出拆解、缓存命中数、调用次数）。
   - 支持一键导出并复制为 JSON 数据。

4. **主面板双模切换与紧凑化布局**：
   - 顶部胶囊一键切换 `[ ⚡ 配额监控 ]` 与 `[ 📊 Token 统计 ]`。
   - 配额监控视图 Hero 卡片紧凑化优化，常驻露出月度 Token 标签（点击直达 Token 视图）。

5. **状态栏托盘与桌面悬浮组件增强**：
   - macOS 状态栏新增显示选项：可自由选择展示 `⚡ 配额百分比`、`🔥 今日 Token`（如 `🔥 602K`）、`⏱️ 5小时 Token` 等。
   - 桌面单行悬浮小组件实时显示今日 Token 消耗。

---

### ⚠️ macOS 首次打开提示“应用已损坏，无法打开”解决办法
由于开源项目未购买苹果商业开发者证书，从浏览器下载后 macOS Gatekeeper 会自动对应用添加隔离标记。  
**解决方法**：打开系统自带「终端」(Terminal.app)，复制并执行以下命令回车即可：
```bash
xattr -dr com.apple.quarantine /Applications/ArkBar.app
```

---

### 📦 安装包下载指引
- **macOS (Apple Silicon M1/M2/M3/M4 系列)**: `ArkBar_*_aarch64.dmg`
- **macOS (Intel 处理器)**: `ArkBar_*_x64.dmg`
- **Windows (64位安装包)**: `ArkBar_*_x64-setup.exe`（或 `.msi`）
