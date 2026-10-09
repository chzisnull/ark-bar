## 🚀 ArkBar v0.7.1 更新说明

### 🐛 本次修复

1. **修掉「后台轮询凭空弹出 Google Antigravity 登录页」**：
   - 现象：明明本地已经登录过 agy、也正在用它，却时不时自动打开 `accounts.google.com/o/oauth2/auth?...client_id=1071006060591-...` 的 Antigravity 授权页。
   - 原因：agy 的 access token 约每小时过期一次，需要向 `oauth2.googleapis.com/token` 刷新；一旦这次刷新遇到瞬时网络错误（EOF / 连接被重置），agy 就会判定自己「未登录」，**不做重试**直接降级成交互式 OAuth 并调用 `open` 把授权页弹到屏幕上（它自己的日志里还写着 `token may still be valid`）。ArkBar 后台每 5–10 分钟就会跑一次 `agy -p "/usage"` 拉配额，于是这个本该只有用户自己触发的登录页，被轮询顶到了脸上。
   - 修复：查询用量时把 PATH 最前面插进一个只记日志、不跳转的 `open` / `xdg-open` 拦截器，同时把子进程 stdin 接到 null。**agy 的登录凭据、配置和轮询周期都不动**，只是让它无法再自己打开浏览器。
   - 被拦下的授权链接会记到 `<应用数据目录>/browser-guard/blocked-openers.log`（日志自动截断到 32KB），出问题时能直接看到证据。

2. **真正需要重新登录时说清楚**：
   - agy 确实登录失效时（被拦截，或在无终端环境下自行放弃交互式登录），配额卡片不再显示含糊的「未检测到 agy 命令行工具」，而是提示 **「agy 登录已失效，需在终端重新登录」**，按钮相应变成「重新授权」。
   - 处理办法：在终端里直接运行一次 `agy`，完成交互式登录后回到 ArkBar 刷新即可。

---

### 🛠️ 实现说明

1. 新增 `execute_cmd_timeout_no_browser()`：仅在「后台轮询、绝不能打扰用户」的调用上启用浏览器拦截（当前只有 Antigravity 用量查询）；`arkcli auth login` 这类本就该弹浏览器的交互式登录完全不受影响，`execute_cmd_timeout()` 的行为逐字未变。
2. 拦截器目录 `~/Library/Application Support/com.chzisnull.arkbar/browser-guard/`（Linux 为 `~/.local/share/com.chzisnull.arkbar/browser-guard/`），权限 0700，脚本 0755；被清理工具删掉或可执行位被剥掉时会自动重建。
3. 可通过环境变量 `ARKBAR_BROWSER_GUARD_DIR` 覆盖拦截器目录（排障用）。
4. 附带回归测试：拦截生效、正常取用量不受影响、脚本自愈、shell 转义不会被执行命令替换。

---

### ⚠️ Windows 尚未覆盖

Windows 版 agy 是通过 `rundll32` / `cmd /c start` 打开浏览器的，同名 PATH 拦截器拦不住，因此 **Windows 上「后台轮询弹授权页」的问题依旧存在**。等上游 agy 提供 `--no-browser` 之类的开关后会一并修掉；在此之前，Windows 用户可临时关闭 ArkBar 里的 Antigravity 用量监控。

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
- **Windows (64位安装包)**: `ArkBar_*_x64-setup.exe` (或 `.msi`)
