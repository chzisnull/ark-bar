<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { EnvironmentStatus, UpdateInfo, ProviderType } from '../types';
import { ArrowLeft, RefreshCw, Download, CheckCircle2, AlertCircle, Power, Layout, ExternalLink, Eye, EyeOff, Save, Check } from 'lucide-vue-next';
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
const appVersion = computed(() => updateResult.value?.current_version || props.initialUpdateInfo?.current_version || '0.2.2');

// Tray Target Provider selection
const trayTarget = ref<ProviderType | 'auto'>(
  (localStorage.getItem('arkbar_tray_target') as ProviderType | 'auto') || 'auto'
);

// Custom Provider Tokens
const grokToken = ref('');
const codexToken = ref('');
const showGrokToken = ref(false);
const showCodexToken = ref(false);
const grokSavedSuccess = ref(false);
const codexSavedSuccess = ref(false);

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

async function loadTokens() {
  try {
    const gTok = await invoke<string | null>('read_provider_token', { provider: 'grok' });
    if (gTok) grokToken.value = gTok;
    const cTok = await invoke<string | null>('read_provider_token', { provider: 'codex' });
    if (cTok) codexToken.value = cTok;
  } catch (err) {
    console.debug('Failed to read provider tokens:', err);
  }
}

async function saveGrokToken() {
  try {
    await invoke('set_provider_token', { provider: 'grok', token: grokToken.value });
    grokSavedSuccess.value = true;
    setTimeout(() => { grokSavedSuccess.value = false; }, 2000);
    emit('provider-token-updated');
  } catch (err) {
    console.error('Save grok token failed:', err);
  }
}

async function saveCodexToken() {
  try {
    await invoke('set_provider_token', { provider: 'codex', token: codexToken.value });
    codexSavedSuccess.value = true;
    setTimeout(() => { codexSavedSuccess.value = false; }, 2000);
    emit('provider-token-updated');
  } catch (err) {
    console.error('Save codex token failed:', err);
  }
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
  await loadTokens();
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
      <!-- 1. Multi-Provider Tokens & Credentials -->
      <div class="space-y-2">
        <label class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider">服务商授权与令牌</label>

        <!-- A. xAI Grok Token -->
        <div class="p-3 bg-[#131a2a]/70 rounded-xl border border-slate-800/70 space-y-2">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-1.5 font-medium text-white">
              <span>⚡</span>
              <span>xAI Grok 令牌</span>
            </div>
            <span class="text-[10px] text-slate-500">自动读取 ~/.grok/auth.json</span>
          </div>
          <div class="flex items-center gap-1.5">
            <div class="relative flex-1">
              <input
                :type="showGrokToken ? 'text' : 'password'"
                v-model="grokToken"
                placeholder="优先读取本地 auth.json，亦可输入自定义 Token"
                class="w-full bg-slate-900/90 border border-slate-700/80 rounded-lg px-2.5 py-1 text-[11px] text-slate-200 placeholder-slate-600 focus:outline-none focus:border-indigo-500 pr-7 font-mono"
              />
              <button
                type="button"
                @click="showGrokToken = !showGrokToken"
                class="absolute right-2 top-1/2 -translate-y-1/2 text-slate-500 hover:text-slate-300"
              >
                <EyeOff v-if="showGrokToken" class="w-3.5 h-3.5" />
                <Eye v-else class="w-3.5 h-3.5" />
              </button>
            </div>
            <button
              @click="saveGrokToken"
              class="px-2.5 py-1 rounded-lg text-xs font-medium flex items-center gap-1 transition"
              :class="grokSavedSuccess ? 'bg-emerald-600 text-white' : 'bg-indigo-600 hover:bg-indigo-500 text-white'"
            >
              <Check v-if="grokSavedSuccess" class="w-3 h-3" />
              <Save v-else class="w-3 h-3" />
              <span>{{ grokSavedSuccess ? '已保存' : '保存' }}</span>
            </button>
          </div>
        </div>

        <!-- B. OpenAI Codex Token -->
        <div class="p-3 bg-[#131a2a]/70 rounded-xl border border-slate-800/70 space-y-2">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-1.5 font-medium text-white">
              <span>🤖</span>
              <span>OpenAI Codex 令牌</span>
            </div>
            <span class="text-[10px] text-slate-500">自动读取 ~/.codex/auth.json</span>
          </div>
          <div class="flex items-center gap-1.5">
            <div class="relative flex-1">
              <input
                :type="showCodexToken ? 'text' : 'password'"
                v-model="codexToken"
                placeholder="ChatGPT Session Token 或 OpenAI API Key (sk-...)"
                class="w-full bg-slate-900/90 border border-slate-700/80 rounded-lg px-2.5 py-1 text-[11px] text-slate-200 placeholder-slate-600 focus:outline-none focus:border-indigo-500 pr-7 font-mono"
              />
              <button
                type="button"
                @click="showCodexToken = !showCodexToken"
                class="absolute right-2 top-1/2 -translate-y-1/2 text-slate-500 hover:text-slate-300"
              >
                <EyeOff v-if="showCodexToken" class="w-3.5 h-3.5" />
                <Eye v-else class="w-3.5 h-3.5" />
              </button>
            </div>
            <button
              @click="saveCodexToken"
              class="px-2.5 py-1 rounded-lg text-xs font-medium flex items-center gap-1 transition"
              :class="codexSavedSuccess ? 'bg-emerald-600 text-white' : 'bg-indigo-600 hover:bg-indigo-500 text-white'"
            >
              <Check v-if="codexSavedSuccess" class="w-3 h-3" />
              <Save v-else class="w-3 h-3" />
              <span>{{ codexSavedSuccess ? '已保存' : '保存' }}</span>
            </button>
          </div>
        </div>

        <!-- C. Google Antigravity & Volcengine Status -->
        <div class="p-3 bg-[#131a2a]/70 rounded-xl border border-slate-800/70 space-y-2">
          <div class="flex items-center justify-between text-[11px]">
            <span class="text-slate-400">🌐 Antigravity CLI:</span>
            <span class="text-emerald-400 font-mono">已集成 (agy /usage)</span>
          </div>
          <div class="flex items-center justify-between text-[11px]">
            <span class="text-slate-400">🌋 火山方舟状态:</span>
            <span class="text-slate-300 font-mono">{{ envStatus?.logged_in ? '已登录' : '未登录' }}</span>
          </div>
          <div v-if="envStatus?.user_name" class="flex items-center justify-between text-[11px]">
            <span class="text-slate-400">火山方舟账号:</span>
            <span class="text-slate-300 font-mono">{{ envStatus.user_name }}</span>
          </div>
          <div class="pt-1 flex justify-end">
            <button
              @click="$emit('re-login')"
              class="text-[10px] text-sky-400 hover:text-sky-300 flex items-center gap-1"
            >
              <span>重新进行火山 SSO 授权</span>
            </button>
          </div>
        </div>
      </div>

      <!-- 2. Tray Display Settings -->
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
              class="bg-slate-800 text-slate-200 border border-slate-700 rounded-lg px-2 py-1 text-xs focus:outline-none focus:border-indigo-500"
            >
              <option value="auto">🔄 跟随当前查看切换</option>
              <option value="volcengine">🌋 火山方舟 (5小时)</option>
              <option value="antigravity">🌐 Antigravity (5小时)</option>
              <option value="grok">⚡ xAI Grok (周度配额)</option>
              <option value="codex">🤖 OpenAI Codex (5小时)</option>
            </select>
          </div>

          <div class="pt-2 border-t border-slate-800/80 flex items-center justify-between">
            <div>
              <div class="text-xs font-medium text-white">显示近5小时/周期百分比</div>
              <div class="text-[10px] text-slate-400">在顶部状态栏图标旁展示数字</div>
            </div>
            <button
              @click="$emit('update-tray-mode', !showPercentageInTray)"
              class="w-9 h-5 rounded-full transition duration-200 relative p-0.5 border"
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

      <!-- 3. Desktop Floating Widget -->
      <div class="space-y-2">
        <label class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider">桌面显示模式</label>
        <div class="p-3 bg-[#131a2a]/70 rounded-xl border border-slate-800/70 flex items-center justify-between">
          <div class="flex items-center space-x-2.5">
            <div class="w-7 h-7 rounded-lg bg-indigo-500/15 border border-indigo-500/30 flex items-center justify-center text-indigo-400">
              <Layout class="w-4 h-4" />
            </div>
            <div>
              <div class="text-xs font-medium text-white">桌面悬浮监控框</div>
              <div class="text-[10px] text-slate-400">极简悬浮胶囊，支持任意拖拽摆放</div>
            </div>
          </div>
          <button
            @click="toggleFloatWindow"
            class="px-2.5 py-1 rounded-lg text-xs font-medium transition"
            :class="isFloatOpen ? 'bg-indigo-600 text-white shadow-sm' : 'bg-slate-800 text-slate-300 hover:bg-slate-700'"
          >
            {{ isFloatOpen ? '已开启' : '开启' }}
          </button>
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
