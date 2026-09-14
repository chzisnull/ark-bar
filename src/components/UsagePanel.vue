<script setup lang="ts">
import { ref, computed, watch, onUnmounted, defineAsyncComponent } from 'vue';
import type { ProviderType, ProviderUsageData, UpdateInfo, ProviderQuotaPeriod } from '../types';
import { RefreshCw, Settings, Clock, Sparkles, Download, X, KeyRound, ExternalLink } from 'lucide-vue-next';
import { openUrl } from '@tauri-apps/plugin-opener';

const UpdateModal = defineAsyncComponent(() => import('./UpdateModal.vue'));

const props = defineProps<{
  providersData: Record<ProviderType, ProviderUsageData | null>;
  activeProvider: ProviderType;
  isRefreshing: boolean;
  loadingMap?: Record<ProviderType, boolean>;
  updateInfo?: UpdateInfo | null;
}>();

const emit = defineEmits<{
  (e: 'switch-provider', p: ProviderType): void;
  (e: 'refresh'): void;
  (e: 'open-settings'): void;
  (e: 'go-auth', p: ProviderType): void;
}>();

const bannerDismissed = ref(false);
const showUpdateModal = ref(false);

const providerList: { id: ProviderType; name: string }[] = [
  { id: 'volcengine', name: '火山方舟' },
  { id: 'antigravity', name: 'Antigravity' },
  { id: 'grok', name: 'Grok' },
  { id: 'codex', name: 'Codex' },
];

const currentUsage = computed<ProviderUsageData | null>(() => {
  return props.providersData[props.activeProvider] || null;
});

const isProviderLoading = computed(() => {
  return !!props.loadingMap?.[props.activeProvider] || props.isRefreshing;
});

// 徽章基类：统一高度/内边距/圆角，杜绝 py-0.2 之类被静默忽略的非法刻度
const BADGE = 'inline-flex items-center gap-1 whitespace-nowrap h-[18px] px-1.5 rounded-full text-[10px] font-medium border';
const badgeNeutral = `${BADGE} bg-slate-800/80 border-slate-700/60 text-slate-300`;
const badgeBrand = `${BADGE} bg-indigo-500/15 border-indigo-500/30 text-indigo-300`;
const badgeSuccess = `${BADGE} bg-emerald-500/15 border-emerald-500/30 text-emerald-400`;

// Hero 配额卡：首个周期升格为视觉主角，其余周期压成紧凑条
const heroGroup = computed(() => currentUsage.value?.groups?.[0] ?? null);
const heroPeriod = computed<ProviderQuotaPeriod | null>(() => heroGroup.value?.periods?.[0] ?? null);

const secondaryPeriods = computed<{ key: string; groupName: string | null; period: ProviderQuotaPeriod }[]>(() => {
  const usage = currentUsage.value;
  if (!usage) return [];
  const out: { key: string; groupName: string | null; period: ProviderQuotaPeriod }[] = [];
  usage.groups.forEach((g, gi) => {
    g.periods.forEach((p, pi) => {
      if (gi === 0 && pi === 0) return; // 首个周期已进 Hero 卡
      out.push({
        key: `${gi}-${pi}-${p.label}-${p.name}`,
        groupName: gi === 0 ? null : g.group_name,
        period: p,
      });
    });
  });
  return out;
});

// "上次同步"展示：按厂商记录时间戳（仅当停留在同一 Tab 时数据被替换才打点，
// 切换 Tab 不重置），30s 心跳驱动文案
const lastSyncMap = ref<Partial<Record<ProviderType, number>>>({});
let prevProvider: ProviderType | null = null;
watch(
  () => props.providersData[props.activeProvider],
  () => {
    if (prevProvider === props.activeProvider) {
      lastSyncMap.value[props.activeProvider] = Date.now();
    }
    prevProvider = props.activeProvider;
  },
  { immediate: true }
);
const now = ref(Date.now());
const heartbeat = setInterval(() => {
  now.value = Date.now();
}, 30000);
onUnmounted(() => clearInterval(heartbeat));

const syncAgoLabel = computed(() => {
  const ts = lastSyncMap.value[props.activeProvider];
  if (!ts) return '--';
  const diff = now.value - ts;
  if (diff < 60_000) return '刚刚';
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
  const d = new Date(ts);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
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

function clampPercent(percent: number): string {
  return `${Math.min(100, Math.max(0, percent))}%`;
}

async function openConsoleUrl() {
  const url = currentUsage.value?.console_url;
  if (!url) return;
  try {
    await openUrl(url);
  } catch {
    window.open(url, '_blank');
  }
}

function parseResetTime(dateStr?: string): number | null {
  if (!dateStr) return null;
  const trimmed = dateStr.trim();
  if (/^\d+$/.test(trimmed)) {
    const num = parseInt(trimmed, 10);
    return num < 1e11 ? num * 1000 : num;
  }
  const t = new Date(trimmed).getTime();
  return isNaN(t) ? null : t;
}

function formatExactTime(dateStr?: string): string {
  if (dateStr === 'rolling-5h') {
    return '5小时滚动窗口：产生用量后开始倒计时';
  }
  const target = parseResetTime(dateStr);
  if (!target) return dateStr || '';
  const d = new Date(target);
  const year = d.getFullYear();
  const month = String(d.getMonth() + 1).padStart(2, '0');
  const date = String(d.getDate()).padStart(2, '0');
  const hours = String(d.getHours()).padStart(2, '0');
  const mins = String(d.getMinutes()).padStart(2, '0');
  return `${year}-${month}-${date} ${hours}:${mins}`;
}

function formatDetailedReset(dateStr?: string): string {
  if (dateStr === 'rolling-5h') return '使用后开始计时';
  const target = parseResetTime(dateStr);
  if (!target) return dateStr || '';
  const nowTs = Date.now();
  const diff = target - nowTs;
  if (diff <= 0) return '即将重置';

  const targetDate = new Date(target);
  const nowDate = new Date(nowTs);
  const isSameDay =
    targetDate.getDate() === nowDate.getDate() &&
    targetDate.getMonth() === nowDate.getMonth() &&
    targetDate.getFullYear() === nowDate.getFullYear();

  const days = Math.floor(diff / (1000 * 60 * 60 * 24));
  const hours = Math.floor((diff % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
  const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));

  const timeStr = `${String(targetDate.getHours()).padStart(2, '0')}:${String(targetDate.getMinutes()).padStart(2, '0')}`;

  if (days > 0) {
    const monthDayStr = `${targetDate.getMonth() + 1}/${targetDate.getDate()}`;
    return `${monthDayStr} ${timeStr} (${days}天${hours}h后)`;
  } else if (isSameDay) {
    return `今天 ${timeStr} (${hours > 0 ? `${hours}h` : ''}${minutes}m后)`;
  } else {
    return `${hours > 0 ? `${hours}h` : ''}${minutes}m后 (${timeStr})`;
  }
}

// 紧凑条用的短格式重置文案（rolling-5h 必须特判，避免 new Date NaN）
function formatShortReset(dateStr?: string): string {
  if (dateStr === 'rolling-5h') return '使用后计时';
  const target = parseResetTime(dateStr);
  if (!target) return '';
  const diff = target - Date.now();
  if (diff <= 0) return '即将重置';

  const days = Math.floor(diff / (1000 * 60 * 60 * 24));
  if (days >= 1) {
    const targetDate = new Date(target);
    const timeStr = `${String(targetDate.getHours()).padStart(2, '0')}:${String(targetDate.getMinutes()).padStart(2, '0')}`;
    return `${targetDate.getMonth() + 1}/${targetDate.getDate()} ${timeStr}`;
  }
  const hours = Math.floor((diff % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
  const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));
  const nowDate = new Date();
  const targetDate = new Date(target);
  const isSameDay = targetDate.toDateString() === nowDate.toDateString();
  if (isSameDay) {
    const timeStr = `${String(targetDate.getHours()).padStart(2, '0')}:${String(targetDate.getMinutes()).padStart(2, '0')}`;
    return `今天 ${timeStr}`;
  }
  return `${hours > 0 ? `${hours}h` : ''}${minutes}m后`;
}
</script>

<template>
  <div class="flex flex-col h-full select-none text-slate-200 bg-[#0e131f]/95 relative">
    <div
      v-if="isProviderLoading"
      class="absolute top-0 left-0 right-0 h-[2px] overflow-hidden z-20 bg-indigo-500/20"
    >
      <div class="h-full w-1/3 rounded-full bg-gradient-to-r from-indigo-400 via-violet-400 to-cyan-300 arkbar-indet"></div>
    </div>

    <!-- Top Navigation Header -->
    <div class="h-11 px-4 shrink-0 bg-[#141b2d]/90 border-b border-slate-800/80 flex items-center gap-2">
      <!-- Provider Tabs Segmented Bar -->
      <div class="flex-1 min-w-0 flex items-center gap-0.5 bg-slate-900/70 p-1 rounded-xl border border-slate-800/60 overflow-x-auto no-scrollbar">
        <button
          v-for="item in providerList"
          :key="item.id"
          @click="$emit('switch-provider', item.id)"
          :title="item.name"
          class="flex items-center gap-1.5 h-7 px-2 rounded-lg text-[11px] font-medium whitespace-nowrap transition-colors duration-100 shrink-0"
          :class="[
            activeProvider === item.id
              ? 'bg-indigo-500/15 text-indigo-100 ring-1 ring-inset ring-indigo-500/40'
              : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/60'
          ]"
        >
          <RefreshCw
            v-if="loadingMap?.[item.id]"
            class="w-2.5 h-2.5 animate-spin text-indigo-300"
          />
          <span
            v-else
            class="w-1.5 h-1.5 rounded-full shrink-0"
            :class="[
              props.providersData[item.id]?.is_connected
                ? (activeProvider === item.id ? 'bg-emerald-300' : 'bg-emerald-500')
                : 'bg-slate-600'
            ]"
          ></span>
          <span>{{ item.name }}</span>
        </button>
      </div>

      <!-- Action Buttons -->
      <div class="flex items-center gap-1 shrink-0">
        <button
          @click="$emit('refresh')"
          :disabled="isRefreshing"
          class="w-7 h-7 rounded-lg grid place-items-center text-slate-400 hover:text-white hover:bg-slate-800/80 transition-colors"
          title="刷新数据"
        >
          <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': isProviderLoading }" />
        </button>
        <button
          @click="$emit('open-settings')"
          class="w-7 h-7 rounded-lg grid place-items-center text-slate-400 hover:text-white hover:bg-slate-800/80 transition-colors"
          title="设置"
        >
          <Settings class="w-3.5 h-3.5" />
        </button>
      </div>
    </div>

    <!-- Automatic Update Notification Banner -->
    <div
      v-if="updateInfo?.has_update && !bannerDismissed"
      class="bg-gradient-to-r from-amber-500/20 via-orange-500/20 to-amber-500/10 border-b border-amber-500/30 px-4 py-1.5 flex items-center justify-between text-xs shrink-0"
    >
      <div class="flex items-center gap-1.5 text-amber-300 font-medium">
        <Sparkles class="w-3.5 h-3.5 text-amber-400 shrink-0 animate-pulse" />
        <span class="text-[11px]">发现新版本 <strong>v{{ updateInfo.latest_version }}</strong></span>
      </div>
      <div class="flex items-center gap-1.5">
        <button
          @click="showUpdateModal = true"
          class="px-2 py-0.5 bg-gradient-to-r from-amber-500 to-orange-500 hover:from-amber-400 hover:to-orange-400 text-slate-950 font-bold rounded-lg text-[11px] flex items-center gap-1 transition shadow-sm whitespace-nowrap"
        >
          <Download class="w-2.5 h-2.5" />
          立即在线更新
        </button>
        <button
          @click="bannerDismissed = true"
          class="text-slate-400 hover:text-slate-200 p-0.5 rounded transition"
          title="关闭提示"
        >
          <X class="w-3 h-3" />
        </button>
      </div>
    </div>

    <!-- Main Content Area -->
    <div class="flex-1 min-h-0 overflow-y-auto px-4 py-4">
      <Transition name="panel-fade" mode="out-in">
      <!-- Loading Skeleton -->
      <div v-if="!currentUsage && isProviderLoading" key="loading-skel" class="flex flex-col gap-3.5 min-h-full">
        <div class="rounded-xl border border-slate-800/60 bg-[#131a2a]/40 p-4 space-y-3 animate-pulse">
          <div class="flex items-center gap-3">
            <div class="w-9 h-9 rounded-xl bg-slate-800/80"></div>
            <div class="space-y-1.5 flex-1">
              <div class="h-3 bg-slate-800 rounded w-28"></div>
              <div class="h-2.5 bg-slate-800/80 rounded w-40"></div>
            </div>
          </div>
          <div class="h-12 w-24 rounded-lg bg-slate-800/80 mt-2"></div>
          <div class="h-2.5 rounded-full bg-slate-800/80"></div>
          <div class="h-2.5 rounded-full bg-slate-800/60 w-2/3"></div>
        </div>
        <div class="flex items-center justify-center gap-2 text-xs text-slate-400 pt-2">
          <RefreshCw class="w-3.5 h-3.5 animate-spin text-indigo-400" />
          <span>正在同步最新配额数据...</span>
        </div>
      </div>

      <!-- 1. Connected State View -->
      <div v-else-if="currentUsage?.is_connected" :key="activeProvider + '-on'" class="flex flex-col gap-3.5 min-h-full">
        <!-- Identity Card -->
        <div class="shrink-0 rounded-xl border border-slate-800/60 bg-[#131a2a] px-4 py-3 flex items-center gap-3">
          <div class="w-9 h-9 shrink-0 rounded-xl bg-slate-800/60 border border-slate-800/60 grid place-items-center text-lg">
            {{ currentUsage.icon }}
          </div>
          <div class="flex-1 min-w-0 space-y-0.5">
            <div class="flex items-center gap-1.5 min-w-0">
              <span class="text-[13px] font-bold text-white truncate">{{ currentUsage.provider_name }}</span>
              <span v-if="currentUsage.account_info?.plan_name" :class="badgeBrand" class="shrink-0">
                {{ currentUsage.account_info.plan_name }}
              </span>
            </div>
            <div
              v-if="currentUsage.account_info?.email || currentUsage.account_info?.user_name"
              class="text-[11px] font-mono text-slate-500 truncate"
            >
              {{ currentUsage.account_info.email || currentUsage.account_info.user_name }}
            </div>
          </div>
          <span :class="badgeSuccess" class="shrink-0" title="订阅生效中">
            <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
            生效中
          </span>
        </div>

        <!-- HERO Quota Card -->
        <div
          v-if="heroPeriod"
          class="flex-1 flex flex-col relative overflow-hidden rounded-xl border border-slate-800/60 bg-[#131a2a] p-4"
          :class="{ 'min-h-[150px]': secondaryPeriods.length > 0 }"
        >
          <div class="absolute -right-8 -top-10 w-44 h-44 rounded-full bg-indigo-500/10 blur-3xl pointer-events-none"></div>

          <div
            v-if="isProviderLoading"
            class="absolute top-2 right-2 z-10"
          >
            <span :class="badgeBrand">
              <RefreshCw class="w-2.5 h-2.5 animate-spin" />
              同步中
            </span>
          </div>

          <!-- Row A: period name + reset badge -->
          <div class="shrink-0 flex items-center justify-between gap-2">
            <div class="flex items-center gap-1.5 min-w-0">
              <span class="w-1.5 h-1.5 rounded-full bg-indigo-400 shrink-0"></span>
              <span class="text-[11px] font-semibold text-slate-400 truncate">{{ heroPeriod?.name }}</span>
              <span v-if="currentUsage.groups.length > 1 && heroGroup" class="text-[10px] text-slate-500 truncate">
                · {{ heroGroup.group_name }}
              </span>
            </div>
            <span
              v-if="heroPeriod?.reset_at"
              :class="badgeNeutral"
              class="cursor-help hover:text-white transition-colors"
              :title="`额度刷新时间: ${formatExactTime(heroPeriod.reset_at)}`"
            >
              <Clock class="w-2.5 h-2.5 text-indigo-400 shrink-0" />
              {{ formatDetailedReset(heroPeriod.reset_at) }}
            </span>
          </div>

          <!-- Row B: hero number, vertically centered in remaining height -->
          <div class="flex-1 min-h-0 flex flex-col justify-center gap-1.5 py-2">
            <div class="flex items-baseline gap-1">
              <span
                class="text-[40px] leading-none font-bold tracking-tight tabular-nums"
                :class="getTextColor(heroPeriod?.used_percent ?? 0)"
              >{{ Math.round(heroPeriod?.used_percent ?? 0) }}</span>
              <span class="text-lg font-bold text-slate-500">%</span>
            </div>
            <div class="text-[11px] text-slate-400">
              已使用 · 剩余 <span class="text-slate-100 font-semibold">{{ Math.round(heroPeriod?.remaining_percent ?? 0) }}%</span>
              <span v-if="heroPeriod?.used != null && heroPeriod?.total != null" class="font-mono text-slate-500 ml-1.5">
                {{ heroPeriod.used }} / {{ heroPeriod.total }}
              </span>
            </div>
          </div>

          <!-- Row C: progress bar + full-width description -->
          <div class="shrink-0 space-y-2">
            <div class="w-full bg-slate-800 h-2.5 rounded-full overflow-hidden">
              <div
                class="h-full rounded-full bg-gradient-to-r transition-all duration-500"
                :class="getBarColor(heroPeriod?.used_percent ?? 0)"
                :style="{ width: clampPercent(heroPeriod?.used_percent ?? 0) }"
              ></div>
            </div>
            <p v-if="heroPeriod?.description" class="text-[11px] text-slate-500 leading-relaxed">
              {{ heroPeriod.description }}
            </p>
          </div>
        </div>

        <!-- Secondary Periods (volcengine weekly/monthly etc.) -->
        <div
          v-if="secondaryPeriods.length"
          class="shrink-0 rounded-xl border border-slate-800/60 bg-[#131a2a] divide-y divide-slate-800/60 overflow-hidden"
        >
          <div
            v-for="item in secondaryPeriods"
            :key="item.key"
            class="px-4 py-2.5 flex items-center gap-2.5"
          >
            <span class="text-xs text-slate-300 flex-1 min-w-0 truncate">
              <template v-if="item.groupName">{{ item.groupName }} · </template>{{ item.period.name }}
            </span>
            <div class="w-14 h-1 rounded-full bg-slate-800 overflow-hidden shrink-0">
              <div
                class="h-full rounded-full bg-gradient-to-r transition-all duration-500"
                :class="getBarColor(item.period.used_percent)"
                :style="{ width: clampPercent(item.period.used_percent) }"
              ></div>
            </div>
            <span
              class="w-10 text-right text-xs font-semibold tabular-nums shrink-0"
              :class="getTextColor(item.period.used_percent)"
            >{{ Math.round(item.period.used_percent) }}%</span>
            <span
              v-if="item.period.reset_at"
              :class="badgeNeutral"
              class="shrink-0 cursor-help hover:text-white transition-colors"
              :title="`额度刷新时间: ${formatExactTime(item.period.reset_at)}`"
            >
              <Clock class="w-2.5 h-2.5 text-indigo-400 shrink-0" />
              {{ formatShortReset(item.period.reset_at) }}
            </span>
          </div>
        </div>

        <!-- Footer Meta: last sync + console link -->
        <div class="shrink-0 mt-auto pt-1 px-1 flex items-center justify-between text-[10px] text-slate-500">
          <span class="flex items-center gap-1.5">
            <span
              class="w-1.5 h-1.5 rounded-full"
              :class="isProviderLoading ? 'bg-indigo-400 animate-pulse' : 'bg-slate-600'"
            ></span>
            上次同步 {{ syncAgoLabel }}
          </span>
          <button
            v-if="currentUsage.console_url"
            @click="openConsoleUrl"
            class="flex items-center gap-1 hover:text-indigo-300 transition-colors cursor-pointer"
          >
            <ExternalLink class="w-3 h-3" />
            打开控制台
          </button>
        </div>
      </div>

      <!-- 2. Unconnected State View -->
      <div v-else :key="activeProvider + '-off'" class="flex flex-col gap-3.5 min-h-full">
        <div class="flex-1 flex flex-col justify-center">
          <div class="p-5 bg-[#131a2a] border border-slate-800/60 rounded-xl text-center space-y-3.5">
            <div class="w-14 h-14 rounded-2xl bg-indigo-500/10 border border-indigo-500/20 flex items-center justify-center text-2xl mx-auto">
              {{ currentUsage?.icon || '⚙️' }}
            </div>
            <div class="space-y-1">
              <h4 class="text-[13px] font-bold text-white">{{ currentUsage?.provider_name || '服务商未连接' }}</h4>
              <p class="text-[11px] text-slate-400 leading-relaxed px-2">
                {{ currentUsage?.error_message || currentUsage?.status_message || '当前服务商未检测到登录凭据或授权令牌。' }}
              </p>
            </div>

            <div class="pt-1 flex flex-col gap-2">
              <button
                @click="$emit('go-auth', activeProvider)"
                class="w-full h-8 bg-gradient-to-r from-indigo-600 to-violet-600 hover:from-indigo-500 hover:to-violet-500 text-white rounded-xl text-xs font-semibold flex items-center justify-center gap-1.5 transition shadow-md shadow-indigo-500/20 cursor-pointer"
              >
                <KeyRound class="w-3.5 h-3.5" />
                <span>{{ activeProvider === 'volcengine' ? '立即前往安装 / 授权' : '前往填入令牌 / 授权' }}</span>
              </button>
              <button
                @click="$emit('refresh')"
                :disabled="isRefreshing"
                class="w-full h-8 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-xl text-xs font-medium flex items-center justify-center gap-1 transition cursor-pointer"
              >
                <RefreshCw class="w-3 h-3" :class="{ 'animate-spin': isProviderLoading }" />
                <span>重新检测状态</span>
              </button>
            </div>
          </div>
        </div>
      </div>
      </Transition>
    </div>

    <!-- In-App Online Update Modal -->
    <UpdateModal
      v-if="showUpdateModal && updateInfo"
      :update-info="updateInfo"
      @close="showUpdateModal = false"
    />
  </div>
</template>

<style scoped>
.no-scrollbar::-webkit-scrollbar {
  display: none;
}
.no-scrollbar {
  -ms-overflow-style: none;
  scrollbar-width: none;
}
</style>
