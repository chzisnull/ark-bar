<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
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
let refreshTimer: any = null;

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

async function fetchUsage() {
  isRefreshing.value = true;
  try {
    const data = await invoke<ProviderUsageData>('get_unified_usage', {
      provider: primaryProvider.value,
      customToken: null,
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

// Hover expansion
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
  if (!isHovered.value) return;

  hoverTimer = setTimeout(async () => {
    isHovered.value = false;
    try {
      await invoke('set_float_window_size', { width: 260.0, height: 44.0 });
    } catch {}
  }, 150);
}

// Drag logic
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

  // Collapse if expanded during drag to maintain smooth pill movement
  if (isHovered.value) {
    isHovered.value = false;
    invoke('set_float_window_size', { width: 260.0, height: 44.0 }).catch(() => {});
  }

  isDragging = true;
  startScreenX = e.screenX;
  startScreenY = e.screenY;
  pendingTotalDx = 0;
  pendingTotalDy = 0;

  try {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  } catch {}

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

onMounted(() => {
  fetchUsage();
  setTimeout(fetchAllUsage, 1500);
  refreshTimer = setInterval(fetchAllUsage, 10 * 60 * 1000);
  window.addEventListener('storage', handleStorageChange);
});

onUnmounted(() => {
  if (refreshTimer) clearInterval(refreshTimer);
  if (quickMenuTimer) clearTimeout(quickMenuTimer);
  if (hoverTimer) clearTimeout(hoverTimer);
  window.removeEventListener('storage', handleStorageChange);
});
</script>

<template>
  <div
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointercancel="onPointerCancel"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
    @contextmenu.prevent="handleContextMenu"
    class="w-full h-full select-none bg-[#0b0f19]/95 text-slate-200 border border-slate-700/60 rounded-xl shadow-2xl backdrop-blur-md cursor-grab active:cursor-grabbing group hover:border-indigo-500/40 transition-colors overflow-hidden flex flex-col justify-center"
    style="-webkit-user-drag: none; user-select: none;"
  >
    <!-- State 1: Right-Click Quick Actions Overlay (44px) -->
    <div
      v-if="showQuickMenu"
      class="w-full h-11 flex items-center justify-between px-2 text-[11px] font-medium"
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
      class="w-full h-full p-2 flex flex-col justify-between"
      data-no-drag
    >
      <div class="flex items-center justify-between pb-1 border-b border-slate-800/80 mb-1 px-1">
        <div class="flex items-center gap-1 text-[10px] text-slate-400 font-semibold">
          <span>✨</span>
          <span>已授权厂商用量</span>
        </div>
        <div class="flex items-center gap-1">
          <button
            @click.stop="fetchUsage"
            :disabled="isRefreshing"
            class="text-slate-400 hover:text-white p-0.5 rounded hover:bg-slate-800 transition"
            title="刷新数据"
          >
            <RefreshCw class="w-2.5 h-2.5" :class="{ 'animate-spin': isRefreshing }" />
          </button>
          <button
            @click.stop="closeWidget"
            class="text-slate-400 hover:text-rose-400 p-0.5 rounded hover:bg-slate-800 transition"
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
          @click.stop="setPrimary(item.id)"
          class="flex items-center justify-between px-1.5 py-1 rounded-lg hover:bg-slate-800/60 transition cursor-pointer"
          :class="item.id === primaryProvider ? 'bg-indigo-950/40 border border-indigo-500/30' : 'border border-transparent'"
          :title="`点击将 ${item.name} 设为首选常驻`"
        >
          <!-- Left: Provider Icon & Name -->
          <div class="flex items-center gap-1.5 min-w-[72px]">
            <span class="text-xs">{{ item.icon }}</span>
            <span class="text-[10px] font-medium text-slate-200 truncate">{{ item.name }}</span>
          </div>

          <!-- Center: Progress Mini Bar -->
          <div class="flex-1 mx-2 flex items-center gap-1.5">
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
          <div class="text-[9px] font-mono text-slate-500 min-w-[38px] text-right">
            <span v-if="item.id === primaryProvider" class="text-indigo-400 font-bold">首选</span>
            <span v-else>{{ formatMiniReset(item.data.primary_reset_at) }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- State 3: Compact Single-Row Pill Display (44px) -->
    <div
      v-else
      class="w-full h-11 flex items-center justify-between px-2.5 py-1"
    >
      <!-- Left: Provider Pill (Click to cycle) -->
      <div
        data-no-drag
        @click.stop="cycleProvider"
        class="flex items-center space-x-1.5 cursor-pointer hover:opacity-80 transition py-0.5 px-1 rounded-lg hover:bg-slate-800/60"
        title="点击切换展示厂商 (悬停展开所有已授权厂商，右键呼出菜单)"
      >
        <span class="text-sm select-none">{{ currentUsage?.icon || allProvidersList.find(p => p.id === primaryProvider)?.icon || '🌋' }}</span>
        <span class="text-[10px] font-bold text-slate-300 font-mono">
          {{ currentUsage?.provider_name?.split(' ')[0] || allProvidersList.find(p => p.id === primaryProvider)?.name || 'Ark' }}
        </span>
      </div>

      <!-- Center: Progress & Usage -->
      <div class="flex-1 mx-2 flex flex-col justify-center space-y-0.5 pointer-events-none">
        <div class="flex items-center justify-between text-[9px] font-mono leading-none">
          <span class="text-slate-400 font-sans">
            {{ primaryProvider === 'grok' ? '周期用量' : '5h用量' }}
          </span>
          <div class="flex items-center gap-1">
            <span v-if="currentUsage?.primary_reset_at" class="text-slate-500 text-[8px]">
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
    </div>
  </div>
</template>
