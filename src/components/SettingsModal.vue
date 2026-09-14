<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { EnvironmentStatus, UpdateInfo, ProviderType } from '../types';
import { ArrowLeft, RefreshCw, Download, CheckCircle2, AlertCircle, Power, ExternalLink, ChevronDown } from 'lucide-vue-next';
import { openUrl } from '@tauri-apps/plugin-opener';
import UpdateModal from './UpdateModal.vue';

const props = defineProps<{
  envStatus: EnvironmentStatus | null;
  refreshInterval: number;
  showPercentageInTray: boolean;
  initialUpdateInfo?: UpdateInfo | null;
  activeProvider: ProviderType;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'update-interval', val: number): void;
  (e: 'update-tray-mode', val: boolean): void;
  (e: 'update-tray-target', val: ProviderType | 'auto'): void;
  (e: 're-login'): void;
  (e: 'provider-token-updated'): void;
}>();

const isCheckingUpdate = ref(false);
const updateResult = ref<UpdateInfo | null>(props.initialUpdateInfo || null);
const updateError = ref('');
const isFloatOpen = ref(false);
const showUpdateModal = ref(false);
const appVersion = computed(() => updateResult.value?.current_version || props.initialUpdateInfo?.current_version || '0.2.10');

// Tray Target Provider selection
const trayTarget = ref<ProviderType | 'auto'>(
  (localStorage.getItem('arkbar_tray_target') as ProviderType | 'auto') || 'volcengine'
);

// Desktop Floating Widget Primary Provider
const floatPrimaryProvider = ref<ProviderType>(
  (localStorage.getItem('arkbar_float_primary_provider') as ProviderType) ||
  (localStorage.getItem('arkbar_float_provider') as ProviderType) ||
  'volcengine'
);

function handleFloatPrimaryChange(e: Event) {
  const target = (e.target as HTMLSelectElement).value as ProviderType;
  floatPrimaryProvider.value = target;
  localStorage.setItem('arkbar_float_primary_provider', target);
  localStorage.setItem('arkbar_float_provider', target);
  window.dispatchEvent(new StorageEvent('storage', {
    key: 'arkbar_float_primary_provider',
    newValue: target,
  }));
}

async function toggleFloatWindow() {
  if (isFloatOpen.value) {
    await invoke('close_float_window');
    isFloatOpen.value = false;
  } else {
    await invoke('open_float_window');
    isFloatOpen.value = true;
  }
}

function handleTrayTargetChange(e: Event) {
  const target = (e.target as HTMLSelectElement).value as ProviderType | 'auto';
  trayTarget.value = target;
  localStorage.setItem('arkbar_tray_target', target);
  emit('update-tray-target', target);
}

async function handleCheckUpdate() {
  isCheckingUpdate.value = true;
  updateError.value = '';
  try {
    const info = await invoke<UpdateInfo>('check_for_updates');
    updateResult.value = info;
  } catch (err: any) {
    updateError.value = `检查更新失败: ${err}`;
  } finally {
    isCheckingUpdate.value = false;
  }
}

async function openReleaseUrl() {
  if (updateResult.value?.release_url) {
    try {
      await openUrl(updateResult.value.release_url);
    } catch {
      window.open(updateResult.value.release_url, '_blank');
    }
  }
}

async function handleQuit() {
  await invoke('hide_window');
  window.close();
}

onMounted(async () => {
  try {
    isFloatOpen.value = await invoke<boolean>('is_float_window_open');
  } catch {}
});
</script>

<template>
  <div class="flex flex-col h-full select-none text-slate-200 bg-[#0e131f]/95">
    <!-- Top Header（与主面板同规格 chrome） -->
    <div class="h-11 px-4 shrink-0 bg-[#141b2d]/90 border-b border-slate-800/80 flex items-center gap-2">
      <button
        @click="$emit('close')"
        class="w-7 h-7 rounded-lg grid place-items-center text-slate-400 hover:text-white hover:bg-slate-800/80 transition-colors"
      >
        <ArrowLeft class="w-4 h-4" />
      </button>
      <h3 class="text-[13px] font-bold text-white">设置</h3>
      <span class="ml-auto inline-flex items-center h-[18px] px-2 rounded-full text-[10px] font-mono bg-slate-800/80 border border-slate-700/60 text-slate-400">
        v{{ appVersion }}
      </span>
    </div>

    <!-- Content Sections -->
    <div class="flex-1 overflow-y-auto px-4 py-4 flex flex-col gap-4 text-xs">
      <!-- 1. Multi-Provider Integration -->
      <section>
        <div class="flex items-center justify-between px-0.5 mb-2">
          <span class="text-[11px] font-semibold text-slate-400">多模型服务商集成</span>
          <span class="text-[10px] text-emerald-400">⚡ 本地凭证直读</span>
        </div>
        <div class="rounded-xl border border-slate-800/60 bg-[#131a2a] divide-y divide-slate-800/60 overflow-hidden">
          <!-- Volcengine -->
          <div class="px-4 py-3 flex items-center gap-3">
            <span class="w-6 h-6 rounded-lg bg-slate-800/80 grid place-items-center text-xs shrink-0">🌋</span>
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">火山方舟 Coding Plan</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">ArkCLI 登录态与席位配额</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end gap-1.5">
              <span v-if="envStatus?.logged_in" class="flex items-center gap-1 text-[10px] font-mono text-slate-300 min-w-0" :title="envStatus.user_name || ''">
                <CheckCircle2 class="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                <span class="truncate">{{ envStatus.user_name || '已登录' }}</span>
              </span>
              <button
                v-else
                @click="$emit('re-login')"
                class="w-full h-7 rounded-lg bg-sky-600/80 hover:bg-sky-500 text-white text-[11px] font-medium transition-colors cursor-pointer whitespace-nowrap"
              >
                前往登录
              </button>
            </div>
          </div>
          <!-- xAI Grok -->
          <div class="px-4 py-3 flex items-center gap-3">
            <span class="w-6 h-6 rounded-lg bg-slate-800/80 grid place-items-center text-xs shrink-0">⚡</span>
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">xAI Grok</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">自动识别，无需配置</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <span class="text-[10px] font-mono text-slate-500 text-right truncate" title="自动识别 ~/.grok/auth.json">读取 ~/.grok</span>
            </div>
          </div>
          <!-- Google Antigravity -->
          <div class="px-4 py-3 flex items-center gap-3">
            <span class="w-6 h-6 rounded-lg bg-slate-800/80 grid place-items-center text-xs shrink-0">🌐</span>
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">Google Antigravity</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">自动集成，无需配置</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <span class="text-[10px] font-mono text-slate-500 text-right truncate" title="自动集成 agy CLI">agy CLI</span>
            </div>
          </div>
          <!-- OpenAI Codex -->
          <div class="px-4 py-3 flex items-center gap-3">
            <span class="w-6 h-6 rounded-lg bg-slate-800/80 grid place-items-center text-xs shrink-0">🤖</span>
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">OpenAI Codex</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">自动识别，无需配置</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <span class="text-[10px] font-mono text-slate-500 text-right truncate" title="自动识别 ~/.codex/auth.json">读取 ~/.codex</span>
            </div>
          </div>
        </div>
      </section>

      <!-- 2. Desktop Display（悬浮窗 + 托盘合并） -->
      <section>
        <div class="flex items-center justify-between px-0.5 mb-2">
          <span class="text-[11px] font-semibold text-slate-400">桌面展示</span>
        </div>
        <div class="rounded-xl border border-slate-800/60 bg-[#131a2a] divide-y divide-slate-800/60 overflow-hidden">
          <!-- Float window toggle -->
          <div class="px-4 py-3 flex items-center gap-3">
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">悬浮监控框</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">常驻桌面顶层，支持拖拽</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <button
                @click="toggleFloatWindow"
                class="w-9 h-5 rounded-full p-0.5 border transition-colors duration-200 cursor-pointer"
                :class="isFloatOpen ? 'bg-indigo-600 border-indigo-500' : 'bg-slate-800 border-slate-700'"
                :title="isFloatOpen ? '关闭悬浮监控框' : '开启悬浮监控框'"
              >
                <div
                  class="w-3.5 h-3.5 rounded-full bg-white shadow-sm transition-transform duration-200"
                  :class="{ 'translate-x-4': isFloatOpen }"
                ></div>
              </button>
            </div>
          </div>
          <!-- Float primary provider -->
          <div class="px-4 py-3 flex items-center gap-3">
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">悬浮框首选厂商</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">悬停悬浮框展开全部厂商</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <div class="relative">
                <select
                  :value="floatPrimaryProvider"
                  @change="handleFloatPrimaryChange"
                  class="h-7 w-[128px] pl-2.5 pr-6 rounded-lg bg-slate-900/70 border border-slate-800/60 text-xs text-slate-200 truncate cursor-pointer outline-none hover:border-slate-700/60 focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/30 appearance-none"
                >
                  <option value="volcengine">火山方舟</option>
                  <option value="grok">xAI Grok</option>
                  <option value="antigravity">Antigravity</option>
                  <option value="codex">Codex</option>
                </select>
                <ChevronDown class="w-3 h-3 absolute right-2 top-1/2 -translate-y-1/2 text-slate-500 pointer-events-none" />
              </div>
            </div>
          </div>
          <!-- Tray target -->
          <div class="px-4 py-3 flex items-center gap-3">
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">菜单栏监控目标</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">菜单栏标题显示的配额指标</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <div class="relative">
                <select
                  :value="trayTarget"
                  @change="handleTrayTargetChange"
                  class="h-7 w-[128px] pl-2.5 pr-6 rounded-lg bg-slate-900/70 border border-slate-800/60 text-xs text-slate-200 truncate cursor-pointer outline-none hover:border-slate-700/60 focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/30 appearance-none"
                >
                  <option value="volcengine">火山方舟</option>
                  <option value="antigravity">Antigravity</option>
                  <option value="grok">Grok</option>
                  <option value="codex">Codex</option>
                  <option value="auto">跟随当前</option>
                </select>
                <ChevronDown class="w-3 h-3 absolute right-2 top-1/2 -translate-y-1/2 text-slate-500 pointer-events-none" />
              </div>
            </div>
          </div>
          <!-- Tray percentage toggle -->
          <div class="px-4 py-3 flex items-center gap-3">
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">显示周期百分比</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">状态栏图标旁展示数字</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <button
                @click="$emit('update-tray-mode', !showPercentageInTray)"
                class="w-9 h-5 rounded-full p-0.5 border transition-colors duration-200 cursor-pointer"
                :class="showPercentageInTray ? 'bg-indigo-600 border-indigo-500' : 'bg-slate-800 border-slate-700'"
              >
                <div
                  class="w-3.5 h-3.5 rounded-full bg-white shadow-sm transition-transform duration-200"
                  :class="{ 'translate-x-4': showPercentageInTray }"
                ></div>
              </button>
            </div>
          </div>
        </div>
      </section>

      <!-- 3. Data Sync -->
      <section>
        <div class="flex items-center justify-between px-0.5 mb-2">
          <span class="text-[11px] font-semibold text-slate-400">数据同步</span>
        </div>
        <div class="rounded-xl border border-slate-800/60 bg-[#131a2a] divide-y divide-slate-800/60 overflow-hidden">
          <div class="px-4 py-3 flex items-center gap-3">
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">自动刷新频率</p>
              <p class="text-[11px] text-slate-500 leading-4 mt-0.5 truncate">后台静默同步各平台配额</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <div class="relative">
                <select
                  :value="refreshInterval"
                  @change="$emit('update-interval', Number(($event.target as HTMLSelectElement).value))"
                  class="h-7 w-[104px] pl-2.5 pr-6 rounded-lg bg-slate-900/70 border border-slate-800/60 text-xs text-slate-200 truncate cursor-pointer outline-none hover:border-slate-700/60 focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/30 appearance-none"
                >
                  <option :value="5">5 分钟</option>
                  <option :value="10">10 分钟</option>
                  <option :value="15">15 分钟</option>
                  <option :value="30">30 分钟</option>
                  <option :value="60">1 小时</option>
                  <option :value="0">仅手动刷新</option>
                </select>
                <ChevronDown class="w-3 h-3 absolute right-2 top-1/2 -translate-y-1/2 text-slate-500 pointer-events-none" />
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- 4. Version & Update -->
      <section>
        <div class="flex items-center justify-between px-0.5 mb-2">
          <span class="text-[11px] font-semibold text-slate-400">版本与更新</span>
        </div>
        <div class="rounded-xl border border-slate-800/60 bg-[#131a2a] divide-y divide-slate-800/60 overflow-hidden">
          <div class="px-4 py-3 flex items-center gap-3">
            <div class="flex-1 min-w-0">
              <p class="text-xs font-medium text-white leading-5">检查更新</p>
              <p class="text-[11px] font-mono text-slate-500 leading-4 mt-0.5 truncate">GitHub · chzisnull/ark-bar</p>
            </div>
            <div class="w-[128px] h-7 shrink-0 flex items-center justify-end">
              <button
                @click="handleCheckUpdate"
                :disabled="isCheckingUpdate"
                class="h-7 px-2.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] text-slate-200 flex items-center gap-1 transition-colors whitespace-nowrap disabled:opacity-60"
              >
                <RefreshCw class="w-3 h-3" :class="{ 'animate-spin': isCheckingUpdate }" />
                <span>{{ isCheckingUpdate ? '检查中...' : '检查更新' }}</span>
              </button>
            </div>
          </div>

          <div v-if="updateResult && updateResult.has_update" class="px-4 py-3">
            <div class="rounded-lg bg-amber-500/10 border border-amber-500/30 p-2.5 space-y-2">
              <div class="flex items-center justify-between gap-2 text-amber-300">
                <span class="flex items-center gap-1.5 font-bold text-xs whitespace-nowrap">
                  <AlertCircle class="w-3.5 h-3.5 text-amber-400 shrink-0" />
                  发现新版本 v{{ updateResult.latest_version }}
                </span>
                <span class="text-[10px] font-mono text-amber-400/80 whitespace-nowrap">
                  当前 v{{ updateResult.current_version }}
                </span>
              </div>
              <p v-if="updateResult.release_notes" class="text-[11px] text-slate-300 line-clamp-2 leading-relaxed">
                {{ updateResult.release_notes }}
              </p>
              <div class="pt-0.5 flex items-center justify-end gap-1.5">
                <button
                  @click="openReleaseUrl"
                  class="h-7 px-2 bg-slate-800 hover:bg-slate-700 text-slate-300 font-medium rounded-lg text-[11px] flex items-center gap-1 transition-colors whitespace-nowrap"
                >
                  <ExternalLink class="w-2.5 h-2.5 text-slate-400" />
                  网页下载
                </button>
                <button
                  @click="showUpdateModal = true"
                  class="h-7 px-2.5 bg-gradient-to-r from-amber-500 to-orange-500 hover:from-amber-400 hover:to-orange-400 text-slate-950 font-bold rounded-lg text-[11px] flex items-center gap-1 transition shadow-sm whitespace-nowrap"
                >
                  <Download class="w-2.5 h-2.5" />
                  立即在线更新
                </button>
              </div>
            </div>
          </div>
          <div v-else-if="updateResult" class="px-4 py-2.5 flex items-center gap-1.5 text-[11px] text-emerald-400">
            <CheckCircle2 class="w-3.5 h-3.5 shrink-0" />
            <span>当前已是最新版本 (v{{ updateResult.current_version }})</span>
          </div>

          <div v-if="updateError" class="px-4 py-2.5 text-[11px] text-rose-400">
            {{ updateError }}
          </div>
        </div>
      </section>

      <!-- 5. Quit Application -->
      <button
        @click="handleQuit"
        class="mt-1 w-full h-9 rounded-xl border border-rose-500/30 bg-rose-500/10 hover:bg-rose-500/20 text-rose-400 text-xs font-medium flex items-center justify-center gap-1.5 transition-colors"
      >
        <Power class="w-3.5 h-3.5" />
        <span>退出 ArkBar</span>
      </button>
    </div>

    <!-- In-App Online Update Modal -->
    <UpdateModal
      v-if="showUpdateModal && updateResult"
      :update-info="updateResult"
      @close="showUpdateModal = false"
    />
  </div>
</template>
