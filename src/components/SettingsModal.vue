<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { EnvironmentStatus, UpdateInfo, ProviderType } from '../types';
import { ArrowLeft, RefreshCw, Download, CheckCircle2, AlertCircle, Power, ExternalLink } from 'lucide-vue-next';
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
const appVersion = computed(() => updateResult.value?.current_version || props.initialUpdateInfo?.current_version || '0.2.8');

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
    <!-- Top Header -->
    <div class="px-4 py-3 bg-[#141b2d]/80 border-b border-slate-800/80 flex items-center justify-between">
      <div class="flex items-center space-x-2">
        <button
          @click="$emit('close')"
          class="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800/80 transition"
        >
          <ArrowLeft class="w-4 h-4" />
        </button>
        <h3 class="text-xs font-bold text-white tracking-wide">偏好设置与多服务商</h3>
      </div>
      <span class="text-[10px] font-mono text-slate-500">v{{ appVersion }}</span>
    </div>

    <!-- Content Sections -->
    <div class="p-4 flex-1 overflow-y-auto space-y-4 text-xs">
      <!-- 1. Multi-Provider Auto-Detection Overview -->
      <div class="space-y-2">
        <div class="flex items-center justify-between">
          <label class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider">多模型服务商集成</label>
          <span class="text-[10px] text-emerald-400 font-mono">⚡ 自动读取本地凭证</span>
        </div>

        <div class="p-3 bg-[#131a2a]/70 rounded-xl border border-slate-800/70 space-y-2.5">
          <!-- A. Volcengine -->
          <div class="flex items-center justify-between text-[11px] pb-2 border-b border-slate-800/60">
            <div class="flex items-center gap-1.5 font-medium text-slate-200">
              <span>🌋</span>
              <span>火山方舟 Coding Plan</span>
            </div>
            <div class="flex items-center gap-1.5">
              <span v-if="envStatus?.logged_in" class="text-emerald-400 font-mono text-[10px] flex items-center gap-1">
                <CheckCircle2 class="w-3 h-3 text-emerald-400" />
                <span>已登录{{ envStatus.user_name ? ` (${envStatus.user_name})` : '' }}</span>
              </span>
              <button
                v-else
                @click="$emit('re-login')"
                class="px-2 py-0.5 rounded bg-sky-600/80 hover:bg-sky-500 text-white text-[10px] transition cursor-pointer"
              >
                前往登录
              </button>
            </div>
          </div>

          <!-- B. xAI Grok -->
          <div class="flex items-center justify-between text-[11px] pb-2 border-b border-slate-800/60">
            <div class="flex items-center gap-1.5 font-medium text-slate-200">
              <span>⚡</span>
              <span>xAI Grok</span>
            </div>
            <span class="text-slate-400 font-mono text-[10px]">自动识别 ~/.grok/auth.json</span>
          </div>

          <!-- C. Google Antigravity -->
          <div class="flex items-center justify-between text-[11px] pb-2 border-b border-slate-800/60">
            <div class="flex items-center gap-1.5 font-medium text-slate-200">
              <span>🌐</span>
              <span>Google Antigravity</span>
            </div>
            <span class="text-slate-400 font-mono text-[10px]">自动集成 agy CLI</span>
          </div>

          <!-- D. OpenAI Codex -->
          <div class="flex items-center justify-between text-[11px]">
            <div class="flex items-center gap-1.5 font-medium text-slate-200">
              <span>🤖</span>
              <span>OpenAI Codex</span>
            </div>
            <span class="text-slate-400 font-mono text-[10px]">自动识别 ~/.codex/auth.json</span>
          </div>
        </div>
      </div>

      <!-- 2. Desktop Floating Widget Settings -->
      <div class="space-y-2">
        <label class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider">桌面悬浮监控框</label>
        <div class="p-3 bg-[#131a2a]/70 rounded-xl border border-slate-800/70 space-y-3">
          <!-- Toggle float window -->
          <div class="flex items-center justify-between">
            <div>
              <p class="font-medium text-white text-xs">开启桌面悬浮监控框</p>
              <p class="text-[10px] text-slate-400">常驻桌面顶层，极简胶囊卡片，支持任意拖拽摆放</p>
            </div>
            <button
              @click="toggleFloatWindow"
              class="px-2.5 py-1 rounded-lg text-xs font-medium transition cursor-pointer"
              :class="isFloatOpen ? 'bg-indigo-600 text-white shadow-sm' : 'bg-slate-800 text-slate-300 hover:bg-slate-700'"
            >
              {{ isFloatOpen ? '已开启' : '开启' }}
            </button>
          </div>

          <!-- Preferred first provider -->
          <div class="flex items-center justify-between pt-2 border-t border-slate-800/60">
            <div>
              <p class="font-medium text-white text-xs">默认第一位展示厂商</p>
              <p class="text-[10px] text-slate-400">悬浮框常驻展示该厂商，鼠标悬停展开所有已授权厂商</p>
            </div>
            <select
              :value="floatPrimaryProvider"
              @change="handleFloatPrimaryChange"
              class="bg-slate-800 border border-slate-700 text-slate-200 text-xs rounded-lg px-2 py-1 outline-none focus:border-indigo-500 font-medium cursor-pointer"
            >
              <option value="volcengine">🌋 火山方舟 (默认)</option>
              <option value="grok">⚡ xAI Grok</option>
              <option value="antigravity">🌐 Google Antigravity</option>
              <option value="codex">🤖 OpenAI Codex</option>
            </select>
          </div>
        </div>
      </div>

      <!-- 3. Tray Display Settings -->
      <div class="space-y-2">
        <label class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider">系统托盘显示</label>
        <div class="p-3 bg-[#131a2a]/70 rounded-xl border border-slate-800/70 space-y-2.5">
          <div class="flex items-center justify-between">
            <div>
              <div class="text-xs font-medium text-white">托盘栏监控目标</div>
              <div class="text-[10px] text-slate-400">选择菜单栏标题显示的额度指标</div>
            </div>
            <select
              :value="trayTarget"
              @change="handleTrayTargetChange"
              class="bg-slate-800 text-slate-200 border border-slate-700 rounded-lg px-2 py-1 text-xs focus:outline-none focus:border-indigo-500 cursor-pointer"
            >
              <option value="volcengine">🌋 火山方舟 5小时用量 (推荐默认)</option>
              <option value="antigravity">🌐 Antigravity 5小时用量</option>
              <option value="grok">⚡ xAI Grok (周度配额)</option>
              <option value="codex">🤖 OpenAI Codex 5小时用量</option>
              <option value="auto">🔄 跟随当前查看切换</option>
            </select>
          </div>

          <div class="pt-2 border-t border-slate-800/80 flex items-center justify-between">
            <div>
              <div class="text-xs font-medium text-white">显示近5小时/周期百分比</div>
              <div class="text-[10px] text-slate-400">在顶部状态栏图标旁展示数字</div>
            </div>
            <button
              @click="$emit('update-tray-mode', !showPercentageInTray)"
              class="w-9 h-5 rounded-full transition duration-200 relative p-0.5 border cursor-pointer"
              :class="showPercentageInTray ? 'bg-indigo-600 border-indigo-500' : 'bg-slate-800 border-slate-700'"
            >
              <div
                class="w-3.5 h-3.5 rounded-full bg-white transition duration-200 shadow-sm"
                :class="{ 'translate-x-4': showPercentageInTray }"
              ></div>
            </button>
          </div>
        </div>
      </div>

      <!-- 4. Refresh Interval -->
      <div class="space-y-2">
        <label class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider">数据更新设置</label>
        <div class="p-3 bg-[#131a2a]/70 rounded-xl border border-slate-800/70 flex items-center justify-between">
          <div>
            <div class="text-xs font-medium text-white">后台自动检测频率</div>
            <div class="text-[10px] text-slate-400">定时静默同步各大平台配额</div>
          </div>
          <select
            :value="refreshInterval"
            @change="$emit('update-interval', Number(($event.target as HTMLSelectElement).value))"
            class="bg-slate-800 text-slate-200 border border-slate-700 rounded-lg px-2 py-1 text-xs focus:outline-none focus:border-indigo-500"
          >
            <option :value="5">5 分钟</option>
            <option :value="10">10 分钟</option>
            <option :value="15">15 分钟</option>
            <option :value="30">30 分钟</option>
            <option :value="60">1 小时</option>
            <option :value="0">仅手动刷新</option>
          </select>
        </div>
      </div>

      <!-- 5. Update Check -->
      <div class="space-y-2">
        <label class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider">版本与更新</label>
        <div class="p-3 bg-[#131a2a]/70 rounded-xl border border-slate-800/70 space-y-2.5">
          <div class="flex items-center justify-between">
            <div>
              <div class="text-xs font-medium text-white">检查 GitHub Releases</div>
              <div class="text-[10px] text-slate-400">仓库: chzisnull/ark-bar</div>
            </div>
            <button
              @click="handleCheckUpdate"
              :disabled="isCheckingUpdate"
              class="px-2.5 py-1 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-lg text-xs flex items-center gap-1 transition"
            >
              <RefreshCw class="w-3 h-3" :class="{ 'animate-spin': isCheckingUpdate }" />
              <span>{{ isCheckingUpdate ? '检查中...' : '检查更新' }}</span>
            </button>
          </div>

          <div v-if="updateResult" class="pt-2 border-t border-slate-700/60">
            <div v-if="updateResult.has_update" class="p-2.5 bg-amber-500/10 border border-amber-500/30 rounded-xl space-y-2">
              <div class="flex items-center justify-between text-amber-300 font-medium">
                <span class="flex items-center gap-1.5 font-bold text-xs">
                  <AlertCircle class="w-3.5 h-3.5 text-amber-400" />
                  发现新版本 v{{ updateResult.latest_version }}
                </span>
                <span class="text-[10px] text-amber-400/80 font-mono">
                  (当前: v{{ updateResult.current_version }})
                </span>
              </div>
              <p v-if="updateResult.release_notes" class="text-[10px] text-slate-300 line-clamp-2 leading-relaxed">
                {{ updateResult.release_notes }}
              </p>
              <div class="pt-1 flex items-center justify-end gap-1.5">
                <button
                  @click="openReleaseUrl"
                  class="px-2 py-1 bg-slate-800 hover:bg-slate-700 text-slate-300 font-medium rounded-lg text-[10px] flex items-center gap-1 transition"
                >
                  <ExternalLink class="w-2.5 h-2.5 text-slate-400" />
                  网页下载
                </button>
                <button
                  @click="showUpdateModal = true"
                  class="px-2.5 py-1 bg-gradient-to-r from-amber-500 to-orange-500 hover:from-amber-400 hover:to-orange-400 text-slate-950 font-bold rounded-lg text-[10px] flex items-center gap-1 transition shadow-sm"
                >
                  <Download class="w-2.5 h-2.5" />
                  立即在线更新
                </button>
              </div>
            </div>
            <div v-else class="flex items-center gap-1.5 text-emerald-400 text-[11px]">
              <CheckCircle2 class="w-3.5 h-3.5" />
              <span>当前已是最新版本 (v{{ updateResult.current_version }})</span>
            </div>
          </div>

          <div v-if="updateError" class="text-[10px] text-rose-400">
            {{ updateError }}
          </div>
        </div>
      </div>

      <!-- 6. Quit Application -->
      <div class="pt-2">
        <button
          @click="handleQuit"
          class="w-full py-2 bg-rose-500/10 hover:bg-rose-500/20 text-rose-400 border border-rose-500/30 rounded-xl text-xs font-medium flex items-center justify-center gap-1.5 transition"
        >
          <Power class="w-3.5 h-3.5" />
          <span>退出 ArkBar</span>
        </button>
      </div>
    </div>

    <!-- In-App Online Update Modal -->
    <UpdateModal
      v-if="showUpdateModal && updateResult"
      :update-info="updateResult"
      @close="showUpdateModal = false"
    />
  </div>
</template>
