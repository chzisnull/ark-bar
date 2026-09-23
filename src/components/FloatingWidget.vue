<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ProviderType, ProviderUsageData, NotchPosition, NotchMetric } from '../types';
import ProviderIcon from './ProviderIcon.vue';
import { Settings, GripVertical } from 'lucide-vue-next';

const CACHE_KEY = 'arkbar_cached_providers_data';

interface ProviderTabItem {
  id: ProviderType;
  name: string;
  visible: boolean;
  notch_metric?: NotchMetric;
}

const DEFAULT_TABS: ProviderTabItem[] = [
  { id: 'antigravity', name: 'Antigravity', visible: true, notch_metric: 'session' },
  { id: 'grok', name: 'Grok', visible: true, notch_metric: 'session' },
  { id: 'volcengine', name: '火山方舟', visible: true, notch_metric: 'weekly' },
  { id: 'codex', name: 'Codex', visible: true, notch_metric: 'session' },
  { id: 'teamo', name: 'Teamo', visible: true, notch_metric: 'balance' },
];

function loadProviderTabs(): ProviderTabItem[] {
  try {
    const raw = localStorage.getItem('arkbar_provider_tabs');
    if (raw) {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed) && parsed.length > 0) return parsed;
    }
  } catch {}
  return DEFAULT_TABS;
}

function loadCachedData(): Record<ProviderType, ProviderUsageData | null> {
  const defaults: Record<ProviderType, ProviderUsageData | null> = {
    volcengine: null,
    antigravity: null,
    grok: null,
    codex: null,
    teamo: null,
  };
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    if (raw) {
      return { ...defaults, ...JSON.parse(raw) };
    }
  } catch {}
  return defaults;
}

const allCachedData = ref<Record<ProviderType, ProviderUsageData | null>>(loadCachedData());
const providerTabs = ref<ProviderTabItem[]>(loadProviderTabs());
const notchPosition = ref<NotchPosition>(
  (localStorage.getItem('arkbar_notch_position') as NotchPosition) || 'right'
);
const autoHide = ref<boolean>(localStorage.getItem('arkbar_notch_auto_hide') === 'true');

// Hover & Popover management
const activeHoverId = ref<ProviderType | null>(null);
const activeHoverIndex = ref<number>(0);
const isHoveringNotch = ref<boolean>(false);
const isHoveringPopover = ref<boolean>(false);
let closeTimer: any = null;

// Visible active providers to display in the Notch
const notchProviders = computed(() => {
  return providerTabs.value
    .filter((t) => t.visible)
    .map((tab) => {
      const data = allCachedData.value[tab.id];
      return {
        id: tab.id,
        name: tab.name,
        notch_metric: tab.notch_metric || 'session',
        data,
      };
    });
});

// Currently active popover data
const activeHoverData = computed<ProviderUsageData | null>(() => {
  if (!activeHoverId.value) return null;
  return allCachedData.value[activeHoverId.value] || null;
});

// Compute the representative display percentage for each provider ring
function getProviderDisplayPercent(id: ProviderType, metric?: NotchMetric): number {
  const data = allCachedData.value[id];
  if (!data) return 0;

  // Custom metric handling
  if (metric === 'weekly') {
    for (const g of data.groups || []) {
      const p = g.periods.find((x) => x.label.toLowerCase().includes('week'));
      if (p) return Math.round(p.used_percent);
    }
  } else if (metric === 'monthly') {
    for (const g of data.groups || []) {
      const p = g.periods.find((x) => x.label.toLowerCase().includes('month'));
      if (p) return Math.round(p.used_percent);
    }
  } else if (metric === 'balance' && data.extension?.balance) {
    // For balance, if we have balance amount, return percentage of max or cap
    const bal = data.extension.balance.value;
    return Math.min(100, Math.max(0, Math.round(bal)));
  }

  // Fallback to primary session or first valid period
  if (data.primary_session_percent != null) {
    return Math.round(data.primary_session_percent);
  }

  for (const g of data.groups || []) {
    if (g.periods.length > 0) {
      return Math.round(g.periods[0].used_percent);
    }
  }

  return 0;
}

// Compute badge text shown under the ring
function getProviderBadgeText(id: ProviderType, metric?: NotchMetric): string {
  const data = allCachedData.value[id];
  if (!data || !data.is_connected) return '--';

  if (id === 'teamo' && data.extension?.balance?.value != null) {
    const val = data.extension.balance.value;
    return val >= 100 ? `$${Math.round(val)}` : `$${val.toFixed(1)}`;
  }

  const p = getProviderDisplayPercent(id, metric);
  return `${p}%`;
}

// Circular ring stroke color based on percentage
function getRingStrokeColor(percent: number): string {
  if (percent >= 85) return '#f43f5e'; // Rose / red alert
  if (percent >= 60) return '#eab308'; // Amber / Gold (matching Codenotch screenshots)
  return '#38bdf8'; // Sky blue / Emerald safe
}

// Circular ring SVG dash calculation (radius = 18, circumference = 2 * PI * 18 = 113.097)
const CIRCUMFERENCE = 113.1;
function getRingDashOffset(percent: number): number {
  const clamped = Math.max(0, Math.min(100, percent));
  return CIRCUMFERENCE - (clamped / 100) * CIRCUMFERENCE;
}

// Formatting reset time helper
function formatResetLabel(resetAt?: string): string {
  if (!resetAt) return '';
  const trimmed = resetAt.trim();

  // If timestamp
  let date: Date | null = null;
  if (/^\d+$/.test(trimmed)) {
    const n = parseInt(trimmed, 10);
    date = new Date(n < 1e11 ? n * 1000 : n);
  } else {
    const parsed = new Date(trimmed);
    if (!isNaN(parsed.getTime())) date = parsed;
  }

  if (!date) return `${trimmed} 重置`;

  const now = new Date();
  const days = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'];
  const dayName = days[date.getDay()];
  const hh = String(date.getHours()).padStart(2, '0');
  const mm = String(date.getMinutes()).padStart(2, '0');

  // If within this week
  const diffDays = Math.floor((date.getTime() - now.getTime()) / (1000 * 60 * 60 * 24));
  if (diffDays < 7) {
    return `${dayName} ${hh}:${mm} 重置`;
  }
  return `${date.getMonth() + 1}月${date.getDate()}日 重置`;
}

// Hover interaction triggers
function handleProviderMouseEnter(id: ProviderType, index: number) {
  if (closeTimer) {
    clearTimeout(closeTimer);
    closeTimer = null;
  }
  isHoveringNotch.value = true;
  activeHoverId.value = id;
  activeHoverIndex.value = index;
}

function handleProviderMouseLeave() {
  closeTimer = setTimeout(() => {
    if (!isHoveringPopover.value) {
      activeHoverId.value = null;
    }
  }, 180);
}

function handlePopoverMouseEnter() {
  if (closeTimer) {
    clearTimeout(closeTimer);
    closeTimer = null;
  }
  isHoveringPopover.value = true;
}

function handlePopoverMouseLeave() {
  closeTimer = setTimeout(() => {
    isHoveringPopover.value = false;
    activeHoverId.value = null;
  }, 150);
}

// Dragging window along screen edge
let isDragging = false;
let dragStartX = 0;
let dragStartY = 0;

function handleDragMouseDown(e: MouseEvent) {
  isDragging = true;
  dragStartX = e.screenX;
  dragStartY = e.screenY;
  invoke('start_drag_move').catch(() => {});

  const onMouseMove = (ev: MouseEvent) => {
    if (!isDragging) return;
    const dx = ev.screenX - dragStartX;
    const dy = ev.screenY - dragStartY;
    invoke('update_drag_move', { totalDx: dx, totalDy: dy }).catch(() => {});
  };

  const onMouseUp = () => {
    isDragging = false;
    invoke('end_drag_move').catch(() => {});
    window.removeEventListener('mousemove', onMouseMove);
    window.removeEventListener('mouseup', onMouseUp);
  };

  window.addEventListener('mousemove', onMouseMove);
  window.addEventListener('mouseup', onMouseUp);
}

// Open settings window / popover
function openSettings() {
  invoke('show_main_window').catch(() => {});
}

// Sync updates
let unlistenUsage: (() => void) | null = null;

onMounted(async () => {
  // Sync latest cached usage from backend
  try {
    const list: ProviderUsageData[] = await invoke('get_all_providers_usage');
    if (Array.isArray(list)) {
      list.forEach((item) => {
        allCachedData.value[item.provider] = item;
      });
      localStorage.setItem(CACHE_KEY, JSON.stringify(allCachedData.value));
    }
  } catch {}

  // Listen to background sync updates
  unlistenUsage = await listen<Record<ProviderType, ProviderUsageData>>('usage-updated', (event) => {
    if (event.payload) {
      allCachedData.value = { ...allCachedData.value, ...event.payload };
      localStorage.setItem(CACHE_KEY, JSON.stringify(allCachedData.value));
    }
  });

  // Listen to local storage changes from settings
  window.addEventListener('storage', (e) => {
    if (e.key === 'arkbar_provider_tabs') {
      providerTabs.value = loadProviderTabs();
    } else if (e.key === 'arkbar_notch_position') {
      notchPosition.value = (e.newValue as NotchPosition) || 'right';
    } else if (e.key === 'arkbar_notch_auto_hide') {
      autoHide.value = e.newValue === 'true';
    } else if (e.key === CACHE_KEY) {
      allCachedData.value = loadCachedData();
    }
  });
});

onUnmounted(() => {
  if (unlistenUsage) unlistenUsage();
});
</script>

<template>
  <div
    class="w-screen h-screen relative select-none overflow-visible flex items-center"
    :class="[
      notchPosition === 'left' ? 'justify-start pl-0' : 'justify-end pr-0',
    ]"
  >
    <!-- ============================================================ -->
    <!-- SPEECH BUBBLE FLYOUT POPOVER CARD (Images 0 & 1)             -->
    <!-- ============================================================ -->
    <Transition :name="notchPosition === 'left' ? 'spring-pop-left' : 'spring-pop'">
      <div
        v-if="activeHoverId && activeHoverData"
        class="absolute z-50 pointer-events-auto"
        :style="{
          top: `${Math.max(24, Math.min(360, 48 + activeHoverIndex * 70))}px`,
          [notchPosition === 'left' ? 'left' : 'right']: '82px',
        }"
        @mouseenter="handlePopoverMouseEnter"
        @mouseleave="handlePopoverMouseLeave"
      >
        <!-- Speech Bubble Container -->
        <div class="relative flex items-center">
          <!-- Caret Pointer Triangle (Right-docked: points right to the notch icon) -->
          <div
            v-if="notchPosition !== 'left'"
            class="absolute -right-2.5 w-0 h-0 border-t-[8px] border-t-transparent border-b-[8px] border-b-transparent border-l-[10px] border-l-[#1e1f24]/95 drop-shadow-[2px_0_2px_rgba(0,0,0,0.15)]"
          />

          <!-- Main Card Box -->
          <div
            class="w-[304px] glass-popover dark:bg-[#1e1f24]/95 text-neutral-900 dark:text-neutral-100 rounded-[22px] p-4 shadow-[0_22px_45px_rgba(0,0,0,0.38)] border border-black/10 dark:border-white/15 backdrop-blur-3xl overflow-hidden"
          >
            <!-- 1. Header: Icon + Title + Action -->
            <div class="flex items-center justify-between pb-3 border-b border-black/5 dark:border-white/10">
              <div class="flex items-center gap-2.5">
                <div
                  class="w-7 h-7 rounded-lg bg-neutral-200 dark:bg-neutral-800/80 flex items-center justify-center p-1.5 shadow-sm text-neutral-800 dark:text-white"
                >
                  <ProviderIcon :name="activeHoverId" class="w-4 h-4" />
                </div>
                <h3 class="text-[15px] font-bold tracking-tight text-neutral-900 dark:text-white">
                  {{ activeHoverData.provider_name }} 用量
                </h3>
              </div>

              <button
                @click="openSettings"
                class="p-1.5 rounded-lg text-neutral-400 hover:text-white hover:bg-white/10 transition-colors"
                title="打开偏好设置"
              >
                <Settings class="w-4 h-4" />
              </button>
            </div>

            <!-- 2. Groups & Limit Cards -->
            <div class="pt-3 max-h-[360px] overflow-y-auto space-y-3">
              <!-- Teamo Special Card: Balance & Today Cost -->
              <div
                v-if="activeHoverId === 'teamo' && activeHoverData.extension?.balance"
                class="bg-black/[0.04] dark:bg-white/[0.05] rounded-xl p-3 border border-black/5 dark:border-white/5"
              >
                <div class="flex items-center justify-between text-xs text-neutral-500 dark:text-neutral-400 mb-1">
                  <span>账户余额</span>
                  <span>今日消费</span>
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-xl font-bold text-emerald-500 dark:text-emerald-400">
                    ${{ activeHoverData.extension.balance.value.toFixed(2) }}
                    <span class="text-xs font-normal text-neutral-400">{{ activeHoverData.extension.balance.currency }}</span>
                  </span>
                  <span class="text-sm font-semibold text-neutral-700 dark:text-neutral-300">
                    ${{ (activeHoverData.extension.today_cost?.value ?? 0).toFixed(2) }}
                  </span>
                </div>
              </div>

              <!-- Standard Plan Groups (Gemini Models, Coding Plan, Grok Build, etc.) -->
              <div
                v-for="(group, gIdx) in activeHoverData.groups"
                :key="gIdx"
                class="bg-black/[0.03] dark:bg-white/[0.05] rounded-xl p-3 border border-black/5 dark:border-white/5 space-y-3"
              >
                <div v-if="group.group_name" class="text-xs font-semibold text-neutral-500 dark:text-neutral-400">
                  {{ group.group_name }}
                </div>

                <div
                  v-for="(period, pIdx) in group.periods"
                  :key="pIdx"
                  class="space-y-1.5"
                >
                  <!-- Label & Reset Time -->
                  <div class="flex items-center justify-between text-xs">
                    <span class="font-medium text-neutral-700 dark:text-neutral-300">
                      {{ period.name || period.label }}
                    </span>
                    <span class="text-neutral-400 dark:text-neutral-400 text-[11px]">
                      {{ formatResetLabel(period.reset_at) }}
                    </span>
                  </div>

                  <!-- Progress Bar -->
                  <div class="h-1.5 w-full bg-neutral-200 dark:bg-neutral-700/60 rounded-full overflow-hidden">
                    <div
                      class="h-full rounded-full transition-all duration-700 ease-out"
                      :style="{
                        width: `${Math.min(100, Math.max(0, period.used_percent))}%`,
                        backgroundColor: getRingStrokeColor(period.used_percent),
                      }"
                    />
                  </div>

                  <!-- Percentage & Remaining -->
                  <div class="text-[11px] text-neutral-500 dark:text-neutral-400 font-medium">
                    {{ Math.round(period.used_percent) }}% 已用 · {{ Math.max(0, 100 - Math.round(period.used_percent)) }}% 剩余
                  </div>
                </div>
              </div>

              <!-- Token / Cache Hit Rate badge if supported -->
              <div
                v-if="activeHoverData.token_summary"
                class="flex items-center justify-between px-1 text-[11px] text-neutral-400"
              >
                <span>今日 Token: {{ activeHoverData.token_summary.today_tokens.toLocaleString() }}</span>
                <span v-if="activeHoverData.token_summary.cache_hit_rate">
                  ⚡ 缓存命中 {{ activeHoverData.token_summary.cache_hit_rate }}%
                </span>
              </div>
            </div>

            <!-- 3. Footer: Status Indicator (工作中 刚刚) -->
            <div class="pt-3 mt-2 border-t border-black/5 dark:border-white/10 flex items-center justify-between text-xs text-neutral-500 dark:text-neutral-400">
              <div class="flex items-center gap-1.5">
                <span class="font-medium text-neutral-700 dark:text-neutral-300">{{ activeHoverData.provider_name }}</span>
                <span class="text-[11px] text-neutral-400">· 已连接</span>
              </div>

              <div class="flex items-center gap-1 text-[11px] text-emerald-500 dark:text-emerald-400">
                <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                <span>正常同步</span>
                <span class="text-neutral-400 ml-1">刚刚</span>
              </div>
            </div>
          </div>

          <!-- Caret Pointer Triangle (Left-docked: points left) -->
          <div
            v-if="notchPosition === 'left'"
            class="absolute -left-2.5 w-0 h-0 border-t-[8px] border-t-transparent border-b-[8px] border-b-transparent border-r-[10px] border-r-[#1e1f24]/95 drop-shadow-[-2px_0_2px_rgba(0,0,0,0.15)]"
          />
        </div>
      </div>
    </Transition>

    <!-- ============================================================ -->
    <!-- THE SCREEN EDGE NOTCH DOCK (Images 0 & 1)                   -->
    <!-- ============================================================ -->
    <div
      class="group relative z-40 pointer-events-auto flex flex-col items-center select-none"
      :class="[
        notchPosition === 'left' ? 'rounded-r-[26px] pl-2 pr-3.5' : 'rounded-l-[26px] pr-2 pl-3.5',
        autoHide ? 'opacity-80 hover:opacity-100' : 'opacity-100',
      ]"
      @mouseenter="isHoveringNotch = true"
      @mouseleave="isHoveringNotch = false"
    >
      <!-- Background Frosted Pill Container -->
      <div
        class="glass-notch dark:bg-[#16171b]/92 border border-white/10 shadow-[-10px_0_30px_rgba(0,0,0,0.5)] py-4 flex flex-col items-center gap-4 transition-all duration-300"
        :class="[
          notchPosition === 'left' ? 'rounded-r-[26px] border-l-0 pl-1.5 pr-2.5' : 'rounded-l-[26px] border-r-0 pr-1.5 pl-2.5',
          'w-[64px]',
        ]"
      >
        <!-- Top Drag Grip Handle -->
        <div
          class="cursor-grab active:cursor-grabbing text-neutral-500 hover:text-neutral-300 py-1 transition-colors"
          title="按住拖拽调整刘海位置"
          @mousedown="handleDragMouseDown"
        >
          <GripVertical class="w-4 h-4 opacity-50 hover:opacity-100" />
        </div>

        <!-- Provider Circular Rings List -->
        <div class="flex flex-col items-center gap-4">
          <div
            v-for="(item, index) in notchProviders"
            :key="item.id"
            class="relative flex flex-col items-center cursor-pointer group/ring transition-transform duration-200"
            :class="activeHoverId === item.id ? 'scale-110' : 'hover:scale-105'"
            @mouseenter="handleProviderMouseEnter(item.id, index)"
            @mouseleave="handleProviderMouseLeave"
            @click="openSettings"
          >
            <!-- Circular Activity Gauge Ring -->
            <div class="relative w-11 h-11 flex items-center justify-center">
              <svg class="w-11 h-11 -rotate-90 origin-center" viewBox="0 0 44 44">
                <!-- Track background circle -->
                <circle
                  cx="22"
                  cy="22"
                  r="18"
                  fill="none"
                  stroke="rgba(255, 255, 255, 0.12)"
                  stroke-width="3.5"
                />
                <!-- Progress animated circle -->
                <circle
                  cx="22"
                  cy="22"
                  r="18"
                  fill="none"
                  stroke-width="3.5"
                  stroke-linecap="round"
                  class="gauge-ring"
                  :stroke="getRingStrokeColor(getProviderDisplayPercent(item.id, item.notch_metric))"
                  :stroke-dasharray="CIRCUMFERENCE"
                  :stroke-dashoffset="getRingDashOffset(getProviderDisplayPercent(item.id, item.notch_metric))"
                />
              </svg>

              <!-- Center Provider SVG Logo -->
              <div
                class="absolute inset-0 m-auto w-7 h-7 rounded-full bg-[#202126] flex items-center justify-center p-1.5 shadow-inner text-neutral-200 group-hover/ring:text-white transition-colors"
              >
                <ProviderIcon :name="item.id" class="w-4 h-4" />
              </div>
            </div>

            <!-- Bold Percentage Text beneath Ring -->
            <div class="text-[13px] font-bold text-white tracking-tight text-center mt-1 leading-none drop-shadow-sm">
              {{ getProviderBadgeText(item.id, item.notch_metric) }}
            </div>
          </div>
        </div>

        <!-- Bottom Settings Shortcut -->
        <button
          @click="openSettings"
          class="mt-1 p-1.5 rounded-full text-neutral-500 hover:text-neutral-200 hover:bg-white/10 transition-colors"
          title="打开 ArkBar 设置中心"
        >
          <Settings class="w-3.5 h-3.5" />
        </button>
      </div>
    </div>
  </div>
</template>
