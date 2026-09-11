<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { UsagePlanResponse, PlanItem, Period } from '../types';
import { X, RefreshCw, GripHorizontal } from 'lucide-vue-next';

const planData = ref<UsagePlanResponse | null>(null);
const isRefreshing = ref(false);
let timer: any = null;

async function fetchPlan() {
  isRefreshing.value = true;
  try {
    const data = await invoke<UsagePlanResponse>('get_usage_plan');
    planData.value = data;
  } catch (err) {
    console.error('Floating widget failed to fetch plan:', err);
  } finally {
    isRefreshing.value = false;
  }
}

async function closeWidget() {
  await invoke('close_float_window');
}

let isDragging = false;
let lastScreenX = 0;
let lastScreenY = 0;
let accumDx = 0;
let accumDy = 0;
let rafId: number | null = null;

function onPointerDown(e: PointerEvent) {
  if (e.button !== 0) return;
  const target = e.target as HTMLElement;
  if (target.closest('button') || target.closest('a')) return;

  isDragging = true;
  lastScreenX = e.screenX;
  lastScreenY = e.screenY;
  accumDx = 0;
  accumDy = 0;

  try {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  } catch {}

  // Attempt native OS drag if available
  invoke('start_drag').catch(() => {});
}

function onPointerMove(e: PointerEvent) {
  if (!isDragging) return;

  const dx = e.screenX - lastScreenX;
  const dy = e.screenY - lastScreenY;
  lastScreenX = e.screenX;
  lastScreenY = e.screenY;

  accumDx += dx;
  accumDy += dy;

  if (!rafId) {
    rafId = requestAnimationFrame(async () => {
      rafId = null;
      if (!isDragging && accumDx === 0 && accumDy === 0) return;
      const moveX = accumDx;
      const moveY = accumDy;
      accumDx = 0;
      accumDy = 0;
      if (moveX !== 0 || moveY !== 0) {
        try {
          await invoke('drag_move_window', { dx: moveX, dy: moveY });
        } catch {}
      }
    });
  }
}

function onPointerUp(e: PointerEvent) {
  if (isDragging) {
    isDragging = false;
    if (rafId) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
    if (accumDx !== 0 || accumDy !== 0) {
      const moveX = accumDx;
      const moveY = accumDy;
      accumDx = 0;
      accumDy = 0;
      invoke('drag_move_window', { dx: moveX, dy: moveY }).catch(() => {});
    }
    try {
      (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
    } catch {}
  }
}

const activePlan = computed<PlanItem | null>(() => {
  if (!planData.value?.items || planData.value.items.length === 0) return null;
  const anySub = planData.value.items.find(i => i.subscribed);
  return anySub || planData.value.items[0];
});

const editionTag = computed(() => {
  if (!activePlan.value) return '套餐';
  const ed = (activePlan.value.edition || '').toLowerCase();
  const prod = (activePlan.value.product || '').toLowerCase();
  if (ed === 'pro' || prod.includes('pro')) return 'Pro';
  if (ed === 'team' || prod.includes('team')) return 'Team';
  if (ed === 'lite' || prod.includes('lite')) return 'Lite';
  if (ed === 'enterprise' || prod.includes('enterprise')) return '企业';
  if (activePlan.value.edition) {
    return activePlan.value.edition.toUpperCase();
  }
  return 'VIP';
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

function getBarColor(percent: number): string {
  if (percent >= 90) return 'from-rose-500 to-red-500';
  if (percent >= 75) return 'from-amber-500 to-orange-500';
  return 'from-indigo-500 to-blue-500';
}

function getTextColor(percent: number): string {
  if (percent >= 90) return 'text-rose-400 font-bold';
  if (percent >= 75) return 'text-amber-400 font-bold';
  return 'text-slate-200';
}

onMounted(() => {
  fetchPlan();
  timer = setInterval(fetchPlan, 15 * 60 * 1000);
});

onUnmounted(() => {
  if (timer) clearInterval(timer);
});
</script>

<template>
  <div
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointercancel="onPointerUp"
    class="w-[240px] h-[136px] bg-[#0e131f] border border-slate-700/60 rounded-2xl p-2.5 flex flex-col justify-between select-none shadow-2xl overflow-hidden font-sans cursor-grab active:cursor-grabbing">
    <!-- Header -->
    <div class="flex items-center justify-between pb-1 border-b border-slate-800/60">
      <div class="flex items-center space-x-1.5 pointer-events-none">
        <GripHorizontal class="w-3.5 h-3.5 text-slate-500" />
        <span class="text-[11px] font-bold text-white tracking-wide">ArkBar</span>
        <span class="text-[9px] bg-indigo-500/20 text-indigo-300 px-1 py-0.2 rounded font-mono">
          {{ editionTag }}
        </span>
      </div>

      <div class="flex items-center space-x-0.5">
        <button @click.stop="fetchPlan" :disabled="isRefreshing"
          class="p-1 rounded text-slate-400 hover:text-white hover:bg-slate-800 transition cursor-pointer" title="刷新">
          <RefreshCw class="w-3 h-3" :class="{ 'animate-spin': isRefreshing }" />
        </button>
        <button @click.stop="closeWidget"
          class="p-1 rounded text-slate-400 hover:text-rose-400 hover:bg-slate-800 transition cursor-pointer" title="关闭悬浮框">
          <X class="w-3 h-3" />
        </button>
      </div>
    </div>

    <!-- 3 Mini Rows -->
    <div class="flex-1 flex flex-col justify-around py-1 pointer-events-none">
      <!-- Row 1: 5小时 -->
      <div class="space-y-0.5">
        <div class="flex items-center justify-between text-[10px]">
          <span class="text-slate-400">5小时</span>
          <span class="font-mono text-[11px]" :class="getTextColor(sessionPeriod?.percent || 0)">
            {{ sessionPeriod ? sessionPeriod.percent.toFixed(1) : '-' }}%
          </span>
        </div>
        <div class="h-1.5 w-full bg-slate-800/80 rounded-full overflow-hidden">
          <div class="h-full rounded-full bg-gradient-to-r transition-all duration-300"
            :class="getBarColor(sessionPeriod?.percent || 0)"
            :style="{ width: `${Math.min(100, Math.max(0, sessionPeriod?.percent || 0))}%` }">
          </div>
        </div>
      </div>

      <!-- Row 2: 周用量 -->
      <div class="space-y-0.5">
        <div class="flex items-center justify-between text-[10px]">
          <span class="text-slate-400">本周</span>
          <span class="font-mono text-[11px]" :class="getTextColor(weeklyPeriod?.percent || 0)">
            {{ weeklyPeriod ? weeklyPeriod.percent.toFixed(1) : '-' }}%
          </span>
        </div>
        <div class="h-1.5 w-full bg-slate-800/80 rounded-full overflow-hidden">
          <div class="h-full rounded-full bg-gradient-to-r transition-all duration-300"
            :class="getBarColor(weeklyPeriod?.percent || 0)"
            :style="{ width: `${Math.min(100, Math.max(0, weeklyPeriod?.percent || 0))}%` }">
          </div>
        </div>
      </div>

      <!-- Row 3: 月用量 -->
      <div class="space-y-0.5">
        <div class="flex items-center justify-between text-[10px]">
          <span class="text-slate-400">本月</span>
          <span class="font-mono text-[11px]" :class="getTextColor(monthlyPeriod?.percent || 0)">
            {{ monthlyPeriod ? monthlyPeriod.percent.toFixed(1) : '-' }}%
          </span>
        </div>
        <div class="h-1.5 w-full bg-slate-800/80 rounded-full overflow-hidden">
          <div class="h-full rounded-full bg-gradient-to-r transition-all duration-300"
            :class="getBarColor(monthlyPeriod?.percent || 0)"
            :style="{ width: `${Math.min(100, Math.max(0, monthlyPeriod?.percent || 0))}%` }">
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
