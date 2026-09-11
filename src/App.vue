<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { EnvironmentStatus, UpdateInfo, ProviderType, ProviderUsageData } from './types';
import OnboardingWizard from './components/OnboardingWizard.vue';
import UsagePanel from './components/UsagePanel.vue';
import SettingsModal from './components/SettingsModal.vue';
import FloatingWidget from './components/FloatingWidget.vue';
import { Loader2 } from 'lucide-vue-next';

const isFloatWindow = ref(window.location.hash === '#float');
const currentView = ref<'loading' | 'onboarding' | 'panel' | 'settings'>('loading');
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

const providersData = ref<Record<ProviderType, ProviderUsageData | null>>({
  volcengine: null,
  antigravity: null,
  grok: null,
  codex: null,
});

let timer: any = null;

async function checkEnv() {
  try {
    const status = await invoke<EnvironmentStatus>('check_environment');
    envStatus.value = status;
  } catch (err) {
    console.debug('Environment check failed:', err);
  }
}

async function fetchAllUsage() {
  isRefreshing.value = true;
  try {
    const list = await invoke<ProviderUsageData[]>('get_all_providers_usage');
    for (const item of list) {
      if (item.provider in providersData.value) {
        providersData.value[item.provider as ProviderType] = item;
      }
    }
    updateTrayTitle();
  } catch (err) {
    console.error('Failed to fetch all providers usage:', err);
  } finally {
    isRefreshing.value = false;
  }
}

async function fetchActiveProviderUsage() {
  isRefreshing.value = true;
  try {
    const data = await invoke<ProviderUsageData>('get_unified_usage', {
      provider: activeProvider.value,
      customToken: null,
    });
    providersData.value[activeProvider.value] = data;
    updateTrayTitle();
  } catch (err) {
    console.error(`Failed to fetch ${activeProvider.value} usage:`, err);
  } finally {
    isRefreshing.value = false;
  }
}

function handleSwitchProvider(provider: ProviderType) {
  activeProvider.value = provider;
  localStorage.setItem('arkbar_active_provider', provider);
  updateTrayTitle();
  // If no data cached yet, fetch it
  if (!providersData.value[provider]) {
    fetchActiveProviderUsage();
  }
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
        fetchAllUsage();
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

onMounted(async () => {
  currentView.value = 'loading';
  await Promise.allSettled([checkEnv(), fetchAllUsage()]);
  currentView.value = 'panel';
  setupTimer();
  checkAutoUpdate();
});

onUnmounted(() => {
  if (timer) clearInterval(timer);
});
</script>

<template>
  <!-- 1. Floating Desktop Widget Mode -->
  <FloatingWidget v-if="isFloatWindow" />

  <!-- 2. Main Menu Bar Popover Mode -->
  <main v-else class="w-full h-full rounded-2xl bg-[#0e131f] border border-slate-700/60 shadow-2xl overflow-hidden flex flex-col font-sans">
    <!-- 1. Loading View -->
    <div v-if="currentView === 'loading'" class="flex-1 flex flex-col items-center justify-center space-y-3">
      <Loader2 class="w-8 h-8 text-indigo-500 animate-spin" />
      <span class="text-xs text-slate-400 font-medium">正在检测各大平台配额...</span>
    </div>

    <!-- 2. Onboarding Wizard (Only when user explicitly asks or Volcano setup needed) -->
    <OnboardingWizard
      v-else-if="currentView === 'onboarding'"
      :status="envStatus!"
      @refresh="fetchAllUsage"
      @complete="() => { checkEnv(); fetchAllUsage(); currentView = 'panel'; }"
    />

    <!-- 3. Quota Usage Panel -->
    <UsagePanel
      v-else-if="currentView === 'panel'"
      :providers-data="providersData"
      :active-provider="activeProvider"
      :is-refreshing="isRefreshing"
      :update-info="updateInfo"
      @switch-provider="handleSwitchProvider"
      @refresh="fetchAllUsage"
      @open-settings="currentView = 'settings'"
    />

    <!-- 4. Settings View -->
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
      @provider-token-updated="fetchAllUsage"
      @close="currentView = 'panel'"
    />
  </main>
</template>
