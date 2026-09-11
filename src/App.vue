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

const activeProvider = ref<ProviderType>(
  (localStorage.getItem('arkbar_active_provider') as ProviderType) || 'volcengine'
);

const trayTarget = ref<ProviderType | 'auto'>(
  (localStorage.getItem('arkbar_tray_target') as ProviderType | 'auto') || 'auto'
);

const showPercentageInTray = ref<boolean>(
  localStorage.getItem('arkbar_tray_percent') !== 'false'
);

const refreshInterval = ref<number>(
  Number(localStorage.getItem('arkbar_refresh_interval')) || 15
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
async function fetchProviderUsage(provider: ProviderType, silent = false) {
  if (inFlight.has(provider)) return;
  inFlight.add(provider);

  if (!silent) {
    isRefreshing.value = true;
  }
  try {
    const data = await invoke<ProviderUsageData>('get_unified_usage', {
      provider,
      customToken: null,
    });
    providersData.value[provider] = data;
    saveCachedProviders();
    updateTrayTitle();
  } catch (err) {
    console.error(`Failed to fetch ${provider} usage:`, err);
  } finally {
    inFlight.delete(provider);
    if (!silent) {
      isRefreshing.value = false;
    }
  }
}

function handleSwitchProvider(provider: ProviderType) {
  activeProvider.value = provider;
  localStorage.setItem('arkbar_active_provider', provider);
  updateTrayTitle();
  // 0ms instant switch: if data already exists in memory/cache, render immediately without blocking UI
  const hasData = !!providersData.value[provider];
  fetchProviderUsage(provider, hasData);
}

// Background prefetch remaining providers to guarantee 0ms instant tab switching
async function prefetchOtherProviders() {
  const allProviders: ProviderType[] = ['volcengine', 'antigravity', 'grok', 'codex'];
  for (const p of allProviders) {
    if (p !== activeProvider.value) {
      await fetchProviderUsage(p, true);
    }
  }
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
  await fetchProviderUsage(activeProvider.value, false);
}

function updateTrayTitle() {
  if (!showPercentageInTray.value) {
    invoke('update_tray_title', { title: '' });
    return;
  }

  const target = trayTarget.value === 'auto' ? activeProvider.value : trayTarget.value;
  const data = providersData.value[target];

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
        fetchProviderUsage(activeProvider.value);
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

function handleWindowBlur() {
  if (!isFloatWindow.value) {
    invoke('hide_window').catch(() => {});
  }
}

onMounted(() => {
  if (!isFloatWindow.value) {
    window.addEventListener('blur', handleWindowBlur);
  }

  // 1. Instantly update tray title from cached data (0ms)
  updateTrayTitle();

  // 2. Fetch active provider (silent if we already loaded from local cache)
  const hasCached = !!providersData.value[activeProvider.value];
  fetchProviderUsage(activeProvider.value, hasCached);

  // 3. Setup timer
  setupTimer();

  // 4. Background prefetch remaining providers after 1.2s to guarantee 0ms instant tab switching
  setTimeout(() => {
    prefetchOtherProviders();
  }, 1200);

  // 5. Deferred non-critical tasks: check auto-updates after 4s idle to avoid startup CPU/network contention
  setTimeout(() => {
    checkAutoUpdate();
  }, 4000);
});

onUnmounted(() => {
  if (!isFloatWindow.value) {
    window.removeEventListener('blur', handleWindowBlur);
  }
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
