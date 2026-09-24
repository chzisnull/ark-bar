<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ProviderType, ProviderUsageData, NotchMetric } from '../types';
import ProviderIcon from './ProviderIcon.vue';
import { RotateCw } from 'lucide-vue-next';

const CACHE_KEY = 'arkbar_cached_providers_data';
const FOLD_GRACE = 450; // Grace period before folding back to rest pill (Codenotch spec: 450ms)
// 收起态的几个尺寸：上报给 Rust 的热区、页面自己的唤醒判定、以及收起胶囊的绘制
// 共用这一份，避免三处各写一个数（以前热区 65px 深、唤醒判定 50px 深，对不上）
const REST_DEPTH = 10;   // 贴边胶囊的厚度
const REST_LENGTH = 79;  // 贴边胶囊的长度
const WAKE_BAND = 40;    // 唤醒带往屏幕里延伸的宽度

interface ProviderTabItem {
  id: ProviderType;
  name: string;
  visible: boolean;
  notch_metric?: NotchMetric;
  model_filter?: string;
}

const DEFAULT_TABS: ProviderTabItem[] = [
  { id: 'antigravity', name: 'Antigravity', visible: true, notch_metric: 'session', model_filter: 'gemini' },
  { id: 'grok', name: 'Grok', visible: true, notch_metric: 'session', model_filter: 'all' },
  { id: 'volcengine', name: '火山方舟', visible: true, notch_metric: 'session', model_filter: 'all' },
  { id: 'codex', name: 'Codex', visible: true, notch_metric: 'session', model_filter: 'all' },
  { id: 'teamo', name: 'Teamo', visible: true, notch_metric: 'balance', model_filter: 'all' },
];

function loadProviderTabs(): ProviderTabItem[] {
  try {
    const raw = localStorage.getItem('arkbar_provider_tabs');
    if (raw) {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed) && parsed.length > 0) {
        const validIds: ProviderType[] = ['antigravity', 'grok', 'volcengine', 'codex', 'teamo'];
        const nameMap: Record<ProviderType, string> = {
          antigravity: 'Antigravity',
          grok: 'Grok',
          volcengine: '火山方舟',
          codex: 'Codex',
          teamo: 'Teamo',
        };
        const result: ProviderTabItem[] = [];
        for (const item of parsed) {
          if (item && item.id && validIds.includes(item.id) && !result.some((r) => r.id === item.id)) {
            result.push({
              id: item.id,
              name: item.name || nameMap[item.id as ProviderType],
              visible: item.visible !== false,
              notch_metric: item.notch_metric || 'session',
              model_filter: item.model_filter || (item.id === 'antigravity' ? 'gemini' : 'all'),
            });
          }
        }
        for (const id of validIds) {
          if (!result.some((r) => r.id === id)) {
            result.push({
              id,
              name: nameMap[id],
              visible: true,
              notch_metric: 'session',
              model_filter: id === 'antigravity' ? 'gemini' : 'all',
            });
          }
        }
        return result;
      }
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

// 跨窗口总线：把全量写合并为去抖，避免一个后台周期内逐厂商写 5 次全量 blob
let cacheWriteTimer: any = null;
function flushCacheWrite() {
  cacheWriteTimer = null;
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(allCachedData.value));
  } catch {}
}
function scheduleCacheWrite() {
  if (cacheWriteTimer) clearTimeout(cacheWriteTimer);
  cacheWriteTimer = setTimeout(flushCacheWrite, 350);
}

// Display mode: 'hover' (default: collapsed pill, expands on hover) | 'always' (stay open) | 'hidden'
const notchMode = ref<'hover' | 'always' | 'hidden'>(
  (localStorage.getItem('arkbar_notch_mode') as any) || 'hover'
);

// Edge placement: 'right' (default) | 'top'
export type NotchEdge = 'right' | 'top';
const notchEdge = ref<NotchEdge>(
  (localStorage.getItem('arkbar_notch_edge') as NotchEdge) || 'right'
);

// Folded state: default true if in hover mode (rest pill shown, expands on mouseover)
const isFolded = ref<boolean>(notchMode.value === 'hover');
let foldTimer: any = null;
let hideTimer: any = null;
let pointerIn = false;
let isDragging = false;
// 刘海本体上的一次按下（Codenotch 的 press）：不拖动就是一次点击
let pillPress: { x: number; y: number; id: ProviderType | null; alt: boolean } | null = null;
// 右键菜单打开期间不折叠：菜单弹出时指针已经不在刘海上，否则刘海会在菜单底下收起来
let menuOpen = false;

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
        model_filter: tab.model_filter || (tab.id === 'antigravity' ? 'gemini' : 'all'),
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
const cardLeft = ref(100);
const tailLeft = ref(150);

// Handles hover states
const isHoveringMove = ref(false);
const isHoveringOrb = ref(false);

// Currently active popover data
const activeHoverData = computed<ProviderUsageData | null>(() => {
  if (!activeHoverId.value) return null;
  return allCachedData.value[activeHoverId.value] || null;
});

const activeHoverTab = computed(() => {
  if (!activeHoverId.value) return null;
  return providerTabs.value.find((t) => t.id === activeHoverId.value) || null;
});

// Filter groups based on the active provider's model_filter setting
const activeHoverGroups = computed(() => {
  const data = activeHoverData.value;
  if (!data || !data.groups) return [];
  const filter = activeHoverTab.value?.model_filter || 'all';
  if (filter === 'all') return data.groups;

  return data.groups.filter((g) => {
    const name = (g.group_name || '').toLowerCase();
    if (filter === 'gemini') {
      return name.includes('gemini');
    }
    if (filter === 'claude') {
      return name.includes('claude') || name.includes('gpt');
    }
    return true;
  });
});

// When groups change (e.g. filtered to 1 group), re-center card and tail
watch(activeHoverGroups, () => {
  if (activeHoverId.value && notchPillRef.value) {
    nextTick(() => {
      const el = notchPillRef.value?.querySelector<HTMLElement>(`[data-provider="${activeHoverId.value}"]`);
      if (el) {
        placeCard(el);
      }
    });
  }
}, { deep: true });

// 「它在工作吗？」——由 Rust 端 activity 线程探测（Codenotch 同款引擎：Antigravity 看
// transcript 回合、Codex 看 rollout 的 task_started/complete、Grok 看 updates.jsonl 新鲜度），
// 这里只负责展示，不猜状态。没有信号就不显示状态行。
interface ProviderActivity {
  provider: string;
  state: 'busy' | 'waiting' | 'success';
  name: string;
  detail: string;
  since: number;
}
// 有新版本时后台会广播（不再等用户点「检查更新」）：orb 上点一个小圆点，卡片里给一行
interface UpdateInfoLite { has_update: boolean; latest_version: string; release_notes?: string }
const pendingUpdate = ref<UpdateInfoLite | null>(null);

const activityMap = ref<Record<string, ProviderActivity>>({});
const nowTick = ref(Date.now());
let activityTimer: any = null;

// 未连接时的原因（后端把「未装 arkcli / 未登录 / 凭据失效」都写在 status_message 里）
const disconnectReason = computed<string>(() => {
  const d = activeHoverData.value;
  if (!d || d.is_connected) return '';
  const raw = d.status_message || d.error_message || '';
  return raw.length > 24 ? `${raw.slice(0, 24)}…` : raw;
});

// 凭据失效（不是没装、也不是网络）时，按钮该说「重新授权」
const needsReauth = computed<boolean>(() => {
  const d = activeHoverData.value;
  if (!d || d.is_connected) return false;
  const text = `${d.status_message || ''} ${d.error_message || ''}`;
  return /重新授权|重新登录|登录已失效|STS|refresh_token|refresh token|volc-sso/i.test(text);
});

// 原始报错：卡片放不下整段 JSON，短就全给，长就截断并指向设置
const shortError = computed<string>(() => {
  const raw = (activeHoverData.value?.error_message || '').replace(/\s+/g, ' ').trim();
  return raw.length > 160 ? `${raw.slice(0, 160)}…（详情见设置里的未连接面板）` : raw;
});

const activeActivity = computed<ProviderActivity | null>(() =>
  activeHoverId.value ? activityMap.value[activeHoverId.value] || null : null
);

// Rust 广播的是数组，界面按厂商取用
function indexActivity(list: ProviderActivity[] | null | undefined): Record<string, ProviderActivity> {
  const map: Record<string, ProviderActivity> = {};
  (list || []).forEach((a) => {
    if (a && a.provider) map[a.provider] = a;
  });
  return map;
}

// 等待输入用琥珀色：四个状态里只有它在向你要东西；工作中白、已完成绿
function activityColor(state: string): string {
  if (state === 'waiting') return '#ffd60a';
  if (state === 'success') return '#30d158';
  return '#f5f5f7';
}

// 「刚刚 / N 分钟前」——与 Codenotch 的 elapsed 文案一致
function elapsedText(sinceMs: number): string {
  const secs = Math.max(0, Math.floor((nowTick.value - sinceMs) / 1000));
  if (secs < 60) return '刚刚';
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins} 分钟前`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours} 小时前`;
  return `${Math.floor(hours / 24)} 天前`;
}

// 沿边位置按边各自记住（Codenotch：只有当前这条边会动，其他边保留原处）
function storedAlong(edge: NotchEdge): number | null {
  const raw = localStorage.getItem(`arkbar_notch_along_${edge}`);
  if (raw === null) return null;
  const v = Number(raw);
  if (!Number.isFinite(v)) return null;
  return Math.min(1, Math.max(0, v));
}

// Report hot rectangles to Rust watchdog
let hotRafId: number | null = null;
let lastHotKey: string | null = null;

function computeHotPayload(): { rects: number[][]; expanded: boolean } {
  if (notchMode.value === 'hidden') {
    return { rects: [], expanded: false };
  }

  const k = window.devicePixelRatio || 1;
  const W = window.innerWidth;
  const H = window.innerHeight;

  if (isFolded.value) {
    // 唤醒热区：收起胶囊本体 + 一条往屏幕里伸的唤醒带
    const wakeDepth = REST_DEPTH + WAKE_BAND;
    const wakeSpan = REST_LENGTH + 2 * 20;
    if (notchEdge.value === 'top') {
      const r = [(W / 2 - REST_LENGTH / 2 - 20) * k, 0, wakeSpan * k, wakeDepth * k];
      return { rects: [r], expanded: false };
    }
    const r = [
      (W - wakeDepth) * k,
      (H / 2 - REST_LENGTH / 2 - 20) * k,
      wakeDepth * k,
      wakeSpan * k,
    ];
    return { rects: [r], expanded: false };
  }

  const rects: number[][] = [];
  if (notchPillRef.value) {
    const pr = notchPillRef.value.getBoundingClientRect();
    if (notchEdge.value === 'top') {
      // Notch pill plus fillets and handles buffer for top edge
      rects.push([
        (pr.left - 50) * k,
        0,
        (pr.width + 100) * k,
        (pr.height + 25) * k,
      ]);
    } else {
      // Notch pill plus fillets and handles buffer for right edge
      rects.push([
        (pr.left - 10) * k,
        (pr.top - 50) * k,
        (pr.width + 20) * k,
        (pr.height + 100) * k,
      ]);
    }
  }

  if (activeHoverId.value && cardRef.value) {
    const cr = cardRef.value.getBoundingClientRect();
    if (notchEdge.value === 'top') {
      rects.push([
        (cr.left - 10) * k,
        (cr.top - 40) * k, // bridge gap between card and top pill
        (cr.width + 20) * k,
        (cr.height + 50) * k,
      ]);
    } else {
      rects.push([
        (cr.left - 10) * k,
        (cr.top - 10) * k,
        (cr.width + 50) * k, // bridge gap between card and right pill
        (cr.height + 20) * k,
      ]);
    }
  }

  return { rects, expanded: true };
}

function flushHot() {
  hotRafId = null;
  const { rects, expanded } = computeHotPayload();
  // 几何去重：DOM mousemove 与 Rust notch_cursor 双源会重复上报同一矩形，
  // 同一批矩形 + 同一展开态就不再打扰 Rust
  const key = `${expanded}|${JSON.stringify(rects.map((r) => r.map((v) => Math.round(v))))}`;
  if (key === lastHotKey) return;
  lastHotKey = key;
  invoke('set_hot', { rects, expanded }).catch(() => {});
}

// Coalesce every reportHot() in a frame into a single deduped set_hot
function reportHot() {
  if (hotRafId !== null) return;
  hotRafId = requestAnimationFrame(flushHot);
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
  if (menuOpen) return;
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

// Position card centered on hovered ring, with curved tail pointing directly at ring center
function placeCard(targetEl: HTMLElement) {
  const ring = (targetEl.querySelector('.ringwrap') as HTMLElement) || targetEl;
  const rr = ring.getBoundingClientRect();
  const W = window.innerWidth;
  const H = window.innerHeight;
  const cw = cardRef.value?.offsetWidth || 290;
  const ch = cardRef.value?.offsetHeight || 260;

  if (notchEdge.value === 'top') {
    const ringCenterX = rr.left + rr.width / 2;
    // Center card horizontally under ring, clamped within window bounds
    let left = Math.round(ringCenterX - cw / 2);
    left = Math.max(16, Math.min(left, W - cw - 16));
    cardLeft.value = left;
    cardTop.value = 98; // Positioned under the 70px pill

    // Tail width is 36px; tip is at X = 18px (middle of tail)
    const minTailX = left + 18;
    const maxTailX = left + cw - 18;
    const clampedTipX = Math.max(minTailX, Math.min(maxTailX, ringCenterX));
    tailLeft.value = Math.round(clampedTipX - 18);
    tailTop.value = 69;
  } else {
    const ringCenterY = rr.top + rr.height / 2;
    // Center card vertically on ring, clamped within window bounds
    let top = Math.round(ringCenterY - ch / 2);
    top = Math.max(16, Math.min(top, H - ch - 16));
    cardTop.value = top;

    // Tail height is 36px; tip is at Y = 18px (middle of tail)
    const minTailY = top + 18;
    const maxTailY = top + ch - 18;
    const clampedTipY = Math.max(minTailY, Math.min(maxTailY, ringCenterY));
    tailTop.value = Math.round(clampedTipY - 18);
  }
  reportHot();
}

function scheduleHideCard() {
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = setTimeout(() => {
    activeHoverId.value = null;
    reportHot();
  }, 250); // 与 Codenotch 的 scheduleHide 一致（250ms）；180ms 会显得卡片「急着消失」
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
    // 与 reportHot 上报的唤醒热区同一套数字
    const wakeDepth = REST_DEPTH + WAKE_BAND;
    const wakeHalfSpan = REST_LENGTH / 2 + 20;
    if (notchEdge.value === 'top') {
      const cx = W / 2;
      // Wake up if cursor is near top screen center around the rest pill
      if (clientY <= wakeDepth && Math.abs(clientX - cx) <= wakeHalfSpan) {
        unfold();
      }
    } else {
      const cy = H / 2;
      // Wake up if cursor is near right screen edge around the rest pill
      if (clientX >= W - wakeDepth && Math.abs(clientY - cy) <= wakeHalfSpan) {
        unfold();
      }
    }
    return;
  }

  // Check handles hover
  if (notchPillRef.value) {
    const pr = notchPillRef.value.getBoundingClientRect();
    const R = 38.7;
    if (notchEdge.value === 'top') {
      // Left handle (#move): pocket to the left of pill
      const moveHx = pr.left - R;
      const moveHy = pr.top + 35;
      isHoveringMove.value = Math.hypot(clientX - moveHx, clientY - moveHy) <= 32;

      // Right handle (#orb): pocket to the right of pill
      const orbHx = pr.right + R;
      const orbHy = pr.top + 35;
      isHoveringOrb.value = Math.hypot(clientX - orbHx, clientY - orbHy) <= 32;
    } else {
      // Top handle center: [pr.right - R, pr.top - R]
      const topHx = pr.right - R;
      const topHy = pr.top - R;
      isHoveringMove.value = Math.hypot(clientX - topHx, clientY - topHy) <= 32;

      // Bottom handle center: [pr.right - R, pr.bottom + R]
      const botHx = pr.right - R;
      const botHy = pr.bottom + R;
      isHoveringOrb.value = Math.hypot(clientX - botHx, clientY - botHy) <= 32;
    }

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
      nextTick(() => {
        placeCard(cell.el);
        requestAnimationFrame(() => placeCard(cell.el));
      });
    } else {
      placeCard(cell.el);
    }
    return;
  }

  // Check if pointer is inside card or bridge to pill
  if (activeHoverId.value && cardRef.value) {
    const cr = cardRef.value.getBoundingClientRect();
    const bridgeRect = notchEdge.value === 'top'
      ? new DOMRect(cr.left, cr.top - 36, cr.width, cr.height + 36)
      : new DOMRect(cr.left, cr.top, cr.width + 36, cr.height);
    if (inRect(clientX, clientY, bridgeRect, 8)) {
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

// Compute percentage for each provider, respecting metric and modelFilter
function getProviderDisplayPercent(id: ProviderType, metric?: NotchMetric, modelFilter?: string): number {
  const data = allCachedData.value[id];
  if (!data) return 0;

  let groups = data.groups || [];
  if (modelFilter && modelFilter !== 'all') {
    const f = modelFilter.toLowerCase();
    const filtered = groups.filter((g) => {
      const name = (g.group_name || '').toLowerCase();
      if (f === 'gemini') return name.includes('gemini');
      if (f === 'claude') return name.includes('claude') || name.includes('gpt');
      return true;
    });
    if (filtered.length > 0) {
      groups = filtered;
    }
  }

  if (metric === 'weekly') {
    for (const g of groups) {
      const p = g.periods.find((x) => x.label.toLowerCase().includes('week'));
      if (p) return Math.round(p.used_percent);
    }
  } else if (metric === 'monthly') {
    for (const g of groups) {
      const p = g.periods.find((x) => x.label.toLowerCase().includes('month'));
      if (p) return Math.round(p.used_percent);
    }
  }

  // Session / 5h metric
  for (const g of groups) {
    const p = g.periods.find((x) => x.label.toLowerCase().includes('session') || x.label.toLowerCase().includes('5h'));
    if (p) return Math.round(p.used_percent);
  }

  if ((!modelFilter || modelFilter === 'all') && data.primary_session_percent != null) {
    return Math.round(data.primary_session_percent);
  }

  for (const g of groups) {
    if (g.periods.length > 0) {
      return Math.round(g.periods[0].used_percent);
    }
  }

  return 0;
}

// Percentage badge text from an already-computed percent (shared by the ring computed)
function providerBadgeFromPct(id: ProviderType, pct: number): string {
  const data = allCachedData.value[id];
  if (!data || !data.is_connected) return '--';

  if (id === 'teamo' && data.extension?.balance?.value != null) {
    const val = data.extension.balance.value;
    return val >= 100 ? `$${Math.round(val)}` : `$${val.toFixed(1)}`;
  }

  return `${pct}%`;
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

// 每个圆环一帧只算一次：模板只读这些字段，避免同一厂商重复 filter + find
const ringViews = computed(() => {
  return notchProviders.value.map((item) => {
    const pct = getProviderDisplayPercent(item.id, item.notch_metric, item.model_filter);
    return {
      ...item,
      pct,
      color: getRingStrokeColor(pct),
      dashOffset: getRingDashOffset(pct),
      badge: providerBadgeFromPct(item.id, pct),
    };
  });
});

// Format percent value (keep clean decimals if fractional, otherwise integer)
function formatPercentValue(val: number): string {
  if (val == null || isNaN(val)) return '0';
  return Number.isInteger(val) ? val.toString() : val.toFixed(2);
}

// Format reset time with live countdown matching official console
function formatResetLabel(resetAt?: string): string {
  if (!resetAt) return '';
  const trimmed = resetAt.trim();
  if (trimmed === 'rolling-5h') return '5小时滑动周期';

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
  const diffMs = date.getTime() - now.getTime();
  if (diffMs <= 0) return '刚刚已刷新';

  const totalMins = Math.floor(diffMs / (1000 * 60));
  const totalHours = Math.floor(totalMins / 60);
  const days = Math.floor(totalHours / 24);
  const hours = totalHours % 24;
  const mins = totalMins % 60;

  if (days === 0 && totalHours < 24) {
    if (totalHours === 0) {
      return `${mins}分钟后重置`;
    }
    return `${totalHours}小时${mins > 0 ? `${mins}分钟` : ''}后重置`;
  } else if (days < 7) {
    return `${days}天${hours > 0 ? `${hours}小时` : ''}后重置`;
  }
  return `${days}天后重置 (${date.getMonth() + 1}月${date.getDate()}日)`;
}

// 抓手 = 搬运（Codenotch 的 move handle）：升起四条边的落区，指针挑一条，松手交付。
// 刘海在松手前不动——选的是屏幕上的一块地方，不是移动了多远。
// 需要刘海竖起时的真实尺寸（深度/长度），页面量给自己。
function pillShapes(): { depth: number; length: number } {
  const el = notchPillRef.value;
  const r = el ? el.getBoundingClientRect() : null;
  const depth = r && r.width > 0 ? Math.round(r.width) : 70;
  const length = r && r.height > 0 ? Math.round(r.height) : 260;
  return { depth, length };
}

function handleMoveMouseDown(e: MouseEvent) {
  if (e.button !== 0) return;
  e.stopPropagation();
  isDragging = true;
  activeHoverId.value = null;
  if (foldTimer) clearTimeout(foldTimer);
  invoke('begin_move', pillShapes()).catch(() => { isDragging = false; });

  const onMouseUp = () => {
    isDragging = false;
    // 兜底收尾：Rust 那边自己盯着左键，但页面松手时也说一声（跨平台都靠得住）
    invoke('end_notch_drag').catch(() => {});
    window.removeEventListener('mouseup', onMouseUp);
  };
  window.addEventListener('mouseup', onMouseUp);
}

// ⌥+拖动刘海本体 = 沿当前边微调（Codenotch 的 ⌥-drag）。
// 不带 ⌥ 的按下只是一次点击（点圆环刷新该厂商），所以手滑不会把刘海搬走。
function handlePillMouseDown(e: MouseEvent) {
  if (e.button !== 0 || isFolded.value) return;
  pillPress = {
    x: e.clientX,
    y: e.clientY,
    id: (getCellAt(e.clientX, e.clientY)?.id || activeHoverId.value) as ProviderType | null,
    alt: e.altKey,
  };
}

function handlePillMouseMove(e: MouseEvent) {
  const press = pillPress;
  if (!press || isDragging || !press.alt) return;
  if (Math.abs(e.clientY - press.y) > 4 || Math.abs(e.clientX - press.x) > 4) {
    isDragging = true;
    if (hideTimer) clearTimeout(hideTimer);
    activeHoverId.value = null;
    invoke('drag_begin').catch(() => {
      isDragging = false;
    });
  }
}

function handlePillMouseUp(e: MouseEvent) {
  if (e.button !== 0) return;
  const press = pillPress;
  pillPress = null;
  // 没拖动过：这一下是点击——点在哪个圆环上就刷新哪个厂商
  if (press && !isDragging && press.id) {
    refreshCurrentProvider(press.id);
  }
  isDragging = false;
}

function handleNotchContextMenu(e: MouseEvent) {
  e.preventDefault();
  if (isDragging) return;
  const pid = getCellAt(e.clientX, e.clientY)?.id || activeHoverId.value || null;
  menuOpen = true;
  // 这个调用在菜单关闭后才返回，那之后才轮到考虑折叠
  invoke('show_notch_menu', { provider: pid })
    .catch(() => {})
    .finally(() => {
      menuOpen = false;
      scheduleFold();
    });
}

// 去更新：打开设置窗口并弹出更新弹窗
function openUpdate() {
  activeHoverId.value = null;
  invoke('open_update_modal').catch(() => {});
  scheduleFold();
}

// 去安装/授权：打开设置窗口并直接落到引导（安装 arkcli / SSO 登录）那一屏
function openOnboarding() {
  activeHoverId.value = null;
  invoke('open_onboarding').catch(() => {});
  scheduleFold();
}

// Open settings window from Settings Orb
function openSettings(e?: MouseEvent) {
  if (e) e.stopPropagation();
  activeHoverId.value = null;
  scheduleFold();
  invoke('show_main_window').catch(() => {});
}

// Manual instant refresh for currently hovered provider
const isRefreshingProvider = ref(false);

// 点圆环即刷新该厂商（Codenotch 的 ring 点击语义）：先把卡片切到它，再强制拉一次
function onRingClick(id: ProviderType) {
  activeHoverId.value = id;
  nextTick(() => refreshCurrentProvider(id));
}

async function refreshCurrentProvider(id: ProviderType | null) {
  if (!id || isRefreshingProvider.value) return;
  isRefreshingProvider.value = true;
  try {
    const res = await invoke<ProviderUsageData>('get_unified_usage', {
      provider: id,
      customToken: null,
      force: true,
    });
    if (res && res.provider) {
      allCachedData.value[res.provider as ProviderType] = res;
      scheduleCacheWrite();
    }
  } catch (err) {
    console.error('Failed to refresh provider:', err);
  } finally {
    isRefreshingProvider.value = false;
  }
}

// Event unlisteners
let unlistenUsage: (() => void) | null = null;
let unlistenPointer: (() => void) | null = null;
let unlistenCursor: (() => void) | null = null;
let unlistenTabs: (() => void) | null = null;
let unlistenEdge: (() => void) | null = null;
let unlistenActivity: (() => void) | null = null;
let unlistenAlong: (() => void) | null = null;
let unlistenUpdate: (() => void) | null = null;
let unlistenMove: (() => void) | null = null;
let unlistenDrag: (() => void) | null = null;
let unlistenRefresh: (() => void) | null = null;
let unlistenKeepOpen: (() => void) | null = null;

watch(activeHoverId, (newId) => {
  if (newId && notchPillRef.value) {
    nextTick(() => {
      const el = notchPillRef.value?.querySelector<HTMLElement>(`[data-provider="${newId}"]`);
      if (el) {
        placeCard(el);
        requestAnimationFrame(() => placeCard(el));
      }
    });
  }
});

onMounted(async () => {
  window.addEventListener('mousemove', handleDomMouseMove);


  // 贴边位置以 Rust 侧的 notch.json 为准（启动时它已经按上次的位置摆好了）。
  // 只有在 localStorage 里**明确存过**选择时才回传一次——否则默认值 'right'
  // 会在 webview 存储被清掉之后把用户选好的顶部贴边顶掉。
  const storedEdge = localStorage.getItem('arkbar_notch_edge');
  if (storedEdge === 'top' || storedEdge === 'right') {
    invoke('set_notch_edge', { edge: storedEdge, along: storedAlong(storedEdge) }).catch(() => {});
  }

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

  // 3. Load provider usage — hydrate from the Rust-side read-only cache first so the
  //    notch does not kick off a second full CLI/network pull alongside the main window.
  const allProviderIds: ProviderType[] = ['volcengine', 'antigravity', 'grok', 'codex', 'teamo'];
  // 判据只能用 Rust 内存缓存是否有值：localStorage 是跨会话持久的，用它当判据会导致
  // 每次重启都跳过拉取，非活跃厂商的数字要等一个后台周期才会更新。
  let peeked = false;
  try {
    const cached = await Promise.all(
      allProviderIds.map((id) =>
        invoke<ProviderUsageData | null>('peek_cached_usage', { provider: id }).catch(() => null)
      )
    );
    cached.forEach((item) => {
      if (item && item.provider) {
        allCachedData.value[item.provider as ProviderType] = item;
        peeked = true;
      }
    });
  } catch {}

  if (!peeked) {
    // Rust 缓存为空（冷启动 / 重启后首次）：回退到完整拉取一次
    try {
      const list: ProviderUsageData[] = await invoke('get_all_providers_usage');
      if (Array.isArray(list)) {
        list.forEach((item) => {
          if (item && item.provider) {
            allCachedData.value[item.provider as ProviderType] = item;
          }
        });
      }
    } catch {}
  }
  scheduleCacheWrite();

  unlistenUsage = await listen<any>('usage-updated', (event) => {
    const payload = event.payload;
    if (payload) {
      if (typeof payload === 'object' && payload.provider && typeof payload.provider === 'string') {
        allCachedData.value[payload.provider as ProviderType] = payload;
      } else if (typeof payload === 'object') {
        allCachedData.value = { ...allCachedData.value, ...payload };
      }
      scheduleCacheWrite();
    }
  });

  unlistenTabs = await listen<any>('provider_tabs_updated', (event) => {
    if (event.payload && Array.isArray(event.payload)) {
      providerTabs.value = loadProviderTabs();
      nextTick(() => reportHot());
    }
  });

  // Rust 摆好窗口后会广播它落在哪条边（页面自己看不出），搬运换边后靠它把布局翻过来。
  // 事件名必须与 Rust 端 emit 的一致：place_notch 发的是 `notch_edge`。
  unlistenEdge = await listen<string>('notch_edge', (event) => {
    if (event.payload === 'top' || event.payload === 'right') {
      notchEdge.value = event.payload;
      localStorage.setItem('arkbar_notch_edge', event.payload);
      activeHoverId.value = null;
      nextTick(() => reportHot());
    }
  });

  // 活动状态：先取一份当前值，再听 Rust 端的变化广播
  try {
    const list = await invoke<ProviderActivity[]>('get_activity');
    activityMap.value = indexActivity(list);
  } catch {}
  unlistenActivity = await listen<ProviderActivity[]>('activity', (event) => {
    activityMap.value = indexActivity(event.payload);
  });
  // 「刚刚 / N 分钟前」要随时间刷新；卡片只在悬停时可见，10s 一次可忽略
  activityTimer = setInterval(() => {
    nowTick.value = Date.now();
  }, 10000);

  // 刘海沿边位置（拖动落点）由 Rust 回报，按边存起来，下次启动原样恢复
  unlistenAlong = await listen<{ edge: string; along: number }>('notch_along', (event) => {
    const p = event.payload;
    if (p && (p.edge === 'top' || p.edge === 'right') && Number.isFinite(p.along)) {
      localStorage.setItem(`arkbar_notch_along_${p.edge}`, String(p.along));
    }
  });

  // 搬运/滑动的收尾都由 Rust 通知（它自己盯着左键，不依赖页面回传）
  unlistenMove = await listen('move_end', () => {
    isDragging = false;
    activeHoverId.value = null;
    nextTick(() => reportHot());
    scheduleFold();
  });
  unlistenDrag = await listen('drag_end', () => {
    isDragging = false;
    nextTick(() => reportHot());
    scheduleFold();
  });

  // 后台发现新版本：orb 上亮个点，卡片里提示一行
  unlistenUpdate = await listen<UpdateInfoLite>('update-available', (event) => {
    if (event.payload?.has_update) {
      pendingUpdate.value = event.payload;
    }
  });
  // 挂载时先问一次缓存里的结果（后台线程可能已经查过了）
  invoke<UpdateInfoLite>('check_for_updates', { force: false })
    .then((info) => {
      if (info?.has_update) pendingUpdate.value = info;
    })
    .catch(() => {});

  // 右键菜单
  unlistenRefresh = await listen('notch_refresh', () => {
    // 悬停中的厂商优先；没有就用刘海里的第一个
    const target = (activeHoverId.value ||
      notchProviders.value[0]?.id ||
      'volcengine') as ProviderType;
    refreshCurrentProvider(target);
  });
  unlistenKeepOpen = await listen('notch_keep_open', () => {
    const next = notchMode.value === 'always' ? 'hover' : 'always';
    notchMode.value = next;
    localStorage.setItem('arkbar_notch_mode', next);
    isFolded.value = next === 'hover';
    invoke('set_notch_mode', { mode: next }).catch(() => {});
    nextTick(() => reportHot());
  });
  // 右键菜单要勾选「保持展开」，把当前模式告诉 Rust
  invoke('set_notch_mode', { mode: notchMode.value }).catch(() => {});

  window.addEventListener('storage', (e) => {
    if (e.key === 'arkbar_provider_tabs') {
      providerTabs.value = loadProviderTabs();
      nextTick(() => reportHot());
    } else if (e.key === CACHE_KEY) {
      // 增量合并：逐厂商比较，只有变化的才换引用，未变的保持原引用以减少重渲范围
      const incoming = loadCachedData();
      const current = allCachedData.value;
      (Object.keys(incoming) as ProviderType[]).forEach((id) => {
        const next = incoming[id];
        if (JSON.stringify(next) !== JSON.stringify(current[id])) {
          current[id] = next;
        }
      });
    } else if (e.key === 'arkbar_notch_mode') {
      const mode = (localStorage.getItem('arkbar_notch_mode') as any) || 'hover';
      notchMode.value = mode;
      if (mode === 'always') {
        isFolded.value = false;
      } else if (mode === 'hover') {
        isFolded.value = true;
      }
      nextTick(() => reportHot());
    } else if (e.key === 'arkbar_notch_edge') {
      notchEdge.value = (localStorage.getItem('arkbar_notch_edge') as NotchEdge) || 'right';
      activeHoverId.value = null;
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
  if (unlistenTabs) unlistenTabs();
  if (unlistenEdge) unlistenEdge();
  if (unlistenActivity) unlistenActivity();
  if (unlistenAlong) unlistenAlong();
  if (unlistenUpdate) unlistenUpdate();
  if (unlistenMove) unlistenMove();
  if (unlistenDrag) unlistenDrag();
  if (unlistenRefresh) unlistenRefresh();
  if (unlistenKeepOpen) unlistenKeepOpen();
  if (activityTimer) clearInterval(activityTimer);
  if (foldTimer) clearTimeout(foldTimer);
  if (hideTimer) clearTimeout(hideTimer);
  if (hotRafId !== null) cancelAnimationFrame(hotRafId);
  if (cacheWriteTimer) {
    clearTimeout(cacheWriteTimer);
    flushCacheWrite();
  }
});
</script>

<template>
  <div
    v-if="notchMode !== 'hidden'"
    class="fixed inset-0 pointer-events-none select-none flex overflow-visible font-sans"
    :class="notchEdge === 'top' ? 'items-start justify-center pt-0' : 'items-center justify-end pr-0'"
  >
    <!-- ============================================================ -->
    <!-- 1. SPEECH BUBBLE POPOVER CARD & ORGANIC TAIL (Anchored to Window) -->
    <!-- ============================================================ -->
    <Transition :name="notchEdge === 'top' ? 'codenotch-pop-top' : 'codenotch-pop'">
      <div v-if="!isFolded && activeHoverId" class="contents">
        <!-- Organic Curved Wedge Tail (clip-path from Codenotch spec) -->
        <div
          id="tail"
          :class="{ 'is-edge-top': notchEdge === 'top' }"
          :style="notchEdge === 'top' ? { top: `${tailTop}px`, left: `${tailLeft}px` } : { top: `${tailTop}px` }"
        />

        <!-- Speech Bubble Card Container -->
        <div
          ref="cardRef"
          id="card"
          class="pointer-events-auto"
          :class="{ 'is-edge-top': notchEdge === 'top' }"
          :style="notchEdge === 'top' ? { top: `${cardTop}px`, left: `${cardLeft}px` } : { top: `${cardTop}px` }"
        >
          <!-- Card Header: Logo + Title + Status -->
          <div class="flex items-center justify-between pb-2.5 mb-2.5 border-b border-white/[0.08]">
            <div class="flex items-center gap-2">
              <div class="w-6 h-6 rounded-lg bg-white/[0.07] border border-white/10 flex items-center justify-center p-1 text-white shadow-inner">
                <ProviderIcon :name="activeHoverId" class="w-3.5 h-3.5" />
              </div>
              <div class="flex flex-col">
                <h3 class="text-[13px] font-bold tracking-tight text-white leading-tight">
                  {{ activeHoverData?.provider_name || activeHoverId }}
                </h3>
                <span class="text-[10px] text-[#8e8e93] leading-tight mt-0.5">
                  {{ activeHoverData?.is_connected ? '运行正常' : '未连接' }}
                  <!-- 未连接就把原因摆出来（Windows 上最常见的是没装 arkcli / 没授权） -->
                  <span v-if="!activeHoverData?.is_connected && disconnectReason" class="text-[#ff9f0a] ml-1">
                    · {{ disconnectReason }}
                  </span>
                  <span v-if="activeHoverTab?.model_filter && activeHoverTab.model_filter !== 'all'" class="text-[#00FF88] ml-1 font-medium">
                    · {{ activeHoverTab.model_filter === 'gemini' ? 'Gemini 模型' : 'Claude 模型' }}
                  </span>
                </span>
              </div>
            </div>

            <div class="flex items-center gap-2 text-[11px] text-[#8e8e93]">
              <div class="flex items-center gap-1.5">
                <span class="w-1.5 h-1.5 rounded-full" :class="activeHoverData?.is_connected ? 'bg-[#00FF88] shadow-[0_0_6px_#00FF88]' : 'bg-[#8e8e93]'" />
                <span class="text-[10px]">实时配额</span>
              </div>
              <button
                type="button"
                class="p-1 -mr-1 rounded-md text-[#8e8e93] hover:bg-white/10 hover:text-white transition-colors cursor-pointer"
                :class="{ 'animate-spin text-white': isRefreshingProvider }"
                title="立即刷新配额"
                @click.stop="refreshCurrentProvider(activeHoverId)"
              >
                <RotateCw class="w-3.5 h-3.5" />
              </button>
            </div>
          </div>

          <!-- Card Body: Groups & Limit Rows (Fits naturally without clunky scrollbars!) -->
          <div class="space-y-2.5">
            <!-- 有新版本：主动提示（后台巡检发现，不用自己去点检查更新） -->
            <div
              v-if="pendingUpdate"
              class="bg-[#0a84ff]/10 rounded-xl p-3 border border-[#0a84ff]/30 flex items-center justify-between gap-2"
            >
              <div class="text-[11px] text-[#e8e8ea] leading-snug">
                有新版本 <span class="font-semibold">v{{ pendingUpdate.latest_version }}</span> 可用
              </div>
              <button
                type="button"
                class="shrink-0 px-2.5 py-1 rounded-lg text-[11px] font-medium bg-[#0a84ff] text-white hover:bg-[#0a84ff]/85 transition-colors cursor-pointer"
                @click.stop="openUpdate"
              >
                去更新
              </button>
            </div>

            <!-- 未连接：说清原因 + 一键去安装/授权（不装 CLI、不授权就永远刷不出来） -->
            <div
              v-if="activeHoverData && !activeHoverData.is_connected"
              class="bg-white/[0.03] rounded-xl p-3 border border-white/[0.07] flex flex-col gap-2"
            >
              <div class="text-[11px] text-[#e8e8ea] leading-relaxed">
                {{ activeHoverData.status_message || '当前服务商未检测到登录凭据或授权令牌。' }}
              </div>
              <!-- 原始报错压成一行小字：够短就全给，太长就指向设置里那一屏 -->
              <div v-if="activeHoverData.error_message" class="text-[10px] text-[#8e8e93] font-mono leading-snug break-all">
                {{ shortError }}
              </div>
              <button
                type="button"
                class="self-start px-2.5 py-1 rounded-lg text-[11px] font-medium bg-[#0a84ff] text-white hover:bg-[#0a84ff]/85 transition-colors cursor-pointer"
                @click.stop="openOnboarding"
              >
                {{ needsReauth ? '重新授权' : '安装 / 授权' }}
              </button>
            </div>
            <!-- Teamo Balance Card -->
            <div
              v-if="activeHoverId === 'teamo' && activeHoverData?.extension?.balance"
              class="bg-white/[0.03] rounded-xl p-3 border border-white/[0.07]"
            >
              <div class="flex items-center justify-between text-[11px] text-[#8e8e93] mb-1.5">
                <span>账户余额</span>
                <span>今日消费</span>
              </div>
              <div class="flex items-center justify-between">
                <span class="text-xl font-bold text-[#00FF88] tabular-nums">
                  ${{ activeHoverData.extension.balance.value.toFixed(2) }}
                  <span class="text-xs font-normal text-[#8e8e93] ml-0.5">{{ activeHoverData.extension.balance.currency }}</span>
                </span>
                <span class="text-sm font-semibold text-[#f5f5f7] tabular-nums">
                  ${{ (activeHoverData.extension.today_cost?.value ?? 0).toFixed(2) }}
                </span>
              </div>
            </div>

            <!-- Limit Groups (Gemini Models, Claude Models, Grok Build, etc.) -->
            <template v-if="activeHoverData && activeHoverGroups.length > 0">
              <div
                v-for="(group, gIdx) in activeHoverGroups"
                :key="gIdx"
                class="bg-white/[0.03] rounded-xl p-3 border border-white/[0.07] space-y-2.5"
              >
                <div v-if="group.group_name" class="flex items-center justify-between text-[11px] font-bold text-[#8e8e93] tracking-wide">
                  <span>{{ group.group_name }}</span>
                  <span v-if="group.edition" class="text-[10px] font-normal text-[#8e8e93]/80">{{ group.edition }}</span>
                </div>

                <div
                  v-for="(period, pIdx) in group.periods"
                  :key="pIdx"
                  class="space-y-1.5"
                >
                  <!-- Label & Reset Time (nowrap + truncate to prevent awkward line breaks!) -->
                  <div class="flex items-center justify-between gap-2 text-xs">
                    <span class="font-medium text-[#f5f5f7] truncate">
                      {{ period.name || period.label }}
                    </span>
                    <span class="text-[#8e8e93] text-[11px] whitespace-nowrap shrink-0 tabular-nums font-normal">
                      {{ formatResetLabel(period.reset_at) }}
                    </span>
                  </div>

                  <!-- Progress Bar Track & Fill -->
                  <div class="h-1.5 w-full bg-[#26272b] rounded-full overflow-hidden">
                    <div
                      class="h-full rounded-full transition-all duration-700 ease-out"
                      :style="{
                        width: `${Math.min(100, Math.max(0, period.used_percent))}%`,
                        backgroundColor: getRingStrokeColor(period.used_percent),
                      }"
                    />
                  </div>

                  <!-- Ratio Breakdown -->
                  <div class="flex items-center justify-between text-[11px] text-[#8e8e93] font-medium tabular-nums">
                    <span>{{ formatPercentValue(period.used_percent) }}% 已用</span>
                    <span>{{ formatPercentValue(period.remaining_percent) }}% 剩余</span>
                  </div>
                </div>
              </div>
            </template>

            <!-- Loading / Syncing / Filter empty placeholder -->
            <div v-else-if="activeHoverData && activeHoverData.groups && activeHoverData.groups.length > 0" class="py-6 text-center text-xs text-[#8e8e93]">
              <span>所选模型分类暂无数据</span>
            </div>
            <div v-else class="py-6 text-center text-xs text-[#8e8e93] flex flex-col items-center gap-2">
              <RotateCw class="w-4 h-4 animate-spin text-[#8e8e93]" />
              <span>正在同步用量读数…</span>
            </div>

            <!-- Token Summary (Tokens & Cache hit rate) -->
            <div
              v-if="activeHoverData?.token_summary"
              class="flex items-center justify-between px-1 text-[11px] text-[#8e8e93] pt-0.5"
            >
              <span class="tabular-nums">今日 Token: {{ activeHoverData.token_summary.today_tokens.toLocaleString() }}</span>
              <span v-if="activeHoverData.token_summary.cache_hit_rate" class="tabular-nums">
                ⚡ 缓存命中 {{ activeHoverData.token_summary.cache_hit_rate }}%
              </span>
            </div>
          </div>

          <!-- Footer: Live Status Row (Codenotch: 工作中 刚刚) -->
          <div class="pt-2.5 mt-2.5 border-t border-white/[0.08] flex items-center justify-between text-xs text-[#8e8e93]">
            <div class="flex items-center gap-1.5">
              <span class="font-medium text-[#f5f5f7]">{{ activeHoverData?.provider_name || activeHoverId }}</span>
            </div>

            <div v-if="activeActivity" class="flex items-center gap-1.5 text-[11px]">
              <RotateCw
                v-if="activeActivity.state === 'busy'"
                class="w-3 h-3 animate-spin"
                :style="{ color: activityColor(activeActivity.state) }"
              />
              <span
                v-else
                class="w-1.5 h-1.5 rounded-full"
                :style="{ background: activityColor(activeActivity.state) }"
              />
              <span :style="{ color: activityColor(activeActivity.state) }">{{ activeActivity.detail }}</span>
              <span class="text-[10px] text-[#8e8e93]">· {{ elapsedText(activeActivity.since) }}</span>
            </div>
            <!-- 没有信号就什么都不显示（Codenotch：宁可不报，也不编一个「工作中」） -->
          </div>
        </div>
      </div>
    </Transition>

    <!-- ============================================================ -->
    <!-- 2. UNIFIED INTERACTIVE ZONE                                  -->
    <!-- ============================================================ -->
    <div
      class="relative pointer-events-auto flex items-center"
      :class="[
        notchEdge === 'top' ? 'flex-col pt-0 is-edge-top' : 'flex-row pr-0 is-edge-right',
        { 'is-folded-body': isFolded }
      ]"
      @contextmenu="handleNotchContextMenu"
    >
      <!-- 1. REST PILL (Codenotch #rest: Sleek capsule hugging edge when folded) -->
      <div
        id="rest"
        :class="[
          { 'is-active': isFolded },
          notchEdge === 'top' ? 'is-edge-top' : 'is-edge-right'
        ]"
        @mouseenter="unfold"
        @mousedown="unfold"
      />

      <!-- 2. TOP/LEFT MOVE HANDLE (Codenotch #move handle: hand icon for carrying/dragging) -->
      <div
        id="move"
        class="handle"
        :class="[
          { 'hover': isHoveringMove, 'is-folded-handle': isFolded },
          notchEdge === 'top' ? 'is-edge-top' : 'is-edge-right'
        ]"
        title="按住拖动调整刘海位置"
        @mousedown="handleMoveMouseDown"
        @mouseenter="isHoveringMove = true"
        @mouseleave="isHoveringMove = false"
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

      <!-- 4. BOTTOM/RIGHT SETTINGS ORB (Codenotch #orb handle: gear icon for preferences) -->
      <div
        id="orb"
        class="handle"
        :class="[
          { 'hover': isHoveringOrb, 'is-folded-handle': isFolded, 'has-update': !!pendingUpdate },
          notchEdge === 'top' ? 'is-edge-top' : 'is-edge-right'
        ]"
        :title="pendingUpdate ? `偏好设置 · 有新版本 v${pendingUpdate.latest_version}` : '偏好设置'"
        @click="openSettings"
        @mouseenter="isHoveringOrb = true"
        @mouseleave="isHoveringOrb = false"
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
        :class="[
          { 'is-folded': isFolded },
          notchEdge === 'top' ? 'is-edge-top' : 'is-edge-right'
        ]"
        @mousedown="handlePillMouseDown"
        @mousemove="handlePillMouseMove"
        @mouseup="handlePillMouseUp"
      >
        <!-- Provider Circular Rings Stack -->
        <div
          v-for="(ring, idx) in ringViews"
          :key="ring.id"
          :data-provider="ring.id"
          class="cell provider-cell"
          :style="{ '--i': idx }"
          :title="`${ring.name} · 点击立即刷新`"
          @click.stop="onRingClick(ring.id)"
        >
          <!-- Circular Ring Gauge (Diameter 44px, matching Codenotch) -->
          <div
            class="ringwrap"
            :class="{ 'active': activeHoverId === ring.id }"
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
                :stroke="ring.color"
                :stroke-dasharray="CIRCUMFERENCE"
                :stroke-dashoffset="ring.dashOffset"
              />
            </svg>

            <!-- Center Provider Logo inside ring -->
            <div class="glyph">
              <ProviderIcon :name="ring.id" class="w-5 h-5 text-neutral-200" />
            </div>
          </div>

          <!-- Percentage Text Below Ring (Tabular numerals, Codenotch style) -->
          <!-- 未连接的厂商用琥珀色，一眼看出「不是 0%，是没拿到读数」 -->
          <div class="pct" :class="{ 'is-offline': ring.data && !ring.data.is_connected }">
            {{ ring.badge }}
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
/* 1. REST PILL (Codenotch #rest: Sleek capsule hugging edge)            */
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

#rest.is-edge-top {
  right: auto;
  top: 0;
  left: 50%;
  transform: translateX(-50%);
  width: 79px;
  height: 10px;
  border-radius: 0 0 6px 6px;
  border: 1px solid #2e2e2e;
  border-top: none;
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

#pill.is-edge-top {
  top: 0;
  right: auto;
  width: auto;
  min-width: 120px;
  height: 70px;
  padding: 0 20px;
  border-radius: 0 0 20px 20px;
  border: 1px solid #2e2e2e;
  border-top: none;
  flex-direction: row;
  align-items: center;
  justify-content: center;
  gap: 18px;
  --clip-open: inset(-1px -40px -40px -40px round 0);
  --clip-rest: inset(0 calc(50% - 39.5px) calc(100% - 10px) calc(50% - 39.5px) round 0 0 6px 6px);
}

#pill.is-folded {
  clip-path: var(--clip-rest);
  pointer-events: none;
}

/* Fillets: Concave arcs between pill and screen edge for Right Placement */
#pill:not(.is-edge-top)::before,
#pill:not(.is-edge-top)::after {
  content: "";
  position: absolute;
  right: 0;
  width: var(--fillet);
  height: var(--fillet);
  pointer-events: none;
}

#pill:not(.is-edge-top)::before {
  top: calc(-1 * var(--fillet));
  background: radial-gradient(circle at 0 0, transparent calc(var(--fillet) - 1.5px), #2e2e2e calc(var(--fillet) - 0.5px), #000000 calc(var(--fillet) + 0.5px));
}

#pill:not(.is-edge-top)::after {
  bottom: calc(-1 * var(--fillet));
  background: radial-gradient(circle at 0 100%, transparent calc(var(--fillet) - 1.5px), #2e2e2e calc(var(--fillet) - 0.5px), #000000 calc(var(--fillet) + 0.5px));
}

/* Fillets: Concave arcs between pill and screen edge for Top Placement */
#pill.is-edge-top::before,
#pill.is-edge-top::after {
  content: "";
  position: absolute;
  top: 0;
  width: var(--fillet);
  height: var(--fillet);
  pointer-events: none;
}

#pill.is-edge-top::before {
  left: calc(-1 * var(--fillet));
  right: auto;
  bottom: auto;
  background: radial-gradient(circle at 0 100%, transparent calc(var(--fillet) - 1.5px), #2e2e2e calc(var(--fillet) - 0.5px), #000000 calc(var(--fillet) + 0.5px));
}

#pill.is-edge-top::after {
  right: calc(-1 * var(--fillet));
  left: auto;
  bottom: auto;
  background: radial-gradient(circle at 100% 100%, transparent calc(var(--fillet) - 1.5px), #2e2e2e calc(var(--fillet) - 0.5px), #000000 calc(var(--fillet) + 0.5px));
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

.is-edge-top.is-folded .cell {
  opacity: 0;
  transform: translateY(-10px);
  pointer-events: none;
}

.ringwrap {
  position: relative;
  width: 44px;
  height: 44px;
  transition: transform 0.3s cubic-bezier(0.34, 1.4, 0.64, 1);
}

/* Codenotch：悬停不放大（否则指针一靠近就「粘」上去），按下才缩到 .93 给回执 */
/* 有新版本：设置齿轮上点一个小圆点（收起态看不到 orb，展开就能看见） */
.handle.has-update::after {
  content: '';
  position: absolute;
  top: 6px;
  right: 6px;
  width: 7px;
  height: 7px;
  border-radius: 9999px;
  background: #ff9f0a;
  box-shadow: 0 0 6px rgba(255, 159, 10, 0.9);
  z-index: 2;
}

.pct.is-offline {
  color: #ff9f0a;
}

.ringwrap:active {
  transform: scale(0.93);
  transition: transform 0.12s ease-out;
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

#move:not(.is-edge-top) {
  --arc: 0deg;
  right: 10.2px;
  top: -67.2px; /* Positioned directly over the top fillet pocket */
  transform: none;
}

#orb:not(.is-edge-top) {
  --arc: 270deg;
  right: 10.2px;
  bottom: -67.2px; /* Positioned directly over the bottom fillet pocket */
  transform: none;
}

#move.is-edge-top {
  --arc: 270deg;
  left: -67.2px; /* Left fillet pocket */
  right: auto;
  top: 10.2px;
  bottom: auto;
  transform: none;
}

#orb.is-edge-top {
  --arc: 180deg;
  right: -67.2px; /* Right fillet pocket */
  left: auto;
  top: 10.2px;
  bottom: auto;
  transform: none;
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
  width: 290px;
  max-width: 290px;
  background: #0e0f14;
  border: 1px solid rgba(255, 255, 255, 0.09);
  border-radius: 18px;
  padding: 14px 16px;
  color: #ffffff;
  box-shadow: 0 24px 50px -10px rgba(0, 0, 0, 0.8), 0 0 0 1px rgba(255, 255, 255, 0.04);
  max-height: calc(100% - 32px);
  overflow-y: auto;
  overflow-x: hidden;
  z-index: 50;
  transition: top 0.16s cubic-bezier(0.32, 0.72, 0.24, 1);

  /* Minimalist Apple overlay scrollbar: invisible by default, subtle pill on hover */
  scrollbar-width: thin;
  scrollbar-color: transparent transparent;
}

#card.is-edge-top {
  right: auto;
  max-height: calc(100% - 110px);
  transition: left 0.16s cubic-bezier(0.32, 0.72, 0.24, 1);
}

#card:hover {
  scrollbar-color: rgba(255, 255, 255, 0.2) transparent;
}

#card::-webkit-scrollbar {
  width: 4px;
}

#card::-webkit-scrollbar-track {
  background: transparent;
  margin: 10px 0;
}

#card::-webkit-scrollbar-thumb {
  background: transparent;
  border-radius: 9999px;
  transition: background-color 0.2s ease;
}

#card:hover::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.2);
}

#card::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.45);
}

#tail {
  position: absolute;
  right: 69px;
  width: 32px;
  height: 36px;
  background: #0e0f14;
  clip-path: path('M0 0C0 9 18.56 13.68 32 18C18.56 22.32 0 27 0 36Z');
  z-index: 50;
  pointer-events: none;
  transition: top 0.16s cubic-bezier(0.32, 0.72, 0.24, 1);
}

#tail.is-edge-top {
  right: auto;
  width: 36px;
  height: 32px;
  clip-path: path('M0 32C9 32 13.68 13.44 18 0C22.32 13.44 27 32 36 32Z');
  transition: left 0.16s cubic-bezier(0.32, 0.72, 0.24, 1);
}

/* Popover Speech-Bubble Spring Animation (Right Edge) */
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

/* Popover Speech-Bubble Spring Animation (Top Edge) */
.codenotch-pop-top-enter-active {
  transition: opacity 0.18s ease-out, transform 0.22s cubic-bezier(0.34, 1.4, 0.64, 1);
}
.codenotch-pop-top-leave-active {
  transition: opacity 0.12s ease-in, transform 0.12s ease-in;
}
.codenotch-pop-top-enter-from {
  opacity: 0;
  transform: translateY(-10px) scale(0.96);
}
.codenotch-pop-top-leave-to {
  opacity: 0;
  transform: translateY(-8px) scale(0.97);
}
</style>
