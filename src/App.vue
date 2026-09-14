<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, defineAsyncComponent } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { EnvironmentStatus, UpdateInfo, ProviderType, ProviderUsageData } from './types';
import UsagePanel from './components/UsagePanel.vue';

// Lazy load non-critical components to minimize initial bundle parsing
const OnboardingWizard = defineAsyncComponent(() => import('./components/OnboardingWizard.vue'));
const SettingsModal = defineAsyncComponent(() => import('./components/SettingsModal.vue'));
const FloatingWidget = defineAsyncComponent(() => import('./components/FloatingWidget.vue'));

const isFloatWindow = ref(window.location.hash === '#float');
const currentView = ref<'onboarding' | 'panel' | 'settings'>('panel');
const envStatus = ref<EnvironmentStatus | null>(null);
const updateInfo = ref<UpdateInfo | null>(null);
const isRefreshing = ref(false);
const loadingMap = ref<Record<ProviderType, boolean>>({
  volcengine: false,
  antigravity: false,
  grok: false,
  codex: false,
});

const activeProvider = ref<ProviderType>(
  (localStorage.getItem('arkbar_active_provider') as ProviderType) || 'volcengine'
);

const trayTarget = ref<ProviderType | 'auto'>(
  (localStorage.getItem('arkbar_tray_target') as ProviderType | 'auto') || 'volcengine'
);

const showPercentageInTray = ref<boolean>(
  localStorage.getItem('arkbar_tray_percent') !== 'false'
);

const refreshInterval = ref<number>(
  Number(localStorage.getItem('arkbar_refresh_interval')) || 5
);

// 0ms startup: hydrate from local cache immediately
const CACHE_KEY = 'arkbar_cached_providers_data';

function loadCachedProviders(): Record<ProviderType, ProviderUsageData | null> {
  const defaults: Record<ProviderType, ProviderUsageData | null> = {
    volcengine: null,
    antigravity: null,
    grok: null,
    codex: null,
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

let timer: any = null;

function saveCachedProviders() {
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(providersData.value));
  } catch {}
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
  // Never block the tab switch. Cached rows render immediately; network
  // refresh always runs silently in the background.
  fetchProviderUsage(provider, true);
}

function prefetchOtherProviders() {
  const allProviders: ProviderType[] = ['volcengine', 'antigravity', 'grok', 'codex'];
  void Promise.all(
    allProviders
      .filter((p) => p !== activeProvider.value)
      .map((p) => fetchProviderUsage(p, true))
  );
}

function handleGoAuth(provider: ProviderType) {
  if (provider === 'volcengine') {
    checkEnv();
    currentView.value = 'onboarding';
  } else {
    currentView.value = 'settings';
  }
}

async function handleRefreshCurrent() {
  await fetchProviderUsage(activeProvider.value, false, true);
}

function updateTrayTitle() {
  if (!showPercentageInTray.value) {
    invoke('update_tray_title', { title: '' });
    return;
  }

  // 固定展示火山方舟 5 小时用量，不随界面 Tab 切换而变动
  const target: ProviderType = trayTarget.value === 'auto' ? 'volcengine' : trayTarget.value;
  const data = providersData.value[target] || providersData.value['volcengine'];

  if (data?.primary_session_percent != null && data.is_connected) {
    const p = Math.round(data.primary_session_percent);
    invoke('update_tray_title', { title: ` ${p}%` });
  } else {
    invoke('update_tray_title', { title: '' });
  }
}

function setupTimer() {
  if (timer) clearInterval(timer);
  if (refreshInterval.value > 0) {
    timer = setInterval(() => {
      if (currentView.value === 'panel') {
        fetchProviderUsage(activeProvider.value, true, true);
      }
    }, refreshInterval.value * 60 * 1000);
  }
}

watch(refreshInterval, (val) => {
  localStorage.setItem('arkbar_refresh_interval', String(val));
  setupTimer();
});

watch(showPercentageInTray, (val) => {
  localStorage.setItem('arkbar_tray_percent', String(val));
  updateTrayTitle();
});

watch(trayTarget, (val) => {
  localStorage.setItem('arkbar_tray_target', val);
  updateTrayTitle();
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
  currentView.value = 'panel';
  await fetchProviderUsage('volcengine');
}

let lastVisibleRefresh = 0;
function onPanelBecomeVisible() {
  if (isFloatWindow.value) return;
  const now = Date.now();
  if (now - lastVisibleRefresh < 20000) return;
  lastVisibleRefresh = now;
  fetchProviderUsage(activeProvider.value, true, false);
}

onMounted(() => {
  // The float webview must stay a cheap renderer. All CLI/network work
  // belongs to the main window so tray clicks are not fighting a second
  // copy of arkcli / curl / agy.
  if (isFloatWindow.value) {
    return;
  }

  updateTrayTitle();

  const hasCached = !!providersData.value[activeProvider.value];
  fetchProviderUsage(activeProvider.value, hasCached, !hasCached);

  setupTimer();

  window.addEventListener('focus', onPanelBecomeVisible);
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') {
      onPanelBecomeVisible();
    }
  });

  setTimeout(() => {
    prefetchOtherProviders();
  }, 2500);

  setTimeout(() => {
    checkAutoUpdate();
  }, 8000);
});

onUnmounted(() => {
  window.removeEventListener('focus', onPanelBecomeVisible);
  if (timer) clearInterval(timer);
});
</script>

<template>
  <!-- 1. Floating Desktop Widget Mode -->
  <FloatingWidget v-if="isFloatWindow" />

  <!-- 2. Main Menu Bar Popover Mode -->
  <main v-else class="w-full h-full rounded-2xl bg-[#0e131f] border border-slate-700/60 shadow-2xl overflow-hidden flex flex-col font-sans">
    <!-- 1. Onboarding Wizard -->
    <OnboardingWizard
      v-if="currentView === 'onboarding'"
      :status="envStatus || { has_node: false, node_version: null, has_npm: false, npm_version: null, has_arkcli: false, arkcli_version: null, logged_in: false, user_name: null, account_id: null, active_profile: null, error_message: null }"
      @env-updated="envStatus = $event"
      @refresh="checkEnv"
      @complete="handleOnboardingComplete"
      @back="currentView = 'panel'"
    />

    <!-- 2. Quota Usage Panel -->
    <UsagePanel
      v-else-if="currentView === 'panel'"
      :providers-data="providersData"
      :active-provider="activeProvider"
      :is-refreshing="isRefreshing"
      :loading-map="loadingMap"
      :update-info="updateInfo"
      @switch-provider="handleSwitchProvider"
      @refresh="handleRefreshCurrent"
      @go-auth="handleGoAuth"
      @open-settings="currentView = 'settings'"
    />

    <!-- 3. Settings View -->
    <SettingsModal
      v-else-if="currentView === 'settings'"
      :env-status="envStatus"
      :refresh-interval="refreshInterval"
      :show-percentage-in-tray="showPercentageInTray"
      :initial-update-info="updateInfo"
      :active-provider="activeProvider"
      @update-interval="refreshInterval = $event"
      @update-tray-mode="showPercentageInTray = $event"
      @update-tray-target="trayTarget = $event"
      @re-login="currentView = 'onboarding'"
      @provider-token-updated="handleRefreshCurrent"
      @close="currentView = 'panel'"
    />
  </main>
</template>
