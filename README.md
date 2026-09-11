# 🌋 ArkBar (火山方舟配额监控菜单栏小工具)

<p align="center">
  <img src="https://raw.githubusercontent.com/chzisnull/ark-bar/main/src-tauri/icons/128x128.png" width="96" height="96" alt="ArkBar Logo" />
</p>

<p align="center">
  <b>专为火山引擎火山方舟 (Volcengine Ark) 打造的轻量级桌面状态栏配额监控工具</b><br>
  实时监控 Coding Plan (个人版 / 企业版席位) 与 Agent Plan 的配额消耗及重置倒计时
</p>

<p align="center">
  <a href="https://github.com/chzisnull/ark-bar/releases"><img src="https://img.shields.io/github/v/release/chzisnull/ark-bar?style=flat-square&color=rose" alt="Release"></a>
  <img src="https://img.shields.io/badge/Platform-macOS%20%7C%20Windows-blue?style=flat-square" alt="Platform">
  <img src="https://img.shields.io/badge/Tech-Tauri%202%20%2B%20Vue%203-emerald?style=flat-square" alt="Tech">
  <img src="https://img.shields.io/badge/License-MIT-purple?style=flat-square" alt="License">
</p>

---

## ✨ 核心特性

- ⚡ **macOS 顶部状态栏常驻指示**：在菜单栏图标旁直观显示当前最重要的周度/会话配额百分比（如 `⚡ 37%`），一眼掌握余量。
- 📊 **毛玻璃悬浮卡片 (Popover)**：鼠标点击菜单栏图标，即刻滑出 Apple 原生质感卡片，详细展示：
  - **会话窗口 (Session)**：瞬时限流滑动窗口实时占用率及健康状态。
  - **周度配额 (Weekly)**：本周已用百分比，进度条自动变色（<60% 绿色安全、60-85% 橙色注意、>85% 红色告警）。
  - **月度配额 (Monthly)**：本月累计占用率与**精确到天/时的下次重置倒计时**。
  - **席位与账号信息**：绑定席位 ID、当前登录子用户名与账号 ID。
- 🚀 **极度轻量与低能耗**：基于 **Tauri 2 + Rust + Vue 3** 打造，常驻后台内存仅需 **~25 MB**（相较于传统 Electron 应用的 150MB+ 节省逾 80% 内存），安装包仅 **~10 MB**。
- 🛠️ **全自动环境检测与新手向导 (Onboarding Doctor)**：
  - 自动探测系统 Node.js 运行环境（智能解析 macOS GUI 缺失的 `PATH`，兼容 nvm 与 Homebrew）。
  - 自动检测 `@volcengine/ark-cli`，未安装时支持一键自动安装。
  - 自动检测登录态，未登录时一键呼起默认浏览器完成火山 SSO 授权登录 (`arkcli auth login volc-sso`)。
- 🔄 **智能后台刷新**：默认每 15 分钟静默拉取更新（契合火山方舟数据 5–30 分钟聚合延迟特性），亦可在悬浮窗中一键即时刷新。
- 🔔 **版本检查与更新**：内置 GitHub Releases 版本检测，新版本一键直达下载。

---

## 📸 界面预览

* **顶部状态栏**：`[🌋] ⚡ 37%`
* **悬浮面板**：
  - 会话配额卡片（瞬时占用条）
  - 周配额卡片（周期进度条）
  - 月配额卡片（重置倒计时）
  - 偏好设置（刷新周期、托盘显示模式、环境诊断、退出）

---

## 📦 安装与下载

前往 [Releases 页面](https://github.com/chzisnull/ark-bar/releases) 下载适合您操作系统的最新安装包：

* **macOS (Apple Silicon M 系列)**：`ArkBar_aarch64.dmg`
* **macOS (Intel 系列)**：`ArkBar_x64.dmg`
* **Windows**：`ArkBar_x64_en-US.msi` 或 便携版 `.exe`

---

## 🛠️ 本地开发与构建

### 1. 前置依赖
- [Node.js](https://nodejs.org/) (>= 18.x) 与 [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/) (>= 1.75) 与 Cargo
- 官方 CLI：`npm install -g @volcengine/ark-cli`

### 2. 启动开发模式
```bash
# 1. 克隆代码仓库
git clone https://github.com/chzisnull/ark-bar.git
cd ark-bar

# 2. 安装前端依赖
pnpm install

# 3. 启动 Tauri 开发模式 (含菜单栏图标与前端热重载)
pnpm tauri dev
```

### 3. 本地打包构建
```bash
# 编译生产环境产物
pnpm tauri build
```
编译产物位于 `src-tauri/target/release/bundle/` 目录下。

---

## 💡 技术架构与设计

```
ark-bar/
├── .github/workflows/      # GitHub Actions 跨平台自动打包构建
├── src/                    # 前端视图层 (Vue 3 + Tailwind CSS + Lucide Icons)
│   ├── components/
│   │   ├── OnboardingWizard.vue  # 三步环境检测与授权向导
│   │   ├── UsagePanel.vue        # 主配额监控悬浮卡片
│   │   └── SettingsModal.vue     # 偏好设置与更新检测
│   ├── types/              # 完整的 TypeScript 数据模型
│   ├── App.vue             # 顶层状态与自动轮询调度
│   └── main.ts
└── src-tauri/              # 原生层 (Tauri 2 + Rust)
    ├── src/
    │   ├── env_resolver.rs # 智能环境变量注入 (解决 macOS GUI PATH 丢失)
    │   ├── ark_cli.rs      # arkcli 交互命令封装 (Usage plan / Auth / Install)
    │   ├── tray.rs         # 状态栏托盘管理与 Popover 定位
    │   └── lib.rs          # Tauri 运行时生命周期与插件注册
    ├── tauri.conf.json     # 窗口、托盘与打包配置
    └── Cargo.toml
```

---

## 🤝 鸣谢与参考

- 火山方舟官方 CLI: [@volcengine/ark-cli](https://github.com/volcengine/ark-cli)
- 跨平台框架: [Tauri v2](https://v2.tauri.app/)
- 前端框架: [Vue 3](https://vuejs.org/) & [Tailwind CSS](https://tailwindcss.com/)

---

## 📄 开源许可

本项目基于 [MIT License](LICENSE) 开源。
