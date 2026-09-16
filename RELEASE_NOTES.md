## 🚀 ArkBar v__VERSION__ 更新说明

### 🌟 本次版本更新内容
1. **macOS 27 兼容**：托盘菜单改为右键在鼠标光标处弹出，左键恢复「直接打开 / 收起配额面板」的显隐切换。

---

### 🛠️ 修复的问题
1. **修复 macOS 27 上左键点击托盘图标只弹出菜单、无法打开面板**：
   - 新版 macOS 会接管挂载了菜单的状态栏项的左键，导致配额面板无法打开。
   - 本版不再把菜单常驻挂载到状态栏项，改为右键时手动弹出；建议所有已升级 macOS 27 的用户更新。

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
