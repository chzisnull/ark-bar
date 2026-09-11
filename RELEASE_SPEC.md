# ArkBar 项目版本发布与更新日志规范 (Release Specification)

为了保证 ArkBar 各平台发布包质量、向用户提供清晰准确的更新说明，并维护整洁专业的 GitHub Releases 页面，特制定本发布规范。每次发布新版本（Git Tag `v*`）时必须严格遵守本规范。

---

## 一、版本号管理规范 (SemVer)

版本号遵循 [语义化版本 2.0.0 (Semantic Versioning)](https://semver.org/lang/zh-CN/) 规范：`vMAJOR.MINOR.PATCH`。

- **MAJOR (主版本号)**：发生重大架构变更或不兼容的破坏性更新时递增。
- **MINOR (次版本号)**：新增功能模块（例如接入新的大模型服务商、重大 UI 模式新增）但向下兼容时递增。
- **PATCH (补丁版本号)**：日常 Bug 修复、交互性能调优、代码优化时递增。

### 统一同步的文件清单
每次发布前，必须确保以下文件中的版本号完全一致：
1. `package.json` -> `"version"`
2. `src-tauri/Cargo.toml` -> `[package].version`
3. `src-tauri/tauri.conf.json` -> `"version"`
4. `src/components/SettingsModal.vue` -> 默认回退版本号变量 `appVersion`

---

## 二、GitHub Release 发布说明规范 (Release Notes Policy)

### 1. 核心准则
- **拒绝草率与空洞**：严禁仅写“更新了一些内容”或“修复了已知问题”等无实质信息的说明。
- **专注当期变更，严禁堆砌历史日志**：
  - **只展示本次版本新增的功能与修复的问题**。
  - **严禁在 Release 说明中堆砌上一个或数个过时版本的更新记录**，避免发布页冗长嘈杂。
- **版本说明文件单一可信源**：统一在项目根目录维护 `RELEASE_NOTES.md`。GitHub Actions 在自动化构建时动态读取该文件内容作为 GitHub Release 的 Body，严禁在 CI/CD 配置文件（`.github/workflows/release.yml`）中硬编码固定说明文本。

### 2. 标准更新日志结构模板
`RELEASE_NOTES.md` 必须遵循以下 Markdown 结构模板：

```markdown
## 🚀 ArkBar v__VERSION__ 更新说明

### 🌟 本次版本更新内容
1. **[功能模块名称]**：
   - 详细说明新增能力及用户使用方式。
2. **[功能模块名称]**：
   - 详细说明新增能力及用户使用方式。

---

### 🛠️ 修复的问题
1. **[问题名称/表现]**：
   - 简明阐述修复的问题根因或改进前后的对比。
2. **[问题名称/表现]**：
   - 简明阐述修复的问题根因或改进前后的对比。

---

### ⚠️ macOS 首次打开提示“应用已损坏，无法打开”解决办法
由于开源项目未购买苹果商业开发者证书，从浏览器下载后 macOS Gatekeeper 会自动对应用添加隔离标记。  
**解决方法**：打开系统自带「终端」(Terminal.app)，复制并执行以下命令回车即可：
\`\`\`bash
xattr -dr com.apple.quarantine /Applications/ArkBar.app
\`\`\`

---

### 📦 安装包下载指引
- **macOS (Apple Silicon M1/M2/M3/M4 系列)**: \`ArkBar_*_aarch64.dmg\`
- **macOS (Intel 处理器)**: \`ArkBar_*_x64.dmg\`
- **Windows (64位安装包)**: \`ArkBar_*_x64-setup.exe\` (或 \`.msi\`)
```

---

## 三、发布前自检流程 Checklist

在执行发版推 Tag 前，请按以下步骤逐项核对：

- [ ] **1. 本地代码编译校验**：
  - 前端类型检查与构建通过：`pnpm build`
  - Rust 编译与单元测试通过：`cargo check --manifest-path src-tauri/Cargo.toml`
- [ ] **2. 版本号四处同步**：
  - `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`SettingsModal.vue` 版本号一致。
- [ ] **3. 撰写当期 RELEASE_NOTES.md**：
  - 覆盖当期全部核心功能、UX 调优与 Bug 修复，剔除上一版本的过时内容。
- [ ] **4. 提交代码与创建 Tag**：
  ```bash
  git add .
  git commit -m "chore(release): vX.Y.Z"
  git tag vX.Y.Z
  git push origin main --tags
  ```
- [ ] **5. 观测 GitHub Actions 构建与交付**：
  - 打开 GitHub 仓库 Actions 标签页，确保 macOS aarch64、macOS x64、Windows 自动化构建绿色通过，且 Release 页面渲染正常。
