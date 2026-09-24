<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import type {
  EnvironmentStatus,
  UpdateInfo,
  ProviderType,
  TrayPercentMode,
  ProviderTabConfig,
  SettingsNavTab,
  NotchMetric,
} from '../types';
import ProviderIcon from './ProviderIcon.vue';
import UpdateModal from './UpdateModal.vue';
import {
  GripVertical,
  Bell,
  Palette,
  KeyRound,
  Sliders,
  Power,
  RefreshCw,
  RotateCcw,
  Check,
} from 'lucide-vue-next';
import { openUrl } from '@tauri-apps/plugin-opener';
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
import pkg from '../../package.json';

const appVersion = pkg.version;

const props = defineProps<{
  envStatus: EnvironmentStatus | null;
  refreshInterval: number;
  trayPercentMode: TrayPercentMode;
  initialUpdateInfo?: UpdateInfo | null;
  activeProvider: ProviderType;
  providerTabs?: ProviderTabConfig[];
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'update-interval', val: number): void;
  (e: 'update-tray-percent-mode', val: TrayPercentMode): void;
  (e: 'update-tray-target', val: ProviderType | 'auto'): void;
  (e: 'update-provider-tabs', tabs: ProviderTabConfig[]): void;
  (e: 're-login'): void;
  (e: 'provider-token-updated'): void;
  (e: 'update-checked', val: UpdateInfo): void;
}>();

// Navigation Tabs
const activeNavTab = ref<SettingsNavTab>('accounts');

// Normalize tabs so all 5 supported providers are always available to configure or restore
function normalizeTabs(tabs?: ProviderTabConfig[]): ProviderTabConfig[] {
  const validIds: ProviderType[] = ['antigravity', 'grok', 'volcengine', 'codex', 'teamo'];
  const nameMap: Record<ProviderType, string> = {
    antigravity: 'Antigravity',
    grok: 'Grok',
    volcengine: '火山方舟',
    codex: 'Codex',
    teamo: 'Teamo',
  };
  const result: ProviderTabConfig[] = [];
  if (tabs && Array.isArray(tabs)) {
    for (const t of tabs) {
      if (t && t.id && validIds.includes(t.id) && !result.some((r) => r.id === t.id)) {
        result.push({
          ...t,
          name: t.name || nameMap[t.id],
          visible: t.visible !== false,
          notch_metric: t.notch_metric || (t.id === 'teamo' ? 'balance' : 'session'),
          model_filter: t.model_filter || 'all',
        });
      }
    }
  }
  for (const id of validIds) {
    if (!result.some((r) => r.id === id)) {
      result.push({
        id,
        name: nameMap[id],
        visible: true,
        notch_metric: id === 'teamo' ? 'balance' : 'session',
        model_filter: 'all',
      });
    }
  }
  return result;
}

// Provider tabs copy
const localTabs = ref<ProviderTabConfig[]>(normalizeTabs(props.providerTabs));

watch(
  () => props.providerTabs,
  (newVal) => {
    if (newVal) {
      localTabs.value = normalizeTabs(newVal);
    }
  },
  { deep: true }
);

// Notch mode preferences: 'hover' (default) | 'always' | 'hidden'
const notchMode = ref<'hover' | 'always' | 'hidden'>(
  (localStorage.getItem('arkbar_notch_mode') as any) || 'hover'
);

function setNotchMode(mode: 'hover' | 'always' | 'hidden') {
  notchMode.value = mode;
  localStorage.setItem('arkbar_notch_mode', mode);
  window.dispatchEvent(new Event('storage'));
}

// Notch edge placement: 'right' (default) | 'top'
export type NotchEdge = 'right' | 'top';

const notchEdge = ref<NotchEdge>(
  (localStorage.getItem('arkbar_notch_edge') as NotchEdge) || 'right'
);

async function setNotchEdge(edge: NotchEdge) {
  notchEdge.value = edge;
  localStorage.setItem('arkbar_notch_edge', edge);
  window.dispatchEvent(new Event('storage'));
  try {
    // 每条边各自记住自己被拖到的位置，切回来时原样恢复
    const raw = localStorage.getItem(`arkbar_notch_along_${edge}`);
    const parsed = raw === null ? null : Number(raw);
    await invoke('set_notch_edge', {
      edge,
      along: parsed !== null && Number.isFinite(parsed) ? parsed : null,
    });
  } catch (e) {
    console.error('Failed to set notch edge:', e);
  }
}

// 刘海覆盖范围（对齐 Codenotch 的 NotchScreenScope）：主显示器 / 所有显示器
type NotchScope = 'main' | 'all';
const notchScope = ref<NotchScope>(
  (localStorage.getItem('arkbar_notch_scope') as NotchScope) || 'main'
);

// 覆盖范围以 Rust 侧的 notch.json 为准（和贴边位置同一份状态）：
// 挂载时对齐一次，免得「存储被清过 / 手改过文件」时界面与实际行为各说各话
async function syncNotchScope() {
  try {
    const scope = await invoke<string>('get_notch_scope');
    if (scope === 'all' || scope === 'main') {
      notchScope.value = scope as NotchScope;
      localStorage.setItem('arkbar_notch_scope', scope);
    }
  } catch {}
}

async function setNotchScope(scope: NotchScope) {
  notchScope.value = scope;
  localStorage.setItem('arkbar_notch_scope', scope);
  try {
    await invoke('set_notch_scope', { scope });
  } catch (e) {
    console.error('Failed to set notch scope:', e);
  }
}

async function recentreNotch() {
  try {
    await invoke('recentre_notch', { edge: notchEdge.value });
  } catch (e) {
    console.error('Failed to recentre notch:', e);
  }
}

const showTrayIcon = ref<boolean>(localStorage.getItem('arkbar_show_tray') === 'true');

function toggleTrayIcon() {
  showTrayIcon.value = !showTrayIcon.value;
  localStorage.setItem('arkbar_show_tray', showTrayIcon.value ? 'true' : 'false');
  invoke('set_tray_icon_visible', { visible: showTrayIcon.value }).catch(() => {});
}

// Connected providers list
const connectedTabs = computed(() => localTabs.value.filter((t) => t.visible));

// Disconnected / Inactive services
interface InactiveService {
  id: string;
  name: string;
  desc: string;
  icon: string;
  enabled: boolean;
}

const inactiveServices = ref<InactiveService[]>([
  { id: 'claude', name: 'Claude', desc: '已退出 — 不再读取，也不再保存读数。', icon: 'claude', enabled: false },
  { id: 'cursor', name: 'Cursor', desc: '未连接 — 需要本地已运行并登录 Cursor。', icon: 'cursor', enabled: false },
  { id: 'deepseek', name: 'DeepSeek', desc: '未配置 — 支持填入 API Key 监控余额。', icon: 'deepseek', enabled: false },
  { id: 'ollama', name: 'Ollama', desc: '未连接 — 本地 11434 端口模型监控。', icon: 'ollama', enabled: false },
]);

// Drag & Drop Reordering
let draggedIndex: number | null = null;

function handleDragStart(index: number, e: DragEvent) {
  draggedIndex = index;
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'move';
  }
}

function handleDragOver(e: DragEvent) {
  e.preventDefault();
}

function handleDrop(targetIndex: number) {
  if (draggedIndex == null || draggedIndex === targetIndex) return;
  const [moved] = localTabs.value.splice(draggedIndex, 1);
  localTabs.value.splice(targetIndex, 0, moved);
  draggedIndex = null;
  emit('update-provider-tabs', [...localTabs.value]);
}

function toggleProviderVisibility(id: ProviderType) {
  const item = localTabs.value.find((t) => t.id === id);
  if (!item) return;
  if (item.visible && connectedTabs.value.length <= 1) return; // Keep at least one
  item.visible = !item.visible;
  emit('update-provider-tabs', [...localTabs.value]);
}

function updateProviderNotchMetric(id: ProviderType, metric: NotchMetric) {
  const item = localTabs.value.find((t) => t.id === id);
  if (item) {
    item.notch_metric = metric;
    emit('update-provider-tabs', [...localTabs.value]);
  }
}

function updateProviderModelFilter(id: ProviderType, filter: string) {
  const item = localTabs.value.find((t) => t.id === id);
  if (item) {
    item.model_filter = filter;
    emit('update-provider-tabs', [...localTabs.value]);
  }
}

// Provider Metadata details
function getProviderMeta(id: ProviderType) {
  switch (id) {
    case 'antigravity':
      return {
        title: 'Antigravity',
        sub: '个人 · 来自 Antigravity',
        hint: '在 Antigravity 里切换账号，刘海会跟上。',
        url: 'https://antigravity.google.com',
        urlText: '打开 Antigravity',
      };
    case 'grok':
      return {
        title: 'Grok',
        sub: 'caohanzhou123@gmail.com · 来自 Grok',
        hint: '在持有该账号的工具里切换，刘海会跟上。',
        url: 'https://grok.com',
        urlText: '打开 grok.com',
      };
    case 'volcengine':
      return {
        title: '火山方舟',
        sub: props.envStatus?.user_name ? `${props.envStatus.user_name} · 来自 ArkCLI` : '主账号 · 来自 ArkCLI',
        hint: '在终端执行 arkcli 切换账号，刘海会跟上。',
        url: 'https://console.volcengine.com/ark',
        urlText: '打开 火山方舟',
      };
    case 'codex':
      return {
        title: 'OpenAI Codex',
        sub: '本地会话与日志 · 来自 Codex',
        hint: '实时提取本地会话与 Token 消耗。',
        url: 'https://platform.openai.com',
        urlText: '打开 OpenAI',
      };
    case 'teamo':
      return {
        title: 'TeamoRouter',
        sub: '官方开放接口 · 来自 Teamo',
        hint: '填入 sk-teamo- API Key 实时读取余额与用量。',
        url: 'https://teamorouter.cn',
        urlText: '打开 TeamoRouter',
      };
  }
}

// Token Key Storage for Teamo and custom tokens
const showTeamoKeyInput = ref(false);
const teamoKey = ref('');
const teamoKeySaved = ref(false);

let unlistenOpenUpdate: (() => void) | null = null;
let teamoKeySavedTimer: any = null;

onMounted(async () => {
  try {
    const key = await invoke<string | null>('read_provider_token', { provider: 'teamo' });
    if (key) teamoKey.value = key;
  } catch {}

  try {
    autostartEnabled.value = await isEnabled();
  } catch {}

  await syncNotchScope();

  // 刘海上的「去更新」按钮：直接弹更新弹窗（顺便实时查一次，避免拿到过期结论）
  unlistenOpenUpdate = await listen('open_update', () => {
    checkForUpdate();
  });
});

async function saveTeamoKey() {
  try {
    await invoke('set_provider_token', { provider: 'teamo', token: teamoKey.value.trim() });
    teamoKeySaved.value = true;
    emit('provider-token-updated');
    if (teamoKeySavedTimer) clearTimeout(teamoKeySavedTimer);
    teamoKeySavedTimer = setTimeout(() => {
      teamoKeySaved.value = false;
    }, 2000);
  } catch {}
}

// Autostart toggle
const autostartEnabled = ref(false);
async function toggleAutostart() {
  try {
    if (autostartEnabled.value) {
      await disable();
      autostartEnabled.value = false;
    } else {
      await enable();
      autostartEnabled.value = true;
    }
  } catch {}
}

// Exit App
function handleExit() {
  invoke('exit_app').catch(() => {});
}

// Open External URL
function openExternal(url: string) {
  openUrl(url).catch(() => window.open(url, '_blank'));
}

// Notifications Toggles
const alertHighQuota = ref(true);
const alertReset = ref(true);

// Update Modal & checking
const isCheckingUpdate = ref(false);
const showUpdateModal = ref(false);
// 自动检查更新（默认开）：与 Rust 后台巡检线程同源
const autoUpdate = ref<boolean>(localStorage.getItem('arkbar_auto_update') !== 'false');

function toggleAutoUpdate() {
  autoUpdate.value = !autoUpdate.value;
  localStorage.setItem('arkbar_auto_update', autoUpdate.value ? 'true' : 'false');
  invoke('set_auto_update', { enabled: autoUpdate.value }).catch(() => {});
}
const currentUpdateInfo = ref<UpdateInfo | null>(props.initialUpdateInfo || null);
const updateCheckStatus = ref<'idle' | 'latest' | 'error'>('idle');
const lastCheckedTime = ref<string>('');
let statusResetTimer: any = null;

async function checkForUpdate() {
  if (isCheckingUpdate.value) return;
  isCheckingUpdate.value = true;
  updateCheckStatus.value = 'idle';
  if (statusResetTimer) clearTimeout(statusResetTimer);

  try {
    const res = await invoke<UpdateInfo>('check_for_updates', { force: true });
    currentUpdateInfo.value = res;
    emit('update-checked', res);

    const now = new Date();
    const timeStr = `${now.getHours().toString().padStart(2, '0')}:${now.getMinutes().toString().padStart(2, '0')}`;
    lastCheckedTime.value = timeStr;

    if (res.has_update) {
      showUpdateModal.value = true;
      updateCheckStatus.value = 'idle';
    } else {
      updateCheckStatus.value = 'latest';
      statusResetTimer = setTimeout(() => {
        updateCheckStatus.value = 'idle';
      }, 8000);
    }
  } catch (err) {
    console.error('Check update failed:', err);
    updateCheckStatus.value = 'error';
    statusResetTimer = setTimeout(() => {
      updateCheckStatus.value = 'idle';
    }, 5000);
  } finally {
    isCheckingUpdate.value = false;
  }
}

// Float notch toggle
async function toggleNotchWindow() {
  try {
    const isOpen = await invoke<boolean>('is_float_window_open');
    if (isOpen) {
      await invoke('close_float_window');
    } else {
      await invoke('open_float_window');
    }
  } catch {}
}

onUnmounted(() => {
  if (unlistenOpenUpdate) unlistenOpenUpdate();
  if (teamoKeySavedTimer) clearTimeout(teamoKeySavedTimer);
  if (statusResetTimer) clearTimeout(statusResetTimer);
});
</script>

<template>
  <div class="w-full h-full flex bg-[#18191c] text-neutral-100 select-none overflow-hidden font-sans">
    <!-- ============================================================ -->
    <!-- LEFT SIDEBAR (Images 2)                                      -->
    <!-- ============================================================ -->
    <div class="w-[210px] shrink-0 bg-[#121316] border-r border-white/5 flex flex-col justify-between p-3.5 select-none">
      <div class="space-y-4" data-tauri-drag-region>
        <!-- 1. Window Traffic Lights (macOS standard) -->
        <div class="flex items-center gap-2 pt-1 pb-2 px-1" data-tauri-drag-region>
          <button
            @click="emit('close')"
            class="w-3 h-3 rounded-full bg-[#ff5f56] hover:brightness-110 active:brightness-90 transition-all flex items-center justify-center group"
            title="关闭设置"
          >
            <span class="opacity-0 group-hover:opacity-100 text-[9px] text-black font-bold leading-none">×</span>
          </button>
          <div class="w-3 h-3 rounded-full bg-[#ffbd2e]" />
          <div class="w-3 h-3 rounded-full bg-[#27c93f]" />
        </div>

        <!-- 2. Brand Logo & Name -->
        <div class="flex items-center gap-2 px-1" data-tauri-drag-region>
          <div class="w-6 h-6 rounded-md bg-gradient-to-br from-rose-500 to-amber-500 flex items-center justify-center text-white shadow-sm p-1">
            <ProviderIcon name="volcengine" class="w-4 h-4" />
          </div>
          <span class="font-bold text-[15px] tracking-tight text-white">ArkBar</span>
        </div>

        <!-- 3. Navigation Menu Items -->
        <nav class="space-y-1 pt-2">
          <!-- 账号 (Accounts) -->
          <button
            @click="activeNavTab = 'accounts'"
            class="w-full flex items-center justify-between px-2.5 py-2 rounded-lg text-sm font-medium transition-colors"
            :class="activeNavTab === 'accounts' ? 'bg-white/10 text-white shadow-sm' : 'text-neutral-400 hover:text-neutral-200 hover:bg-white/5'"
          >
            <div class="flex items-center gap-2.5">
              <KeyRound class="w-4 h-4 text-neutral-400" />
              <span>账号</span>
            </div>
            <span class="px-1.5 py-0.5 text-[11px] font-bold rounded-full bg-white/10 text-neutral-300">
              {{ connectedTabs.length }}
            </span>
          </button>

          <!-- 外观 (Appearance) -->
          <button
            @click="activeNavTab = 'appearance'"
            class="w-full flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-sm font-medium transition-colors"
            :class="activeNavTab === 'appearance' ? 'bg-white/10 text-white shadow-sm' : 'text-neutral-400 hover:text-neutral-200 hover:bg-white/5'"
          >
            <Palette class="w-4 h-4 text-neutral-400" />
            <span>外观</span>
          </button>

          <!-- 通知 (Notifications) -->
          <button
            @click="activeNavTab = 'notifications'"
            class="w-full flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-sm font-medium transition-colors"
            :class="activeNavTab === 'notifications' ? 'bg-white/10 text-white shadow-sm' : 'text-neutral-400 hover:text-neutral-200 hover:bg-white/5'"
          >
            <Bell class="w-4 h-4 text-neutral-400" />
            <span>通知</span>
          </button>

          <!-- 通用 (General) -->
          <button
            @click="activeNavTab = 'general'"
            class="w-full flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-sm font-medium transition-colors"
            :class="activeNavTab === 'general' ? 'bg-white/10 text-white shadow-sm' : 'text-neutral-400 hover:text-neutral-200 hover:bg-white/5'"
          >
            <Sliders class="w-4 h-4 text-neutral-400" />
            <span>通用</span>
          </button>
        </nav>
      </div>

      <!-- Sidebar Bottom: Exit & Version -->
      <div class="pt-4 border-t border-white/5 space-y-2 px-1">
        <button
          @click="handleExit"
          class="w-full flex items-center gap-2 text-xs font-medium text-neutral-400 hover:text-rose-400 transition-colors py-1.5"
        >
          <Power class="w-4 h-4" />
          <span>退出 ArkBar</span>
        </button>

        <div class="text-[11px] text-neutral-500">
          ArkBar {{ appVersion }}
        </div>
      </div>
    </div>

    <!-- ============================================================ -->
    <!-- RIGHT CONTENT PANE (Images 2)                                -->
    <!-- ============================================================ -->
    <div class="flex-1 overflow-y-auto p-6 space-y-6">
      <!-- TAB 1: 账号 (ACCOUNTS) -->
      <div v-if="activeNavTab === 'accounts'" class="space-y-6">
        <div>
          <h2 class="text-xl font-bold text-white tracking-tight">账号</h2>
          <p class="text-xs text-neutral-400 mt-1">选择刘海读取哪些服务。</p>
        </div>

        <!-- 1. 已连接 / 服务列表 (Connected Section) -->
        <div class="space-y-3">
          <div class="flex items-center justify-between">
            <h3 class="text-xs font-bold text-neutral-400 uppercase tracking-wider">已连接服务</h3>
            <span class="text-[11px] text-neutral-500">
              {{ connectedTabs.length }} 个服务在刘海中显示
            </span>
          </div>

          <!-- Provider Cards -->
          <div class="space-y-2.5">
            <div
              v-for="(item, index) in localTabs"
              :key="item.id"
              draggable="true"
              @dragstart="handleDragStart(index, $event)"
              @dragover="handleDragOver"
              @drop="handleDrop(index)"
              class="bg-[#202126] border border-white/5 rounded-xl p-3.5 hover:border-white/10 transition-all space-y-2.5"
              :class="{ 'opacity-75': !item.visible }"
            >
              <!-- Card Header Row -->
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2.5">
                  <!-- Drag Handle -->
                  <div class="cursor-grab active:cursor-grabbing text-neutral-500 hover:text-neutral-300">
                    <GripVertical class="w-4 h-4" />
                  </div>

                  <!-- Provider Logo -->
                  <div class="w-6 h-6 rounded-md bg-neutral-800 flex items-center justify-center p-1 text-white">
                    <ProviderIcon :name="item.id" class="w-4 h-4" />
                  </div>

                  <!-- Title & Status Badge -->
                  <div class="flex items-center gap-2">
                    <span class="font-bold text-sm text-white">{{ getProviderMeta(item.id).title }}</span>
                    <span
                      v-if="!item.visible"
                      class="text-[10px] px-1.5 py-0.5 rounded bg-white/5 text-neutral-400 font-normal border border-white/5"
                    >
                      已在刘海中隐藏
                    </span>
                  </div>
                </div>

                <!-- Right Action Controls -->
                <div class="flex items-center gap-2.5">
                  <button
                    class="text-neutral-400 hover:text-white p-1 rounded transition-colors"
                    title="推送通知"
                  >
                    <Bell class="w-3.5 h-3.5" />
                  </button>

                  <button
                    @click="openExternal(getProviderMeta(item.id).url)"
                    class="px-2.5 py-1 text-xs font-medium rounded-full bg-white/10 text-neutral-200 hover:bg-white/15 hover:text-white transition-colors"
                  >
                    {{ getProviderMeta(item.id).urlText }}
                  </button>

                  <!-- Apple-style Blue Switch Toggle -->
                  <button
                    @click="toggleProviderVisibility(item.id)"
                    class="w-10 h-6 rounded-full transition-colors relative flex items-center px-0.5 focus:outline-none cursor-pointer"
                    :class="item.visible ? 'bg-[#0a84ff]' : 'bg-neutral-700'"
                    :title="item.visible ? '点击在刘海中隐藏' : '点击在刘海中恢复显示'"
                  >
                    <span
                      class="w-5 h-5 rounded-full bg-white shadow-md transform transition-transform"
                      :class="item.visible ? 'translate-x-4' : 'translate-x-0'"
                    />
                  </button>
                </div>
              </div>

              <!-- Card Body: Subtext & Details -->
              <div class="pl-7 space-y-2">
                <div class="text-xs text-neutral-400 leading-relaxed">
                  <span class="text-neutral-300">{{ getProviderMeta(item.id).sub }}</span>
                  <p class="text-[11px] text-neutral-500 mt-0.5">{{ getProviderMeta(item.id).hint }}</p>
                </div>

                <!-- Dropdown Selectors Row -->
                <div class="flex items-center gap-4 text-xs pt-1">
                  <!-- 刘海显示 Metric Dropdown -->
                  <div class="flex items-center gap-1.5">
                    <span class="text-neutral-400">刘海显示</span>
                    <select
                      :value="item.notch_metric || 'session'"
                      @change="updateProviderNotchMetric(item.id, ($event.target as HTMLSelectElement).value as NotchMetric)"
                      class="bg-[#18191c] text-[#0a84ff] hover:text-blue-400 font-medium px-2 py-1 rounded border border-white/5 focus:outline-none cursor-pointer text-xs"
                    >
                      <option value="session">5 小时额度</option>
                      <option value="weekly">周度额度</option>
                      <option value="monthly">月度额度</option>
                      <option value="today_tokens">今日 Token</option>
                      <option value="balance" v-if="item.id === 'teamo'">账户余额</option>
                    </select>
                  </div>

                  <!-- 模型数据 Dropdown -->
                  <div class="flex items-center gap-1.5">
                    <span class="text-neutral-400">模型数据</span>
                    <select
                      :value="item.model_filter || 'all'"
                      @change="updateProviderModelFilter(item.id, ($event.target as HTMLSelectElement).value)"
                      class="bg-[#18191c] text-[#0a84ff] hover:text-blue-400 font-medium px-2 py-1 rounded border border-white/5 focus:outline-none cursor-pointer text-xs"
                    >
                      <option value="all">全部模型</option>
                      <option value="gemini" v-if="item.id === 'antigravity'">Gemini 模型</option>
                      <option value="claude" v-if="item.id === 'antigravity'">Claude 模型</option>
                    </select>
                  </div>
                </div>

                <!-- Special Token Input for Teamo -->
                <div v-if="item.id === 'teamo'" class="pt-2">
                  <button
                    @click="showTeamoKeyInput = !showTeamoKeyInput"
                    class="text-[11px] text-neutral-400 hover:text-neutral-200 underline decoration-dotted"
                  >
                    {{ showTeamoKeyInput ? '收起 API Key 配置' : '修改 / 填入 Teamo API Key' }}
                  </button>

                  <div v-if="showTeamoKeyInput" class="mt-2 flex items-center gap-2">
                    <input
                      v-model="teamoKey"
                      type="password"
                      placeholder="sk-teamo-..."
                      class="bg-[#18191c] text-xs text-white px-2.5 py-1.5 rounded-lg border border-white/10 flex-1 focus:outline-none focus:border-blue-500 font-mono"
                    />
                    <button
                      @click="saveTeamoKey"
                      class="px-3 py-1.5 text-xs font-semibold rounded-lg bg-blue-600 hover:bg-blue-500 text-white transition-colors"
                    >
                      {{ teamoKeySaved ? '已保存 ✓' : '保存' }}
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <p class="text-[11px] text-neutral-500 pt-1">刘海按这个顺序画。拖动一行的手柄即可改顺序。</p>

          <!-- Explanatory note callout card (Images 2) -->
          <div class="bg-white/[0.03] border border-white/5 rounded-xl p-3 text-xs text-neutral-400 leading-relaxed space-y-1">
            <p class="text-neutral-300 font-medium">大多数读数借助本地已持有的工具自动同步。</p>
            <p class="text-neutral-500 text-[11px]">
              例如 Antigravity 通过本地 agy CLI，火山方舟通过 ArkCLI，Grok/Codex 自动读取本地会话。TeamoRouter 支持通过 API Key 直连查询，凭据仅保存在本地 ~/.ark-bar/tokens.json。
            </p>
          </div>
        </div>

        <!-- 2. 未连接 (Not Connected Section) -->
        <div class="space-y-3 pt-4 border-t border-white/5">
          <h3 class="text-xs font-bold text-neutral-400 uppercase tracking-wider">未连接</h3>

          <div class="space-y-2">
            <div
              v-for="service in inactiveServices"
              :key="service.id"
              class="bg-[#202126]/60 border border-white/5 rounded-xl p-3 flex items-center justify-between opacity-70 hover:opacity-100 transition-opacity"
            >
              <div class="flex items-center gap-3">
                <div class="w-6 h-6 rounded-md bg-neutral-800 flex items-center justify-center p-1 text-neutral-400">
                  <ProviderIcon :name="service.icon" class="w-4 h-4" />
                </div>
                <div>
                  <h4 class="text-sm font-semibold text-neutral-200">{{ service.name }}</h4>
                  <p class="text-[11px] text-neutral-500">{{ service.desc }}</p>
                </div>
              </div>

              <button
                @click="service.enabled = !service.enabled"
                class="w-10 h-6 rounded-full bg-neutral-700 relative flex items-center px-0.5 focus:outline-none cursor-not-allowed opacity-50"
                title="服务即将支持"
              >
                <span class="w-5 h-5 rounded-full bg-white shadow-md transform translate-x-0" />
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- TAB 2: 外观 (APPEARANCE) -->
      <div v-else-if="activeNavTab === 'appearance'" class="space-y-6">
        <div>
          <h2 class="text-xl font-bold text-white tracking-tight">外观</h2>
          <p class="text-xs text-neutral-400 mt-1">自定义屏幕刘海与贴边交互。</p>
        </div>

        <div class="space-y-4">
          <!-- 刘海展示方式 (Codenotch Style) -->
          <div class="bg-[#202126] border border-white/5 rounded-xl p-4 space-y-3">
            <h3 class="text-sm font-semibold text-white">刘海展示方式</h3>
            <div class="grid grid-cols-3 gap-3">
              <button
                @click="setNotchMode('hover')"
                class="p-3 rounded-lg border text-center transition-all"
                :class="notchMode === 'hover' ? 'border-[#0a84ff] bg-[#0a84ff]/10 text-white' : 'border-white/5 bg-white/5 text-neutral-400 hover:text-white'"
              >
                <div class="text-sm font-bold">悬停时展开</div>
                <div class="text-[11px] text-neutral-400 mt-0.5">默认（对齐 Codenotch）</div>
              </button>

              <button
                @click="setNotchMode('always')"
                class="p-3 rounded-lg border text-center transition-all"
                :class="notchMode === 'always' ? 'border-[#0a84ff] bg-[#0a84ff]/10 text-white' : 'border-white/5 bg-white/5 text-neutral-400 hover:text-white'"
              >
                <div class="text-sm font-bold">始终常驻</div>
                <div class="text-[11px] text-neutral-400 mt-0.5">一直保持展开</div>
              </button>

              <button
                @click="setNotchMode('hidden')"
                class="p-3 rounded-lg border text-center transition-all"
                :class="notchMode === 'hidden' ? 'border-[#0a84ff] bg-[#0a84ff]/10 text-white' : 'border-white/5 bg-white/5 text-neutral-400 hover:text-white'"
              >
                <div class="text-sm font-bold">隐藏刘海</div>
                <div class="text-[11px] text-neutral-400 mt-0.5">不显示屏幕刘海</div>
              </button>
            </div>
            <p class="text-[11px] text-neutral-500 leading-relaxed">
              悬停时展开模式下：鼠标未移入时，刘海收拢为屏幕边缘极简黑胶囊；鼠标放上去时即刻向外展开完整环形指标与气泡卡片。
            </p>
          </div>

          <!-- 屏幕停留位置 (Screen Edge Placement - Codenotch Style) -->
          <div class="bg-[#202126] border border-white/5 rounded-xl p-4 space-y-3">
            <div class="flex items-center justify-between">
              <div>
                <h3 class="text-sm font-semibold text-white">屏幕停留位置</h3>
                <p class="text-xs text-neutral-400 mt-0.5">设置屏幕刘海停靠边缘（支持右侧或顶部贴边）</p>
              </div>
              <button
                @click="recentreNotch"
                class="px-2.5 py-1 text-xs font-medium rounded-lg bg-white/10 hover:bg-white/15 text-neutral-200 hover:text-white transition-colors flex items-center gap-1.5 cursor-pointer"
                title="复位贴边到当前屏幕居中位置"
              >
                <RotateCcw class="w-3.5 h-3.5" />
                <span>复位居中</span>
              </button>
            </div>

            <div class="grid grid-cols-2 gap-3">
              <button
                @click="setNotchEdge('right')"
                class="p-3 rounded-lg border text-left transition-all cursor-pointer flex items-center gap-3"
                :class="notchEdge === 'right' ? 'border-[#0a84ff] bg-[#0a84ff]/10 text-white' : 'border-white/5 bg-white/5 text-neutral-400 hover:text-white'"
              >
                <!-- Right Edge mini preview -->
                <div class="w-9 h-7 rounded border border-white/20 bg-black/40 flex items-center justify-end pr-0.5 shrink-0">
                  <div class="w-1.5 h-4 bg-[#0a84ff] rounded-l-sm" />
                </div>
                <div>
                  <div class="text-sm font-bold">右侧屏幕边缘</div>
                  <div class="text-[11px] text-neutral-400 mt-0.5">垂直贴边 · 气泡向左展开</div>
                </div>
              </button>

              <button
                @click="setNotchEdge('top')"
                class="p-3 rounded-lg border text-left transition-all cursor-pointer flex items-center gap-3"
                :class="notchEdge === 'top' ? 'border-[#0a84ff] bg-[#0a84ff]/10 text-white' : 'border-white/5 bg-white/5 text-neutral-400 hover:text-white'"
              >
                <!-- Top Edge mini preview -->
                <div class="w-9 h-7 rounded border border-white/20 bg-black/40 flex flex-col items-center pt-0.5 shrink-0">
                  <div class="w-4 h-1.5 bg-[#0a84ff] rounded-b-sm" />
                </div>
                <div>
                  <div class="text-sm font-bold">顶部屏幕边缘</div>
                  <div class="text-[11px] text-neutral-400 mt-0.5">水平居中 · 气泡向下展开</div>
                </div>
              </button>
            </div>
            <p class="text-[11px] text-neutral-500 leading-relaxed">
              顶部边缘贴合 MacBook 刘海或菜单栏中央；拖动手柄可自由沿边缘微调，点击“复位居中”可瞬间重置至正中。
            </p>
          </div>

          <!-- 刘海覆盖范围（Codenotch 的 NotchScreenScope） -->
          <div class="bg-[#202126] border border-white/5 rounded-xl p-4 space-y-3">
            <div>
              <h3 class="text-sm font-semibold text-white">刘海显示范围</h3>
              <p class="text-xs text-neutral-400 mt-0.5">每块屏幕各一个刘海，还是只留带菜单栏那一块</p>
            </div>

            <div class="grid grid-cols-2 gap-3">
              <button
                @click="setNotchScope('main')"
                class="p-3 rounded-lg border text-left transition-all cursor-pointer flex items-center gap-3"
                :class="notchScope === 'main' ? 'border-[#0a84ff] bg-[#0a84ff]/10 text-white' : 'border-white/5 bg-white/5 text-neutral-400 hover:text-white'"
              >
                <div class="w-9 h-7 rounded border border-white/20 bg-black/40 flex items-center justify-center shrink-0">
                  <div class="w-4 h-2.5 bg-[#0a84ff] rounded-sm" />
                </div>
                <div>
                  <div class="text-sm font-bold">主显示器</div>
                  <div class="text-[11px] text-neutral-400 mt-0.5">只有带菜单栏那一块</div>
                </div>
              </button>

              <button
                @click="setNotchScope('all')"
                class="p-3 rounded-lg border text-left transition-all cursor-pointer flex items-center gap-3"
                :class="notchScope === 'all' ? 'border-[#0a84ff] bg-[#0a84ff]/10 text-white' : 'border-white/5 bg-white/5 text-neutral-400 hover:text-white'"
              >
                <div class="w-9 h-7 rounded border border-white/20 bg-black/40 flex items-center justify-center gap-0.5 shrink-0">
                  <div class="w-2 h-2.5 bg-[#0a84ff] rounded-sm" />
                  <div class="w-2 h-2.5 bg-[#0a84ff]/60 rounded-sm" />
                </div>
                <div>
                  <div class="text-sm font-bold">所有显示器</div>
                  <div class="text-[11px] text-neutral-400 mt-0.5">每块屏各一个，各开各的</div>
                </div>
              </button>
            </div>
            <p class="text-[11px] text-neutral-500 leading-relaxed">
              与 Codenotch 一致：贴边位置、沿边落点、展开状态都是整队共享，但「悬停哪块屏就展开哪块」是每块屏自己的事。
            </p>
          </div>

          <!-- 顶部系统菜单栏图标 -->
          <div class="bg-[#202126] border border-white/5 rounded-xl p-4 flex items-center justify-between">
            <div>
              <h3 class="text-sm font-semibold text-white">在系统菜单栏显示图标</h3>
              <p class="text-xs text-neutral-400 mt-0.5">默认关闭（推荐纯屏幕刘海交互模式）；开启后在顶部菜单栏显示常驻图标</p>
            </div>

            <button
              @click="toggleTrayIcon"
              class="w-10 h-6 rounded-full transition-colors relative flex items-center px-0.5"
              :class="showTrayIcon ? 'bg-[#0a84ff]' : 'bg-neutral-700'"
            >
              <span
                class="w-5 h-5 rounded-full bg-white shadow-md transform transition-transform"
                :class="showTrayIcon ? 'translate-x-4' : 'translate-x-0'"
              />
            </button>
          </div>

          <!-- 桌面悬浮刘海窗口控制 -->
          <div class="bg-[#202126] border border-white/5 rounded-xl p-4 flex items-center justify-between">
            <div>
              <h3 class="text-sm font-semibold text-white">屏幕刘海快捷开关</h3>
              <p class="text-xs text-neutral-400 mt-0.5">立即召唤或隐藏桌面上的 Codenotch 灵动贴边刘海</p>
            </div>

            <button
              @click="toggleNotchWindow"
              class="px-3 py-1.5 text-xs font-semibold rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors"
            >
              切换显示 / 隐藏
            </button>
          </div>
        </div>
      </div>

      <!-- TAB 3: 通知 (NOTIFICATIONS) -->
      <div v-else-if="activeNavTab === 'notifications'" class="space-y-6">
        <div>
          <h2 class="text-xl font-bold text-white tracking-tight">通知</h2>
          <p class="text-xs text-neutral-400 mt-1">配额使用预警与重置推送提醒。</p>
        </div>

        <div class="space-y-4">
          <div class="bg-[#202126] border border-white/5 rounded-xl p-4 flex items-center justify-between">
            <div>
              <h3 class="text-sm font-semibold text-white">高配额消耗告警 (≥85%)</h3>
              <p class="text-xs text-neutral-400 mt-0.5">当任一模型平台配额达到 85% 警戒水位时发送桌面通知</p>
            </div>
            <button
              @click="alertHighQuota = !alertHighQuota"
              class="w-10 h-6 rounded-full transition-colors relative flex items-center px-0.5"
              :class="alertHighQuota ? 'bg-[#0a84ff]' : 'bg-neutral-700'"
            >
              <span class="w-5 h-5 rounded-full bg-white shadow-md transform transition-transform" :class="alertHighQuota ? 'translate-x-4' : 'translate-x-0'" />
            </button>
          </div>

          <div class="bg-[#202126] border border-white/5 rounded-xl p-4 flex items-center justify-between">
            <div>
              <h3 class="text-sm font-semibold text-white">额度重置提醒</h3>
              <p class="text-xs text-neutral-400 mt-0.5">5小时窗口或周度额度重置时自动推送通知</p>
            </div>
            <button
              @click="alertReset = !alertReset"
              class="w-10 h-6 rounded-full transition-colors relative flex items-center px-0.5"
              :class="alertReset ? 'bg-[#0a84ff]' : 'bg-neutral-700'"
            >
              <span class="w-5 h-5 rounded-full bg-white shadow-md transform transition-transform" :class="alertReset ? 'translate-x-4' : 'translate-x-0'" />
            </button>
          </div>
        </div>
      </div>

      <!-- TAB 4: 通用 (GENERAL) -->
      <div v-else-if="activeNavTab === 'general'" class="space-y-6">
        <div>
          <h2 class="text-xl font-bold text-white tracking-tight">通用</h2>
          <p class="text-xs text-neutral-400 mt-1">系统设置与自动更新维护。</p>
        </div>

        <div class="space-y-4">
          <!-- 开机自启动 -->
          <div class="bg-[#202126] border border-white/5 rounded-xl p-4 flex items-center justify-between">
            <div>
              <h3 class="text-sm font-semibold text-white">开机自动启动</h3>
              <p class="text-xs text-neutral-400 mt-0.5">登录系统时自动在后台唤起 ArkBar 菜单栏与屏幕刘海</p>
            </div>
            <button
              @click="toggleAutostart"
              class="w-10 h-6 rounded-full transition-colors relative flex items-center px-0.5"
              :class="autostartEnabled ? 'bg-[#0a84ff]' : 'bg-neutral-700'"
            >
              <span class="w-5 h-5 rounded-full bg-white shadow-md transform transition-transform" :class="autostartEnabled ? 'translate-x-4' : 'translate-x-0'" />
            </button>
          </div>

          <!-- 自动检查更新 -->
          <div class="bg-[#202126] border border-white/5 rounded-xl p-4 flex items-center justify-between">
            <div>
              <h3 class="text-sm font-semibold text-white">自动检查更新</h3>
              <p class="text-xs text-neutral-400 mt-0.5">启动后与每 6 小时各查一次，发现新版本在刘海上提示（关掉则只在你手动点击时检查）</p>
            </div>
            <button
              @click="toggleAutoUpdate"
              class="w-10 h-6 rounded-full transition-colors relative flex items-center px-0.5 shrink-0"
              :class="autoUpdate ? 'bg-[#0a84ff]' : 'bg-neutral-700'"
            >
              <span class="w-5 h-5 rounded-full bg-white shadow-md transform transition-transform" :class="autoUpdate ? 'translate-x-4' : 'translate-x-0'" />
            </button>
          </div>

          <!-- 后台刷新频率 -->
          <div class="bg-[#202126] border border-white/5 rounded-xl p-4 flex items-center justify-between">
            <div>
              <h3 class="text-sm font-semibold text-white">后台同步频率</h3>
              <p class="text-xs text-neutral-400 mt-0.5">原生 Rust 后台轮询间隔（毫秒级低功耗）</p>
            </div>
            <select
              :value="props.refreshInterval"
              @change="emit('update-interval', Number(($event.target as HTMLSelectElement).value))"
              class="bg-[#18191c] text-white text-xs font-semibold px-3 py-1.5 rounded-lg border border-white/10 focus:outline-none"
            >
              <option :value="1">每 1 分钟</option>
              <option :value="3">每 3 分钟</option>
              <option :value="5">每 5 分钟（默认）</option>
              <option :value="10">每 10 分钟</option>
              <option :value="0">仅手动刷新</option>
            </select>
          </div>

          <!-- 菜单栏托盘模式 -->
          <div class="bg-[#202126] border border-white/5 rounded-xl p-4 flex items-center justify-between">
            <div>
              <h3 class="text-sm font-semibold text-white">macOS 菜单栏图标显示</h3>
              <p class="text-xs text-neutral-400 mt-0.5">控制顶部状态栏文字信息</p>
            </div>
            <select
              :value="props.trayPercentMode"
              @change="emit('update-tray-percent-mode', ($event.target as HTMLSelectElement).value as TrayPercentMode)"
              class="bg-[#18191c] text-white text-xs font-semibold px-3 py-1.5 rounded-lg border border-white/10 focus:outline-none"
            >
              <option value="always">始终显示配额百分比</option>
              <option value="today_tokens">显示今日 Token (如 🔥 602K)</option>
              <option value="alert">仅告警时显示</option>
              <option value="never">纯图标模式</option>
            </select>
          </div>

          <!-- 全局快捷键 -->
          <div class="bg-[#202126] border border-white/5 rounded-xl p-4 flex items-center justify-between">
            <div>
              <h3 class="text-sm font-semibold text-white">全局呼出快捷键</h3>
              <p class="text-xs text-neutral-400 mt-0.5">随时随地快速唤出或收起面板</p>
            </div>
            <kbd class="px-2.5 py-1 text-xs font-mono font-semibold rounded bg-white/10 text-neutral-200 border border-white/10 shadow-inner">
              ⌘ Shift A
            </kbd>
          </div>

          <!-- 检查更新 -->
          <div class="bg-[#202126] border border-white/5 rounded-xl p-4 flex items-center justify-between">
            <div>
              <div class="flex items-center gap-2">
                <h3 class="text-sm font-semibold text-white">检查新版本</h3>
                <!-- Status badge: 已是最新版本 -->
                <span
                  v-if="updateCheckStatus === 'latest'"
                  class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-[#00FF88]/15 text-[#00FF88] border border-[#00FF88]/25 animate-fadeIn"
                >
                  <Check class="w-3 h-3" />
                  当前已是最新版本
                </span>
                <!-- Status badge: 检查失败 -->
                <span
                  v-else-if="updateCheckStatus === 'error'"
                  class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-rose-500/15 text-rose-400 border border-rose-500/25 animate-fadeIn"
                >
                  检查失败，请检查网络
                </span>
              </div>
              <p class="text-xs text-neutral-400 mt-0.5">
                当前版本: v{{ appVersion }}
                <span v-if="lastCheckedTime" class="text-neutral-500 ml-1.5">
                  · 刚刚检查于 {{ lastCheckedTime }}
                </span>
              </p>
            </div>
            <button
              @click="checkForUpdate"
              :disabled="isCheckingUpdate"
              class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-semibold rounded-lg transition-all cursor-pointer"
              :class="[
                updateCheckStatus === 'latest'
                  ? 'bg-[#00FF88]/15 text-[#00FF88] border border-[#00FF88]/30 hover:bg-[#00FF88]/25'
                  : 'bg-white/10 hover:bg-white/20 text-white',
                isCheckingUpdate ? 'opacity-70 cursor-not-allowed' : ''
              ]"
            >
              <Check v-if="updateCheckStatus === 'latest' && !isCheckingUpdate" class="w-3.5 h-3.5 text-[#00FF88]" />
              <RefreshCw v-else class="w-3.5 h-3.5" :class="{ 'animate-spin': isCheckingUpdate }" />
              <span>
                {{ isCheckingUpdate ? '检测中...' : updateCheckStatus === 'latest' ? '已是最新版' : updateCheckStatus === 'error' ? '重试检查' : '在线检查更新' }}
              </span>
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Update Modal -->
    <UpdateModal
      v-if="showUpdateModal && currentUpdateInfo"
      :update-info="currentUpdateInfo"
      @close="showUpdateModal = false"
    />
  </div>
</template>
