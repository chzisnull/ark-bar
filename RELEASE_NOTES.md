## 🚀 ArkBar v__VERSION__ 更新说明

### 🌟 本次版本更新内容
1. **近5小时用量**：用量为 0%（窗口未开始）时显示「使用后开始计时」，有真实重置时间再显示倒计时。
2. **交互**：切换厂商、刷新用量改为应用内 loading（顶部进度条、Tab 转圈、卡片「同步中」、淡入淡出），不再出现系统鼠标转圈。
3. **性能**：用量查询与环境检测放到后台线程，打开窗口先出缓存，避免点击卡死。

---

### 🛠️ 修复的问题
1. **修复近5小时到期时间在 0% 时不显示**（接口返回 `ShortTermResetMilestone: -1`）。
2. **修复点击菜单栏 / 切换厂商时主线程阻塞导致卡顿和等待光标**。

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
