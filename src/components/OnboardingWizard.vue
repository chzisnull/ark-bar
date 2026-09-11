<script setup lang="ts">
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { EnvironmentStatus, CommandResult } from '../types';
import { CheckCircle2, XCircle, AlertTriangle, RefreshCw, Terminal, ExternalLink, ArrowRight, ShieldCheck } from 'lucide-vue-next';

const props = defineProps<{
  status: EnvironmentStatus;
}>();

const emit = defineEmits<{
  (e: 'refresh'): void;
  (e: 'complete'): void;
}>();

const isInstalling = ref(false);
const isLoggingIn = ref(false);
const installLogs = ref('');
const actionMessage = ref('');
const errorMessage = ref('');

async function handleInstallArkCli() {
  isInstalling.value = true;
  actionMessage.value = '正在通过 npm 全局安装 @volcengine/ark-cli，请稍候...';
  errorMessage.value = '';
  installLogs.value = '';

  try {
    const result = await invoke<CommandResult>('install_arkcli');
    if (result.success) {
      actionMessage.value = 'arkcli 安装成功！正在刷新环境...';
      installLogs.value = result.details || result.message;
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
      emit('refresh');
    } else {
      errorMessage.value = result.message;
    }
  } catch (err: any) {
    errorMessage.value = `登录授权出错: ${err}`;
  } finally {
    isLoggingIn.value = false;
  }
}

function openNodeDownload() {
  window.open('https://nodejs.org/', '_blank');
}
</script>

<template>
  <div class="p-5 flex flex-col h-full select-none text-slate-200">
    <!-- Header -->
    <div class="flex items-center space-x-3 mb-4 pb-3 border-b border-slate-800">
      <div class="w-9 h-9 rounded-xl bg-gradient-to-tr from-rose-500 to-amber-500 flex items-center justify-center text-white shadow-lg shadow-rose-500/20">
        <span class="text-xl">🌋</span>
      </div>
      <div>
        <h2 class="text-base font-semibold text-white tracking-wide">环境配置向导</h2>
        <p class="text-xs text-slate-400">检测并初始化 arkcli 运行环境</p>
      </div>
    </div>

    <!-- Steps -->
    <div class="space-y-3 flex-1 overflow-y-auto pr-1">
      <!-- Step 1: Node.js -->
      <div class="p-3.5 rounded-xl border transition-all duration-200"
        :class="status.has_node ? 'bg-slate-800/40 border-emerald-500/30' : 'bg-rose-950/20 border-rose-500/30'">
        <div class="flex items-start justify-between">
          <div class="flex items-center space-x-2.5">
            <CheckCircle2 v-if="status.has_node" class="w-5 h-5 text-emerald-400 flex-shrink-0" />
            <XCircle v-else class="w-5 h-5 text-rose-400 flex-shrink-0" />
            <div>
              <div class="text-sm font-medium text-white flex items-center gap-2">
                1. Node.js 环境
                <span v-if="status.has_node" class="text-[10px] bg-emerald-500/20 text-emerald-300 px-1.5 py-0.5 rounded font-mono">
                  {{ status.node_version }}
                </span>
              </div>
              <p class="text-xs text-slate-400 mt-0.5">
                {{ status.has_node ? 'Node.js 运行环境已就绪' : '系统未检测到 Node.js，需先安装环境' }}
              </p>
            </div>
          </div>
          <button v-if="!status.has_node" @click="openNodeDownload"
            class="text-xs bg-slate-800 hover:bg-slate-700 text-sky-400 px-2.5 py-1 rounded-lg flex items-center gap-1 transition">
            <span>下载</span>
            <ExternalLink class="w-3 h-3" />
          </button>
        </div>
      </div>

      <!-- Step 2: arkcli CLI -->
      <div class="p-3.5 rounded-xl border transition-all duration-200"
        :class="status.has_arkcli ? 'bg-slate-800/40 border-emerald-500/30' : 'bg-slate-800/60 border-amber-500/30'">
        <div class="flex items-start justify-between">
          <div class="flex items-center space-x-2.5">
            <CheckCircle2 v-if="status.has_arkcli" class="w-5 h-5 text-emerald-400 flex-shrink-0" />
            <AlertTriangle v-else class="w-5 h-5 text-amber-400 flex-shrink-0" />
            <div>
              <div class="text-sm font-medium text-white flex items-center gap-2">
                2. 官方 arkcli 工具
                <span v-if="status.has_arkcli" class="text-[10px] bg-emerald-500/20 text-emerald-300 px-1.5 py-0.5 rounded font-mono">
                  {{ status.arkcli_version }}
                </span>
              </div>
              <p class="text-xs text-slate-400 mt-0.5">
                {{ status.has_arkcli ? '已安装 @volcengine/ark-cli' : '需要安装火山方舟官方命令行工具' }}
              </p>
            </div>
          </div>
        </div>

        <div v-if="!status.has_arkcli && status.has_node" class="mt-3 pt-2.5 border-t border-slate-800">
          <button @click="handleInstallArkCli" :disabled="isInstalling"
            class="w-full py-1.5 px-3 bg-amber-500/20 hover:bg-amber-500/30 text-amber-300 border border-amber-500/40 rounded-lg text-xs font-medium flex items-center justify-center gap-1.5 transition">
            <RefreshCw v-if="isInstalling" class="w-3.5 h-3.5 animate-spin" />
            <Terminal v-else class="w-3.5 h-3.5" />
            <span>{{ isInstalling ? '正在安装 arkcli...' : '一键自动安装 arkcli' }}</span>
          </button>
        </div>
      </div>

      <!-- Step 3: Login Status -->
      <div class="p-3.5 rounded-xl border transition-all duration-200"
        :class="status.logged_in ? 'bg-slate-800/40 border-emerald-500/30' : 'bg-slate-800/60 border-sky-500/30'">
        <div class="flex items-start justify-between">
          <div class="flex items-center space-x-2.5">
            <CheckCircle2 v-if="status.logged_in" class="w-5 h-5 text-emerald-400 flex-shrink-0" />
            <ShieldCheck v-else class="w-5 h-5 text-sky-400 flex-shrink-0" />
            <div>
              <div class="text-sm font-medium text-white flex items-center gap-2">
                3. 火山方舟授权登录
                <span v-if="status.logged_in" class="text-[10px] bg-emerald-500/20 text-emerald-300 px-1.5 py-0.5 rounded">
                  已登录 ({{ status.user_name || '用户' }})
                </span>
              </div>
              <p class="text-xs text-slate-400 mt-0.5">
                {{ status.logged_in ? `账号 ID: ${status.account_id || '-'}` : '需授权登录以获取套餐配额数据' }}
              </p>
            </div>
          </div>
        </div>

        <div v-if="status.has_arkcli && !status.logged_in" class="mt-3 pt-2.5 border-t border-slate-800">
          <button @click="handleLoginSso" :disabled="isLoggingIn"
            class="w-full py-1.5 px-3 bg-gradient-to-r from-sky-500 to-indigo-600 hover:from-sky-400 hover:to-indigo-500 text-white rounded-lg text-xs font-medium flex items-center justify-center gap-1.5 shadow-md shadow-sky-500/20 transition">
            <RefreshCw v-if="isLoggingIn" class="w-3.5 h-3.5 animate-spin" />
            <ExternalLink v-else class="w-3.5 h-3.5" />
            <span>{{ isLoggingIn ? '正在等待浏览器授权...' : '浏览器授权登录 (volc-sso)' }}</span>
          </button>
        </div>
      </div>

      <!-- Feedback / Log Section -->
      <div v-if="actionMessage || errorMessage" class="p-2.5 rounded-lg text-xs"
        :class="errorMessage ? 'bg-rose-950/40 text-rose-300 border border-rose-800/40' : 'bg-sky-950/30 text-sky-300 border border-sky-800/30'">
        <p>{{ errorMessage || actionMessage }}</p>
      </div>

      <div v-if="installLogs" class="p-2 bg-black/60 rounded-lg border border-slate-800 font-mono text-[10px] text-slate-400 max-h-24 overflow-y-auto whitespace-pre-wrap">
        {{ installLogs }}
      </div>
    </div>

    <!-- Bottom actions -->
    <div class="mt-4 pt-3 border-t border-slate-800 flex items-center justify-between">
      <button @click="$emit('refresh')"
        class="text-xs text-slate-400 hover:text-white flex items-center gap-1 py-1 px-2 rounded hover:bg-slate-800 transition">
        <RefreshCw class="w-3.5 h-3.5" />
        <span>重新检测</span>
      </button>

      <button v-if="status.has_node && status.has_arkcli && status.logged_in"
        @click="$emit('complete')"
        class="text-xs bg-emerald-500 hover:bg-emerald-400 text-slate-950 font-semibold px-3 py-1.5 rounded-lg flex items-center gap-1 shadow-md shadow-emerald-500/20 transition">
        <span>进入配额监控</span>
        <ArrowRight class="w-3.5 h-3.5" />
      </button>
    </div>
  </div>
</template>
