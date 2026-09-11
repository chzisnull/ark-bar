<script setup lang="ts">
import { ref, computed } from 'vue';
import type { UsagePlanResponse, PlanItem, Period, UpdateInfo } from '../types';
import { RefreshCw, ExternalLink, Settings, Clock, User, Shield, AlertTriangle, Sparkles, Download, X } from 'lucide-vue-next';
import { openUrl } from '@tauri-apps/plugin-opener';

const props = defineProps<{
  planData: UsagePlanResponse | null;
  isRefreshing: boolean;
  updateInfo?: UpdateInfo | null;
}>();

const bannerDismissed = ref(false);

const emit = defineEmits<{
  (e: 'refresh'): void;
  (e: 'open-settings'): void;
}>();

const viewer = computed(() => props.planData?.viewer);

const activePlan = computed<PlanItem | null>(() => {
  if (!props.planData?.items || props.planData.items.length === 0) return null;
  const team = props.planData.items.find(i => i.product.includes('team') && i.subscribed);
  if (team) return team;
  const anySub = props.planData.items.find(i => i.subscribed);
  return anySub || props.planData.items[0];
});

// 1. 近5小时用量 (Session)
const sessionPeriod = computed<Period | undefined>(() => {
  return activePlan.value?.periods?.find(p => p.label.toLowerCase() === 'session');
});

// 2. 近一周用量 (Weekly)
const weeklyPeriod = computed<Period | undefined>(() => {
  return activePlan.value?.periods?.find(p => p.label.toLowerCase() === 'weekly');
});

// 3. 近一月用量 (Monthly)
const monthlyPeriod = computed<Period | undefined>(() => {
  return activePlan.value?.periods?.find(p => p.label.toLowerCase() === 'monthly');
});

function getBarColor(percent: number): string {
  if (percent >= 90) return 'from-rose-500 to-red-600';
  if (percent >= 75) return 'from-amber-500 to-orange-500';
  return 'from-indigo-500 to-blue-500';
}

function getTextColor(percent: number): string {
  if (percent >= 90) return 'text-rose-400';
  if (percent >= 75) return 'text-amber-400';
  return 'text-white';
}

function formatDetailedReset(dateStr?: string): string {
  if (!dateStr) return '';
  try {
    const target = new Date(dateStr).getTime();
    const now = Date.now();
    const diff = target - now;
    if (diff <= 0) return '即将重置';

    const days = Math.floor(diff / (1000 * 60 * 60 * 24));
    const hours = Math.floor((diff % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
    const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));

    if (days > 0) {
      return `${days}天${hours}小时${minutes}分后重置`;
    } else if (hours > 0) {
      return `${hours}小时${minutes}分后重置`;
    } else {
      return `${minutes}分后重置`;
    }
  } catch {
    return dateStr;
  }
}

async function openConsole() {
  const targetUrl = 'https://console.volcengine.com/ark/region:cn-beijing/subscription/coding-plan-enterprise';
  try {
    await openUrl(targetUrl);
  } catch {
    window.open(targetUrl, '_blank');
  }
}

async function openReleaseUrl() {
  if (props.updateInfo?.release_url) {
    try {
      await openUrl(props.updateInfo.release_url);
    } catch {
      window.open(props.updateInfo.release_url, '_blank');
    }
  }
}
</script>

<template>
  <div class="flex flex-col h-full select-none text-slate-200 bg-[#0e131f]/95">
    <!-- Top Bar -->
    <div class="px-4 py-3 bg-[#141b2d]/80 border-b border-slate-800/80 flex items-center justify-between">
      <div class="flex items-center space-x-2.5">
        <div class="w-7 h-7 rounded-lg bg-gradient-to-tr from-indigo-500 to-violet-500 flex items-center justify-center text-white shadow-md shadow-indigo-500/20 text-sm font-black">
          🌋
        </div>
        <div>
          <div class="flex items-center gap-1.5">
            <span class="text-xs font-bold text-white tracking-wide">ArkBar</span>
            <span class="text-[10px] bg-indigo-500/15 text-indigo-300 px-1.5 py-0.2 rounded font-medium border border-indigo-500/30">
              💎 Lite 套餐
            </span>
            <span class="text-[10px] bg-emerald-500/15 text-emerald-400 px-1.5 py-0.2 rounded font-medium flex items-center gap-1 border border-emerald-500/30">
              <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
              生效中
            </span>
          </div>
        </div>
      </div>

      <div class="flex items-center space-x-1">
        <button @click="$emit('refresh')" :disabled="isRefreshing"
          class="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800/80 transition" title="刷新数据">
          <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': isRefreshing }" />
        </button>
        <button @click="$emit('open-settings')"
          class="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800/80 transition" title="设置">
          <Settings class="w-3.5 h-3.5" />
        </button>
      </div>
    </div>

    <!-- Automatic Update Notification Banner -->
    <div v-if="updateInfo?.has_update && !bannerDismissed"
      class="mx-3 mt-2 px-3 py-2 bg-gradient-to-r from-amber-500/15 via-orange-500/15 to-indigo-500/15 border border-amber-500/30 rounded-xl flex items-center justify-between text-xs shadow-sm">
      <div class="flex items-center space-x-2 truncate">
        <Sparkles class="w-3.5 h-3.5 text-amber-400 flex-shrink-0 animate-pulse" />
        <div class="truncate">
          <span class="font-medium text-amber-300 text-[11px]">发现新版本 v{{ updateInfo.latest_version }}</span>
        </div>
      </div>
      <div class="flex items-center space-x-1.5 flex-shrink-0 ml-2">
        <button @click="openReleaseUrl"
          class="px-2 py-0.5 bg-gradient-to-r from-amber-500 to-orange-500 hover:from-amber-400 hover:to-orange-400 text-slate-950 font-bold rounded-md text-[10px] flex items-center gap-1 transition shadow-sm">
          <Download class="w-2.5 h-2.5" />
          <span>立即更新</span>
        </button>
        <button @click="bannerDismissed = true"
          class="p-0.5 rounded text-slate-500 hover:text-slate-300 transition" title="忽略">
          <X class="w-3 h-3" />
        </button>
      </div>
    </div>

    <!-- Seat & User Meta Strip -->
    <div class="px-4 py-2 bg-[#121826]/60 border-b border-slate-800/50 flex items-center justify-between text-[11px] text-slate-400">
      <div class="flex items-center space-x-1.5 truncate">
        <User class="w-3 h-3 text-slate-500 flex-shrink-0" />
        <span class="text-slate-300 truncate max-w-[140px]">{{ viewer?.user_name || '已认证' }}</span>
        <span class="text-slate-600">·</span>
        <span class="font-mono text-slate-400 text-[10px]">{{ viewer?.account_id }}</span>
      </div>
      <div v-if="activePlan?.seat_id" class="flex items-center space-x-1 font-mono text-[10px] text-sky-400 bg-sky-950/40 px-1.5 py-0.5 rounded border border-sky-800/40" :title="activePlan.seat_id">
        <Shield class="w-2.5 h-2.5" />
        <span>{{ activePlan.seat_id.replace('seat-', '') }}</span>
      </div>
    </div>

    <!-- Main Content Area -->
    <div class="px-4 py-3 flex-1 overflow-y-auto space-y-4">
      <!-- Section Title & Notice -->
      <div class="flex items-center justify-between text-[11px]">
        <span class="font-semibold text-white tracking-wide">用量统计</span>
        <span class="text-[10px] text-slate-500 flex items-center gap-1">
          <AlertTriangle class="w-3 h-3 text-amber-500/70" />
          达 90% 请关注重置时间
        </span>
      </div>

      <!-- Metric 1: 近5小时用量 -->
      <div v-if="sessionPeriod" class="p-3.5 rounded-xl bg-[#131a2a]/70 border border-slate-800/70 transition-all">
        <div class="flex items-baseline justify-between">
          <span class="text-xs font-medium text-slate-300">近5小时用量</span>
          <div class="flex items-baseline">
            <span class="text-2xl font-bold font-mono tracking-tight" :class="getTextColor(sessionPeriod.percent)">
              {{ sessionPeriod.percent.toFixed(2) }}
            </span>
            <span class="text-xs font-medium text-slate-400 ml-1">%</span>
          </div>
        </div>

        <!-- Custom Scale Progress Bar -->
        <div class="relative w-full mt-2.5">
          <div class="h-2 w-full bg-slate-800/90 rounded-full overflow-hidden relative">
            <div class="h-full rounded-full bg-gradient-to-r transition-all duration-500"
              :class="getBarColor(sessionPeriod.percent)"
              :style="{ width: `${Math.min(100, Math.max(0, sessionPeriod.percent))}%` }">
            </div>
          </div>
          <!-- 90% Marker -->
          <div class="absolute -top-0.5 bottom-3.5 left-[90%] w-[1.5px] bg-amber-400/80 pointer-events-none"></div>

          <!-- Scale Axis Labels -->
          <div class="flex justify-between text-[9px] text-slate-500 mt-1 font-mono">
            <span>0%</span>
            <span class="relative -ml-2 text-slate-500">50%</span>
            <span class="relative -mr-1 text-amber-400/80 font-medium">90%</span>
            <span>100%</span>
          </div>
        </div>
      </div>

      <!-- Metric 2: 近一周用量 -->
      <div v-if="weeklyPeriod" class="p-3.5 rounded-xl bg-[#131a2a]/70 border border-slate-800/70 transition-all">
        <div class="flex items-baseline justify-between">
          <span class="text-xs font-medium text-slate-300">近一周用量</span>
          <div class="flex items-baseline">
            <span class="text-2xl font-bold font-mono tracking-tight" :class="getTextColor(weeklyPeriod.percent)">
              {{ weeklyPeriod.percent.toFixed(2) }}
            </span>
            <span class="text-xs font-medium text-slate-400 ml-1">%</span>
          </div>
        </div>

        <!-- Custom Scale Progress Bar -->
        <div class="relative w-full mt-2.5">
          <div class="h-2 w-full bg-slate-800/90 rounded-full overflow-hidden relative">
            <div class="h-full rounded-full bg-gradient-to-r transition-all duration-500"
              :class="getBarColor(weeklyPeriod.percent)"
              :style="{ width: `${Math.min(100, Math.max(0, weeklyPeriod.percent))}%` }">
            </div>
          </div>
          <!-- 90% Marker -->
          <div class="absolute -top-0.5 bottom-3.5 left-[90%] w-[1.5px] bg-amber-400/80 pointer-events-none"></div>

          <!-- Scale Axis Labels -->
          <div class="flex justify-between text-[9px] text-slate-500 mt-1 font-mono">
            <span>0%</span>
            <span class="relative -ml-2 text-slate-500">50%</span>
            <span class="relative -mr-1 text-amber-400/80 font-medium">90%</span>
            <span>100%</span>
          </div>
        </div>
      </div>

      <!-- Metric 3: 近一月用量 -->
      <div v-if="monthlyPeriod" class="p-3.5 rounded-xl bg-[#131a2a]/70 border border-slate-800/70 transition-all">
        <div class="flex items-center justify-between mb-1">
          <span class="text-xs font-medium text-slate-300">近一月用量</span>
          <div v-if="monthlyPeriod.reset_at" class="flex items-center gap-1 text-[10px] text-indigo-400 font-medium">
            <Clock class="w-3 h-3" />
            <span>{{ formatDetailedReset(monthlyPeriod.reset_at) }}</span>
          </div>
        </div>

        <div class="flex items-baseline justify-end">
          <span class="text-2xl font-bold font-mono tracking-tight" :class="getTextColor(monthlyPeriod.percent)">
            {{ monthlyPeriod.percent.toFixed(2) }}
          </span>
          <span class="text-xs font-medium text-slate-400 ml-1">%</span>
        </div>

        <!-- Custom Scale Progress Bar -->
        <div class="relative w-full mt-2.5">
          <div class="h-2 w-full bg-slate-800/90 rounded-full overflow-hidden relative">
            <div class="h-full rounded-full bg-gradient-to-r transition-all duration-500"
              :class="getBarColor(monthlyPeriod.percent)"
              :style="{ width: `${Math.min(100, Math.max(0, monthlyPeriod.percent))}%` }">
            </div>
          </div>
          <!-- 90% Marker -->
          <div class="absolute -top-0.5 bottom-3.5 left-[90%] w-[1.5px] bg-amber-400/80 pointer-events-none"></div>

          <!-- Scale Axis Labels -->
          <div class="flex justify-between text-[9px] text-slate-500 mt-1 font-mono">
            <span>0%</span>
            <span class="relative -ml-2 text-slate-500">50%</span>
            <span class="relative -mr-1 text-amber-400/80 font-medium">90%</span>
            <span>100%</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Bottom Actions -->
    <div class="px-4 py-2.5 bg-[#121826] border-t border-slate-800/80 flex items-center justify-between text-xs">
      <button @click="openConsole"
        class="text-indigo-400 hover:text-indigo-300 font-medium flex items-center gap-1 py-1 px-2 -ml-2 rounded-lg hover:bg-slate-800/80 transition">
        <span>控制台用量查看</span>
        <ExternalLink class="w-3 h-3" />
      </button>

      <span class="text-[10px] text-slate-500 font-mono">
        5–30 min 延迟
      </span>
    </div>
  </div>
</template>
