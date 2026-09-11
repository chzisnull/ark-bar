<script setup lang="ts">
import { computed } from 'vue';
import type { UsagePlanResponse, PlanItem, Period } from '../types';
import { RefreshCw, ExternalLink, Settings, Sparkles, User, Shield, Clock } from 'lucide-vue-next';
import { openUrl } from '@tauri-apps/plugin-opener';

const props = defineProps<{
  planData: UsagePlanResponse | null;
  isRefreshing: boolean;
}>();

const emit = defineEmits<{
  (e: 'refresh'): void;
  (e: 'open-settings'): void;
}>();

const viewer = computed(() => props.planData?.viewer);

const activePlan = computed<PlanItem | null>(() => {
  if (!props.planData?.items || props.planData.items.length === 0) return null;
  // Prioritize coding-plan-team or coding-plan or agent-plan
  const team = props.planData.items.find(i => i.product.includes('team') && i.subscribed);
  if (team) return team;
  const anySub = props.planData.items.find(i => i.subscribed);
  return anySub || props.planData.items[0];
});

const sessionPeriod = computed<Period | undefined>(() => {
  return activePlan.value?.periods?.find(p => p.label.toLowerCase() === 'session');
});

const weeklyPeriod = computed<Period | undefined>(() => {
  return activePlan.value?.periods?.find(p => p.label.toLowerCase() === 'weekly');
});

const monthlyPeriod = computed<Period | undefined>(() => {
  return activePlan.value?.periods?.find(p => p.label.toLowerCase() === 'monthly');
});

function getProgressColor(percent: number): { bar: string; text: string; bg: string } {
  if (percent >= 85) {
    return { bar: 'bg-rose-500', text: 'text-rose-400', bg: 'bg-rose-500/10 border-rose-500/30' };
  } else if (percent >= 60) {
    return { bar: 'bg-amber-500', text: 'text-amber-400', bg: 'bg-amber-500/10 border-amber-500/30' };
  }
  return { bar: 'bg-emerald-500', text: 'text-emerald-400', bg: 'bg-emerald-500/10 border-emerald-500/30' };
}

function formatResetDate(dateStr?: string): string {
  if (!dateStr) return '';
  try {
    const d = new Date(dateStr);
    const now = new Date();
    const diffMs = d.getTime() - now.getTime();
    const diffDays = Math.ceil(diffMs / (1000 * 60 * 60 * 24));

    const month = d.getMonth() + 1;
    const date = d.getDate();
    const hours = d.getHours().toString().padStart(2, '0');
    const mins = d.getMinutes().toString().padStart(2, '0');

    if (diffDays > 0) {
      return `${month}月${date}日 ${hours}:${mins} (剩 ${diffDays} 天)`;
    }
    return `${month}月${date}日 ${hours}:${mins}`;
  } catch {
    return dateStr;
  }
}

async function openConsole() {
  try {
    await openUrl('https://console.volcengine.com/ark/region:ark+cn-beijing/openManagement?advancedActiveKey=enterprise');
  } catch (e) {
    window.open('https://console.volcengine.com/ark/region:ark+cn-beijing/openManagement?advancedActiveKey=enterprise', '_blank');
  }
}
</script>

<template>
  <div class="flex flex-col h-full select-none text-slate-200">
    <!-- Top Bar -->
    <div class="px-4 py-3 bg-slate-900/60 border-b border-slate-800/80 flex items-center justify-between">
      <div class="flex items-center space-x-2.5">
        <div class="w-7 h-7 rounded-lg bg-gradient-to-tr from-rose-500 to-amber-500 flex items-center justify-center text-white shadow-md shadow-rose-500/20 text-sm">
          🌋
        </div>
        <div>
          <div class="text-xs font-bold text-white flex items-center gap-1.5 tracking-wide">
            <span>ArkBar</span>
            <span class="text-[9px] bg-rose-500/20 text-rose-300 px-1.5 py-0.2 rounded font-medium border border-rose-500/30">
              Coding Plan
            </span>
          </div>
          <p class="text-[10px] text-slate-400 font-mono truncate max-w-[170px]" :title="viewer?.user_name">
            {{ viewer?.user_name || '已认证' }} ({{ viewer?.account_id || '' }})
          </p>
        </div>
      </div>

      <div class="flex items-center space-x-1">
        <button @click="$emit('refresh')" :disabled="isRefreshing"
          class="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition" title="刷新配额">
          <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': isRefreshing }" />
        </button>
        <button @click="$emit('open-settings')"
          class="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition" title="偏好设置">
          <Settings class="w-3.5 h-3.5" />
        </button>
      </div>
    </div>

    <!-- Seat Info Badge -->
    <div v-if="activePlan?.seat_id" class="mx-4 mt-3 px-3 py-1.5 rounded-lg bg-slate-800/50 border border-slate-700/50 flex items-center justify-between text-[11px]">
      <div class="flex items-center space-x-1.5 text-slate-300">
        <Shield class="w-3 h-3 text-sky-400" />
        <span>席位 ID</span>
      </div>
      <span class="font-mono text-sky-300 text-[10px]">{{ activePlan.seat_id }}</span>
    </div>

    <!-- Main Quota Cards -->
    <div class="px-4 py-3 flex-1 overflow-y-auto space-y-3">
      <!-- 1. Session Quota -->
      <div v-if="sessionPeriod" class="p-3 rounded-xl border transition-all"
        :class="getProgressColor(sessionPeriod.percent).bg">
        <div class="flex items-center justify-between mb-1.5">
          <div class="flex items-center space-x-1.5">
            <Sparkles class="w-3.5 h-3.5 text-amber-400" />
            <span class="text-xs font-semibold text-white">会话窗口 (Session)</span>
          </div>
          <span class="text-sm font-bold font-mono" :class="getProgressColor(sessionPeriod.percent).text">
            {{ sessionPeriod.percent.toFixed(1) }}%
          </span>
        </div>
        <!-- Progress Bar -->
        <div class="w-full bg-slate-800/80 rounded-full h-2 overflow-hidden">
          <div class="h-full rounded-full transition-all duration-500"
            :class="getProgressColor(sessionPeriod.percent).bar"
            :style="{ width: `${Math.min(100, Math.max(0, sessionPeriod.percent))}%` }"></div>
        </div>
        <p class="text-[10px] text-slate-400 mt-1.5 flex items-center justify-between">
          <span>滚动窗口瞬时限额</span>
          <span>按调用频率动态计算</span>
        </p>
      </div>

      <!-- 2. Weekly Quota -->
      <div v-if="weeklyPeriod" class="p-3 rounded-xl border transition-all"
        :class="getProgressColor(weeklyPeriod.percent).bg">
        <div class="flex items-center justify-between mb-1.5">
          <div class="flex items-center space-x-1.5">
            <Clock class="w-3.5 h-3.5 text-sky-400" />
            <span class="text-xs font-semibold text-white">本周配额 (Weekly)</span>
          </div>
          <span class="text-sm font-bold font-mono" :class="getProgressColor(weeklyPeriod.percent).text">
            {{ weeklyPeriod.percent.toFixed(1) }}%
          </span>
        </div>
        <!-- Progress Bar -->
        <div class="w-full bg-slate-800/80 rounded-full h-2 overflow-hidden">
          <div class="h-full rounded-full transition-all duration-500"
            :class="getProgressColor(weeklyPeriod.percent).bar"
            :style="{ width: `${Math.min(100, Math.max(0, weeklyPeriod.percent))}%` }"></div>
        </div>
        <p class="text-[10px] text-slate-400 mt-1.5 flex items-center justify-between">
          <span>每周周期限额</span>
          <span>每周一 00:00 自动刷新</span>
        </p>
      </div>

      <!-- 3. Monthly Quota -->
      <div v-if="monthlyPeriod" class="p-3 rounded-xl border transition-all"
        :class="getProgressColor(monthlyPeriod.percent).bg">
        <div class="flex items-center justify-between mb-1.5">
          <div class="flex items-center space-x-1.5">
            <User class="w-3.5 h-3.5 text-indigo-400" />
            <span class="text-xs font-semibold text-white">月度配额 (Monthly)</span>
          </div>
          <span class="text-sm font-bold font-mono" :class="getProgressColor(monthlyPeriod.percent).text">
            {{ monthlyPeriod.percent.toFixed(1) }}%
          </span>
        </div>
        <!-- Progress Bar -->
        <div class="w-full bg-slate-800/80 rounded-full h-2 overflow-hidden">
          <div class="h-full rounded-full transition-all duration-500"
            :class="getProgressColor(monthlyPeriod.percent).bar"
            :style="{ width: `${Math.min(100, Math.max(0, monthlyPeriod.percent))}%` }"></div>
        </div>
        <p class="text-[10px] text-slate-400 mt-1.5 flex items-center justify-between">
          <span>重置时间</span>
          <span class="text-slate-300 font-medium font-mono">{{ formatResetDate(monthlyPeriod.reset_at) }}</span>
        </p>
      </div>

      <!-- Fallback if no periods -->
      <div v-if="!sessionPeriod && !weeklyPeriod && !monthlyPeriod" class="p-6 text-center text-slate-500 text-xs">
        暂无配额周期数据，请点击右上角刷新重试
      </div>
    </div>

    <!-- Bottom Actions -->
    <div class="p-3 bg-slate-900/80 border-t border-slate-800/80 flex items-center justify-between text-xs">
      <button @click="openConsole"
        class="text-sky-400 hover:text-sky-300 flex items-center gap-1 py-1 px-2 rounded hover:bg-slate-800 transition">
        <span>控制台用量查看</span>
        <ExternalLink class="w-3 h-3" />
      </button>

      <span class="text-[10px] text-slate-500">
        数据有 5–30 分钟聚合延迟
      </span>
    </div>
  </div>
</template>
