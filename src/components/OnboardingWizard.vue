<script setup lang="ts">
import { ref, watch, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { EnvironmentStatus, CommandResult } from '../types';
import {
  ArrowLeft,
  CheckCircle2,
  XCircle,
  AlertTriangle,
  RefreshCw,
  Terminal,
  ExternalLink,
  ArrowRight,
  ShieldCheck,
} from 'lucide-vue-next';
import { openUrl } from '@tauri-apps/plugin-opener';

const props = defineProps<{
  status: EnvironmentStatus;
}>();

const emit = defineEmits<{
  (e: 'refresh'): void;
  (e: 'complete'): void;
  (e: 'back'): void;
  (e: 'env-updated', status: EnvironmentStatus): void;
}>();

const currentStatus = ref<EnvironmentStatus>({ ...props.status });

watch(
  () => props.status,
  (newVal) => {
    if (newVal) {
      currentStatus.value = { ...newVal };
    }
  },
  { deep: true }
);

const isInstalling = ref(false);
const isLoggingIn = ref(false);
const installLogs = ref('');
const actionMessage = ref('');
const errorMessage = ref('');

// Code login mode for robust cross-device / Windows fallback
const loginMode = ref<'browser' | 'code'>('browser');
const authCode = ref('');
const authUrl = ref('');
const isGettingAuthUrl = ref(false);
const isSubmittingCode = ref(false);

async function refreshEnvStatus(): Promise<EnvironmentStatus | null> {
  try {
    const res = await invoke<EnvironmentStatus>('check_environment');
    currentStatus.value = res;
    emit('env-updated', res);
    return res;
  } catch (err) {
    console.error('Failed to check environment:', err);
    return null;
  }
}

async function handleInstallArkCli() {
  isInstalling.value = true;
  actionMessage.value = '正在通过 npm 全局安装 @volcengine/ark-cli，请稍候...';
  errorMessage.value = '';
  installLogs.value = '';

  try {
    const result = await invoke<CommandResult>('install_arkcli');
    if (result.success) {
      installLogs.value = result.details || result.message;
      actionMessage.value = '正在重新检测环境状态...';
      const fresh = await refreshEnvStatus();
      if (fresh?.has_arkcli) {
        actionMessage.value = 'arkcli 安装成功并就绪！请完成下方第 3 步授权登录。';
      } else {
        actionMessage.value = 'arkcli 已安装，正在等待环境识别，可点击下方「重新检测」。';
      }
      emit('refresh');
    } else {
      errorMessage.value = result.message;
      installLogs.value = result.details || '';
    }
  } catch (err: any) {
    errorMessage.value = `执行安装出错: ${err}`;
  } finally {
    isInstalling.value = false;
  }
}

async function handleLoginSso() {
  isLoggingIn.value = true;
  actionMessage.value = '已唤起系统默认浏览器，请在浏览器中完成火山方舟 SSO 授权登录...';
  errorMessage.value = '';

  try {
    const result = await invoke<CommandResult>('login_volc_sso');
    if (result.success) {
      actionMessage.value = '登录授权成功！';
      await refreshEnvStatus();
      emit('refresh');
      setTimeout(() => {
        emit('complete');
      }, 800);
    } else {
      errorMessage.value = `${result.message}。若浏览器提示 redirect_uri 错误或未回调，请使用下方「授权码模式」登录。`;
      loginMode.value = 'code';
    }
  } catch (err: any) {
    errorMessage.value = `登录授权出错: ${err}。推荐切换为「授权码模式」登录。`;
    loginMode.value = 'code';
  } finally {
    isLoggingIn.value = false;
  }
}

async function handleOpenAuthUrl() {
  isGettingAuthUrl.value = true;
  errorMessage.value = '';
  actionMessage.value = '正在生成火山方舟 SSO 授权链接...';
  try {
    const url = await invoke<string>('get_sso_auth_url');
    authUrl.value = url;
    actionMessage.value = '已在浏览器打开授权页面，完成授权后请复制显示的授权码粘贴到下方。';
    try {
      await openUrl(url);
    } catch {
      window.open(url, '_blank');
    }
  } catch (err: any) {
    errorMessage.value = `获取授权链接失败: ${err}`;
  } finally {
    isGettingAuthUrl.value = false;
  }
}

async function handleLoginWithCode() {
  if (!authCode.value.trim()) {
    errorMessage.value = '请先粘贴浏览器中获取的授权码';
    return;
  }

  isSubmittingCode.value = true;
  errorMessage.value = '';
  actionMessage.value = '正在验证授权码并绑定火山方舟账号...';

  try {
    const result = await invoke<CommandResult>('login_with_code', { code: authCode.value });
    if (result.success) {
      actionMessage.value = '授权成功！正在刷新环境...';
      await refreshEnvStatus();
      emit('refresh');
      setTimeout(() => {
        emit('complete');
      }, 800);
    } else {
      errorMessage.value = result.message;
    }
  } catch (err: any) {
    errorMessage.value = `提交授权码出错: ${err}`;
  } finally {
    isSubmittingCode.value = false;
  }
}

async function handleManualRefresh() {
  actionMessage.value = '正在重新检测环境...';
  await refreshEnvStatus();
  emit('refresh');
  actionMessage.value = '';
}

function openNodeDownload() {
  window.open('https://nodejs.org/', '_blank');
}

onMounted(() => {
  refreshEnvStatus();
});
</script>

<template>
  <div class="p-5 flex flex-col h-full select-none text-slate-200">
    <!-- Header with Back Button -->
    <div class="flex items-center justify-between mb-4 pb-3 border-b border-slate-800">
      <div class="flex items-center space-x-2.5">
        <button
          @click="$emit('back')"
          class="p-1.5 -ml-1 text-slate-400 hover:text-white hover:bg-slate-800 rounded-lg transition cursor-pointer"
          title="返回监控面板"
        >
          <ArrowLeft class="w-4 h-4" />
        </button>
        <div class="w-8 h-8 rounded-xl bg-gradient-to-tr from-rose-500 to-amber-500 flex items-center justify-center text-white shadow-lg shadow-rose-500/20">
          <span class="text-base">🌋</span>
        </div>
        <div>
          <h2 class="text-sm font-semibold text-white tracking-wide">火山方舟环境配置</h2>
          <p class="text-[11px] text-slate-400">检测并初始化 arkcli 运行环境</p>
        </div>
      </div>
      <button
        @click="$emit('back')"
        class="text-xs text-slate-400 hover:text-white px-2 py-1 rounded hover:bg-slate-800 transition cursor-pointer"
      >
        返回面板
      </button>
    </div>

    <!-- Steps -->
    <div class="space-y-3 flex-1 overflow-y-auto pr-1">
      <!-- Step 1: Node.js -->
      <div
        class="p-3.5 rounded-xl border transition-all duration-200"
        :class="currentStatus.has_node ? 'bg-slate-800/40 border-emerald-500/30' : 'bg-rose-950/20 border-rose-500/30'"
      >
        <div class="flex items-start justify-between">
          <div class="flex items-center space-x-2.5">
            <CheckCircle2 v-if="currentStatus.has_node" class="w-5 h-5 text-emerald-400 flex-shrink-0" />
            <XCircle v-else class="w-5 h-5 text-rose-400 flex-shrink-0" />
            <div>
              <div class="text-sm font-medium text-white flex items-center gap-2">
                1. Node.js 环境
                <span
                  v-if="currentStatus.has_node"
                  class="text-[10px] bg-emerald-500/20 text-emerald-300 px-1.5 py-0.5 rounded font-mono"
                >
                  {{ currentStatus.node_version }}
                </span>
              </div>
              <p class="text-xs text-slate-400 mt-0.5">
                {{ currentStatus.has_node ? 'Node.js 运行环境已就绪' : '系统未检测到 Node.js，需先安装环境' }}
              </p>
            </div>
          </div>
          <button
            v-if="!currentStatus.has_node"
            @click="openNodeDownload"
            class="text-xs bg-slate-800 hover:bg-slate-700 text-sky-400 px-2.5 py-1 rounded-lg flex items-center gap-1 transition cursor-pointer"
          >
            <span>下载</span>
            <ExternalLink class="w-3 h-3" />
          </button>
        </div>
      </div>

      <!-- Step 2: arkcli CLI -->
      <div
        class="p-3.5 rounded-xl border transition-all duration-200"
        :class="currentStatus.has_arkcli ? 'bg-slate-800/40 border-emerald-500/30' : 'bg-slate-800/60 border-amber-500/30'"
      >
        <div class="flex items-start justify-between">
          <div class="flex items-center space-x-2.5">
            <CheckCircle2 v-if="currentStatus.has_arkcli" class="w-5 h-5 text-emerald-400 flex-shrink-0" />
            <AlertTriangle v-else class="w-5 h-5 text-amber-400 flex-shrink-0" />
            <div>
              <div class="text-sm font-medium text-white flex items-center gap-2">
                2. 官方 arkcli 工具
                <span
                  v-if="currentStatus.has_arkcli"
                  class="text-[10px] bg-emerald-500/20 text-emerald-300 px-1.5 py-0.5 rounded font-mono"
                >
                  {{ currentStatus.arkcli_version || '已就绪' }}
                </span>
              </div>
              <p class="text-xs text-slate-400 mt-0.5">
                {{ currentStatus.has_arkcli ? '已就绪 @volcengine/ark-cli' : '需要安装火山方舟官方命令行工具' }}
              </p>
            </div>
          </div>
        </div>

        <div v-if="!currentStatus.has_arkcli && currentStatus.has_node" class="mt-3 pt-2.5 border-t border-slate-800">
          <button
            @click="handleInstallArkCli"
            :disabled="isInstalling"
            class="w-full py-1.5 px-3 bg-amber-500/20 hover:bg-amber-500/30 text-amber-300 border border-amber-500/40 rounded-lg text-xs font-medium flex items-center justify-center gap-1.5 transition cursor-pointer"
          >
            <RefreshCw v-if="isInstalling" class="w-3.5 h-3.5 animate-spin" />
            <Terminal v-else class="w-3.5 h-3.5" />
            <span>{{ isInstalling ? '正在安装 arkcli...' : '一键自动安装 arkcli' }}</span>
          </button>
        </div>
      </div>

      <!-- Step 3: Login Status -->
      <div
        class="p-3.5 rounded-xl border transition-all duration-200"
        :class="currentStatus.logged_in ? 'bg-slate-800/40 border-emerald-500/30' : 'bg-slate-800/60 border-sky-500/30'"
      >
        <div class="flex items-start justify-between">
          <div class="flex items-center space-x-2.5">
            <CheckCircle2 v-if="currentStatus.logged_in" class="w-5 h-5 text-emerald-400 flex-shrink-0" />
            <ShieldCheck v-else class="w-5 h-5 text-sky-400 flex-shrink-0" />
            <div>
              <div class="text-sm font-medium text-white flex items-center gap-2">
                3. 火山方舟授权登录
                <span
                  v-if="currentStatus.logged_in"
                  class="text-[10px] bg-emerald-500/20 text-emerald-300 px-1.5 py-0.5 rounded font-medium"
                >
                  已登录 ({{ currentStatus.user_name || '用户' }})
                </span>
              </div>
              <p class="text-xs text-slate-400 mt-0.5">
                {{ currentStatus.logged_in ? `账号 ID: ${currentStatus.account_id || '-'}` : '需授权登录以获取套餐配额数据' }}
              </p>
            </div>
          </div>
        </div>

        <div v-if="currentStatus.has_arkcli && !currentStatus.logged_in" class="mt-3 pt-2.5 border-t border-slate-800 space-y-2.5">
          <!-- Mode Switcher -->
          <div class="flex items-center justify-between text-[11px] bg-slate-900/60 p-1 rounded-lg border border-slate-800">
            <button
              @click="loginMode = 'browser'"
              class="flex-1 py-1 rounded text-center transition font-medium cursor-pointer"
              :class="loginMode === 'browser' ? 'bg-indigo-600 text-white shadow-sm' : 'text-slate-400 hover:text-white'"
            >
              浏览器一键授权
            </button>
            <button
              @click="loginMode = 'code'"
              class="flex-1 py-1 rounded text-center transition font-medium cursor-pointer"
              :class="loginMode === 'code' ? 'bg-indigo-600 text-white shadow-sm' : 'text-slate-400 hover:text-white'"
            >
              授权码登录 (备用)
            </button>
          </div>

          <!-- Mode 1: Browser One-Click -->
          <div v-if="loginMode === 'browser'" class="space-y-1.5">
            <button
              @click="handleLoginSso"
              :disabled="isLoggingIn"
              class="w-full py-1.5 px-3 bg-gradient-to-r from-sky-500 to-indigo-600 hover:from-sky-400 hover:to-indigo-500 text-white rounded-lg text-xs font-medium flex items-center justify-center gap-1.5 shadow-md shadow-sky-500/20 transition cursor-pointer"
            >
              <RefreshCw v-if="isLoggingIn" class="w-3.5 h-3.5 animate-spin" />
              <ExternalLink v-else class="w-3.5 h-3.5" />
              <span>{{ isLoggingIn ? '正在等待浏览器授权完成...' : '在浏览器中授权登录' }}</span>
            </button>
            <p class="text-[10px] text-slate-500 text-center">
              唤起浏览器登录火山方舟，完成后自动回调
            </p>
          </div>

          <!-- Mode 2: Authorization Code -->
          <div v-else class="space-y-2">
            <div class="p-2.5 bg-slate-900/80 rounded-lg border border-slate-800 space-y-2 text-xs">
              <div class="flex items-center justify-between">
                <span class="text-[11px] text-slate-300 font-medium">第 1 步：获取授权链接</span>
                <button
                  @click="handleOpenAuthUrl"
                  :disabled="isGettingAuthUrl"
                  class="px-2 py-1 bg-sky-600 hover:bg-sky-500 text-white rounded text-[10px] flex items-center gap-1 transition shadow-sm cursor-pointer"
                >
                  <RefreshCw v-if="isGettingAuthUrl" class="w-3 h-3 animate-spin" />
                  <ExternalLink v-else class="w-3 h-3" />
                  <span>在浏览器打开</span>
                </button>
              </div>
              <div class="space-y-1">
                <span class="text-[11px] text-slate-300 font-medium">第 2 步：粘贴浏览器中的授权码</span>
                <div class="flex gap-1.5">
                  <input
                    v-model="authCode"
                    type="text"
                    placeholder="粘贴 base64 授权码"
                    class="flex-1 bg-slate-950 border border-slate-700 text-slate-200 text-xs rounded px-2 py-1 outline-none focus:border-indigo-500 font-mono"
                  />
                  <button
                    @click="handleLoginWithCode"
                    :disabled="isSubmittingCode || !authCode.trim()"
                    class="px-2.5 py-1 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white font-medium rounded text-xs transition cursor-pointer"
                  >
                    <RefreshCw v-if="isSubmittingCode" class="w-3 h-3 animate-spin" />
                    <span v-else>确认登录</span>
                  </button>
                </div>
              </div>
            </div>
            <p class="text-[10px] text-slate-400 text-center">
              💡 无需本地端口，不受网络代理与 Windows 端口限制
            </p>
          </div>
        </div>
      </div>

      <!-- Feedback / Log Section -->
      <div
        v-if="actionMessage || errorMessage"
        class="p-2.5 rounded-lg text-xs"
        :class="errorMessage ? 'bg-rose-950/40 text-rose-300 border border-rose-800/40' : 'bg-sky-950/30 text-sky-300 border border-sky-800/30'"
      >
        <p>{{ errorMessage || actionMessage }}</p>
      </div>

      <div
        v-if="installLogs"
        class="p-2 bg-black/60 rounded-lg border border-slate-800 font-mono text-[10px] text-slate-400 max-h-24 overflow-y-auto whitespace-pre-wrap"
      >
        {{ installLogs }}
      </div>
    </div>

    <!-- Bottom actions -->
    <div class="mt-4 pt-3 border-t border-slate-800 flex items-center justify-between">
      <button
        @click="handleManualRefresh"
        class="text-xs text-slate-400 hover:text-white flex items-center gap-1 py-1 px-2 rounded hover:bg-slate-800 transition cursor-pointer"
      >
        <RefreshCw class="w-3.5 h-3.5" />
        <span>重新检测</span>
      </button>

      <button
        v-if="currentStatus.has_node && currentStatus.has_arkcli && currentStatus.logged_in"
        @click="$emit('complete')"
        class="text-xs bg-emerald-500 hover:bg-emerald-400 text-slate-950 font-semibold px-3 py-1.5 rounded-lg flex items-center gap-1 shadow-md shadow-emerald-500/20 transition cursor-pointer"
      >
        <span>进入配额监控</span>
        <ArrowRight class="w-3.5 h-3.5" />
      </button>
    </div>
  </div>
</template>
