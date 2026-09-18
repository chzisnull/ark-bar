<script setup lang="ts">
import { computed } from 'vue';
import type { ProviderTokenSummary, ProviderType } from '../types';
import { Clock, Calendar, Zap, Activity, ChevronRight, BarChart3, ShieldCheck } from 'lucide-vue-next';

const props = defineProps<{
  tokenSummary?: ProviderTokenSummary | null;
  provider: ProviderType;
  providerName: string;
  isRefreshing?: boolean;
}>();

const emit = defineEmits<{
  (e: 'open-history'): void;
}>();

function formatTokens(tokens: number): string {
  if (tokens == null || isNaN(tokens)) return '0';
  if (tokens >= 1_000_000_000) return `${(tokens / 1_000_000_000).toFixed(2)}B`;
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(2)}M`;
  if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(1)}K`;
  return tokens.toLocaleString();
}

const recent7Days = computed(() => {
  if (!props.tokenSummary?.daily_history?.length) return [];
  const sorted = [...props.tokenSummary.daily_history].sort((a, b) => a.date.localeCompare(b.date));
  const slice = sorted.slice(-7);
  const max = Math.max(...slice.map((d) => d.total_tokens), 1);
  return slice.map((d) => ({
    ...d,
    heightPct: Math.max(12, Math.round((d.total_tokens / max) * 100)),
    shortDate: d.date.slice(5), // MM-DD
  }));
});

const sourceBadge = computed(() => {
  const type = props.tokenSummary?.data_source_type;
  if (type === 'api') return { text: '官方 OpenAPI 实时聚合', class: 'bg-emerald-500/15 text-emerald-400 border-emerald-500/30' };
  if (type === 'local_logs') return { text: '本地会话日志提取', class: 'bg-cyan-500/15 text-cyan-400 border-cyan-500/30' };
  if (type === 'credits') return { text: '额度积分与消耗换算', class: 'bg-amber-500/15 text-amber-400 border-amber-500/30' };
  return { text: '订阅模型权重估算', class: 'bg-indigo-500/15 text-indigo-400 border-indigo-500/30' };
});

const promptPct = computed(() => {
  const s = props.tokenSummary;
  if (!s || (s.input_tokens + s.output_tokens) === 0) return 80;
  return Math.round((s.input_tokens / (s.input_tokens + s.output_tokens)) * 100);
});

const completionPct = computed(() => {
  return 100 - promptPct.value;
});
</script>

<template>
  <div class="flex flex-col gap-3">
    <!-- Data Source Badge Row -->
    <div class="flex items-center justify-between px-1 text-[10px] text-slate-400">
      <div class="flex items-center gap-1.5">
        <span
          class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full border text-[9px] font-medium"
          :class="sourceBadge.class"
        >
          <ShieldCheck class="w-2.5 h-2.5" />
          {{ sourceBadge.text }}
        </span>
      </div>
      <span v-if="tokenSummary?.updated_at" class="text-slate-500 font-mono text-[9px]">
        {{ tokenSummary.updated_at.slice(11, 19) }} 同步
      </span>
    </div>

    <!-- 4 Metric Cards Grid -->
    <div class="grid grid-cols-2 gap-2.5">
      <!-- 1. 5h window -->
      <div class="rounded-xl border border-slate-800/80 bg-[#131a2a] p-3 flex flex-col justify-between relative overflow-hidden group hover:border-slate-700/80 transition-colors">
        <div class="absolute -right-6 -bottom-6 w-16 h-16 rounded-full bg-cyan-500/10 blur-xl pointer-events-none"></div>
        <div class="flex items-center justify-between text-[11px] text-slate-400">
          <span class="flex items-center gap-1 font-medium text-slate-300">
            <Clock class="w-3 h-3 text-cyan-400" />
            近 5 小时
          </span>
          <span class="text-[9px] px-1 rounded bg-cyan-500/15 text-cyan-300 font-medium">滑动窗口</span>
        </div>
        <div class="mt-2 flex items-baseline gap-1">
          <span class="text-2xl font-bold text-white tracking-tight font-mono">
            {{ formatTokens(tokenSummary?.session_5h_tokens ?? 0) }}
          </span>
          <span class="text-xs text-slate-500">Tokens</span>
        </div>
      </div>

      <!-- 2. Today -->
      <div class="rounded-xl border border-slate-800/80 bg-[#131a2a] p-3 flex flex-col justify-between relative overflow-hidden group hover:border-slate-700/80 transition-colors">
        <div class="absolute -right-6 -bottom-6 w-16 h-16 rounded-full bg-indigo-500/10 blur-xl pointer-events-none"></div>
        <div class="flex items-center justify-between text-[11px] text-slate-400">
          <span class="flex items-center gap-1 font-medium text-slate-300">
            <Calendar class="w-3 h-3 text-indigo-400" />
            今日累计
          </span>
          <span class="text-[9px] px-1 rounded bg-indigo-500/15 text-indigo-300 font-medium">00:00 至今</span>
        </div>
        <div class="mt-2 flex items-baseline gap-1">
          <span class="text-2xl font-bold text-indigo-100 tracking-tight font-mono">
            {{ formatTokens(tokenSummary?.today_tokens ?? 0) }}
          </span>
          <span class="text-xs text-slate-500">Tokens</span>
        </div>
      </div>

      <!-- 3. This Week -->
      <div class="rounded-xl border border-slate-800/80 bg-[#131a2a] p-3 flex flex-col justify-between relative overflow-hidden group hover:border-slate-700/80 transition-colors">
        <div class="flex items-center justify-between text-[11px] text-slate-400">
          <span class="flex items-center gap-1 font-medium text-slate-300">
            <Activity class="w-3 h-3 text-violet-400" />
            本周用量
          </span>
          <span class="text-[9px] px-1 rounded bg-slate-800 text-slate-400 font-mono">周一起</span>
        </div>
        <div class="mt-2 flex items-baseline gap-1">
          <span class="text-xl font-bold text-white tracking-tight font-mono">
            {{ formatTokens(tokenSummary?.this_week_tokens ?? 0) }}
          </span>
          <span class="text-xs text-slate-500">Tokens</span>
        </div>
      </div>

      <!-- 4. This Month -->
      <div class="rounded-xl border border-slate-800/80 bg-[#131a2a] p-3 flex flex-col justify-between relative overflow-hidden group hover:border-slate-700/80 transition-colors">
        <div class="flex items-center justify-between text-[11px] text-slate-400">
          <span class="flex items-center gap-1 font-medium text-slate-300">
            <BarChart3 class="w-3 h-3 text-emerald-400" />
            本月用量
          </span>
          <span class="text-[9px] px-1 rounded bg-slate-800 text-slate-400 font-mono">1日起</span>
        </div>
        <div class="mt-2 flex items-baseline gap-1">
          <span class="text-xl font-bold text-emerald-300 tracking-tight font-mono">
            {{ formatTokens(tokenSummary?.this_month_tokens ?? 0) }}
          </span>
          <span class="text-xs text-slate-500">Tokens</span>
        </div>
      </div>
    </div>

    <!-- Token Composition & Cache Hit Card -->
    <div class="rounded-xl border border-slate-800/80 bg-[#131a2a] p-3.5 flex flex-col gap-2.5">
      <div class="flex items-center justify-between text-xs">
        <span class="font-semibold text-white flex items-center gap-1.5">
          <Zap class="w-3.5 h-3.5 text-amber-400" />
          今日构成与效率
        </span>
        <span
          v-if="tokenSummary && tokenSummary.cache_hit_rate > 0"
          class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full bg-emerald-500/15 border border-emerald-500/30 text-emerald-400 text-[10px] font-medium"
        >
          <Zap class="w-2.5 h-2.5" />
          缓存节省 {{ tokenSummary.cache_hit_rate }}%
        </span>
      </div>

      <!-- Composition bar -->
      <div class="w-full bg-slate-800/80 h-2 rounded-full overflow-hidden flex">
        <div
          class="h-full bg-indigo-500 transition-all duration-300"
          :style="{ width: `${promptPct}%` }"
          :title="`输入: ${formatTokens(tokenSummary?.input_tokens ?? 0)}`"
        ></div>
        <div
          class="h-full bg-cyan-400 transition-all duration-300"
          :style="{ width: `${completionPct}%` }"
          :title="`输出: ${formatTokens(tokenSummary?.output_tokens ?? 0)}`"
        ></div>
      </div>

      <!-- Detail rows -->
      <div class="grid grid-cols-3 gap-2 pt-1 text-[11px] font-mono border-t border-slate-800/60">
        <div>
          <div class="text-[10px] text-slate-400 font-sans">输入 (Prompt)</div>
          <div class="font-bold text-indigo-300 mt-0.5">
            {{ formatTokens(tokenSummary?.input_tokens ?? 0) }}
          </div>
        </div>
        <div>
          <div class="text-[10px] text-slate-400 font-sans">输出 (Reply)</div>
          <div class="font-bold text-cyan-300 mt-0.5">
            {{ formatTokens(tokenSummary?.output_tokens ?? 0) }}
          </div>
        </div>
        <div>
          <div class="text-[10px] text-slate-400 font-sans">缓存命中</div>
          <div class="font-bold text-emerald-400 mt-0.5">
            {{ tokenSummary?.cache_hit_tokens ? formatTokens(tokenSummary.cache_hit_tokens) : '--' }}
          </div>
        </div>
      </div>
    </div>

    <!-- Mini 7-day Trend Sparkline (if has history) -->
    <div
      v-if="recent7Days.length"
      class="rounded-xl border border-slate-800/80 bg-[#131a2a] p-3 flex flex-col gap-2"
    >
      <div class="flex items-center justify-between text-[11px] text-slate-400 font-medium">
        <span>近 7 天每日趋势</span>
        <span class="text-[10px] text-slate-500 font-mono">
          今日请求: {{ tokenSummary?.request_count ?? 0 }} 次
        </span>
      </div>

      <div class="h-16 flex items-end gap-1.5 pt-2">
        <div
          v-for="d in recent7Days"
          :key="d.date"
          class="flex-1 flex flex-col items-center gap-1 group relative h-full justify-end"
        >
          <!-- Tooltip -->
          <div
            class="absolute -top-6 opacity-0 group-hover:opacity-100 transition-opacity bg-slate-900 border border-slate-700 px-1 py-0.5 rounded text-[8px] text-white whitespace-nowrap z-20 pointer-events-none"
          >
            {{ d.date }}: {{ formatTokens(d.total_tokens) }}
          </div>
          <!-- Bar -->
          <div
            class="w-full rounded-t transition-all duration-300"
            :class="d.total_tokens > 0 ? 'bg-gradient-to-t from-indigo-500 to-cyan-400 group-hover:from-indigo-400 group-hover:to-cyan-300' : 'bg-slate-800/40'"
            :style="{ height: `${d.heightPct}%` }"
          ></div>
          <span class="text-[8px] text-slate-500 font-mono scale-90 -mx-1">
            {{ d.shortDate }}
          </span>
        </div>
      </div>
    </div>

    <!-- View Full History Button -->
    <button
      @click="$emit('open-history')"
      class="w-full h-9 rounded-xl bg-slate-900/80 hover:bg-slate-800 border border-slate-800/80 hover:border-slate-700 text-xs font-medium text-slate-300 hover:text-white flex items-center justify-center gap-1.5 transition shadow-sm"
    >
      <BarChart3 class="w-3.5 h-3.5 text-indigo-400" />
      <span>查看历史完整报表与按日明细</span>
      <ChevronRight class="w-3.5 h-3.5 text-slate-500" />
    </button>
  </div>
</template>
