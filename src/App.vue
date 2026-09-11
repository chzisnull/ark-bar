<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { EnvironmentStatus, UsagePlanResponse } from './types';
import OnboardingWizard from './components/OnboardingWizard.vue';
import UsagePanel from './components/UsagePanel.vue';
import SettingsModal from './components/SettingsModal.vue';
import { Loader2 } from 'lucide-vue-next';

const currentView = ref<'loading' | 'onboarding' | 'panel' | 'settings'>('loading');
const envStatus = ref<EnvironmentStatus | null>(null);
const planData = ref<UsagePlanResponse | null>(null);
const isRefreshing = ref(false);
const refreshInterval = ref(15); // in minutes
const showPercentageInTray = ref(true);

let timer: any = null;

async function checkEnv() {
  currentView.value = 'loading';
  try {
    const status = await invoke<EnvironmentStatus>('check_environment');
    envStatus.value = status;

    if (!status.has_node || !status.has_arkcli || !status.logged_in) {
      currentView.value = 'onboarding';
      await invoke('update_tray_title', { title: 'ArkBar ⚠️' });
    } else {
      await fetchUsagePlan();
      currentView.value = 'panel';
    }
  } catch (err) {
    console.error('Environment check failed:', err);
    currentView.value = 'onboarding';
  }
}

async function fetchUsagePlan() {
  isRefreshing.value = true;
  try {
    const data = await invoke<UsagePlanResponse>('get_usage_plan');
    planData.value = data;
    updateTrayTitleFromData(data);
  } catch (err) {
    console.error('Failed to fetch usage plan:', err);
  } finally {
    isRefreshing.value = false;
  }
}

function updateTrayTitleFromData(data: UsagePlanResponse) {
  if (!showPercentageInTray.value) {
    invoke('update_tray_title', { title: '' });
    return;
  }

  // Find weekly percentage
  const item = data.items?.find(i => i.subscribed) || data.items?.[0];
  const weekly = item?.periods?.find(p => p.label.toLowerCase() === 'weekly');
  if (weekly && typeof weekly.percent === 'number') {
    const p = Math.round(weekly.percent);
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
        fetchUsagePlan();
      }
    }, refreshInterval.value * 60 * 1000);
  }
}

watch(refreshInterval, () => {
  setupTimer();
});

watch(showPercentageInTray, () => {
  if (planData.value) {
    updateTrayTitleFromData(planData.value);
  }
});

onMounted(async () => {
  await checkEnv();
  setupTimer();
});

onUnmounted(() => {
  if (timer) clearInterval(timer);
});
</script>

<template>
  <main class="w-[380px] h-[550px] rounded-2xl bg-[#0e131f]/95 backdrop-blur-2xl border border-slate-700/60 shadow-2xl overflow-hidden flex flex-col font-sans">
    <!-- 1. Loading View -->
    <div v-if="currentView === 'loading'" class="flex-1 flex flex-col items-center justify-center space-y-3">
      <Loader2 class="w-8 h-8 text-rose-500 animate-spin" />
      <span class="text-xs text-slate-400 font-medium">正在检测环境与配额...</span>
    </div>

    <!-- 2. Onboarding Wizard -->
    <OnboardingWizard
      v-else-if="currentView === 'onboarding'"
      :status="envStatus!"
      @refresh="checkEnv"
      @complete="checkEnv"
    />

    <!-- 3. Quota Usage Panel -->
    <UsagePanel
      v-else-if="currentView === 'panel'"
      :plan-data="planData"
      :is-refreshing="isRefreshing"
      @refresh="fetchUsagePlan"
      @open-settings="currentView = 'settings'"
    />

    <!-- 4. Settings View -->
    <SettingsModal
      v-else-if="currentView === 'settings'"
      :env-status="envStatus"
      :refresh-interval="refreshInterval"
      :show-percentage-in-tray="showPercentageInTray"
      @close="currentView = 'panel'"
      @update-interval="refreshInterval = $event"
      @update-tray-mode="showPercentageInTray = $event"
      @re-login="currentView = 'onboarding'"
    />
  </main>
</template>
