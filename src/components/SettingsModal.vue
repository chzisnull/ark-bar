<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { EnvironmentStatus, UpdateInfo, ProviderType, TrayPercentMode, ProviderTabConfig } from '../types';
import { ArrowLeft, RefreshCw, Download, CheckCircle2, AlertCircle, Power, ExternalLink, ChevronDown, ChevronUp, Sparkles, Eye, EyeOff } from 'lucide-vue-next';
import { openUrl } from '@tauri-apps/plugin-opener';
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
import UpdateModal from './UpdateModal.vue';

const props = defineProps<{
  envStatus: EnvironmentStatus | null;
  refreshInterval: number;
  trayPercentMode: TrayPercentMode;
  initialUpdateInfo?: UpdateInfo | null;
  activeProvider: ProviderType;
  providerTabs?: ProviderTabConfig[];
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'update-interval', val: number): void;
  (e: 'update-tray-percent-mode', val: TrayPercentMode): void;
  (e: 'update-tray-target', val: ProviderType | 'auto'): void;
  (e: 'update-provider-tabs', tabs: ProviderTabConfig[]): void;
  (e: 're-login'): void;
  (e: 'provider-token-updated'): void;
  (e: 'update-checked', val: UpdateInfo): void;
}>();

const localTabs = ref<ProviderTabConfig[]>(
  props.providerTabs ? JSON.parse(JSON.stringify(props.providerTabs)) : [
    { id: 'volcengine', name: '火山方舟', visible: true },
    { id: 'antigravity', name: 'Antigravity', visible: true },
    { id: 'grok', name: 'Grok', visible: true },
    { id: 'codex', name: 'Codex', visible: true },
  ]
);

watch(() => props.providerTabs, (newVal) => {
  if (newVal) {
    localTabs.value = JSON.parse(JSON.stringify(newVal));
  }
}, { deep: true });

const visibleCount = computed(() => localTabs.value.filter((t) => t.visible).length);

function moveProvider(index: number, delta: number) {
  const newIndex = index + delta;
  if (newIndex < 0 || newIndex >= localTabs.value.length) return;
  const [moved] = localTabs.value.splice(index, 1);
  localTabs.value.splice(newIndex, 0, moved);
  emit('update-provider-tabs', [...localTabs.value]);
}

function toggleProviderVisibility(id: ProviderType) {
  const item = localTabs.value.find((t) => t.id === id);
  if (!item) return;
  if (item.visible && visibleCount.value <= 1) {
    return;
  }
  item.visible = !item.visible;
  emit('update-provider-tabs', [...localTabs.value]);
}

function getProviderMeta(id: ProviderType) {
  switch (id) {
    case 'volcengine':
      return {
        icon: '🌋',
        title: '火山方舟 Coding Plan',
        sub: 'ArkCLI 登录态与席位配额',
      };
    case 'grok':
      return {
        icon: '⚡',
        title: 'xAI Grok',
        sub: '自动识别，无需配置',
      };
    case 'antigravity':
      return {
        icon: '🌐',
        title: 'Google Antigravity',
        sub: '自动集成，无需配置',
      };
    case 'codex':
      return {
        icon: '🤖',
        title: 'OpenAI Codex',
        sub: '自动识别，无需配置',
      };
  }
}

const isCheckingUpdate = ref(false);
const updateResult = ref<UpdateInfo | null>(props.initialUpdateInfo || null);
const scrollContainerRef = ref<HTMLElement | null>(null);

watch(() => props.initialUpdateInfo, (val) => {
  if (val) updateResult.value = val;
});
const updateError = ref('');
const isFloatOpen = ref(false);
// 菜单栏图标显隐；系统注册状态由前端持久化 + set_tray_icon_visible 应用
const trayIconVisible = ref(localStorage.getItem('arkbar_tray_icon_visible') !== 'false');
const isMac = navigator.platform.toUpperCase().includes('MAC');
const trayHotkeyLabel = isMac ? '⌘⇧A' : 'Ctrl+Shift+A';

async function toggleTrayIcon() {
  const next = !trayIconVisible.value;
  trayIconVisible.value = next;
  localStorage.setItem('arkbar_tray_icon_visible', String(next));
  try {
    await invoke('set_tray_icon_visible', { visible: next });
  } catch {
    trayIconVisible.value = !next;
    localStorage.setItem('arkbar_tray_icon_visible', String(!next));
  }
}
const isAutostartEnabled = ref<boolean | null>(null);
const isAutostartLoading = ref(false);
const isAutostartUpdating = ref(false);
const autostartError = ref('');
let autostartStatusRequestId = 0;
const showUpdateModal = ref(false);
const appVersion = computed(() => updateResult.value?.current_version || props.initialUpdateInfo?.current_version || '0.3.2');

// Tray Target Provider selection
const trayTarget = ref<ProviderType | 'auto'>(
  (localStorage.getItem('arkbar_tray_target') as ProviderType | 'auto') || 'volcengine'
);

// Desktop Floating Widget Primary Provider
const floatPrimaryProvider = ref<ProviderType>(
  (localStorage.getItem('arkbar_float_primary_provider') as ProviderType) ||
  (localStorage.getItem('arkbar_float_provider') as ProviderType) ||
  'volcengine'
);

function handleFloatPrimaryChange(e: Event) {
  const target = (e.target as HTMLSelectElement).value as ProviderType;
  floatPrimaryProvider.value = target;
  localStorage.setItem('arkbar_float_primary_provider', target);
  localStorage.setItem('arkbar_float_provider', target);
  window.dispatchEvent(new StorageEvent('storage', {
    key: 'arkbar_float_primary_provider',
    newValue: target,
  }));
}

async function toggleFloatWindow() {
  if (isFloatOpen.value) {
    await invoke('close_float_window');
    isFloatOpen.value = false;
  } else {
    await invoke('open_float_window');
    isFloatOpen.value = true;
  }
}

function handleTrayTargetChange(e: Event) {
  const target = (e.target as HTMLSelectElement).value as ProviderType | 'auto';
  trayTarget.value = target;
  localStorage.setItem('arkbar_tray_target', target);
  emit('update-tray-target', target);
}

function formatAutostartError(err: unknown): string {
  if (typeof err === 'string' && err.trim()) return err;
  if (err && typeof err === 'object' && 'message' in err) {
    const message = String((err as { message?: unknown }).message || '').trim();
    if (message) return message;
  }
  return '未知错误';
}

async function loadAutostartStatus() {
  if (isAutostartLoading.value || isAutostartUpdating.value) return;
  const requestId = ++autostartStatusRequestId;
  isAutostartLoading.value = true;
  autostartError.value = '';
  try {
    const enabled = await isEnabled();
    if (requestId === autostartStatusRequestId) {
      isAutostartEnabled.value = enabled;
    }
  } catch (err) {
    if (requestId === autostartStatusRequestId) {
      isAutostartEnabled.value = null;
      autostartError.value = `读取开机自动启动状态失败：${formatAutostartError(err)}`;
    }
  } finally {
    if (requestId === autostartStatusRequestId) {
      isAutostartLoading.value = false;
    }
  }
}

async function toggleAutostart() {
  if (
    isAutostartEnabled.value === null ||
    isAutostartLoading.value ||
    isAutostartUpdating.value
  ) {
    return;
  }

  const previous = isAutostartEnabled.value;
  const next = !previous;
  isAutostartUpdating.value = true;
  autostartError.value = '';
  isAutostartEnabled.value = next;

  try {
    if (next) {
      await enable();
    } else {
      await disable();
    }
  } catch (err) {
    // 注册/取消本身失败时，系统状态仍应保持 previous。
    isAutostartEnabled.value = previous;
    autostartError.value = `设置开机自动启动失败：${formatAutostartError(err)}`;
    isAutostartUpdating.value = false;
    return;
  }

  try {
    // 以系统注册状态为准，避免底层调用部分成功时 UI 假状态。
    isAutostartEnabled.value = await isEnabled();
  } catch (err) {
    // enable/disable 已经成功，校验失败时不能假装回滚；标记未知，
    // 让用户通过重试重新读取真实系统状态。
    isAutostartEnabled.value = null;
    autostartError.value = `已更新开机启动设置，但读取最新状态失败：${formatAutostartError(err)}`;
  } finally {
    isAutostartUpdating.value = false;
  }
}

async function handleCheckUpdate() {
  if (isCheckingUpdate.value) return;
  const savedScrollTop = scrollContainerRef.value?.scrollTop;
  isCheckingUpdate.value = true;
  updateError.value = '';
  try {
    const info = await invoke<UpdateInfo>('check_for_updates', { force: true });
    updateResult.value = info;
    emit('update-checked', info);
  } catch (err: any) {
    updateError.value = `检查更新失败: ${err}`;
  } finally {
    isCheckingUpdate.value = false;
    await nextTick();
    if (scrollContainerRef.value && savedScrollTop != null) {
      scrollContainerRef.value.scrollTop = savedScrollTop;
    }
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
  window.close();
}

onMounted(async () => {
  void loadAutostartStatus();
  try {
    isFloatOpen.value = await invoke<boolean>('is_float_window_open');
  } catch {}
});
</script>

<template>
  <div class="flex flex-col h-full select-none text-slate-200 bg-[#0e131f]/95">
    <!-- Top Header（与主面板同规格 chrome） -->
    <div class="h-11 px-4 shrink-0 bg-[#141b2d]/90 border-b border-slate-800/80 flex items-center gap-2">
      <button
        type="button"
        @click="$emit('close')"
        class="w-7 h-7 rounded-lg grid place-items-center text-slate-400 hover:text-white hover:bg-slate-800/80 transition-colors"
      >
        <ArrowLeft class="w-4 h-4" />
      </button>
      <h3 class="text-[13px] font-bold text-white">设置</h3>
      <div class="ml-auto flex items-center gap-1.5">
        <span
          v-if="updateResult?.has_update"
          class="inline-flex items-center gap-1 h-[18px] px-2 rounded-full text-[10px] font-medium bg-amber-500/15 border border-amber-500/40 text-amber-400"
        >
          <span class="w-1.5 h-1.5 rounded-full bg-amber-400 animate-ping"></span>
          有新版 v{{ updateResult.latest_version }}
        </span>
        <span class="inline-flex items-center h-[18px] px-2 rounded-full text-[10px] font-mono bg-slate-800/80 border border-slate-700/60 text-slate-400">
          v{{ appVersion }}
        </span>
      </div>
    </div>

    <!-- 🚀 Top Update Notification Banner (新版本置顶提醒) -->
    <div
      v-if="updateResult?.has_update"
      class="shrink-0 bg-gradient-to-r from-amber-500/20 via-orange-500/20 to-amber-500/10 border-b border-amber-500/30 px-4 py-2 flex items-center justify-between text-xs"
    >
      <div class="flex items-center gap-1.5 text-amber-300 font-medium">
        <Sparkles class="w-3.5 h-3.5 text-amber-400 shrink-0 animate-pulse" />
        <span class="text-[11px]">发现新版本 <strong>v{{ updateResult.latest_version }}</strong></span>
      </div>
      <div class="flex items-center gap-1.5">
        <button
          type="button"
          @click="openReleaseUrl"
          class="h-6 px-2 bg-slate-800/80 hover:bg-slate-700 text-slate-300 rounded text-[10px] transition"
        >
          网页下载
        </button>
        <button
          type="button"
          @click="showUpdateModal = true"
          class="h-6 px-2.5 bg-gradient-to-r from-amber-500 to-orange-500 hover:from-amber-400 hover:to-orange-400 text-slate-950 font-bold rounded text-[10px] flex items-center gap-1 transition shadow-sm"
        >
          <Download class="w-2.5 h-2.5" />
          立即在线更新
        </button>
      </div>
    </div>

    <!-- Content Sections -->
    <div ref="scrollContainerRef" class="flex-1 overflow-y-auto px-4 py-4 flex flex-col gap-4 text-xs" style="overflow-anchor: auto;">
      <!-- 1. Multi-Provider Integration -->
      <section>
        <div class="flex items-center justify-between px-0.5 mb-2">
          <span class="text-[11px] font-semibold text-slate-400">服务商集成与顶部排序</span>
          <span class="text-[10px] text-slate-400">支持排序与显隐开关</span>
        </div>
        <div class="rounded-xl border border-slate-800/60 bg-[#131a2a] divide-y divide-slate-800/60 overflow-hidden">
          <div
            v-for="(item, idx) in localTabs"
            :key="item.id"
            class="px-3 py-2.5 flex items-center gap-2.5 transition-colors"
            :class="[!item.visible ? 'opacity-50 bg-slate-950/20' : '']"
          >
            <!-- Reorder Arrows (↑ / ↓) -->
            <div class="flex flex-col gap-0.5 shrink-0">
              <button
                type="button"
                @click="moveProvider(idx, -1)"
                :disabled="idx === 0"
                class="w-5 h-3.5 rounded grid place-items-center text-slate-400 hover:text-white hover:bg-slate-800 disabled:opacity-20 disabled:hover:bg-transparent transition cursor-pointer"
                title="上移顺序"
              >
                <ChevronUp class="w-3 h-3" />
              </button>
              <button
                type="button"
                @click="moveProvider(idx, 1)"
                :disabled="idx === localTabs.length - 1"
                class="w-5 h-3.5 rounded grid place-items-center text-slate-400 hover:text-white hover:bg-slate-800 disabled:opacity-20 disabled:hover:bg-transparent transition cursor-pointer"
                title="下移顺序"
              >
                <ChevronDown class="w-3 h-3" />
              </button>
            </div>

            <!-- Provider Icon -->
            <span class="w-6 h-6 rounded-lg bg-slate-800/80 grid place-items-center text-xs shrink-0 select-none">
              {{ getProviderMeta(item.id)?.icon }}
            </span>

            <!-- Provider Info -->
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1.5">
                <p class="text-xs font-medium text-white leading-5 truncate">{{ getProviderMeta(item.id)?.title }}</p>
                <span
                  v-if="!item.visible"
                  class="text-[9px] px-1.5 py-0.5 rounded bg-slate-800/90 text-slate-400 font-mono shrink-0"
                >
                  顶部已隐藏
                </span>
              </div>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">{{ getProviderMeta(item.id)?.sub }}</p>
            </div>

            <!-- Visibility Eye Toggle -->
            <button
              type="button"
              @click="toggleProviderVisibility(item.id)"
              :disabled="item.visible && visibleCount <= 1"
              class="w-7 h-7 rounded-lg grid place-items-center text-slate-400 hover:text-white hover:bg-slate-800/80 disabled:opacity-25 disabled:hover:bg-transparent transition cursor-pointer shrink-0"
              :title="item.visible ? (visibleCount <= 1 ? '至少保留一个展示服务商' : '在顶部标签栏隐藏') : '在顶部标签栏展示'"
            >
              <Eye v-if="item.visible" class="w-3.5 h-3.5 text-emerald-400" />
              <EyeOff v-else class="w-3.5 h-3.5 text-slate-500" />
            </button>

            <!-- Status / Action (Right slot) -->
            <div class="w-[105px] h-7 shrink-0 flex items-center justify-end">
              <!-- Volcengine -->
              <template v-if="item.id === 'volcengine'">
                <span v-if="envStatus?.logged_in" class="flex items-center gap-1 text-[10px] font-mono text-slate-300 min-w-0" :title="envStatus.user_name || ''">
                  <CheckCircle2 class="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                  <span class="truncate">{{ envStatus.user_name || '已登录' }}</span>
                </span>
                <button
                  v-else
                  type="button"
                  @click="$emit('re-login')"
                  class="w-full h-7 rounded-lg bg-sky-600/80 hover:bg-sky-500 text-white text-[11px] font-medium transition-colors cursor-pointer whitespace-nowrap"
                >
                  前往登录
                </button>
              </template>
              <!-- Grok -->
              <template v-else-if="item.id === 'grok'">
                <span class="text-[10px] font-mono text-slate-500 text-right truncate" title="自动识别 ~/.grok/auth.json">读取 ~/.grok</span>
              </template>
              <!-- Antigravity -->
              <template v-else-if="item.id === 'antigravity'">
                <span class="text-[10px] font-mono text-slate-500 text-right truncate" title="自动集成 agy CLI">agy CLI</span>
              </template>
              <!-- Codex -->
              <template v-else-if="item.id === 'codex'">
                <span class="text-[10px] font-mono text-slate-500 text-right truncate" title="自动识别 ~/.codex/auth.json">读取 ~/.codex</span>
              </template>
            </div>
          </div>
        </div>
      </section>

      <!-- 2. Desktop Display（悬浮窗 + 托盘合并） -->
      <section>
        <div class="flex items-center justify-between px-0.5 mb-2">
          <span class="text-[11px] font-semibold text-slate-400">桌面展示</span>
        </div>
        <div class="rounded-xl border border-slate-800/60 bg-[#131a2a] divide-y divide-slate-800/60 overflow-hidden">
          <!-- Float window toggle -->
          <div class="px-4 py-3 flex items-center gap-3">
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">悬浮监控框</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">常驻桌面顶层，支持拖拽</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <button
                @click="toggleFloatWindow"
                class="w-9 h-5 rounded-full p-0.5 border transition-colors duration-200 cursor-pointer"
                :class="isFloatOpen ? 'bg-indigo-600 border-indigo-500' : 'bg-slate-800 border-slate-700'"
                :title="isFloatOpen ? '关闭悬浮监控框' : '开启悬浮监控框'"
              >
                <div
                  class="w-3.5 h-3.5 rounded-full bg-white shadow-sm transition-transform duration-200"
                  :class="{ 'translate-x-4': isFloatOpen }"
                ></div>
              </button>
            </div>
          </div>
          <!-- Float primary provider -->
          <div class="px-4 py-3 flex items-center gap-3">
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">悬浮框首选厂商</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">悬停悬浮框展开全部厂商</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <div class="relative">
                <select
                  :value="floatPrimaryProvider"
                  @change="handleFloatPrimaryChange"
                  class="h-7 w-[128px] pl-2.5 pr-6 rounded-lg bg-slate-900/70 border border-slate-800/60 text-xs text-slate-200 truncate cursor-pointer outline-none hover:border-slate-700/60 focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/30 appearance-none"
                >
                  <option value="volcengine">火山方舟</option>
                  <option value="grok">xAI Grok</option>
                  <option value="antigravity">Antigravity</option>
                  <option value="codex">Codex</option>
                </select>
                <ChevronDown class="w-3 h-3 absolute right-2 top-1/2 -translate-y-1/2 text-slate-500 pointer-events-none" />
              </div>
            </div>
          </div>
          <!-- Tray target -->
          <div class="px-4 py-3 flex items-center gap-3">
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">菜单栏监控目标</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">菜单栏标题显示的配额指标</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <div class="relative">
                <select
                  :value="trayTarget"
                  @change="handleTrayTargetChange"
                  class="h-7 w-[128px] pl-2.5 pr-6 rounded-lg bg-slate-900/70 border border-slate-800/60 text-xs text-slate-200 truncate cursor-pointer outline-none hover:border-slate-700/60 focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/30 appearance-none"
                >
                  <option value="volcengine">火山方舟</option>
                  <option value="antigravity">Antigravity</option>
                  <option value="grok">Grok</option>
                  <option value="codex">Codex</option>
                  <option value="auto">跟随当前</option>
                </select>
                <ChevronDown class="w-3 h-3 absolute right-2 top-1/2 -translate-y-1/2 text-slate-500 pointer-events-none" />
              </div>
            </div>
          </div>
          <!-- Tray icon visibility -->
          <div class="px-4 py-3 flex items-center gap-3">
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">菜单栏图标</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">
                {{ trayIconVisible ? '占用一个菜单栏图标位' : `已隐藏：按 ${trayHotkeyLabel} 或悬浮窗按钮恢复` }}
              </p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <button
                type="button"
                @click="toggleTrayIcon"
                :aria-pressed="trayIconVisible"
                aria-label="显示菜单栏图标"
                :title="trayIconVisible ? '隐藏菜单栏图标' : '显示菜单栏图标'"
                class="w-9 h-5 rounded-full p-0.5 border transition-colors duration-200 cursor-pointer"
                :class="trayIconVisible ? 'bg-indigo-600 border-indigo-500' : 'bg-slate-800 border-slate-700'"
              >
                <div
                  class="w-3.5 h-3.5 rounded-full bg-white shadow-sm transition-transform duration-200"
                  :class="{ 'translate-x-4': trayIconVisible }"
                ></div>
              </button>
            </div>
          </div>
          <!-- Tray percentage mode -->
          <div class="px-4 py-3 flex items-center gap-3">
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">菜单栏指示文本</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">
                {{ trayIconVisible ? '可选择显示配额百分比或 Token 用量' : '图标已隐藏' }}
              </p>
            </div>
            <div class="w-[136px] h-7 shrink-0 flex items-center justify-end">
              <div class="relative">
                <select
                  :value="trayPercentMode"
                  :disabled="!trayIconVisible"
                  @change="$emit('update-tray-percent-mode', ($event.target as HTMLSelectElement).value as TrayPercentMode)"
                  class="h-7 w-[130px] pl-2.5 pr-6 rounded-lg bg-slate-900/70 border border-slate-800/60 text-xs text-slate-200 truncate cursor-pointer outline-none hover:border-slate-700/60 focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/30 appearance-none disabled:opacity-50 disabled:cursor-not-allowed"
                  :title="trayIconVisible ? '' : '菜单栏图标已隐藏'"
                >
                  <option value="always">⚡ 配额百分比</option>
                  <option value="today_tokens">🔥 今日 Token</option>
                  <option value="session_tokens">⏱️ 5小时 Token</option>
                  <option value="alert">⚠️ 仅告警时</option>
                  <option value="never">纯图标 (不显示)</option>
                </select>
                <ChevronDown class="w-3 h-3 absolute right-2 top-1/2 -translate-y-1/2 text-slate-500 pointer-events-none" />
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- 3. Startup Behavior -->
      <section>
        <div class="flex items-center justify-between px-0.5 mb-2">
          <span class="text-[11px] font-semibold text-slate-400">启动设置</span>
        </div>
        <div class="rounded-xl border border-slate-800/60 bg-[#131a2a] divide-y divide-slate-800/60 overflow-hidden">
          <div class="px-4 py-3 flex items-center gap-3">
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">开机自动启动</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">登录系统后自动启动 ArkBar</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <button
                type="button"
                @click="toggleAutostart"
                :disabled="isAutostartLoading || isAutostartUpdating || isAutostartEnabled === null"
                :aria-pressed="isAutostartEnabled === true"
                aria-label="开机自动启动"
                :title="isAutostartLoading ? '正在读取状态' : isAutostartUpdating ? '正在更新开机启动设置' : '开机自动启动'"
                class="w-9 h-5 rounded-full p-0.5 border transition-colors duration-200 cursor-pointer disabled:cursor-not-allowed disabled:opacity-50"
                :class="isAutostartEnabled ? 'bg-indigo-600 border-indigo-500' : 'bg-slate-800 border-slate-700'"
              >
                <RefreshCw
                  v-if="isAutostartLoading || isAutostartUpdating"
                  class="w-3.5 h-3.5 p-0.5 text-slate-300 animate-spin"
                />
                <div
                  v-else
                  class="w-3.5 h-3.5 rounded-full bg-white shadow-sm transition-transform duration-200"
                  :class="{ 'translate-x-4': isAutostartEnabled === true }"
                ></div>
              </button>
            </div>
          </div>
          <div v-if="autostartError" class="px-4 py-2.5 flex items-center justify-between gap-2 text-[11px] text-rose-400" role="alert">
            <span class="min-w-0">{{ autostartError }}</span>
            <button
              type="button"
              @click="loadAutostartStatus"
              class="shrink-0 text-slate-300 hover:text-white underline underline-offset-2"
            >
              重试
            </button>
          </div>
        </div>
      </section>

      <!-- 4. Data Sync -->
      <section>
        <div class="flex items-center justify-between px-0.5 mb-2">
          <span class="text-[11px] font-semibold text-slate-400">数据同步</span>
        </div>
        <div class="rounded-xl border border-slate-800/60 bg-[#131a2a] divide-y divide-slate-800/60 overflow-hidden">
          <div class="px-4 py-3 flex items-center gap-3">
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">自动刷新频率</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">后台静默同步各平台配额</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <div class="relative">
                <select
                  :value="refreshInterval"
                  @change="$emit('update-interval', Number(($event.target as HTMLSelectElement).value))"
                  class="h-7 w-[104px] pl-2.5 pr-6 rounded-lg bg-slate-900/70 border border-slate-800/60 text-xs text-slate-200 truncate cursor-pointer outline-none hover:border-slate-700/60 focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/30 appearance-none"
                >
                  <option :value="5">5 分钟</option>
                  <option :value="10">10 分钟</option>
                  <option :value="15">15 分钟</option>
                  <option :value="30">30 分钟</option>
                  <option :value="60">1 小时</option>
                  <option :value="0">仅手动刷新</option>
                </select>
                <ChevronDown class="w-3 h-3 absolute right-2 top-1/2 -translate-y-1/2 text-slate-500 pointer-events-none" />
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- 4. Version & Update -->
      <section>
        <div class="flex items-center justify-between px-0.5 mb-2">
          <span class="text-[11px] font-semibold text-slate-400">版本与更新</span>
        </div>
        <div class="rounded-xl border border-slate-800/60 bg-[#131a2a] divide-y divide-slate-800/60 overflow-hidden">
          <div class="px-4 py-3 flex items-center gap-3">
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">检查更新</p>
              <p class="text-[11px] font-mono text-slate-500 leading-4 mt-0.5 truncate">GitHub · chzisnull/ark-bar</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <button
                type="button"
                @click="handleCheckUpdate"
                :class="[
                  'h-7 px-2.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] text-slate-200 flex items-center gap-1 transition-colors whitespace-nowrap cursor-pointer select-none',
                  isCheckingUpdate ? 'opacity-60 pointer-events-none' : ''
                ]"
              >
                <RefreshCw class="w-3 h-3" :class="{ 'animate-spin': isCheckingUpdate }" />
                <span>{{ isCheckingUpdate ? '检查中...' : '检查更新' }}</span>
              </button>
            </div>
          </div>

          <div v-if="updateResult && updateResult.has_update" class="px-4 py-3">
            <div class="rounded-lg bg-amber-500/10 border border-amber-500/30 p-2.5 space-y-2">
              <div class="flex items-center justify-between gap-2 text-amber-300">
                <span class="flex items-center gap-1.5 font-bold text-xs whitespace-nowrap">
                  <AlertCircle class="w-3.5 h-3.5 text-amber-400 shrink-0" />
                  发现新版本 v{{ updateResult.latest_version }}
                </span>
                <span class="text-[10px] font-mono text-amber-400/80 whitespace-nowrap">
                  当前 v{{ updateResult.current_version }}
                </span>
              </div>
              <p v-if="updateResult.release_notes" class="text-[11px] text-slate-300 line-clamp-2 leading-relaxed">
                {{ updateResult.release_notes }}
              </p>
              <div class="pt-0.5 flex items-center justify-end gap-1.5">
                <button
                  @click="openReleaseUrl"
                  class="h-7 px-2 bg-slate-800 hover:bg-slate-700 text-slate-300 font-medium rounded-lg text-[11px] flex items-center gap-1 transition-colors whitespace-nowrap"
                >
                  <ExternalLink class="w-2.5 h-2.5 text-slate-400" />
                  网页下载
                </button>
                <button
                  @click="showUpdateModal = true"
                  class="h-7 px-2.5 bg-gradient-to-r from-amber-500 to-orange-500 hover:from-amber-400 hover:to-orange-400 text-slate-950 font-bold rounded-lg text-[11px] flex items-center gap-1 transition shadow-sm whitespace-nowrap"
                >
                  <Download class="w-2.5 h-2.5" />
                  立即在线更新
                </button>
              </div>
            </div>
          </div>
          <div v-else-if="updateResult" class="px-4 py-2.5 flex items-center gap-1.5 text-[11px] text-emerald-400">
            <CheckCircle2 class="w-3.5 h-3.5 shrink-0" />
            <span>当前已是最新版本 (v{{ updateResult.current_version }})</span>
          </div>

          <div v-if="updateError" class="px-4 py-2.5 text-[11px] text-rose-400">
            {{ updateError }}
          </div>
        </div>
      </section>

      <!-- 5. Quit Application -->
      <button
        @click="handleQuit"
        class="mt-1 w-full h-9 rounded-xl border border-rose-500/30 bg-rose-500/10 hover:bg-rose-500/20 text-rose-400 text-xs font-medium flex items-center justify-center gap-1.5 transition-colors"
      >
        <Power class="w-3.5 h-3.5" />
        <span>退出 ArkBar</span>
      </button>
    </div>

    <!-- In-App Online Update Modal -->
    <UpdateModal
      v-if="showUpdateModal && updateResult"
      :update-info="updateResult"
      @close="showUpdateModal = false"
    />
  </div>
</template>
