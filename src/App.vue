<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, defineAsyncComponent } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, emit as tauriEmit } from '@tauri-apps/api/event';
import type { EnvironmentStatus, UpdateInfo, ProviderType, ProviderUsageData, TrayPercentMode, ProviderTabConfig } from './types';

// Lazy load non-critical components to minimize initial bundle parsing
const OnboardingWizard = defineAsyncComponent(() => import('./components/OnboardingWizard.vue'));
const SettingsModal = defineAsyncComponent(() => import('./components/SettingsModal.vue'));
const FloatingWidget = defineAsyncComponent(() => import('./components/FloatingWidget.vue'));

const isFloatWindow = ref(window.location.hash === '#float');
const currentView = ref<'onboarding' | 'settings'>('settings');
const envStatus = ref<EnvironmentStatus | null>(null);
const updateInfo = ref<UpdateInfo | null>(null);
const isRefreshing = ref(false);
const loadingMap = ref<Record<ProviderType, boolean>>({
  volcengine: false,
  antigravity: false,
  grok: false,
  codex: false,
  teamo: false,
});

const DEFAULT_PROVIDER_TABS: ProviderTabConfig[] = [
  { id: 'antigravity', name: 'Antigravity', visible: true, notch_metric: 'session', model_filter: 'gemini' },
  { id: 'grok', name: 'Grok', visible: true, notch_metric: 'session', model_filter: 'all' },
  { id: 'volcengine', name: '火山方舟', visible: true, notch_metric: 'session', model_filter: 'all' },
  { id: 'codex', name: 'Codex', visible: true, notch_metric: 'session', model_filter: 'all' },
  { id: 'teamo', name: 'Teamo', visible: true, notch_metric: 'balance', model_filter: 'all' },
];

function loadProviderTabsConfig(): ProviderTabConfig[] {
  try {
    const raw = localStorage.getItem('arkbar_provider_tabs');
    if (!raw) return DEFAULT_PROVIDER_TABS;
    const parsed = JSON.parse(raw) as Partial<ProviderTabConfig>[];
    if (!Array.isArray(parsed)) return DEFAULT_PROVIDER_TABS;

    const result: ProviderTabConfig[] = [];
    const validIds: ProviderType[] = ['volcengine', 'antigravity', 'grok', 'codex', 'teamo'];
    const nameMap: Record<ProviderType, string> = {
      volcengine: '火山方舟',
      antigravity: 'Antigravity',
      grok: 'Grok',
      codex: 'Codex',
      teamo: 'Teamo',
    };

    for (const item of parsed) {
      if (item && item.id && validIds.includes(item.id) && !result.some((r) => r.id === item.id)) {
        result.push({
          id: item.id,
          name: nameMap[item.id],
          visible: item.visible !== false,
          notch_metric: item.notch_metric,
          model_filter: item.model_filter,
          notification_enabled: item.notification_enabled,
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
    if (!result.some((r) => r.visible)) {
      result[0].visible = true;
    }
    return result;
  } catch {
    return DEFAULT_PROVIDER_TABS;
  }
}

const providerTabs = ref<ProviderTabConfig[]>(loadProviderTabsConfig());

function saveProviderTabs(tabs: ProviderTabConfig[]) {
  providerTabs.value = tabs;
  localStorage.setItem('arkbar_provider_tabs', JSON.stringify(tabs));
  tauriEmit('provider_tabs_updated', tabs).catch(() => {});
  const visible = tabs.filter((t) => t.visible);
  if (visible.length > 0 && !visible.some((t) => t.id === activeProvider.value)) {
    handleSwitchProvider(visible[0].id);
  }
}

const activeProvider = ref<ProviderType>(
  (localStorage.getItem('arkbar_active_provider') as ProviderType) || 'volcengine'
);

const trayTarget = ref<ProviderType | 'auto'>(
  (localStorage.getItem('arkbar_tray_target') as ProviderType | 'auto') || 'volcengine'
);

// 菜单栏百分比显示模式；迁移旧布尔开关 arkbar_tray_percent
function loadTrayPercentMode(): TrayPercentMode {
  const stored = localStorage.getItem('arkbar_tray_percent_mode');
  if (stored === 'always' || stored === 'alert' || stored === 'never' || stored === 'today_tokens' || stored === 'session_tokens') return stored;
  return localStorage.getItem('arkbar_tray_percent') === 'false' ? 'never' : 'always';
}

const trayPercentMode = ref<TrayPercentMode>(loadTrayPercentMode());

const refreshInterval = ref<number>(
  Number(localStorage.getItem('arkbar_refresh_interval')) || 5
);

function formatTokensShort(tokens: number): string {
  if (tokens >= 1_000_000_000) return `${(tokens / 1_000_000_000).toFixed(2)}B`;
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(2)}M`;
  if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(1)}K`;
  return tokens.toLocaleString();
}

// 0ms startup: hydrate from local cache immediately
const CACHE_KEY = 'arkbar_cached_providers_data';

function loadCachedProviders(): Record<ProviderType, ProviderUsageData | null> {
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
      const parsed = JSON.parse(raw);
      return { ...defaults, ...parsed };
    }
  } catch {}
  return defaults;
}

const providersData = ref<Record<ProviderType, ProviderUsageData | null>>(loadCachedProviders());

let cacheWriteTimer: any = null;
function flushCachedProviders() {
  cacheWriteTimer = null;
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(providersData.value));
  } catch {}
}

// 去抖：后台一轮会逐厂商更新，合并为一次全量写，避免每窗口每周期写 5 次 blob
function saveCachedProviders() {
  if (cacheWriteTimer) clearTimeout(cacheWriteTimer);
  cacheWriteTimer = setTimeout(flushCachedProviders, 350);
}

async function checkEnv() {
  try {
    const status = await invoke<EnvironmentStatus>('check_environment');
    envStatus.value = status;
  } catch (err) {
    console.debug('Environment check failed:', err);
  }
}

const inFlight = new Set<ProviderType>();

// Fetch provider usage: supports silent background refresh to avoid UI freezes
async function fetchProviderUsage(provider: ProviderType, silent = false, force = false) {
  if (inFlight.has(provider) && !force) return;
  inFlight.add(provider);
  loadingMap.value[provider] = true;

  if (!silent) {
    isRefreshing.value = true;
  }
  try {
    const data = await invoke<ProviderUsageData>('get_unified_usage', {
      provider,
      customToken: null,
      force,
    });
    providersData.value[provider] = data;
    saveCachedProviders();
    const effectiveTarget = trayTarget.value === 'auto' ? 'volcengine' : trayTarget.value;
    if (provider === effectiveTarget || provider === 'volcengine') {
      updateTrayTitle();
    }
  } catch (err) {
    console.error(`Failed to fetch ${provider} usage:`, err);
  } finally {
    inFlight.delete(provider);
    loadingMap.value[provider] = false;
    if (!silent) {
      isRefreshing.value = false;
    }
  }
}

function handleSwitchProvider(provider: ProviderType) {
  activeProvider.value = provider;
  localStorage.setItem('arkbar_active_provider', provider);
  // 零阻塞平滑切换：本地缓存秒级渲染，后台以 force=true 静默获取最新实时用量
  fetchProviderUsage(provider, true, true);
}

function prefetchOtherProviders() {
  const visibleProviders = providerTabs.value.filter((p) => p.visible).map((p) => p.id);
  void Promise.all(
    visibleProviders
      .filter((p) => p !== activeProvider.value)
      .map((p) => fetchProviderUsage(p, true))
  );
}

async function handleRefreshCurrent() {
  await fetchProviderUsage(activeProvider.value, false, true);
}

// 托盘标题按模式计算；须与 background.rs refresh_cycle 的逻辑保持一致
function computeTrayTitle(target: ProviderType): string {
  if (trayPercentMode.value === 'never') return '';
  const data = providersData.value[target] || providersData.value['volcengine'];
  if (!data) return '';

  if (trayPercentMode.value === 'today_tokens') {
    if (data.token_summary?.today_tokens != null) {
      return ` 🔥 ${formatTokensShort(data.token_summary.today_tokens)}`;
    }
  } else if (trayPercentMode.value === 'session_tokens') {
    if (data.token_summary?.session_5h_tokens != null) {
      return ` ⏱️ ${formatTokensShort(data.token_summary.session_5h_tokens)}`;
    }
  }

  const p = data.primary_session_percent;
  if (trayPercentMode.value === 'alert') {
    if (!data.is_connected) return ' ⚠';
    return p != null && p >= 75 ? ` ${Math.round(p)}%` : '';
  }
  return data.is_connected && p != null ? ` ${Math.round(p)}%` : '';
}

// 菜单栏图标显隐偏好的单一可信源：SettingsModal 写入的 `arkbar_show_tray`
// （缺省隐藏，与 Rust 端启动时默认隐藏一致）。历史上这里读的是
// `arkbar_tray_icon_visible`（v0.2.12 的旧键），导致设置里关掉图标后
// 重启仍会重新出现——必须再手动开/关一次才生效。旧键仅做一次迁移。
function trayIconVisiblePref(): boolean {
  const stored = localStorage.getItem('arkbar_show_tray');
  if (stored !== null) return stored === 'true';
  const legacy = localStorage.getItem('arkbar_tray_icon_visible');
  const migrated = legacy === null ? 'false' : legacy === 'false' ? 'false' : 'true';
  localStorage.setItem('arkbar_show_tray', migrated);
  return migrated === 'true';
}

function updateTrayTitle() {
  // 固定展示火山方舟 5 小时用量，不随界面 Tab 切换而变动
  const target: ProviderType = trayTarget.value === 'auto' ? 'volcengine' : trayTarget.value;
  invoke('update_tray_title', { title: computeTrayTitle(target) });
}

// 周期刷新由 Rust 后台线程负责（主窗口隐藏时 webview 定时器会被 macOS/
// Windows 挂起，因此不能依赖前端 setInterval）。这里只把用户设置同步给后
// 端，并监听后台线程广播的 usage-updated 事件更新界面与托盘。
function syncBackgroundPrefs() {
  invoke('set_background_interval', { minutes: refreshInterval.value }).catch(() => {});
  // 自动检查更新（默认开）：关掉之后后台巡检真的不再发起请求
  invoke('set_auto_update', { enabled: localStorage.getItem('arkbar_auto_update') !== 'false' }).catch(() => {});
  invoke('set_tray_prefs', {
    target: trayTarget.value,
    percentMode: trayPercentMode.value,
  }).catch(() => {});
}

watch(refreshInterval, (val) => {
  localStorage.setItem('arkbar_refresh_interval', String(val));
  invoke('set_background_interval', { minutes: val }).catch(() => {});
});

watch(trayPercentMode, (val) => {
  localStorage.setItem('arkbar_tray_percent_mode', val);
  localStorage.removeItem('arkbar_tray_percent');
  updateTrayTitle();
  invoke('set_tray_prefs', {
    target: trayTarget.value,
    percentMode: val,
  }).catch(() => {});
});

watch(trayTarget, (val) => {
  localStorage.setItem('arkbar_tray_target', val);
  updateTrayTitle();
  invoke('set_tray_prefs', {
    target: val,
    percentMode: trayPercentMode.value,
  }).catch(() => {});
});

async function checkAutoUpdate() {
  try {
    const res = await invoke<UpdateInfo>('check_for_updates');
    updateInfo.value = res;
  } catch (err) {
    console.debug('Auto update check skipped/failed:', err);
  }
}

async function handleOnboardingComplete() {
  currentView.value = 'settings';
  await fetchProviderUsage('volcengine');
}

let lastVisibleRefresh = 0;
function onPanelBecomeVisible() {
  if (isFloatWindow.value) return;
  const now = Date.now();
  if (now - lastVisibleRefresh < 15000) return;
  lastVisibleRefresh = now;
  // 面板唤醒时静默强制刷新当前活跃厂商，保证最新用量无需手动点击刷新即可呈现
  fetchProviderUsage(activeProvider.value, true, true);
}

// 后台线程每轮刷新后逐厂商广播，这里更新本地状态并落盘
// （localStorage 变更也会通过 storage 事件同步给悬浮窗）。
let unlistenUsage: (() => void) | null = null;

onMounted(() => {
  // The float webview must stay a cheap renderer. All CLI/network work
  // belongs to the main window so tray clicks are not fighting a second
  // copy of arkcli / curl / agy.
  if (isFloatWindow.value) {
    return;
  }

  updateTrayTitle();
  syncBackgroundPrefs();
  // 按持久化偏好应用菜单栏图标显隐（隐藏场景下托盘仍在后台刷新）；
  // 直接读 localStorage，避免维护第二份状态
  invoke('set_tray_icon_visible', {
    visible: trayIconVisiblePref(),
  }).catch(() => {});

  const hasCached = !!providersData.value[activeProvider.value];
  fetchProviderUsage(activeProvider.value, hasCached, !hasCached);

  listen<ProviderUsageData>('usage-updated', (event) => {
    const data = event.payload;
    if (!data || !(data.provider in providersData.value)) return;
    providersData.value[data.provider as ProviderType] = data;
    saveCachedProviders();
    updateTrayTitle();
  }).then((un) => {
    unlistenUsage = un;
  });

  // 后台巡检发现新版本：把信息交给设置窗口（它负责弹更新弹窗）
  listen<UpdateInfo>('update-available', (event) => {
    if (event.payload?.has_update) updateInfo.value = event.payload;
  });

  // 刘海卡片上的「安装 / 授权」按钮：切到引导视图（装 arkcli / SSO 登录都在那一屏）
  listen('open_onboarding', () => {
    currentView.value = 'onboarding';
    checkEnv();
  });

  window.addEventListener('focus', onPanelBecomeVisible);
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') {
      onPanelBecomeVisible();
    }
  });

  // 面板隐藏时不必预热其余厂商（用户看不到、后台线程也在刷新）；
  // 空闲时执行，Safari/WKWebView 无 requestIdleCallback 时回退到 setTimeout
  const runPrefetch = () => {
    if (document.visibilityState === 'visible') {
      prefetchOtherProviders();
    }
  };
  if (typeof (window as any).requestIdleCallback === 'function') {
    (window as any).requestIdleCallback(runPrefetch, { timeout: 5000 });
  } else {
    setTimeout(runPrefetch, 2500);
  }

  setTimeout(() => {
    checkAutoUpdate();
  }, 8000);
});

onUnmounted(() => {
  window.removeEventListener('focus', onPanelBecomeVisible);
  if (unlistenUsage) unlistenUsage();
  if (cacheWriteTimer) {
    clearTimeout(cacheWriteTimer);
    flushCachedProviders();
  }
});
function handleCloseSettings() {
  invoke('hide_window').catch(() => {});
}
</script>

<template>
  <!-- 1. Floating Desktop Widget Mode (Screen Edge Notch) -->
  <FloatingWidget v-if="isFloatWindow" />

  <!-- 2. Settings Window Mode (Codenotch 2-Column Settings) -->
  <main v-else class="w-full h-full rounded-2xl bg-[#18191c] shadow-2xl overflow-hidden flex flex-col font-sans">
    <!-- Onboarding Wizard -->
    <OnboardingWizard
      v-if="currentView === 'onboarding'"
      :status="envStatus || { has_node: false, node_version: null, has_npm: false, npm_version: null, has_arkcli: false, arkcli_version: null, logged_in: false, user_name: null, account_id: null, active_profile: null, error_message: null }"
      @env-updated="envStatus = $event"
      @refresh="checkEnv"
      @complete="handleOnboardingComplete"
      @back="currentView = 'settings'"
    />

    <!-- Settings Window (Codenotch Style) -->
    <SettingsModal
      v-else
      :env-status="envStatus"
      :refresh-interval="refreshInterval"
      :tray-percent-mode="trayPercentMode"
      :initial-update-info="updateInfo"
      :active-provider="activeProvider"
      :provider-tabs="providerTabs"
      @update-interval="refreshInterval = $event"
      @update-tray-percent-mode="trayPercentMode = $event"
      @update-tray-target="trayTarget = $event"
      @update-provider-tabs="saveProviderTabs"
      @re-login="currentView = 'onboarding'"
      @provider-token-updated="handleRefreshCurrent"
      @update-checked="updateInfo = $event"
      @close="handleCloseSettings"
    />
  </main>
</template>
