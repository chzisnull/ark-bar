## 🚀 ArkBar v__VERSION__ 更新说明

### 🌟 本次版本更新内容
1. **性能**：菜单栏点击与厂商切换先读缓存秒开，`arkcli` / `agy` / `curl` 改为后台静默刷新，不再卡住界面。
2. **Logo**：全新用量柱状图应用图标，菜单栏使用更清晰的模板图标。
3. **悬浮窗**：系统原生拖拽，位置会记住。

---

### 🛠️ 修复的问题
1. **点击顶部菜单栏图标卡顿**：托盘点击与窗口失焦互相抢焦点。
2. **切换厂商卡顿**：去掉全局转圈、串行预取、主窗和悬浮窗双重拉数。
3. **每次查询都冷启动 Node CLI**：缓存 arkcli 路径；席位时间戳不再每次同步再打一轮接口。

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
