<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { openUrl } from '@tauri-apps/plugin-opener';
import type { UpdateInfo, UpdateProgress } from '../types';
import {
  Sparkles,
  Download,
  ExternalLink,
  X,
  AlertTriangle,
  Loader2,
  Rocket
} from 'lucide-vue-next';

const props = defineProps<{
  updateInfo: UpdateInfo;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const isUpdating = ref(false);
const currentProgress = ref<UpdateProgress | null>(null);
const errorMessage = ref('');
let unlistenFn: UnlistenFn | null = null;

onMounted(async () => {
  try {
    unlistenFn = await listen<UpdateProgress>('update-progress', (event) => {
      currentProgress.value = event.payload;
      if (event.payload.stage === 'error') {
        errorMessage.value = event.payload.message;
        isUpdating.value = false;
      }
    });
  } catch (err) {
    console.error('Failed to listen to update-progress:', err);
  }
});

onUnmounted(() => {
  if (unlistenFn) {
    unlistenFn();
    unlistenFn = null;
  }
});

async function startInAppUpdate() {
  isUpdating.value = true;
  errorMessage.value = '';
  currentProgress.value = {
    stage: 'preparing',
    percent: 0,
    current_bytes: 0,
    total_bytes: 0,
    message: '正在准备下载...'
  };

  try {
    await invoke('install_app_update', {
      version: props.updateInfo.latest_version,
      customUrl: props.updateInfo.download_url || null
    });
  } catch (err: any) {
    errorMessage.value = typeof err === 'string' ? err : err.message || '更新执行失败';
    isUpdating.value = false;
  }
}

async function handleOpenBrowser() {
  if (props.updateInfo.release_url) {
    try {
      await openUrl(props.updateInfo.release_url);
    } catch {
      window.open(props.updateInfo.release_url, '_blank');
    }
  }
}
</script>

<template>
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-sm animate-fade-in">
    <div class="w-full max-w-sm bg-[#101726] border border-slate-700/80 rounded-2xl shadow-2xl overflow-hidden flex flex-col select-none">
      <!-- Header -->
      <div class="px-4 py-3 bg-[#162035] border-b border-slate-800 flex items-center justify-between">
        <div class="flex items-center space-x-2">
          <div class="w-6 h-6 rounded-lg bg-indigo-500/20 border border-indigo-500/40 flex items-center justify-center text-indigo-400">
            <Sparkles class="w-3.5 h-3.5" />
          </div>
          <span class="text-xs font-bold text-white tracking-wide">软件在线更新</span>
        </div>
        <button
          v-if="currentProgress?.stage !== 'restarting'"
          @click="emit('close')"
          class="p-1 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition">
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Content Body -->
      <div class="p-4 space-y-3.5 text-xs">
        <!-- Version Tag Pill -->
        <div class="flex items-center justify-between p-2.5 bg-[#141c2e] rounded-xl border border-slate-800/80">
          <div class="flex items-center space-x-2">
            <span class="text-[11px] text-slate-400">当前版本:</span>
            <span class="font-mono text-slate-300 font-semibold">v{{ updateInfo.current_version }}</span>
          </div>
          <span class="text-slate-500">➔</span>
          <div class="flex items-center space-x-1 text-emerald-400 font-semibold">
            <Rocket class="w-3.5 h-3.5 text-emerald-400 animate-bounce" />
            <span class="font-mono">v{{ updateInfo.latest_version }}</span>
          </div>
        </div>

        <!-- Release Notes Preview -->
        <div v-if="!isUpdating && !errorMessage" class="space-y-1">
          <div class="text-[10px] uppercase font-semibold text-slate-400 tracking-wider">更新日志与亮点</div>
          <div class="p-2.5 bg-[#0d1322] border border-slate-800/80 rounded-xl text-[11px] text-slate-300 max-h-32 overflow-y-auto whitespace-pre-wrap leading-relaxed">
            {{ updateInfo.release_notes || '包含稳定性提升、性能优化以及跨平台交互升级。' }}
          </div>
        </div>

        <!-- Progress State Section -->
        <div v-if="isUpdating" class="space-y-2.5 p-3 bg-[#141c2e] border border-indigo-500/30 rounded-xl">
          <div class="flex items-center justify-between">
            <div class="flex items-center space-x-2 text-indigo-300 font-medium">
              <Loader2 class="w-3.5 h-3.5 animate-spin text-indigo-400" />
              <span class="text-[11px] truncate max-w-[200px]">
                {{ currentProgress?.message || '正在更新...' }}
              </span>
            </div>
            <span class="font-mono font-bold text-indigo-300 text-xs">
              {{ (currentProgress?.percent || 0).toFixed(1) }}%
            </span>
          </div>

          <!-- Progress Bar Track -->
          <div class="w-full bg-slate-900 rounded-full h-2 overflow-hidden p-0.5 border border-slate-800">
            <div
              class="h-full bg-gradient-to-r from-indigo-500 via-purple-500 to-emerald-400 rounded-full transition-all duration-300 shadow-sm"
              :style="{ width: `${Math.max(currentProgress?.percent || 0, 3)}%` }">
            </div>
          </div>

          <!-- Stage hints -->
          <div class="flex justify-between text-[9px] text-slate-500 pt-0.5">
            <span :class="{ 'text-indigo-300 font-semibold': currentProgress?.stage === 'preparing' }">① 连接</span>
            <span :class="{ 'text-indigo-300 font-semibold': currentProgress?.stage === 'downloading' }">② 下载包</span>
            <span :class="{ 'text-indigo-300 font-semibold': currentProgress?.stage === 'extracting' || currentProgress?.stage === 'installing' }">③ 解压安装</span>
            <span :class="{ 'text-emerald-300 font-semibold animate-pulse': currentProgress?.stage === 'restarting' }">④ 重启生效</span>
          </div>
        </div>

        <!-- Error Alert -->
        <div v-if="errorMessage" class="p-3 bg-rose-500/15 border border-rose-500/30 rounded-xl space-y-1.5">
          <div class="flex items-center space-x-1.5 text-rose-300 font-semibold">
            <AlertTriangle class="w-3.5 h-3.5 text-rose-400 flex-shrink-0" />
            <span>在线更新遇到异常</span>
          </div>
          <p class="text-[11px] text-rose-200/80 break-words leading-tight">
            {{ errorMessage }}
          </p>
        </div>
      </div>

      <!-- Action Buttons Footer -->
      <div class="px-4 py-3 bg-[#162035] border-t border-slate-800 flex items-center justify-between gap-2">
        <button
          @click="handleOpenBrowser"
          class="px-2.5 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-xl text-xs flex items-center gap-1 transition">
          <ExternalLink class="w-3 h-3 text-slate-400" />
          <span>网页下载</span>
        </button>

        <div class="flex items-center gap-2">
          <button
            v-if="!isUpdating && !errorMessage"
            @click="emit('close')"
            class="px-2.5 py-1.5 rounded-xl text-xs text-slate-400 hover:text-white transition">
            稍后
          </button>

          <button
            v-if="!isUpdating"
            @click="startInAppUpdate"
            class="px-3.5 py-1.5 bg-gradient-to-r from-indigo-600 to-violet-600 hover:from-indigo-500 hover:to-violet-500 text-white font-bold rounded-xl text-xs flex items-center gap-1.5 shadow-md shadow-indigo-500/25 transition active:scale-95">
            <Download class="w-3.5 h-3.5" />
            <span>{{ errorMessage ? '重试在线更新' : '一键在线更新' }}</span>
          </button>

          <div
            v-else
            class="px-3 py-1.5 text-[11px] font-medium text-indigo-300 flex items-center gap-1.5">
            <Loader2 class="w-3 h-3 animate-spin" />
            <span>更新进行中...</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
