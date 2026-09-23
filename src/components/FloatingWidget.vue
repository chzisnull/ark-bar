<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ProviderType, ProviderUsageData, NotchMetric } from '../types';
import ProviderIcon from './ProviderIcon.vue';
import { RotateCw } from 'lucide-vue-next';

const CACHE_KEY = 'arkbar_cached_providers_data';
const FOLD_GRACE = 450; // Grace period before folding back to rest pill (Codenotch spec: 450ms)
const WAKE_BAND = 40;   // Wake zone width extending into the screen

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
let foldTimer: any = null;
let hideTimer: any = null;
let pointerIn = false;
let isDragging = false;
let dragStartX = 0;
let dragStartY = 0;

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

// Handles hover states
const isHoveringMove = ref(false);
const isHoveringOrb = ref(false);

// Currently active popover data
const activeHoverData = computed<ProviderUsageData | null>(() => {
  if (!activeHoverId.value) return null;
  return allCachedData.value[activeHoverId.value] || null;
});

// Report hot rectangles to Rust watchdog
function reportHot() {
  if (notchMode.value === 'hidden') {
    invoke('set_hot', { rects: [], expanded: false }).catch(() => {});
    return;
  }

  const k = window.devicePixelRatio || 1;
  const W = window.innerWidth;
  const H = window.innerHeight;

  if (isFolded.value) {
    const pillH = 79;
    const r = [
      (W - 10 - WAKE_BAND) * k,
      (H / 2 - pillH / 2 - 20) * k,
      (10 + WAKE_BAND + 15) * k,
      (pillH + 40) * k,
    ];
    invoke('set_hot', { rects: [r], expanded: false }).catch(() => {});
    return;
  }

  const rects: number[][] = [];
  if (notchPillRef.value) {
    const pr = notchPillRef.value.getBoundingClientRect();
    // Notch pill plus fillets and handles buffer
    rects.push([
      (pr.left - 10) * k,
      (pr.top - 50) * k,
      (pr.width + 20) * k,
      (pr.height + 100) * k,
    ]);
  }

  if (activeHoverId.value && cardRef.value) {
    const cr = cardRef.value.getBoundingClientRect();
    rects.push([
      (cr.left - 10) * k,
      (cr.top - 10) * k,
      (cr.width + 50) * k, // bridge gap between card and pill
      (cr.height + 20) * k,
    ]);
  }

  invoke('set_hot', { rects, expanded: true }).catch(() => {});
}

// Unfold the notch smoothly
function unfold() {
  if (notchMode.value === 'hidden') return;
  if (foldTimer) {
    clearTimeout(foldTimer);
    foldTimer = null;
  }
  if (isFolded.value) {
    isFolded.value = false;
    nextTick(() => reportHot());
  }
}

// Schedule fold back to resting pill
function scheduleFold() {
  if (notchMode.value === 'always' || notchMode.value === 'hidden') return;
  if (foldTimer) clearTimeout(foldTimer);
  if (isDragging || pointerIn) return;

  foldTimer = setTimeout(() => {
    if (!pointerIn && !isDragging) {
      isFolded.value = true;
      activeHoverId.value = null;
      isHoveringMove.value = false;
      isHoveringOrb.value = false;
      nextTick(() => reportHot());
    }
  }, FOLD_GRACE);
}

// Position card vertically centered on cell, with curved tail pointing at ring center
function placeCard(targetEl: HTMLElement) {
  const cr = targetEl.getBoundingClientRect();
  const cy = cr.top + cr.height / 2;
  const H = window.innerHeight;
  const ch = cardRef.value?.offsetHeight || 260;

  let top = Math.round(cy - ch / 2);
  top = Math.max(12, Math.min(top, H - ch - 12));
  cardTop.value = top;

  const ry = cy; // Ring vertical center
  const th = 36;
  const ty = Math.max(top + 16 + th / 2, Math.min(top + ch - 16 - th / 2, ry));
  tailTop.value = Math.round(ty - th / 2);
  reportHot();
}

function scheduleHideCard() {
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = setTimeout(() => {
    activeHoverId.value = null;
    reportHot();
  }, 180);
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

// Core unified pointer movement handler (called by DOM mousemove AND native Rust watchdog notch_cursor)
function handlePointerAt(clientX: number, clientY: number) {
  if (isDragging) return;

  if (isFolded.value) {
    const W = window.innerWidth;
    const H = window.innerHeight;
    const cy = H / 2;
    // Wake up if cursor is near right screen edge around the rest pill
    if (clientX >= W - 10 - WAKE_BAND && Math.abs(clientY - cy) <= 55) {
      unfold();
    }
    return;
  }

  // Check handles hover
  if (notchPillRef.value) {
    const pr = notchPillRef.value.getBoundingClientRect();
    const R = 38.7;
    // Top handle center: [pr.right - R, pr.top - R]
    const topHx = pr.right - R;
    const topHy = pr.top - R;
    const distTop = Math.hypot(clientX - topHx, clientY - topHy);
    isHoveringMove.value = distTop <= 32;

    // Bottom handle center: [pr.right - R, pr.bottom + R]
    const botHx = pr.right - R;
    const botHy = pr.bottom + R;
    const distBot = Math.hypot(clientX - botHx, clientY - botHy);
    isHoveringOrb.value = distBot <= 32;

    if (isHoveringMove.value || isHoveringOrb.value) {
      if (activeHoverId.value) scheduleHideCard();
      return;
    }
  }

  // Check card or cell hover
  const cell = getCellAt(clientX, clientY);
  if (cell) {
    if (hideTimer) {
      clearTimeout(hideTimer);
      hideTimer = null;
    }
    const changed = activeHoverId.value !== cell.id;
    activeHoverId.value = cell.id;
    if (changed) {
      nextTick(() => placeCard(cell.el));
    } else {
      placeCard(cell.el);
    }
    return;
  }

  // Check if pointer is inside card
  if (activeHoverId.value && cardRef.value) {
    const cr = cardRef.value.getBoundingClientRect();
    if (inRect(clientX, clientY, cr, 8)) {
      if (hideTimer) {
        clearTimeout(hideTimer);
        hideTimer = null;
      }
      return;
    }
  }

  // If outside cells and card, schedule card hide
  if (activeHoverId.value) {
    scheduleHideCard();
  }
}

function handleDomMouseMove(e: MouseEvent) {
  handlePointerAt(e.clientX, e.clientY);
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

// Stroke color based on Codenotch palette (tone function: ample, watch, crit)
function getRingStrokeColor(percent: number): string {
  if (percent >= 80) return '#FF3F00'; // CRIT: Red
  if (percent >= 50) return '#F2FF00'; // WATCH: Warm yellow
  return '#00FF88';                   // AMPLE: Apple green
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

// Dragging window along edge using Move Handle
function handleDragMouseDown(e: MouseEvent) {
  if (e.button !== 0) return;
  e.stopPropagation();
  isDragging = true;
  dragStartX = e.screenX;
  dragStartY = e.screenY;
  activeHoverId.value = null;
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

// Open settings window from Settings Orb
function openSettings(e?: MouseEvent) {
  if (e) e.stopPropagation();
  activeHoverId.value = null;
  scheduleFold();
  invoke('show_main_window').catch(() => {});
}

// Event unlisteners
let unlistenUsage: (() => void) | null = null;
let unlistenPointer: (() => void) | null = null;
let unlistenCursor: (() => void) | null = null;

onMounted(async () => {
  window.addEventListener('mousemove', handleDomMouseMove);

  // 1. Listen to native watchdog pointer in/out events
  unlistenPointer = await listen<boolean>('notch_pointer', (event) => {
    const inside = event.payload === true;
    pointerIn = inside;
    if (inside) {
      unfold();
    } else {
      scheduleFold();
    }
  });

  // 2. Listen to native watchdog cursor position updates (logical coords)
  unlistenCursor = await listen<[number, number]>('notch_cursor', (event) => {
    if (event.payload) {
      const [cx, cy] = event.payload;
      handlePointerAt(cx, cy);
    }
  });

  // 3. Load provider usage
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
      nextTick(() => reportHot());
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
      nextTick(() => reportHot());
    }
  });

  // Initial report of hot rectangles
  nextTick(() => {
    reportHot();
  });
});

onUnmounted(() => {
  window.removeEventListener('mousemove', handleDomMouseMove);
  if (unlistenUsage) unlistenUsage();
  if (unlistenPointer) unlistenPointer();
  if (unlistenCursor) unlistenCursor();
  if (foldTimer) clearTimeout(foldTimer);
  if (hideTimer) clearTimeout(hideTimer);
});
</script>

<template>
  <div
    v-if="notchMode !== 'hidden'"
    class="fixed inset-0 pointer-events-none select-none flex items-center justify-end overflow-visible font-sans"
  >
    <!-- ============================================================ -->
    <!-- UNIFIED INTERACTIVE ZONE (Right Screen Edge)                 -->
    <!-- ============================================================ -->
    <div
      class="relative pointer-events-auto flex items-center pr-0"
      :class="{ 'is-folded-body': isFolded }"
    >
      <!-- 1. REST PILL (Codenotch #rest: Sleek 10px x 79px capsule hugging edge when folded) -->
      <div
        id="rest"
        :class="{ 'is-active': isFolded }"
        @mouseenter="unfold"
      />

      <!-- 2. SPEECH BUBBLE POPOVER CARD & ORGANIC TAIL (Codenotch Style) -->
      <Transition name="codenotch-pop">
        <div v-if="!isFolded && activeHoverId" class="contents">
          <!-- Organic Curved Wedge Tail (clip-path from Codenotch spec) -->
          <div
            id="tail"
            :style="{
              top: `${tailTop}px`,
            }"
          />

          <!-- Speech Bubble Card Container -->
          <div
            ref="cardRef"
            id="card"
            :style="{
              top: `${cardTop}px`,
            }"
          >
            <!-- Card Header: Logo + Title -->
            <div class="flex items-center justify-between pb-3 border-b border-[#242424]">
              <div class="flex items-center gap-2.5">
                <div class="w-6 h-6 rounded-lg bg-[#1e1e1e] flex items-center justify-center p-1 text-white shadow-inner">
                  <ProviderIcon :name="activeHoverId" class="w-3.5 h-3.5" />
                </div>
                <h3 class="text-sm font-bold tracking-tight text-[#ffffff]">
                  {{ activeHoverData?.provider_name || activeHoverId }} 用量
                </h3>
              </div>
            </div>

            <!-- Card Body: Groups & Limit Rows -->
            <div class="pt-3 max-h-[360px] overflow-y-auto space-y-3">
              <!-- Teamo Balance Card -->
              <div
                v-if="activeHoverId === 'teamo' && activeHoverData?.extension?.balance"
                class="bg-white/[0.04] rounded-xl p-3 border border-[#242424]"
              >
                <div class="flex items-center justify-between text-xs text-[#808080] mb-1">
                  <span>账户余额</span>
                  <span>今日消费</span>
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-xl font-bold text-[#00FF88]">
                    ${{ activeHoverData.extension.balance.value.toFixed(2) }}
                    <span class="text-xs font-normal text-[#808080]">{{ activeHoverData.extension.balance.currency }}</span>
                  </span>
                  <span class="text-sm font-semibold text-[#e8e8ea]">
                    ${{ (activeHoverData.extension.today_cost?.value ?? 0).toFixed(2) }}
                  </span>
                </div>
              </div>

              <!-- Limit Groups (Gemini Models, Claude Models, Grok Build, etc.) -->
              <template v-if="activeHoverData && activeHoverData.groups && activeHoverData.groups.length > 0">
                <div
                  v-for="(group, gIdx) in activeHoverData.groups"
                  :key="gIdx"
                  class="bg-white/[0.03] rounded-xl p-3 border border-[#242424] space-y-3"
                >
                  <div v-if="group.group_name" class="text-xs font-semibold text-[#808080]">
                    {{ group.group_name }}
                  </div>

                  <div
                    v-for="(period, pIdx) in group.periods"
                    :key="pIdx"
                    class="space-y-1.5"
                  >
                    <!-- Label & Reset Time -->
                    <div class="flex items-center justify-between text-xs">
                      <span class="font-medium text-[#e8e8ea]">
                        {{ period.name || period.label }}
                      </span>
                      <span class="text-[#808080] text-[11px]">
                        {{ formatResetLabel(period.reset_at) }}
                      </span>
                    </div>

                    <!-- Progress Bar -->
                    <div class="h-1.5 w-full bg-[#2d2d2d] rounded-full overflow-hidden">
                      <div
                        class="h-full rounded-full transition-all duration-700 ease-out"
                        :style="{
                          width: `${Math.min(100, Math.max(0, period.used_percent))}%`,
                          backgroundColor: getRingStrokeColor(period.used_percent),
                        }"
                      />
                    </div>

                    <!-- Ratio Breakdown -->
                    <div class="text-[11px] text-[#808080] font-medium">
                      {{ Math.round(period.used_percent) }}% 已用 · {{ Math.max(0, 100 - Math.round(period.used_percent)) }}% 剩余
                    </div>
                  </div>
                </div>
              </template>

              <!-- Loading / Syncing placeholder if no data yet -->
              <div v-else class="py-6 text-center text-xs text-[#808080] flex flex-col items-center gap-2">
                <RotateCw class="w-4 h-4 animate-spin text-[#808080]" />
                <span>正在同步用量读数…</span>
              </div>

              <!-- Token Summary (Tokens & Cache hit rate) -->
              <div
                v-if="activeHoverData?.token_summary"
                class="flex items-center justify-between px-1 text-[11px] text-[#808080]"
              >
                <span>今日 Token: {{ activeHoverData.token_summary.today_tokens.toLocaleString() }}</span>
                <span v-if="activeHoverData.token_summary.cache_hit_rate">
                  ⚡ 缓存命中 {{ activeHoverData.token_summary.cache_hit_rate }}%
                </span>
              </div>
            </div>

            <!-- Footer: Live Status Row (Codenotch: 工作中 刚刚) -->
            <div class="pt-3 mt-2 border-t border-[#242424] flex items-center justify-between text-xs text-[#808080]">
              <div class="flex flex-col">
                <span class="font-medium text-[#e8e8ea]">{{ activeHoverData?.provider_name || activeHoverId }}</span>
                <span class="text-[10px] text-[#808080]">已连接</span>
              </div>

              <div class="flex flex-col items-end">
                <div class="flex items-center gap-1 text-[11px] text-[#e8e8ea]">
                  <RotateCw class="w-3 h-3 text-[#808080] animate-spin" />
                  <span>工作中</span>
                </div>
                <span class="text-[10px] text-[#808080] mt-0.5">刚刚</span>
              </div>
            </div>
          </div>
        </div>
      </Transition>

      <!-- 3. TOP MOVE HANDLE (Codenotch #move handle: hand icon for carrying/dragging) -->
      <div
        id="move"
        class="handle"
        :class="{ 'hover': isHoveringMove, 'is-folded-handle': isFolded }"
        title="按住拖动调整刘海位置"
        @mousedown="handleDragMouseDown"
      >
        <svg class="h-rest" viewBox="-36 -36 72 72" aria-hidden="true">
          <circle class="h-ring" r="28.5" fill="none" stroke-width="8.8" stroke-linecap="round" stroke-dasharray="44.77 179.07" />
          <circle class="h-fill" r="28.5" fill="none" stroke-width="6.8" stroke-linecap="round" stroke-dasharray="44.77 179.07" />
        </svg>
        <div class="h-disc" />
        <svg class="h-glyph" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M8 13V5.6a1.5 1.5 0 0 1 3 0V11h.6V3.5a1.5 1.5 0 0 1 3 0V11h.6V5a1.5 1.5 0 0 1 3 0v8.4c0 4.2-2 7.1-5.6 7.1-2.4 0-3.8-1-5.1-2.7l-3.8-5a1.5 1.5 0 0 1 2.2-2z" />
        </svg>
      </div>

      <!-- 4. BOTTOM SETTINGS ORB (Codenotch #orb handle: gear icon for preferences) -->
      <div
        id="orb"
        class="handle"
        :class="{ 'hover': isHoveringOrb, 'is-folded-handle': isFolded }"
        title="偏好设置"
        @click="openSettings"
      >
        <svg class="h-rest" viewBox="-36 -36 72 72" aria-hidden="true">
          <circle class="h-ring" r="28.5" fill="none" stroke-width="8.8" stroke-linecap="round" stroke-dasharray="44.77 179.07" />
          <circle class="h-fill" r="28.5" fill="none" stroke-width="6.8" stroke-linecap="round" stroke-dasharray="44.77 179.07" />
        </svg>
        <div class="h-disc" />
        <svg class="h-glyph" viewBox="0 0 12 12" aria-hidden="true">
          <path fill-rule="evenodd" d="M5.1.6h1.8l.3 1.5 1.1.5 1.3-.8 1.3 1.3-.8 1.3.5 1.1 1.5.3v1.8l-1.5.3-.5 1.1.8 1.3-1.3 1.3-1.3-.8-1.1.5-.3 1.5H5.1l-.3-1.5-1.1-.5-1.3.8-1.3-1.3.8-1.3-.5-1.1L.6 6.9V5.1l1.5-.3.5-1.1-.8-1.3 1.3-1.3 1.3.8 1.1-.5zM6 4.1a1.9 1.9 0 1 0 0 3.8 1.9 1.9 0 0 0 0-3.8z" />
        </svg>
      </div>

      <!-- 5. THE SCREEN EDGE NOTCH PILL (Codenotch #pill) -->
      <div
        ref="notchPillRef"
        id="pill"
        :class="{ 'is-folded': isFolded }"
      >
        <!-- Provider Circular Rings Stack -->
        <div
          v-for="(item, idx) in notchProviders"
          :key="item.id"
          :data-provider="item.id"
          class="cell provider-cell"
          :style="{ '--i': idx }"
        >
          <!-- Circular Ring Gauge (Diameter 44px, matching Codenotch) -->
          <div
            class="ringwrap"
            :class="{ 'active': activeHoverId === item.id }"
          >
            <svg class="w-11 h-11 -rotate-90 origin-center overflow-visible" viewBox="0 0 44 44">
              <circle
                cx="22"
                cy="22"
                r="18"
                fill="none"
                stroke="#303030"
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

            <!-- Center Provider Logo inside ring -->
            <div class="glyph">
              <ProviderIcon :name="item.id" class="w-5 h-5 text-neutral-200" />
            </div>
          </div>

          <!-- Percentage Text Below Ring (Tabular numerals, Codenotch style) -->
          <div class="pct">
            {{ getProviderBadgeText(item.id, item.notch_metric) }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* ===================================================================== */
/* CODENOTCH CSS DESIGN VOCABULARY & APPLE MOTION SYSTEM                 */
/* ===================================================================== */
:root {
  --pill: #000000;
  --edge: #2e2e2e;
  --card: #0a0a0a;
  --card-line: #242424;
  --fillet: 38.7px;
  --ring: 44px;
}

/* ===================================================================== */
/* 1. REST PILL (Codenotch #rest: 10px x 79px capsule hugging edge)       */
/* ===================================================================== */
#rest {
  position: absolute;
  box-sizing: border-box;
  background: #000000;
  border: 1px solid #2e2e2e;
  border-right: none;
  pointer-events: auto;
  cursor: pointer;
  right: 0;
  top: 50%;
  transform: translateY(-50%);
  width: 10px;
  height: 79px;
  border-radius: 6px 0 0 6px;
  opacity: 0;
  transition: opacity 0.16s ease;
  z-index: 45;
}

#rest.is-active {
  opacity: 1;
  transition: opacity 0.2s ease 0.18s; /* arrives as the clip does */
}

/* ===================================================================== */
/* 2. NOTCH PILL (Codenotch #pill: 70px body with concave radial fillets) */
/* ===================================================================== */
#pill {
  position: relative;
  right: 0;
  width: 70px;
  padding: 18px 0;
  background: #000000;
  border-radius: 20px 0 0 20px;
  border: 1px solid #2e2e2e;
  border-right: none;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 14px;
  cursor: pointer;
  z-index: 40;
  --fillet: 38.7px;
  --clip-open: inset(-40px -1px -40px -1px round 0);
  --clip-rest: inset(calc(50% - 39.5px) 0 calc(50% - 39.5px) calc(100% - 10px) round 6px 0 0 6px);
  clip-path: var(--clip-open);
  transition: clip-path 0.36s cubic-bezier(0.32, 0.72, 0.24, 1);
}

#pill.is-folded {
  clip-path: var(--clip-rest);
  pointer-events: none;
}

/* Fillets: Concave arcs between the pill's top/bottom and screen edge */
#pill::before,
#pill::after {
  content: "";
  position: absolute;
  right: 0;
  width: var(--fillet);
  height: var(--fillet);
  pointer-events: none;
}

#pill::before {
  top: calc(-1 * var(--fillet));
  background: radial-gradient(circle at 0 0, transparent calc(var(--fillet) - 1.5px), #2e2e2e calc(var(--fillet) - 0.5px), #000000 calc(var(--fillet) + 0.5px));
}

#pill::after {
  bottom: calc(-1 * var(--fillet));
  background: radial-gradient(circle at 0 100%, transparent calc(var(--fillet) - 1.5px), #2e2e2e calc(var(--fillet) - 0.5px), #000000 calc(var(--fillet) + 0.5px));
}

/* ===================================================================== */
/* 3. PROVIDER CELLS & RINGS                                             */
/* ===================================================================== */
.cell {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  transition: opacity 0.18s ease calc(var(--i, 0) * 28ms), transform 0.3s cubic-bezier(0.32, 0.72, 0.24, 1) calc(var(--i, 0) * 28ms);
}

.is-folded .cell {
  opacity: 0;
  transform: translateX(10px);
  pointer-events: none;
}

.ringwrap {
  position: relative;
  width: 44px;
  height: 44px;
  transition: transform 0.3s cubic-bezier(0.34, 1.4, 0.64, 1);
}

.ringwrap:hover,
.ringwrap.active {
  transform: scale(1.08);
}

.glyph {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}

.pct {
  font-size: 13px;
  font-weight: 700;
  color: #ffffff;
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.2px;
}

.gauge-ring {
  transition: stroke-dashoffset 0.6s cubic-bezier(0.34, 1.4, 0.64, 1);
}

/* ===================================================================== */
/* 4. THE HANDLES (Top Move Handle + Bottom Settings Orb)                */
/* ===================================================================== */
.handle {
  position: absolute;
  width: 57px;
  height: 57px;
  z-index: 46;
  cursor: pointer;
  --arc: 270deg;
  transition: opacity 0.2s ease;
}

#move {
  --arc: 0deg;
  right: calc(38.7px - 28.5px);
  top: calc(50% - 28.5px);
  transform: translateY(-165px); /* Positioned over the top fillet pocket */
}

#orb {
  --arc: 270deg;
  right: calc(38.7px - 28.5px);
  top: calc(50% - 28.5px);
  transform: translateY(165px); /* Positioned over the bottom fillet pocket */
}

.handle.is-folded-handle {
  opacity: 0;
  pointer-events: none;
}

.handle > * {
  position: absolute;
  left: 50%;
  top: 50%;
  pointer-events: none;
  transition: opacity 0.2s ease, transform 0.36s cubic-bezier(0.34, 1.3, 0.64, 1);
}

.h-rest {
  width: 72px;
  height: 72px;
  margin: -36px 0 0 -36px;
  overflow: visible;
  transform: rotate(var(--arc)) scale(1);
}

.h-ring {
  stroke: #2e2e2e;
}

.h-fill {
  stroke: #000000;
}

.h-disc {
  width: 46.6px;
  height: 46.6px;
  margin: -23.3px 0 0 -23.3px;
  border-radius: 50%;
  background: #000000;
  border: 1px solid #2e2e2e;
  opacity: 0;
  transform: scale(1.1);
}

.h-glyph {
  width: 21px;
  height: 21px;
  margin: -10.5px 0 0 -10.5px;
  fill: #e8e8ea;
  color: #e8e8ea;
  opacity: 0;
  transform: rotate(-60deg) scale(0.5);
  transition: opacity 0.2s, transform 0.55s cubic-bezier(0.34, 1.2, 0.64, 1);
}

.handle.hover .h-rest {
  opacity: 0;
  transform: rotate(var(--arc)) scale(0.86);
}

.handle.hover .h-disc {
  opacity: 1;
  transform: scale(1);
}

.handle.hover .h-glyph {
  opacity: 1;
  transform: rotate(0deg) scale(1);
}

/* ===================================================================== */
/* 5. HOVER CARD & ORGANIC TAIL                                         */
/* ===================================================================== */
#card {
  position: absolute;
  right: 98px;
  width: 254px;
  max-width: 254px;
  background: #0a0a0a;
  border: 1px solid #242424;
  border-radius: 16px;
  padding: 16px;
  color: #ffffff;
  box-shadow: 0 20px 45px rgba(0, 0, 0, 0.75);
  overflow-wrap: anywhere;
  min-width: 0;
  max-height: calc(100% - 24px);
  overflow-y: auto;
  scrollbar-width: thin;
  z-index: 50;
  transition: top 0.15s ease-out;
}

#tail {
  position: absolute;
  right: 69px;
  width: 32px;
  height: 36px;
  background: #0a0a0a;
  clip-path: path('M0 0C0 9 18.56 13.68 32 18C18.56 22.32 0 27 0 36Z');
  z-index: 50;
  pointer-events: none;
  transition: top 0.15s ease-out;
}

/* Popover Speech-Bubble Spring Animation */
.codenotch-pop-enter-active {
  transition: opacity 0.18s ease-out, transform 0.22s cubic-bezier(0.34, 1.4, 0.64, 1);
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
