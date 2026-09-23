<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ProviderType, ProviderUsageData, NotchMetric } from '../types';
import ProviderIcon from './ProviderIcon.vue';
import { Settings, GripVertical, RotateCw } from 'lucide-vue-next';

const CACHE_KEY = 'arkbar_cached_providers_data';
const FOLD_GRACE = 450; // Grace period before folding back to rest pill (Codenotch spec: 450ms)

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

// Display mode: 'hover' (default: collapsed pill, expands on hover) | 'always' (stay open) | 'hidden'
const notchMode = ref<'hover' | 'always' | 'hidden'>(
  (localStorage.getItem('arkbar_notch_mode') as any) || 'hover'
);

// Folded state: default true if in hover mode (rest pill shown, expands on mouseover)
const isFolded = ref<boolean>(notchMode.value === 'hover');
const isHoveringNotch = ref<boolean>(false);
let foldTimer: any = null;

// Visible active providers in the Notch
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

// DOM refs for precision positioning & bounding hit testing
const notchPillRef = ref<HTMLElement | null>(null);
const cardRef = ref<HTMLElement | null>(null);

// Hover & speech-bubble positioning
const activeHoverId = ref<ProviderType | null>(null);
const cardTop = ref(40);
const tailTop = ref(60);
let hideTimer: any = null;

// Currently active popover data
const activeHoverData = computed<ProviderUsageData | null>(() => {
  if (!activeHoverId.value) return null;
  return allCachedData.value[activeHoverId.value] || null;
});

// Unfold the notch smoothly
function unfold() {
  if (notchMode.value === 'hidden') return;
  if (foldTimer) {
    clearTimeout(foldTimer);
    foldTimer = null;
  }
  isFolded.value = false;
}

// Schedule fold back to resting pill
function scheduleFold() {
  if (notchMode.value === 'always' || notchMode.value === 'hidden') return;
  if (foldTimer) clearTimeout(foldTimer);
  if (isDragging || isHoveringNotch.value || activeHoverId.value) return;

  foldTimer = setTimeout(() => {
    if (!isHoveringNotch.value && !activeHoverId.value && !isDragging) {
      isFolded.value = true;
    }
  }, FOLD_GRACE);
}

// Helper for rectangle intersection test with optional padding
function inRect(x: number, y: number, r: DOMRect, pad: number): boolean {
  return x >= r.left - pad && y >= r.top - pad && x < r.right + pad && y < r.bottom + pad;
}

// Find which provider ring cell is currently under cursor
function getCellAt(x: number, y: number): { id: ProviderType; el: HTMLElement } | null {
  if (!notchPillRef.value || isFolded.value) return null;
  const cells = notchPillRef.value.querySelectorAll<HTMLElement>('.provider-cell');
  for (const el of cells) {
    const r = el.getBoundingClientRect();
    if (inRect(x, y, r, 6)) {
      const pid = el.dataset.provider as ProviderType;
      if (pid) return { id: pid, el };
    }
  }
  return null;
}

// Check if cursor is in the active interactive area (pill, card, or union bridge)
function pointerInHot(x: number, y: number): boolean {
  if (!notchPillRef.value) return false;

  // When folded, check wake zone near the right edge
  if (isFolded.value) {
    const W = window.innerWidth;
    const H = window.innerHeight;
    const cy = H / 2;
    return x >= W - 42 && Math.abs(y - cy) <= 65;
  }

  const p = notchPillRef.value.getBoundingClientRect();
  if (inRect(x, y, p, 4)) return true;

  if (!activeHoverId.value || !cardRef.value) return false;
  const c = cardRef.value.getBoundingClientRect();
  if (inRect(x, y, c, 4)) return true;

  // Union bounding box between card and pill to eliminate dead gap
  const u = {
    left: Math.min(p.left, c.left),
    top: Math.min(p.top, c.top),
    right: Math.max(p.right, c.right),
    bottom: Math.max(p.bottom, c.bottom),
  };
  return inRect(x, y, u as DOMRect, 0);
}

// Position card vertically centered on cell, with curved tail pointing at ring center
function placeCard(targetEl: HTMLElement) {
  const cr = targetEl.getBoundingClientRect();
  const cy = cr.top + cr.height / 2;
  const H = window.innerHeight;
  const ch = cardRef.value?.offsetHeight || 240;

  let top = Math.round(cy - ch / 2);
  top = Math.max(12, Math.min(top, H - ch - 12));
  cardTop.value = top;

  const ry = cy; // Ring vertical center
  const th = 36;
  const ty = Math.max(top + 16 + th / 2, Math.min(top + ch - 16 - th / 2, ry));
  tailTop.value = Math.round(ty - th / 2);
}

function scheduleHide() {
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = setTimeout(() => {
    activeHoverId.value = null;
  }, 180);
}

function handleMouseMove(e: MouseEvent) {
  if (isDragging) return;

  if (isFolded.value) {
    const W = window.innerWidth;
    const H = window.innerHeight;
    const cy = H / 2;
    // Wake up if cursor is near right screen edge around the rest pill
    if (e.clientX >= W - 44 && Math.abs(e.clientY - cy) <= 75) {
      unfold();
    }
    return;
  }

  const hot = pointerInHot(e.clientX, e.clientY);
  if (hot) {
    if (hideTimer) {
      clearTimeout(hideTimer);
      hideTimer = null;
    }
    if (foldTimer) {
      clearTimeout(foldTimer);
      foldTimer = null;
    }
    isHoveringNotch.value = true;

    const cell = getCellAt(e.clientX, e.clientY);
    if (cell) {
      const changed = activeHoverId.value !== cell.id;
      activeHoverId.value = cell.id;
      if (changed) {
        nextTick(() => placeCard(cell.el));
      } else {
        placeCard(cell.el);
      }
    }
  } else {
    isHoveringNotch.value = false;
    if (activeHoverId.value) {
      scheduleHide();
    }
    scheduleFold();
  }
}

function handleMouseOut(e: MouseEvent) {
  // Cursor left the window entirely
  if (!e.relatedTarget) {
    isHoveringNotch.value = false;
    if (activeHoverId.value) scheduleHide();
    scheduleFold();
  }
}

// Compute percentage for each provider
function getProviderDisplayPercent(id: ProviderType, metric?: NotchMetric): number {
  const data = allCachedData.value[id];
  if (!data) return 0;

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
  }

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

// Percentage badge text shown under ring
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

// Stroke color based on percentage (matching Codenotch color grades)
function getRingStrokeColor(percent: number): string {
  if (percent >= 85) return '#ef4444'; // Red
  if (percent >= 60) return '#d97706'; // Rich warm gold / amber
  return '#3b82f6'; // Clean blue
}

// SVG dash calculation (radius = 18, circumference = 2 * PI * 18 = 113.1)
const CIRCUMFERENCE = 113.1;
function getRingDashOffset(percent: number): number {
  const clamped = Math.max(0, Math.min(100, percent));
  return CIRCUMFERENCE - (clamped / 100) * CIRCUMFERENCE;
}

// Format reset time
function formatResetLabel(resetAt?: string): string {
  if (!resetAt) return '';
  const trimmed = resetAt.trim();

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

  const diffDays = Math.floor((date.getTime() - now.getTime()) / (1000 * 60 * 60 * 24));
  if (diffDays < 7) {
    return `${dayName} ${hh}:${mm} 重置`;
  }
  return `${date.getMonth() + 1}月${date.getDate()}日 重置`;
}

// Dragging window along edge
let isDragging = false;
let dragStartX = 0;
let dragStartY = 0;

function handleDragMouseDown(e: MouseEvent) {
  isDragging = true;
  dragStartX = e.screenX;
  dragStartY = e.screenY;
  activeHoverId.value = null; // hide card during move
  if (foldTimer) clearTimeout(foldTimer);
  invoke('start_drag_move').catch(() => {});

  const onMouseMoveDrag = (ev: MouseEvent) => {
    if (!isDragging) return;
    const dx = ev.screenX - dragStartX;
    const dy = ev.screenY - dragStartY;
    invoke('update_drag_move', { totalDx: dx, totalDy: dy }).catch(() => {});
  };

  const onMouseUpDrag = () => {
    isDragging = false;
    invoke('end_drag_move').catch(() => {});
    window.removeEventListener('mousemove', onMouseMoveDrag);
    window.removeEventListener('mouseup', onMouseUpDrag);
    scheduleFold();
  };

  window.addEventListener('mousemove', onMouseMoveDrag);
  window.addEventListener('mouseup', onMouseUpDrag);
}

// Open settings window
function openSettings() {
  activeHoverId.value = null;
  scheduleFold();
  invoke('show_main_window').catch(() => {});
}

// Sync updates
let unlistenUsage: (() => void) | null = null;

onMounted(async () => {
  window.addEventListener('mousemove', handleMouseMove);
  window.addEventListener('mouseout', handleMouseOut);

  try {
    const list: ProviderUsageData[] = await invoke('get_all_providers_usage');
    if (Array.isArray(list)) {
      list.forEach((item) => {
        allCachedData.value[item.provider] = item;
      });
      localStorage.setItem(CACHE_KEY, JSON.stringify(allCachedData.value));
    }
  } catch {}

  unlistenUsage = await listen<Record<ProviderType, ProviderUsageData>>('usage-updated', (event) => {
    if (event.payload) {
      allCachedData.value = { ...allCachedData.value, ...event.payload };
      localStorage.setItem(CACHE_KEY, JSON.stringify(allCachedData.value));
    }
  });

  window.addEventListener('storage', (e) => {
    if (e.key === 'arkbar_provider_tabs') {
      providerTabs.value = loadProviderTabs();
    } else if (e.key === CACHE_KEY) {
      allCachedData.value = loadCachedData();
    } else if (e.key === 'arkbar_notch_mode') {
      const mode = (localStorage.getItem('arkbar_notch_mode') as any) || 'hover';
      notchMode.value = mode;
      if (mode === 'always') {
        isFolded.value = false;
      } else if (mode === 'hover') {
        isFolded.value = true;
      }
    }
  });
});

onUnmounted(() => {
  window.removeEventListener('mousemove', handleMouseMove);
  window.removeEventListener('mouseout', handleMouseOut);
  if (unlistenUsage) unlistenUsage();
  if (foldTimer) clearTimeout(foldTimer);
  if (hideTimer) clearTimeout(hideTimer);
});
</script>

<template>
  <div
    v-if="notchMode !== 'hidden'"
    class="fixed inset-0 pointer-events-none select-none flex items-center justify-end overflow-visible"
  >
    <!-- ============================================================ -->
    <!-- UNIFIED INTERACTIVE ZONE (Right Screen Edge)                 -->
    <!-- ============================================================ -->
    <div
      class="relative pointer-events-auto flex items-center pr-0"
      :class="{ 'is-folded-notch': isFolded }"
      @mouseenter="unfold"
    >
      <!-- 1. WAKE ZONE: Trigger area when folded to wake up on approach -->
      <div
        v-if="isFolded"
        class="absolute right-0 top-1/2 -translate-y-1/2 w-[42px] h-[130px] pointer-events-auto cursor-pointer z-50"
        @mouseenter="unfold"
        @mousemove="unfold"
      />

      <!-- 2. REST PILL (Codenotch #rest: Sleek 10px x 79px capsule hugging edge when folded) -->
      <div
        class="rest-pill"
        :class="{ 'opacity-100': isFolded, 'opacity-0 pointer-events-none': !isFolded }"
        @mouseenter="unfold"
      />

      <!-- 3. SPEECH BUBBLE POPOVER CARD & ORGANIC TAIL (Codenotch Style) -->
      <Transition name="codenotch-pop">
        <div v-if="!isFolded && activeHoverId" class="contents">
          <!-- Organic Curved Wedge Tail (clip-path from Codenotch spec) -->
          <div
            class="absolute z-50 pointer-events-none w-[32px] h-[36px] transition-[top] duration-150 ease-out"
            :style="{
              right: '69px',
              top: `${tailTop}px`,
              backgroundColor: 'rgba(30, 31, 36, 0.98)',
              clipPath: 'path(\'M0 0C0 9 18.56 13.68 32 18C18.56 22.32 0 27 0 36Z\')',
            }"
          />

          <!-- Speech Bubble Card Container -->
          <div
            ref="cardRef"
            class="absolute z-50 pointer-events-auto w-[304px] bg-[#1e1f24]/98 text-neutral-100 rounded-[20px] p-4 shadow-[0_22px_45px_rgba(0,0,0,0.55)] border border-white/10 backdrop-blur-3xl overflow-hidden transition-[top] duration-150 ease-out"
            :style="{
              right: '98px',
              top: `${cardTop}px`,
            }"
          >
            <!-- Card Header: Logo + Title -->
            <div class="flex items-center justify-between pb-3 border-b border-white/10">
              <div class="flex items-center gap-2.5">
                <div class="w-6 h-6 rounded-lg bg-neutral-800 flex items-center justify-center p-1 text-white shadow-inner">
                  <ProviderIcon :name="activeHoverId" class="w-3.5 h-3.5" />
                </div>
                <h3 class="text-sm font-bold tracking-tight text-white">
                  {{ activeHoverData?.provider_name || activeHoverId }} 用量
                </h3>
              </div>
            </div>

            <!-- Card Body: Groups & Limit Rows -->
            <div class="pt-3 max-h-[360px] overflow-y-auto space-y-3">
              <!-- Teamo Balance Card -->
              <div
                v-if="activeHoverId === 'teamo' && activeHoverData?.extension?.balance"
                class="bg-white/[0.05] rounded-xl p-3 border border-white/5"
              >
                <div class="flex items-center justify-between text-xs text-neutral-400 mb-1">
                  <span>账户余额</span>
                  <span>今日消费</span>
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-xl font-bold text-emerald-400">
                    ${{ activeHoverData.extension.balance.value.toFixed(2) }}
                    <span class="text-xs font-normal text-neutral-400">{{ activeHoverData.extension.balance.currency }}</span>
                  </span>
                  <span class="text-sm font-semibold text-neutral-300">
                    ${{ (activeHoverData.extension.today_cost?.value ?? 0).toFixed(2) }}
                  </span>
                </div>
              </div>

              <!-- Limit Groups (Gemini Models, Claude Models, Grok Build, etc.) -->
              <template v-if="activeHoverData && activeHoverData.groups && activeHoverData.groups.length > 0">
                <div
                  v-for="(group, gIdx) in activeHoverData.groups"
                  :key="gIdx"
                  class="bg-white/[0.04] rounded-xl p-3 border border-white/5 space-y-3"
                >
                  <div v-if="group.group_name" class="text-xs font-semibold text-neutral-400">
                    {{ group.group_name }}
                  </div>

                  <div
                    v-for="(period, pIdx) in group.periods"
                    :key="pIdx"
                    class="space-y-1.5"
                  >
                    <!-- Label & Reset Time -->
                    <div class="flex items-center justify-between text-xs">
                      <span class="font-medium text-neutral-300">
                        {{ period.name || period.label }}
                      </span>
                      <span class="text-neutral-400 text-[11px]">
                        {{ formatResetLabel(period.reset_at) }}
                      </span>
                    </div>

                    <!-- Progress Bar -->
                    <div class="h-1.5 w-full bg-neutral-700/60 rounded-full overflow-hidden">
                      <div
                        class="h-full rounded-full transition-all duration-700 ease-out"
                        :style="{
                          width: `${Math.min(100, Math.max(0, period.used_percent))}%`,
                          backgroundColor: getRingStrokeColor(period.used_percent),
                        }"
                      />
                    </div>

                    <!-- Ratio Breakdown -->
                    <div class="text-[11px] text-neutral-400 font-medium">
                      {{ Math.round(period.used_percent) }}% 已用 · {{ Math.max(0, 100 - Math.round(period.used_percent)) }}% 剩余
                    </div>
                  </div>
                </div>
              </template>

              <!-- Loading / Syncing placeholder if no data yet -->
              <div v-else class="py-6 text-center text-xs text-neutral-400 flex flex-col items-center gap-2">
                <RotateCw class="w-4 h-4 animate-spin text-neutral-500" />
                <span>正在同步用量读数…</span>
              </div>

              <!-- Token Summary (Tokens & Cache hit rate) -->
              <div
                v-if="activeHoverData?.token_summary"
                class="flex items-center justify-between px-1 text-[11px] text-neutral-400"
              >
                <span>今日 Token: {{ activeHoverData.token_summary.today_tokens.toLocaleString() }}</span>
                <span v-if="activeHoverData.token_summary.cache_hit_rate">
                  ⚡ 缓存命中 {{ activeHoverData.token_summary.cache_hit_rate }}%
                </span>
              </div>
            </div>

            <!-- Footer: Live Status Row (Codenotch: 工作中 刚刚) -->
            <div class="pt-3 mt-2 border-t border-white/10 flex items-center justify-between text-xs text-neutral-400">
              <div class="flex flex-col">
                <span class="font-medium text-neutral-300">{{ activeHoverData?.provider_name || activeHoverId }}</span>
                <span class="text-[10px] text-neutral-400">已连接</span>
              </div>

              <div class="flex flex-col items-end">
                <div class="flex items-center gap-1 text-[11px] text-neutral-300">
                  <RotateCw class="w-3 h-3 text-neutral-400 animate-spin" />
                  <span>工作中</span>
                </div>
                <span class="text-[10px] text-neutral-400 mt-0.5">刚刚</span>
              </div>
            </div>
          </div>
        </div>
      </Transition>

      <!-- 4. THE SCREEN EDGE NOTCH PILL (Expands on hover, clips down to rest pill when idle) -->
      <div
        ref="notchPillRef"
        class="notch-pill relative z-40 w-[70px] bg-[#16171b]/95 border-l border-t border-b border-white/10 rounded-l-[28px] py-4 flex flex-col items-center gap-4 shadow-[-12px_0_32px_rgba(0,0,0,0.55)] backdrop-blur-2xl"
        :class="{ 'is-folded': isFolded }"
      >
        <!-- Top Concave Flare connecting smoothly to Screen Edge -->
        <svg
          class="notch-flare absolute -top-6 right-0 w-6 h-6 text-[#16171b]/95 fill-current pointer-events-none"
          viewBox="0 0 24 24"
        >
          <path d="M24,24 C10.745,24 0,13.255 0,0 L24,0 Z" />
        </svg>

        <!-- Top Drag Handle (6 dots grip icon matching user photo) -->
        <div
          class="notch-item cursor-grab active:cursor-grabbing text-neutral-500 hover:text-neutral-300 py-0.5 transition-colors"
          title="按住拖动调整刘海位置"
          @mousedown="handleDragMouseDown"
        >
          <GripVertical class="w-3.5 h-3.5 opacity-40 hover:opacity-100" />
        </div>

        <!-- Provider Circular Rings List -->
        <div class="flex flex-col items-center gap-4">
          <div
            v-for="item in notchProviders"
            :key="item.id"
            :data-provider="item.id"
            class="provider-cell notch-item relative flex flex-col items-center cursor-pointer group/ring transition-transform duration-150"
            :class="activeHoverId === item.id ? 'scale-105' : 'hover:scale-105'"
          >
            <!-- Circular Ring Gauge (Diameter 44px, matching Codenotch) -->
            <div class="relative w-11 h-11 flex items-center justify-center">
              <svg class="w-11 h-11 -rotate-90 origin-center" viewBox="0 0 44 44">
                <circle
                  cx="22"
                  cy="22"
                  r="18"
                  fill="none"
                  stroke="rgba(255, 255, 255, 0.08)"
                  stroke-width="3.5"
                />
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

              <!-- Center Provider SVG Logo inside 28px dark circle -->
              <div
                class="absolute inset-0 m-auto w-7 h-7 rounded-full bg-[#202126] flex items-center justify-center p-1.5 shadow-inner text-neutral-200 group-hover/ring:text-white transition-colors"
              >
                <ProviderIcon :name="item.id" class="w-3.5 h-3.5" />
              </div>
            </div>

            <!-- Percentage Text Below Ring (e.g. 0%, 22%, 64%) -->
            <div class="text-[13px] font-bold text-white tracking-tight text-center mt-1 leading-none drop-shadow-sm">
              {{ getProviderBadgeText(item.id, item.notch_metric) }}
            </div>
          </div>
        </div>

        <!-- Bottom Settings Button (Gear icon to open Settings window) -->
        <button
          @click="openSettings"
          class="notch-item mt-1 p-1.5 rounded-full text-neutral-400 hover:text-white hover:bg-white/10 transition-colors"
          title="偏好设置"
        >
          <Settings class="w-4 h-4" />
        </button>

        <!-- Bottom Concave Flare connecting smoothly to Screen Edge -->
        <svg
          class="notch-flare absolute -bottom-6 right-0 w-6 h-6 text-[#16171b]/95 fill-current pointer-events-none"
          viewBox="0 0 24 24"
        >
          <path d="M24,0 C10.745,0 0,10.745 0,24 L24,24 Z" />
        </svg>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* ===================================================================== */
/* 1. REST PILL (Codenotch #rest: Slim 10px capsule at right screen edge) */
/* ===================================================================== */
.rest-pill {
  position: absolute;
  right: 0;
  top: 50%;
  transform: translateY(-50%);
  width: 10px;
  height: 79px;
  background-color: #16171b;
  border-left: 1px solid rgba(255, 255, 255, 0.22);
  border-top: 1px solid rgba(255, 255, 255, 0.22);
  border-bottom: 1px solid rgba(255, 255, 255, 0.22);
  border-right: none;
  border-radius: 6px 0 0 6px;
  box-shadow: -4px 0 14px rgba(0, 0, 0, 0.5);
  pointer-events: auto;
  cursor: pointer;
  z-index: 45;
  transition: opacity 0.2s ease 0.16s;
}

/* ===================================================================== */
/* 2. NOTCH PILL CLIP-PATH MORPHING (Codenotch Smooth Spring Expansion)  */
/* ===================================================================== */
.notch-pill {
  --clip-open: inset(-30px -1px -30px -1px round 0);
  --clip-rest: inset(calc(50% - 39.5px) 0 calc(50% - 39.5px) calc(100% - 10px) round 6px 0 0 6px);
  clip-path: var(--clip-open);
  transition: clip-path 0.36s cubic-bezier(0.32, 0.72, 0.24, 1);
}

.notch-pill.is-folded {
  clip-path: var(--clip-rest);
  pointer-events: none;
}

/* Items inside notch slide right and fade out when folding */
.notch-item {
  transition: opacity 0.18s ease, transform 0.3s cubic-bezier(0.32, 0.72, 0.24, 1);
}

.is-folded .notch-item {
  opacity: 0;
  transform: translateX(12px);
  pointer-events: none;
}

/* Top & Bottom Concave Fillets */
.notch-flare {
  transition: opacity 0.2s ease;
}

.is-folded .notch-flare {
  opacity: 0;
  pointer-events: none;
}

/* Gauge progress stroke animation */
.gauge-ring {
  transition: stroke-dashoffset 0.6s cubic-bezier(0.34, 1.4, 0.64, 1);
}

/* Popover Speech-Bubble Spring Animation */
.codenotch-pop-enter-active {
  transition: opacity 0.16s ease-out, transform 0.2s cubic-bezier(0.34, 1.4, 0.64, 1);
}
.codenotch-pop-leave-active {
  transition: opacity 0.12s ease-in, transform 0.12s ease-in;
}
.codenotch-pop-enter-from {
  opacity: 0;
  transform: translateX(10px) scale(0.96);
}
.codenotch-pop-leave-to {
  opacity: 0;
  transform: translateX(8px) scale(0.97);
}
</style>
