<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { EnvironmentStatus, UpdateInfo } from '../types';
import { ArrowLeft, RefreshCw, Download, CheckCircle2, AlertCircle, Power } from 'lucide-vue-next';
import { openUrl } from '@tauri-apps/plugin-opener';

const props = defineProps<{
  envStatus: EnvironmentStatus | null;
  refreshInterval: number; // in minutes
  showPercentageInTray: boolean;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'update-interval', val: number): void;
  (e: 'update-tray-mode', val: boolean): void;
  (e: 're-login'): void;
}>();

const isCheckingUpdate = ref(false);
const updateResult = ref<UpdateInfo | null>(null);
const updateError = ref('');

async function handleCheckUpdate() {
  isCheckingUpdate.value = true;
  updateError.value = '';
  try {
    const info = await invoke<UpdateInfo>('check_for_updates');
    updateResult.value = info;
  } catch (err: any) {
    updateError.value = `检查更新失败: ${err}`;
  } finally {
    isCheckingUpdate.value = false;
  }
}

async function openReleaseUrl() {
  if (updateResult.value?.release_url) {
    try {
      await openUrl(updateResult.value.release_url);
    } catch {
      window.open(updateResult.value.release_url, '_blank');
    }
  }
}

async function handleQuit() {
  await invoke('hide_window');
  // Exit app
  window.close();
}

onMounted(() => {
  handleCheckUpdate();
});
</script>

<template>
  <div class="flex flex-col h-full select-none text-slate-200">
    <!-- Top Header -->
    <div class="px-4 py-3 bg-slate-900/60 border-b border-slate-800/80 flex items-center justify-between">
      <div class="flex items-center space-x-2">
        <button @click="$emit('close')"
          class="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition">
          <ArrowLeft class="w-4 h-4" />
        </button>
        <h3 class="text-xs font-bold text-white tracking-wide">偏好设置与关于</h3>
      </div>
      <span class="text-[10px] font-mono text-slate-500">v0.1.0</span>
    </div>

    <!-- Content Sections -->
    <div class="p-4 flex-1 overflow-y-auto space-y-4 text-xs">
      <!-- 1. Display Settings -->
      <div class="space-y-2">
        <label class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider">状态栏显示</label>
        <div class="p-3 bg-slate-800/40 rounded-xl border border-slate-800 flex items-center justify-between">
          <div>
            <div class="text-xs font-medium text-white">状态栏文字</div>
            <div class="text-[10px] text-slate-400">显示当前周配额百分比 (如 ⚡ 37%)</div>
          </div>
          <input type="checkbox" :checked="showPercentageInTray"
            @change="$emit('update-tray-mode', ($event.target as HTMLInputElement).checked)"
            class="w-4 h-4 accent-rose-500 rounded cursor-pointer" />
        </div>
      </div>

      <!-- 2. Refresh Interval -->
      <div class="space-y-2">
        <label class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider">自动刷新频率</label>
        <div class="p-3 bg-slate-800/40 rounded-xl border border-slate-800 flex items-center justify-between">
          <div>
            <div class="text-xs font-medium text-white">后台轮询周期</div>
            <div class="text-[10px] text-slate-400">火山方舟数据有 5–30 分钟聚合延迟</div>
          </div>
          <select :value="refreshInterval"
            @change="$emit('update-interval', Number(($event.target as HTMLSelectElement).value))"
            class="bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-2 py-1 outline-none">
            <option :value="5">5 分钟</option>
            <option :value="15">15 分钟 (推荐)</option>
            <option :value="30">30 分钟</option>
            <option :value="60">1 小时</option>
            <option :value="0">仅手动刷新</option>
          </select>
        </div>
      </div>

      <!-- 3. Update Check -->
      <div class="space-y-2">
        <label class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider">版本与更新</label>
        <div class="p-3 bg-slate-800/40 rounded-xl border border-slate-800 space-y-2.5">
          <div class="flex items-center justify-between">
            <div>
              <div class="text-xs font-medium text-white">检查 GitHub Releases</div>
              <div class="text-[10px] text-slate-400">仓库: chzisnull/ark-bar</div>
            </div>
            <button @click="handleCheckUpdate" :disabled="isCheckingUpdate"
              class="px-2.5 py-1 bg-slate-700 hover:bg-slate-600 text-slate-200 rounded-lg text-xs flex items-center gap-1 transition">
              <RefreshCw class="w-3 h-3" :class="{ 'animate-spin': isCheckingUpdate }" />
              <span>{{ isCheckingUpdate ? '检查中...' : '检查更新' }}</span>
            </button>
          </div>

          <div v-if="updateResult" class="pt-2 border-t border-slate-700/60">
            <div v-if="updateResult.has_update" class="p-2.5 bg-amber-500/10 border border-amber-500/30 rounded-lg">
              <div class="flex items-center justify-between text-amber-300 font-medium">
                <span class="flex items-center gap-1.5">
                  <AlertCircle class="w-3.5 h-3.5" />
                  发现新版本 v{{ updateResult.latest_version }}
                </span>
                <button @click="openReleaseUrl"
                  class="px-2 py-0.5 bg-amber-500 text-slate-950 font-semibold rounded text-[10px] flex items-center gap-1">
                  <Download class="w-3 h-3" />
                  下载更新
                </button>
              </div>
              <p v-if="updateResult.release_notes" class="text-[10px] text-slate-400 mt-1 line-clamp-2">
                {{ updateResult.release_notes }}
              </p>
            </div>
            <div v-else class="flex items-center gap-1.5 text-emerald-400 text-[11px]">
              <CheckCircle2 class="w-3.5 h-3.5" />
              <span>当前已是最新版本 (v{{ updateResult.current_version }})</span>
            </div>
          </div>

          <div v-if="updateError" class="text-[10px] text-rose-400">
            {{ updateError }}
          </div>
        </div>
      </div>

      <!-- 4. Environment Diagnostics -->
      <div class="space-y-2">
        <label class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider">运行环境诊断</label>
        <div class="p-3 bg-slate-800/40 rounded-xl border border-slate-800 space-y-1.5 text-[11px]">
          <div class="flex justify-between">
            <span class="text-slate-400">Node.js 版本:</span>
            <span class="font-mono text-slate-300">{{ envStatus?.node_version || '未安装' }}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-slate-400">arkcli 版本:</span>
            <span class="font-mono text-slate-300">{{ envStatus?.arkcli_version || '未安装' }}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-slate-400">当前 Profile:</span>
            <span class="font-mono text-slate-300 truncate max-w-[180px]">{{ envStatus?.active_profile || '-' }}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-slate-400">登录账号:</span>
            <span class="font-mono text-slate-300">{{ envStatus?.user_name || '-' }}</span>
          </div>

          <div class="pt-2 mt-2 border-t border-slate-700/60 flex justify-end">
            <button @click="$emit('re-login')"
              class="text-[10px] text-sky-400 hover:text-sky-300 flex items-center gap-1">
              <span>重新进行 SSO 授权</span>
            </button>
          </div>
        </div>
      </div>

      <!-- 5. Quit Application -->
      <div class="pt-2">
        <button @click="handleQuit"
          class="w-full py-2 bg-rose-500/10 hover:bg-rose-500/20 text-rose-400 border border-rose-500/30 rounded-xl text-xs font-medium flex items-center justify-center gap-1.5 transition">
          <Power class="w-3.5 h-3.5" />
          <span>退出 ArkBar</span>
        </button>
      </div>
    </div>
  </div>
</template>
