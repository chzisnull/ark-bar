#!/usr/bin/env bash
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$DIR"

echo "==> [1/4] 正在编译前端资源与打包 macOS 安装包 (Tauri Build)..."
pnpm tauri build

APP_SRC="$DIR/src-tauri/target/release/bundle/macos/ArkBar.app"
DMG_SRC=$(ls "$DIR/src-tauri/target/release/bundle/dmg/"*.dmg 2>/dev/null | head -n 1 || true)

if [ ! -d "$APP_SRC" ]; then
  echo "Error: 未找到打包产物 $APP_SRC"
  exit 1
fi

echo "==> [2/4] 打包成功！"
echo "    App 路径: $APP_SRC"
if [ -n "$DMG_SRC" ]; then
  echo "    DMG 路径: $DMG_SRC"
fi

echo "==> [3/4] 准备安装至 /Applications/ArkBar.app..."
pkill -f "ArkBar.app/Contents/MacOS/ark-bar" 2>/dev/null || true
pkill -f "/ark-bar" 2>/dev/null || true
sleep 1

rm -rf /Applications/ArkBar.app
cp -R "$APP_SRC" /Applications/ArkBar.app
echo "    已成功复制至 /Applications/ArkBar.app"

echo "==> [4/4] 启动 ArkBar..."
open /Applications/ArkBar.app

echo "==> 启动命令已执行！请在 macOS 屏幕右上角菜单栏查看 ArkBar。"
