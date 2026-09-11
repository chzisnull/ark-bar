<script setup lang="ts">
import { ref, computed, defineAsyncComponent } from 'vue';
import type { ProviderType, ProviderUsageData, UpdateInfo } from '../types';
import { RefreshCw, Settings, Clock, Sparkles, Download, X, KeyRound } from 'lucide-vue-next';

const UpdateModal = defineAsyncComponent(() => import('./UpdateModal.vue'));

const props = defineProps<{
  providersData: Record<ProviderType, ProviderUsageData | null>;
  activeProvider: ProviderType;
  isRefreshing: boolean;
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

const providerList: { id: ProviderType; name: string; icon: string }[] = [
  { id: 'volcengine', name: '火山方舟', icon: '🌋' },
  { id: 'antigravity', name: 'Antigravity', icon: '🌐' },
  { id: 'grok', name: 'Grok', icon: '⚡' },
  { id: 'codex', name: 'Codex', icon: '🤖' },
];

const currentUsage = computed<ProviderUsageData | null>(() => {
  return props.providersData[props.activeProvider] || null;
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
  const target = parseResetTime(dateStr);
  if (!target) return dateStr || '';
  const now = Date.now();
  const diff = target - now;
  if (diff <= 0) return '即将重置';

  const targetDate = new Date(target);
  const nowDate = new Date(now);
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
</script>

<template>
  <div class="flex flex-col h-full select-none text-slate-200 bg-[#0e131f]/95">
    <!-- Top Navigation Header -->
    <div class="px-3.5 py-2.5 bg-[#141b2d]/90 border-b border-slate-800/80 flex items-center justify-between gap-2">
      <!-- Provider Tabs Segmented Bar -->
      <div class="flex items-center gap-1 bg-slate-900/90 p-0.5 rounded-xl border border-slate-800/80 overflow-x-auto no-scrollbar">
        <button
          v-for="item in providerList"
          :key="item.id"
          @click="$emit('switch-provider', item.id)"
          class="flex items-center gap-1 px-2 py-1 rounded-lg text-xs font-medium transition duration-150"
          :class="[
            activeProvider === item.id
              ? 'bg-gradient-to-r from-indigo-600/90 to-violet-600/90 text-white shadow-sm shadow-indigo-500/25'
              : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/50'
          ]"
        >
          <span class="text-xs">{{ item.icon }}</span>
          <span class="text-[11px] whitespace-nowrap">{{ item.name }}</span>
          <!-- Live connection indicator dot -->
          <span
            class="w-1.5 h-1.5 rounded-full"
            :class="[
              props.providersData[item.id]?.is_connected
                ? (activeProvider === item.id ? 'bg-emerald-300' : 'bg-emerald-500')
                : 'bg-slate-600'
            ]"
          ></span>
        </button>
      </div>

      <!-- Action Buttons -->
      <div class="flex items-center space-x-1 shrink-0">
        <button
          @click="$emit('refresh')"
          :disabled="isRefreshing"
          class="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800/80 transition"
          title="刷新数据"
        >
          <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': isRefreshing }" />
        </button>
        <button
          @click="$emit('open-settings')"
          class="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800/80 transition"
          title="设置"
        >
          <Settings class="w-3.5 h-3.5" />
        </button>
      </div>
    </div>

    <!-- Automatic Update Notification Banner -->
    <div
      v-if="updateInfo?.has_update && !bannerDismissed"
      class="bg-gradient-to-r from-amber-500/20 via-orange-500/20 to-amber-500/10 border-b border-amber-500/30 px-3.5 py-1.5 flex items-center justify-between text-xs"
    >
      <div class="flex items-center gap-1.5 text-amber-300 font-medium">
        <Sparkles class="w-3.5 h-3.5 text-amber-400 shrink-0 animate-pulse" />
        <span class="text-[11px]">发现新版本 <strong>v{{ updateInfo.latest_version }}</strong></span>
      </div>
      <div class="flex items-center gap-1.5">
        <button
          @click="showUpdateModal = true"
          class="px-2 py-0.5 bg-gradient-to-r from-amber-500 to-orange-500 hover:from-amber-400 hover:to-orange-400 text-slate-950 font-bold rounded text-[10px] flex items-center gap-1 transition shadow-sm"
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
    <div class="p-3.5 flex-1 overflow-y-auto space-y-3">
      <!-- 0. Initial Loading Skeleton View (when fetching first time) -->
      <div v-if="!currentUsage && isRefreshing" class="p-4 bg-[#131a2a]/40 border border-slate-800/60 rounded-xl space-y-3 my-1 animate-pulse">
        <div class="flex items-center justify-between">
          <div class="h-4 bg-slate-800 rounded w-24"></div>
          <div class="h-4 bg-slate-800 rounded w-16"></div>
        </div>
        <div class="h-16 bg-slate-800/60 rounded-xl"></div>
        <div class="h-16 bg-slate-800/60 rounded-xl"></div>
        <div class="flex items-center justify-center gap-2 text-xs text-slate-400 pt-2">
          <RefreshCw class="w-3.5 h-3.5 animate-spin text-indigo-400" />
          <span>正在同步最新配额数据...</span>
        </div>
      </div>

      <!-- 1. Connected State View -->
      <div v-else-if="currentUsage?.is_connected" class="space-y-3">
        <!-- Provider Info Header Badge -->
        <div class="flex items-center justify-between bg-[#131a2a]/60 px-3 py-2 rounded-xl border border-slate-800/60">
          <div class="flex items-center gap-2">
            <span class="text-base">{{ currentUsage.icon }}</span>
            <div>
              <div class="flex items-center gap-1.5">
                <span class="text-xs font-bold text-white">{{ currentUsage.provider_name }}</span>
                <span v-if="currentUsage.account_info?.plan_name" class="text-[10px] bg-indigo-500/15 text-indigo-300 px-1.5 py-0.2 rounded font-medium border border-indigo-500/30">
                  {{ currentUsage.account_info.plan_name }}
                </span>
                <span class="text-[10px] bg-emerald-500/15 text-emerald-400 px-1.5 py-0.2 rounded font-medium flex items-center gap-1 border border-emerald-500/30">
                  <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
                  生效中
                </span>
              </div>
            </div>
          </div>
          <span v-if="currentUsage.account_info?.email || currentUsage.account_info?.user_name" class="text-[10px] text-slate-400 font-mono truncate max-w-[120px]">
            {{ currentUsage.account_info.email || currentUsage.account_info.user_name }}
          </span>
        </div>

        <!-- Groups & Quota Cards -->
        <div v-for="(group, gIdx) in currentUsage.groups" :key="gIdx" class="space-y-2">
          <!-- Group Title (if multiple groups) -->
          <div v-if="currentUsage.groups.length > 1" class="flex items-center gap-1.5 pt-1 text-[11px] font-semibold text-slate-400">
            <span class="w-1.5 h-1.5 rounded-full bg-indigo-500"></span>
            <span>{{ group.group_name }}</span>
            <span v-if="group.edition" class="text-[10px] text-slate-500 font-normal">({{ group.edition }})</span>
          </div>

          <!-- Periods Cards within Group -->
          <div
            v-for="(period, pIdx) in group.periods"
            :key="pIdx"
            class="bg-[#131a2a]/80 border border-slate-800/80 rounded-xl p-3 space-y-2 hover:border-slate-700/80 transition relative overflow-hidden"
          >
            <!-- Top Line: Title & Badges -->
            <div class="flex items-center justify-between">
              <span class="text-xs font-semibold text-slate-200">{{ period.name }}</span>
              <div class="flex items-center gap-1.5">
                <span
                  v-if="period.reset_at"
                  class="text-[10px] bg-slate-800/90 text-slate-300 px-1.5 py-0.5 rounded font-medium flex items-center gap-1 cursor-help hover:text-white transition"
                  :title="`额度刷新时间: ${formatExactTime(period.reset_at)}`"
                >
                  <Clock class="w-2.5 h-2.5 text-indigo-400 shrink-0" />
                  {{ formatDetailedReset(period.reset_at) }}
                </span>
                <span class="text-xs font-mono font-bold" :class="getTextColor(period.used_percent)">
                  已用 {{ period.used_percent }}%
                </span>
              </div>
            </div>

            <!-- Progress Bar -->
            <div class="w-full bg-slate-800/90 h-2 rounded-full overflow-hidden p-0.5 border border-slate-700/50">
              <div
                class="h-full rounded-full bg-gradient-to-r transition-all duration-500 shadow-sm"
                :class="getBarColor(period.used_percent)"
                :style="{ width: `${Math.min(100, Math.max(0, period.used_percent))}%` }"
              ></div>
            </div>

            <!-- Bottom Line Info -->
            <div class="flex items-center justify-between text-[10px] text-slate-400">
              <span>
                剩余额度: <strong class="text-slate-300">{{ period.remaining_percent }}%</strong>
              </span>
              <span v-if="period.used != null && period.total != null">
                {{ period.used }} / {{ period.total }}
              </span>
              <span v-else-if="period.description" class="text-slate-500 truncate max-w-[170px]">
                {{ period.description }}
              </span>
            </div>
          </div>
        </div>
      </div>

      <!-- 2. Unconnected State View -->
      <div v-else class="p-4 bg-[#131a2a]/80 border border-slate-800/80 rounded-xl text-center space-y-3.5 my-2">
        <div class="w-12 h-12 rounded-2xl bg-indigo-500/10 border border-indigo-500/20 flex items-center justify-center text-2xl mx-auto shadow-inner">
          {{ currentUsage?.icon || '⚙️' }}
        </div>
        <div class="space-y-1">
          <h4 class="text-xs font-bold text-white">{{ currentUsage?.provider_name || '服务商未连接' }}</h4>
          <p class="text-[11px] text-slate-400 leading-relaxed px-2">
            {{ currentUsage?.error_message || currentUsage?.status_message || '当前服务商未检测到登录凭据或授权令牌。' }}
          </p>
        </div>

        <div class="pt-1 flex flex-col gap-2">
          <button
            @click="$emit('go-auth', activeProvider)"
            class="w-full py-2.5 bg-gradient-to-r from-indigo-600 to-violet-600 hover:from-indigo-500 hover:to-violet-500 text-white rounded-xl text-xs font-semibold flex items-center justify-center gap-1.5 transition shadow-md shadow-indigo-500/20 cursor-pointer"
          >
            <KeyRound class="w-3.5 h-3.5" />
            <span>{{ activeProvider === 'volcengine' ? '立即前往安装 / 授权' : '前往填入令牌 / 授权' }}</span>
          </button>
          <button
            @click="$emit('refresh')"
            :disabled="isRefreshing"
            class="w-full py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-xl text-xs font-medium flex items-center justify-center gap-1 transition cursor-pointer"
          >
            <RefreshCw class="w-3 h-3" :class="{ 'animate-spin': isRefreshing }" />
            <span>重新检测状态</span>
          </button>
        </div>
      </div>
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
