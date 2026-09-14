<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import type { ProviderType, ProviderUsageData } from '../types';
import { X, RefreshCw } from 'lucide-vue-next';

const CACHE_KEY = 'arkbar_cached_providers_data';

const allProvidersList: { id: ProviderType; name: string; icon: string }[] = [
  { id: 'volcengine', name: '火山方舟', icon: '🌋' },
  { id: 'grok', name: 'xAI Grok', icon: '⚡' },
  { id: 'antigravity', name: 'Antigravity', icon: '🌐' },
  { id: 'codex', name: 'Codex', icon: '🤖' },
];

function loadAllCachedData(): Record<ProviderType, ProviderUsageData | null> {
  const defaults: Record<ProviderType, ProviderUsageData | null> = {
    volcengine: null,
    antigravity: null,
    grok: null,
    codex: null,
  };
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    if (raw) {
      return { ...defaults, ...JSON.parse(raw) };
    }
  } catch {}
  return defaults;
}

const allCachedData = ref<Record<ProviderType, ProviderUsageData | null>>(loadAllCachedData());

// Primary default provider (configurable in settings)
const primaryProvider = ref<ProviderType>(
  (localStorage.getItem('arkbar_float_primary_provider') as ProviderType) ||
  (localStorage.getItem('arkbar_float_provider') as ProviderType) ||
  'volcengine'
);

const isRefreshing = ref(false);
const isHovered = ref(false);
const showQuickMenu = ref(false);
let quickMenuTimer: any = null;
let hoverTimer: any = null;

// Filter all authorized providers (is_connected === true)
const authorizedProviders = computed(() => {
  const list: { id: ProviderType; name: string; icon: string; data: ProviderUsageData }[] = [];

  // 1. Primary provider comes first
  const primaryMeta = allProvidersList.find((p) => p.id === primaryProvider.value)!;
  const primaryData = allCachedData.value[primaryProvider.value];
  if (primaryData?.is_connected) {
    list.push({ ...primaryMeta, data: primaryData });
  }

  // 2. Add remaining authorized providers
  for (const p of allProvidersList) {
    if (p.id !== primaryProvider.value) {
      const data = allCachedData.value[p.id];
      if (data?.is_connected) {
        list.push({ ...p, data });
      }
    }
  }

  // If none is marked connected, fallback to primary default
  if (list.length === 0) {
    const fallbackData = primaryData || ({
      provider: primaryProvider.value,
      provider_name: primaryMeta.name,
      icon: primaryMeta.icon,
      is_connected: false,
      groups: [],
    } as ProviderUsageData);
    list.push({ ...primaryMeta, data: fallbackData });
  }

  return list;
});

const currentUsage = computed(() => {
  return allCachedData.value[primaryProvider.value];
});

const currentPercent = computed(() => {
  if (currentUsage.value?.primary_session_percent != null) {
    return Math.round(currentUsage.value.primary_session_percent);
  }
  return null;
});

function getBarColor(percent: number | null): string {
  const p = percent ?? 0;
  if (p >= 90) return 'from-rose-500 to-red-600';
  if (p >= 75) return 'from-amber-500 to-orange-500';
  return 'from-indigo-500 to-violet-500';
}

function getTextColor(percent: number | null): string {
  const p = percent ?? 0;
  if (p >= 90) return 'text-rose-400';
  if (p >= 75) return 'text-amber-400';
  return 'text-white';
}

function parseResetTime(dateStr?: string): number | null {
  if (!dateStr) return null;
  const trimmed = dateStr.trim();
  if (/^\d+$/.test(trimmed)) {
    const num = parseInt(trimmed, 10);
    return num < 1e11 ? num * 1000 : num;
  }
  const t = new Date(trimmed).getTime();
  return isNaN(t) ? null : t;
}

function formatExactTime(dateStr?: string): string {
  const target = parseResetTime(dateStr);
  if (!target) return dateStr || '';
  const d = new Date(target);
  const year = d.getFullYear();
  const month = String(d.getMonth() + 1).padStart(2, '0');
  const date = String(d.getDate()).padStart(2, '0');
  const hours = String(d.getHours()).padStart(2, '0');
  const mins = String(d.getMinutes()).padStart(2, '0');
  return `${year}-${month}-${date} ${hours}:${mins}`;
}

function formatMiniReset(dateStr?: string): string {
  const target = parseResetTime(dateStr);
  if (!target) return '';
  try {
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

async function fetchUsage() {
  isRefreshing.value = true;
  try {
    const data = await invoke<ProviderUsageData>('get_unified_usage', {
      provider: primaryProvider.value,
      customToken: null,
      force: true,
    });
    allCachedData.value[primaryProvider.value] = data;
    try {
      localStorage.setItem(CACHE_KEY, JSON.stringify(allCachedData.value));
    } catch {}
  } catch (err) {
    console.error('Floating widget fetch failed:', err);
  } finally {
    isRefreshing.value = false;
  }
}

async function fetchAllUsage() {
  for (const p of allProvidersList) {
    try {
      const data = await invoke<ProviderUsageData>('get_unified_usage', {
        provider: p.id,
        customToken: null,
      });
      allCachedData.value[p.id] = data;
    } catch {}
  }
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(allCachedData.value));
  } catch {}
}

function setPrimary(provider: ProviderType) {
  primaryProvider.value = provider;
  localStorage.setItem('arkbar_float_primary_provider', provider);
  localStorage.setItem('arkbar_float_provider', provider);
  fetchUsage();
}

function cycleProvider() {
  const ids = allProvidersList.map((p) => p.id);
  const curIdx = ids.indexOf(primaryProvider.value);
  const nextIdx = (curIdx + 1) % ids.length;
  setPrimary(ids[nextIdx]);
}

async function closeWidget() {
  await invoke('close_float_window');
}

// Native OS drag (data-tauri-drag-region) takes over the mouse session.
// Freeze hover resize while a drag is in progress so the window doesn't
// jump or swallow the gesture when its size changes under the cursor.
let isDragging = false;

function freezeHoverForDrag() {
  isDragging = true;
  if (hoverTimer) clearTimeout(hoverTimer);
  getCurrentWebviewWindow().startDragging().catch((err) => {
    console.error('startDragging failed:', err);
  });
}

function releaseHoverFreeze() {
  isDragging = false;
}

function handleMouseEnter() {
  if (isDragging || showQuickMenu.value) return;
  if (hoverTimer) clearTimeout(hoverTimer);

  hoverTimer = setTimeout(async () => {
    if (isDragging || showQuickMenu.value) return;
    const count = authorizedProviders.value.length;
    if (count > 1) {
      isHovered.value = true;
      const targetHeight = Math.min(200, 32 + count * 36);
      try {
        await invoke('set_float_window_size', { width: 260.0, height: Number(targetHeight) });
      } catch {}
    }
  }, 120);
}

function handleMouseLeave() {
  if (hoverTimer) clearTimeout(hoverTimer);
  if (isDragging || !isHovered.value) return;

  hoverTimer = setTimeout(async () => {
    if (isDragging) return;
    isHovered.value = false;
    try {
      await invoke('set_float_window_size', { width: 260.0, height: 44.0 });
    } catch {}
  }, 150);
}

function handleContextMenu(e: MouseEvent) {
  e.preventDefault();
  if (isHovered.value) {
    isHovered.value = false;
    invoke('set_float_window_size', { width: 260.0, height: 44.0 }).catch(() => {});
  }
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

// Storage event to sync primary provider changes from settings
function handleStorageChange(e: StorageEvent) {
  if (e.key === 'arkbar_float_primary_provider' && e.newValue) {
    primaryProvider.value = e.newValue as ProviderType;
    fetchUsage();
  }
  if (e.key === CACHE_KEY && e.newValue) {
    try {
      allCachedData.value = JSON.parse(e.newValue);
    } catch {}
  }
}

// Rust 后台线程每轮刷新后逐厂商广播；悬浮窗自身的 setInterval 在窗口隐藏
// 时同样会被系统挂起，所以数据更新以这里的事件为主。
function handleUsageUpdated(data: ProviderUsageData) {
  if (!data || !(data.provider in allCachedData.value)) return;
  allCachedData.value[data.provider as ProviderType] = data;
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(allCachedData.value));
  } catch {}
}

let unlistenUsage: (() => void) | null = null;
let unlistenFocus: (() => void) | null = null;
let lastFocusRefresh = 0;

onMounted(() => {
  // Render from local cache immediately. A delayed, silent refresh of the
  // primary provider is enough — the main window already prefetches all four.
  if (!currentUsage.value) {
    fetchUsage();
  }
  setTimeout(fetchAllUsage, 8000);
  listen<ProviderUsageData>('usage-updated', (event) => handleUsageUpdated(event.payload)).then((un) => {
    unlistenUsage = un;
  });
  // 窗口隐藏期间事件可能丢失，重新显示/聚焦时补一次非强制刷新
  // （Rust 缓存通常已被后台线程刷新，秒回）。
  getCurrentWebviewWindow().listen('tauri://focus', () => {
    const now = Date.now();
    if (now - lastFocusRefresh < 30000) return;
    lastFocusRefresh = now;
    fetchAllUsage();
  }).then((un) => {
    unlistenFocus = un;
  });
  window.addEventListener('storage', handleStorageChange);
  window.addEventListener('pointerup', releaseHoverFreeze);
  window.addEventListener('mouseup', releaseHoverFreeze);
});

onUnmounted(() => {
  if (unlistenUsage) unlistenUsage();
  if (unlistenFocus) unlistenFocus();
  if (quickMenuTimer) clearTimeout(quickMenuTimer);
  if (hoverTimer) clearTimeout(hoverTimer);
  window.removeEventListener('storage', handleStorageChange);
  window.removeEventListener('pointerup', releaseHoverFreeze);
  window.removeEventListener('mouseup', releaseHoverFreeze);
});
</script>

<template>
  <div
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
    @contextmenu.prevent="handleContextMenu"
    class="w-full h-full select-none bg-[#0b0f19]/95 text-slate-200 border border-slate-700/60 rounded-xl shadow-2xl backdrop-blur-md group hover:border-indigo-500/40 transition-colors overflow-hidden flex flex-col justify-center relative"
    style="-webkit-user-drag: none; user-select: none;"
  >
    <!-- Native drag layer: OS moves the window. Buttons sit above this. -->
    <div
      v-if="!showQuickMenu"
      data-tauri-drag-region
      class="absolute inset-0 z-0 cursor-grab active:cursor-grabbing"
      @pointerdown="freezeHoverForDrag"
    ></div>

    <!-- State 1: Right-Click Quick Actions Overlay (44px) -->
    <div
      v-if="showQuickMenu"
      class="relative z-10 w-full h-11 flex items-center justify-between px-2 text-[11px] font-medium pointer-events-auto"
      data-no-drag
    >
      <button
        @click.stop="cycleProvider(); showQuickMenu = false;"
        class="px-1.5 py-1 hover:bg-slate-800 rounded text-slate-300 hover:text-white transition flex items-center gap-1 cursor-pointer"
        title="切换下一服务商"
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
        title="退出 ArkBar"
      >
        <span>🚪</span>
        <span>退出</span>
      </button>
    </div>

    <!-- State 2: Multi-Provider Hover Dashboard (Expanded View) -->
    <div
      v-else-if="isHovered && authorizedProviders.length > 1"
      class="relative z-10 w-full h-full p-2 flex flex-col justify-between pointer-events-none"
    >
      <div class="flex items-center justify-between pb-1 border-b border-slate-800/80 mb-1 px-1">
        <div class="flex items-center gap-1 text-[10px] text-slate-400 font-semibold">
          <span>✨</span>
          <span>已授权厂商用量</span>
        </div>
        <div class="flex items-center gap-1 pointer-events-auto">
          <button
            @click.stop="fetchUsage"
            :disabled="isRefreshing"
            class="text-slate-400 hover:text-white p-0.5 rounded hover:bg-slate-800 transition cursor-pointer"
            title="刷新数据"
          >
            <RefreshCw class="w-2.5 h-2.5" :class="{ 'animate-spin': isRefreshing }" />
          </button>
          <button
            @click.stop="closeWidget"
            class="text-slate-400 hover:text-rose-400 p-0.5 rounded hover:bg-slate-800 transition cursor-pointer"
            title="关闭悬浮窗"
          >
            <X class="w-2.5 h-2.5" />
          </button>
        </div>
      </div>

      <!-- Provider Rows List -->
      <div class="flex-1 flex flex-col justify-around gap-1">
        <div
          v-for="item in authorizedProviders"
          :key="item.id"
          class="flex items-center justify-between px-1.5 py-1 rounded-lg"
          :class="item.id === primaryProvider ? 'bg-indigo-950/40 border border-indigo-500/30' : 'border border-transparent'"
        >
          <!-- Left: Provider Icon & Name (button so the rest of the row stays draggable) -->
          <button
            type="button"
            class="flex items-center gap-1.5 min-w-[72px] pointer-events-auto cursor-pointer rounded px-0.5 hover:bg-slate-800/60"
            :title="`点击将 ${item.name} 设为首选常驻`"
            @click.stop="setPrimary(item.id)"
          >
            <span class="text-xs">{{ item.icon }}</span>
            <span class="text-[10px] font-medium text-slate-200 truncate">{{ item.name }}</span>
          </button>

          <!-- Center: Progress Mini Bar -->
          <div class="flex-1 mx-2 flex items-center gap-1.5 pointer-events-none">
            <div class="flex-1 bg-slate-800 h-1.5 rounded-full overflow-hidden">
              <div
                class="h-full rounded-full bg-gradient-to-r transition-all duration-300"
                :class="getBarColor(item.data.primary_session_percent != null ? Math.round(item.data.primary_session_percent) : null)"
                :style="{ width: `${item.data.primary_session_percent != null ? Math.round(item.data.primary_session_percent) : 0}%` }"
              ></div>
            </div>
            <span
              class="text-[9px] font-mono font-bold w-7 text-right"
              :class="getTextColor(item.data.primary_session_percent != null ? Math.round(item.data.primary_session_percent) : null)"
            >
              {{ item.data.primary_session_percent != null ? `${Math.round(item.data.primary_session_percent)}%` : '--' }}
            </span>
          </div>

          <!-- Right: Reset Time or Star Badge -->
          <div class="text-[9px] font-mono min-w-[38px] text-right pointer-events-none">
            <span
              v-if="item.id === primaryProvider"
              class="text-indigo-400 font-bold"
              :title="item.data.primary_reset_at ? `刷新时间: ${formatExactTime(item.data.primary_reset_at)}` : '当前首选常驻'"
            >
              {{ formatMiniReset(item.data.primary_reset_at) || '首选' }}
            </span>
            <span
              v-else
              class="text-slate-400"
              :title="item.data.primary_reset_at ? `刷新时间: ${formatExactTime(item.data.primary_reset_at)}` : ''"
            >
              {{ formatMiniReset(item.data.primary_reset_at) }}
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- State 3: Compact Single-Row Pill Display (44px) -->
    <div
      v-else
      class="relative z-10 w-full h-11 flex items-center justify-between px-2.5 py-1 pointer-events-none"
    >
      <!-- Left: Provider Pill (Click to cycle). Rest of the pill is a native drag region. -->
      <button
        type="button"
        class="flex items-center space-x-1.5 pointer-events-auto cursor-pointer hover:opacity-80 transition py-0.5 px-1 rounded-lg hover:bg-slate-800/60"
        title="点击切换展示厂商 (悬停展开所有已授权厂商，右键呼出菜单)"
        @click.stop="cycleProvider"
      >
        <span class="text-sm select-none">{{ currentUsage?.icon || allProvidersList.find(p => p.id === primaryProvider)?.icon || '🌋' }}</span>
        <span class="text-[10px] font-bold text-slate-300 font-mono">
          {{ currentUsage?.provider_name?.split(' ')[0] || allProvidersList.find(p => p.id === primaryProvider)?.name || 'Ark' }}
        </span>
      </button>

      <!-- Center: Progress & Usage -->
      <div class="flex-1 mx-2 flex flex-col justify-center space-y-0.5 pointer-events-none">
        <div class="flex items-center justify-between text-[9px] font-mono leading-none">
          <span class="text-slate-400 font-sans">
            {{ primaryProvider === 'grok' ? '周期用量' : '5h用量' }}
          </span>
          <div class="flex items-center gap-1">
            <span
              v-if="currentUsage?.primary_reset_at"
              class="text-slate-400 text-[8px]"
              :title="`刷新时间: ${formatExactTime(currentUsage.primary_reset_at)}`"
            >
              {{ formatMiniReset(currentUsage.primary_reset_at) }}
            </span>
            <span v-if="currentPercent !== null" class="font-bold" :class="getTextColor(currentPercent)">
              {{ currentPercent }}%
            </span>
            <span v-else class="text-slate-500">
              {{ currentUsage?.is_connected ? '0%' : (isRefreshing ? '同步中' : '未连接') }}
            </span>
          </div>
        </div>
        <div class="w-full bg-slate-800 h-1.5 rounded-full overflow-hidden p-0.5">
          <div
            class="h-full rounded-full bg-gradient-to-r transition-all duration-300"
            :class="getBarColor(currentPercent)"
            :style="{ width: `${currentPercent ?? (currentUsage?.is_connected ? 0 : (isRefreshing ? 50 : 10))}%` }"
          ></div>
        </div>
      </div>

      <!-- Right: Mini Actions -->
      <div class="flex items-center space-x-0.5 shrink-0 pointer-events-auto">
        <button
          @click.stop="fetchUsage"
          :disabled="isRefreshing"
          class="p-1 rounded text-slate-400 hover:text-white hover:bg-slate-800 transition cursor-pointer"
          title="刷新数据"
        >
          <RefreshCw class="w-2.5 h-2.5" :class="{ 'animate-spin': isRefreshing }" />
        </button>
        <button
          @click.stop="closeWidget"
          class="p-1 rounded text-slate-400 hover:text-rose-400 hover:bg-slate-800 transition cursor-pointer"
          title="关闭悬浮窗"
        >
          <X class="w-2.5 h-2.5" />
        </button>
      </div>
    </div>
  </div>
</template>
