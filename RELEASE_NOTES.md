## 🚀 ArkBar v__VERSION__ 更新说明

### 🌟 本次版本更新内容
1. **Antigravity 查询**：`agy` 超时从 8 秒提高到 45 秒，超时后保留上一次成功数据，不再整页变成未连接。
2. **火山方舟重置时间**：近 5 小时、近一周到期时间随用量一起拉取，不再时有时无。
3. **用量刷新**：打开菜单栏、点刷新、定时任务都会跳过旧缓存拉最新；新安装默认 5 分钟刷新。
4. **安装包**：Windows x64、macOS Apple Silicon、macOS Intel 同步构建发布。

---

### 🛠️ 修复的问题
1. **修复 agy 8 秒超时导致 Google Antigravity 显示失败**。
2. **修复 5 小时 / 周用量到期时间偶发不显示**。
3. **修复界面看起来不更新**：后台刷新后面板仍显示旧缓存。
4. **修复 Intel macOS 安装包经常因 macos-13 排队被取消**：改为在 macos-latest 交叉编译。

---

### ⚠️ macOS 首次打开提示“应用已损坏，无法打开”解决办法
由于开源项目未购买苹果商业开发者证书，从浏览器下载后 macOS Gatekeeper 会自动对应用添加隔离标记。  
**解决方法**：打开系统自带「终端」(Terminal.app)，复制并执行以下命令回车即可：
```bash
xattr -dr com.apple.quarantine /Applications/ArkBar.app
```

---

### 📦 安装包下载指引
- **macOS (Apple Silicon)**: `ArkBar_*_aarch64.dmg`
- **macOS (Intel)**: `ArkBar_*_x64.dmg`
- **Windows**: `ArkBar_*_x64-setup.exe`（或 `.msi`）
