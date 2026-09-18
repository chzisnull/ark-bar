<script setup lang="ts">
import { computed } from 'vue';
import type { ProviderTokenSummary, DailyTokenRecord } from '../types';
import { X, Calendar, Database, Zap, Activity, Download } from 'lucide-vue-next';

const props = defineProps<{
  show: boolean;
  tokenSummary?: ProviderTokenSummary | null;
  providerName: string;
  providerIcon: string;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

function formatTokens(tokens: number): string {
  if (tokens >= 1_000_000_000) return `${(tokens / 1_000_000_000).toFixed(2)}B`;
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(2)}M`;
  if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(1)}K`;
  return tokens.toLocaleString();
}

const historyList = computed<DailyTokenRecord[]>(() => {
  if (!props.tokenSummary?.daily_history?.length) return [];
  // 降序排列，最新日期排在最前
  return [...props.tokenSummary.daily_history].sort((a, b) => b.date.localeCompare(a.date));
});

// 计算用于柱状图的最大值（正序排列，取最近 14 天）
const chartData = computed(() => {
  if (!props.tokenSummary?.daily_history?.length) return [];
  const sorted = [...props.tokenSummary.daily_history].sort((a, b) => a.date.localeCompare(b.date));
  const recent = sorted.slice(-14);
  const max = Math.max(...recent.map((d) => d.total_tokens), 1);
  return recent.map((d) => ({
    ...d,
    heightPct: Math.max(8, Math.round((d.total_tokens / max) * 100)),
    shortDate: d.date.slice(5), // MM-DD
  }));
});

const totalTokensInHistory = computed(() => {
  return historyList.value.reduce((acc, curr) => acc + curr.total_tokens, 0);
});

const totalReqInHistory = computed(() => {
  return historyList.value.reduce((acc, curr) => acc + curr.request_count, 0);
});

const totalCacheInHistory = computed(() => {
  return historyList.value.reduce((acc, curr) => acc + curr.cache_hit_tokens, 0);
});

function copyJsonData() {
  if (!props.tokenSummary?.daily_history) return;
  const str = JSON.stringify(props.tokenSummary.daily_history, null, 2);
  navigator.clipboard.writeText(str).then(() => {
    alert('历史 Token 记录已复制为 JSON！');
  });
}
</script>

<template>
  <div
    v-if="show"
    class="fixed inset-0 z-50 bg-black/75 backdrop-blur-md flex flex-col justify-end animate-fade-in"
    @click.self="$emit('close')"
  >
    <div
      class="bg-[#111726] border-t border-slate-800 rounded-t-2xl max-h-[90vh] h-[520px] flex flex-col overflow-hidden shadow-2xl animate-slide-up"
    >
      <!-- Header -->
      <div class="h-12 px-4 border-b border-slate-800/80 flex items-center justify-between shrink-0 bg-[#161f33]/90">
        <div class="flex items-center gap-2 min-w-0">
          <span class="text-base">{{ providerIcon }}</span>
          <h3 class="text-xs font-bold text-white truncate">{{ providerName }} · 历史 Token 统计报表</h3>
        </div>
        <div class="flex items-center gap-1.5">
          <button
            @click="copyJsonData"
            class="h-6 px-2 rounded bg-slate-800 hover:bg-slate-700 text-[10px] text-slate-300 hover:text-white flex items-center gap-1 transition"
            title="导出 JSON 数据"
          >
            <Download class="w-3 h-3" />
            导出
          </button>
          <button
            @click="$emit('close')"
            class="w-6 h-6 rounded-lg grid place-items-center text-slate-400 hover:text-white hover:bg-slate-800 transition"
          >
            <X class="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      <!-- Overview Metric Bar -->
      <div class="grid grid-cols-3 gap-2 px-4 py-2.5 bg-[#0e1422] border-b border-slate-800/60 shrink-0">
        <div class="bg-slate-900/60 rounded-lg p-2 border border-slate-800/60">
          <div class="text-[10px] text-slate-400 flex items-center gap-1">
            <Activity class="w-2.5 h-2.5 text-indigo-400" />
            历史总 Token
          </div>
          <div class="text-sm font-bold text-white font-mono mt-0.5">
            {{ formatTokens(totalTokensInHistory) }}
          </div>
        </div>
        <div class="bg-slate-900/60 rounded-lg p-2 border border-slate-800/60">
          <div class="text-[10px] text-slate-400 flex items-center gap-1">
            <Zap class="w-2.5 h-2.5 text-emerald-400" />
            总缓存节省
          </div>
          <div class="text-sm font-bold text-emerald-400 font-mono mt-0.5">
            {{ formatTokens(totalCacheInHistory) }}
          </div>
        </div>
        <div class="bg-slate-900/60 rounded-lg p-2 border border-slate-800/60">
          <div class="text-[10px] text-slate-400 flex items-center gap-1">
            <Database class="w-2.5 h-2.5 text-cyan-400" />
            总调用请求
          </div>
          <div class="text-sm font-bold text-cyan-300 font-mono mt-0.5">
            {{ totalReqInHistory.toLocaleString() }} 次
          </div>
        </div>
      </div>

      <!-- Trend Chart Section -->
      <div v-if="chartData.length" class="px-4 py-2.5 border-b border-slate-800/60 shrink-0 bg-[#121828]/50">
        <div class="flex items-center justify-between text-[11px] text-slate-400 mb-1.5 font-medium">
          <span>近 14 天用量走势 (柱状图)</span>
          <span class="text-[10px] text-slate-500">单位: Tokens / 日</span>
        </div>
        <div class="h-24 flex items-end gap-1.5 pt-4 pb-1">
          <div
            v-for="item in chartData"
            :key="item.date"
            class="flex-1 flex flex-col items-center gap-1 group relative h-full justify-end"
          >
            <!-- Tooltip -->
            <div
              class="absolute -top-7 opacity-0 group-hover:opacity-100 transition-opacity bg-slate-950/95 border border-slate-700 px-1.5 py-0.5 rounded text-[9px] text-slate-200 pointer-events-none whitespace-nowrap z-20 shadow-lg"
            >
              {{ item.date }}: {{ formatTokens(item.total_tokens) }}
            </div>

            <!-- Bar -->
            <div
              class="w-full rounded-t transition-all duration-300"
              :class="item.total_tokens > 0 ? 'bg-gradient-to-t from-indigo-600 to-cyan-400 group-hover:from-indigo-500 group-hover:to-cyan-300' : 'bg-slate-800/40'"
              :style="{ height: `${item.heightPct}%` }"
            ></div>
            <span class="text-[8px] text-slate-500 group-hover:text-slate-300 font-mono scale-90 -mx-1">
              {{ item.shortDate }}
            </span>
          </div>
        </div>
      </div>

      <!-- Table Section -->
      <div class="flex-1 min-h-0 overflow-y-auto px-4 py-2">
        <div v-if="historyList.length" class="rounded-xl border border-slate-800/80 overflow-hidden">
          <table class="w-full text-[11px] text-left border-collapse">
            <thead class="bg-slate-900/80 text-slate-400 border-b border-slate-800">
              <tr>
                <th class="py-2 px-2.5 font-medium">日期</th>
                <th class="py-2 px-2 font-medium text-right">总用量</th>
                <th class="py-2 px-2 font-medium text-right">输入 / 输出</th>
                <th class="py-2 px-2 font-medium text-right">缓存命中</th>
                <th class="py-2 px-2.5 font-medium text-right">请求数</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-slate-800/60 font-mono">
              <tr
                v-for="row in historyList"
                :key="row.date"
                class="hover:bg-slate-800/40 transition-colors"
                :class="{ 'opacity-60': row.total_tokens === 0 }"
              >
                <td class="py-2 px-2.5 text-slate-300 whitespace-nowrap">
                  {{ row.date }}
                </td>
                <td class="py-2 px-2 text-right font-bold" :class="row.total_tokens > 0 ? 'text-indigo-200' : 'text-slate-500'">
                  {{ formatTokens(row.total_tokens) }}
                </td>
                <td class="py-2 px-2 text-right text-[10px] text-slate-400 whitespace-nowrap">
                  {{ formatTokens(row.input_tokens) }} / {{ formatTokens(row.output_tokens) }}
                </td>
                <td class="py-2 px-2 text-right text-emerald-400/90 whitespace-nowrap">
                  {{ row.cache_hit_tokens > 0 ? formatTokens(row.cache_hit_tokens) : '--' }}
                </td>
                <td class="py-2 px-2.5 text-right text-slate-400">
                  {{ row.request_count > 0 ? row.request_count : '--' }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <div v-else class="h-full flex flex-col items-center justify-center text-xs text-slate-500 py-12 gap-2">
          <Calendar class="w-6 h-6 text-slate-600" />
          <span>暂无历史明细记录</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
@keyframes slideUp {
  from {
    transform: translateY(100%);
  }
  to {
    transform: translateY(0);
  }
}

.animate-slide-up {
  animation: slideUp 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

.animate-fade-in {
  animation: fadeIn 0.2s ease-out;
}

@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}
</style>
