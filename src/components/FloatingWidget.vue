<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { ProviderType, ProviderUsageData } from '../types';
import { X, RefreshCw } from 'lucide-vue-next';

const currentProvider = ref<ProviderType>(
  (localStorage.getItem('arkbar_float_provider') as ProviderType) || 'volcengine'
);

const CACHE_KEY = 'arkbar_cached_providers_data';

function getCachedUsage(provider: ProviderType): ProviderUsageData | null {
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      return parsed[provider] || null;
    }
  } catch {}
  return null;
}

const usageData = ref<ProviderUsageData | null>(getCachedUsage(currentProvider.value));
const isRefreshing = ref(false);
let timer: any = null;

async function fetchUsage() {
  isRefreshing.value = true;
  try {
    const data = await invoke<ProviderUsageData>('get_unified_usage', {
      provider: currentProvider.value,
      customToken: null,
    });
    usageData.value = data;
    try {
      const raw = localStorage.getItem(CACHE_KEY);
      const parsed = raw ? JSON.parse(raw) : {};
      parsed[currentProvider.value] = data;
      localStorage.setItem(CACHE_KEY, JSON.stringify(parsed));
    } catch {}
  } catch (err) {
    console.error('Floating widget fetch failed:', err);
  } finally {
    isRefreshing.value = false;
  }
}

function cycleProvider() {
  const ids: ProviderType[] = ['volcengine', 'antigravity', 'grok', 'codex'];
  const curIdx = ids.indexOf(currentProvider.value);
  const nextIdx = (curIdx + 1) % ids.length;
  currentProvider.value = ids[nextIdx];
  localStorage.setItem('arkbar_float_provider', currentProvider.value);
  const cached = getCachedUsage(currentProvider.value);
  if (cached) {
    usageData.value = cached;
  }
  fetchUsage();
}

async function closeWidget() {
  await invoke('close_float_window');
}

let isDragging = false;
let startScreenX = 0;
let startScreenY = 0;
let pendingTotalDx = 0;
let pendingTotalDy = 0;
let rafId: number | null = null;

function onPointerDown(e: PointerEvent) {
  if (e.button !== 0) return;
  const target = e.target as HTMLElement;
  if (target.closest('button') || target.closest('a') || target.closest('[data-no-drag]')) return;

  isDragging = true;
  startScreenX = e.screenX;
  startScreenY = e.screenY;
  pendingTotalDx = 0;
  pendingTotalDy = 0;

  try {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  } catch {}

  // Request Rust to record window start position
  invoke('start_drag_move').catch((err) => {
    console.error('start_drag_move failed:', err);
  });
}

function onPointerMove(e: PointerEvent) {
  if (!isDragging) return;

  pendingTotalDx = e.screenX - startScreenX;
  pendingTotalDy = e.screenY - startScreenY;

  if (!rafId) {
    rafId = requestAnimationFrame(async () => {
      rafId = null;
      if (!isDragging) return;
      const dx = pendingTotalDx;
      const dy = pendingTotalDy;
      try {
        await invoke('update_drag_move', { totalDx: dx, totalDy: dy });
      } catch {}
    });
  }
}

async function onPointerUp(e: PointerEvent) {
  if (isDragging) {
    isDragging = false;
    if (rafId) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
    const finalDx = e.screenX - startScreenX;
    const finalDy = e.screenY - startScreenY;
    try {
      (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
    } catch {}
    try {
      await invoke('update_drag_move', { totalDx: finalDx, totalDy: finalDy });
    } catch {}
    try {
      await invoke('end_drag_move');
    } catch {}
  }
}

function onPointerCancel(e: PointerEvent) {
  onPointerUp(e);
}

const currentPercent = computed(() => {
  if (usageData.value?.primary_session_percent != null) {
    return Math.round(usageData.value.primary_session_percent);
  }
  return null;
});

const barColor = computed(() => {
  const p = currentPercent.value ?? 0;
  if (p >= 90) return 'from-rose-500 to-red-600';
  if (p >= 75) return 'from-amber-500 to-orange-500';
  return 'from-indigo-500 to-violet-500';
});

const textColor = computed(() => {
  const p = currentPercent.value ?? 0;
  if (p >= 90) return 'text-rose-400';
  if (p >= 75) return 'text-amber-400';
  return 'text-white';
});

function formatMiniReset(dateStr?: string): string {
  if (!dateStr) return '';
  try {
    const target = new Date(dateStr).getTime();
    if (isNaN(target)) return dateStr;
    const diff = target - Date.now();
    if (diff <= 0) return '即重置';
    const hours = Math.floor(diff / (1000 * 60 * 60));
    const mins = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));
    if (hours > 24) {
      const days = Math.floor(hours / 24);
      return `${days}d后`;
    }
    if (hours > 0) return `${hours}h${mins}m`;
    return `${mins}m`;
  } catch {
    return '';
  }
}

const showQuickMenu = ref(false);
let quickMenuTimer: any = null;

function handleContextMenu(e: MouseEvent) {
  e.preventDefault();
  showQuickMenu.value = !showQuickMenu.value;
  if (showQuickMenu.value) {
    if (quickMenuTimer) clearTimeout(quickMenuTimer);
    quickMenuTimer = setTimeout(() => {
      showQuickMenu.value = false;
    }, 6000);
  }
}

async function openMain() {
  showQuickMenu.value = false;
  try {
    await invoke('show_main_window');
  } catch (err) {
    console.error('Failed to show main window:', err);
  }
}

async function handleExitApp() {
  try {
    await invoke('exit_app');
  } catch (err) {
    console.error('Failed to exit app:', err);
  }
}

onMounted(() => {
  fetchUsage();
  timer = setInterval(fetchUsage, 10 * 60 * 1000);
});

onUnmounted(() => {
  if (timer) clearInterval(timer);
  if (quickMenuTimer) clearTimeout(quickMenuTimer);
});
</script>

<template>
  <div
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointercancel="onPointerCancel"
    @contextmenu.prevent="handleContextMenu"
    class="w-full h-full select-none flex items-center justify-between px-2.5 py-1 bg-[#0b0f19]/95 text-slate-200 border border-slate-700/60 rounded-xl shadow-2xl backdrop-blur-md cursor-grab active:cursor-grabbing group hover:border-indigo-500/40 transition-colors"
    style="-webkit-user-drag: none; user-select: none;"
  >
    <!-- Right-Click Quick Menu Overlay -->
    <div
      v-if="showQuickMenu"
      class="w-full h-full flex items-center justify-between px-1 text-[11px] font-medium"
      data-no-drag
    >
      <button
        @click.stop="cycleProvider(); showQuickMenu = false;"
        class="px-1.5 py-1 hover:bg-slate-800 rounded text-slate-300 hover:text-white transition flex items-center gap-1 cursor-pointer"
        title="切换监控服务商"
      >
        <span>🔄</span>
        <span>切换</span>
      </button>
      <button
        @click.stop="openMain()"
        class="px-1.5 py-1 hover:bg-slate-800 rounded text-slate-300 hover:text-white transition flex items-center gap-1 cursor-pointer"
        title="打开主面板"
      >
        <span>🪟</span>
        <span>面板</span>
      </button>
      <button
        @click.stop="closeWidget"
        class="px-1.5 py-1 hover:bg-slate-800 rounded text-slate-300 hover:text-white transition flex items-center gap-1 cursor-pointer"
        title="隐藏悬浮窗"
      >
        <span>✖</span>
        <span>隐藏</span>
      </button>
      <button
        @click.stop="handleExitApp"
        class="px-1.5 py-1 bg-rose-500/20 hover:bg-rose-500/30 text-rose-300 rounded transition flex items-center gap-1 font-bold cursor-pointer"
        title="退出 ArkBar 应用程序"
      >
        <span>🚪</span>
        <span>退出</span>
      </button>
    </div>

    <!-- Normal Floating Pill Display -->
    <template v-else>
      <!-- Left: Provider Switcher Pill -->
      <div
        data-no-drag
        @click.stop="cycleProvider"
        class="flex items-center space-x-1.5 cursor-pointer hover:opacity-80 transition py-0.5 px-1 rounded-lg hover:bg-slate-800/60"
        title="点击切换监控服务商 (右键呼出菜单)"
      >
        <span class="text-sm select-none">{{ usageData?.icon || '🌋' }}</span>
        <span class="text-[10px] font-bold text-slate-300 font-mono">
          {{ usageData?.provider_name?.split(' ')[0] || 'Ark' }}
        </span>
      </div>

      <!-- Center: Progress & Usage -->
      <div class="flex-1 mx-2 flex flex-col justify-center space-y-0.5 pointer-events-none">
        <div class="flex items-center justify-between text-[9px] font-mono leading-none">
          <span class="text-slate-400 font-sans">
            {{ currentProvider === 'grok' ? '周期用量' : '5h用量' }}
          </span>
          <div class="flex items-center gap-1">
            <span v-if="usageData?.primary_reset_at" class="text-slate-500 text-[8px]">
              {{ formatMiniReset(usageData.primary_reset_at) }}
            </span>
            <span v-if="currentPercent !== null" class="font-bold" :class="textColor">
              {{ currentPercent }}%
            </span>
            <span v-else class="text-slate-500">
              {{ usageData?.is_connected ? '0%' : (isRefreshing ? '同步中' : '未连接') }}
            </span>
          </div>
        </div>
        <div class="w-full bg-slate-800 h-1.5 rounded-full overflow-hidden p-0.5">
          <div
            class="h-full rounded-full bg-gradient-to-r transition-all duration-300"
            :class="barColor"
            :style="{ width: `${currentPercent ?? (usageData?.is_connected ? 0 : (isRefreshing ? 50 : 10))}%` }"
          ></div>
        </div>
      </div>

      <!-- Right: Mini Actions -->
      <div data-no-drag class="flex items-center space-x-0.5 shrink-0">
        <button
          data-no-drag
          @click.stop="fetchUsage"
          :disabled="isRefreshing"
          class="p-1 rounded text-slate-400 hover:text-white hover:bg-slate-800 transition cursor-pointer"
          title="刷新数据"
        >
          <RefreshCw class="w-2.5 h-2.5" :class="{ 'animate-spin': isRefreshing }" />
        </button>
        <button
          data-no-drag
          @click.stop="closeWidget"
          class="p-1 rounded text-slate-400 hover:text-rose-400 hover:bg-slate-800 transition cursor-pointer"
          title="关闭悬浮窗"
        >
          <X class="w-2.5 h-2.5" />
        </button>
      </div>
    </template>
  </div>
</template>
