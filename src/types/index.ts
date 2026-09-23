export interface EnvironmentStatus {
  has_node: boolean;
  node_version: string | null;
  has_npm: boolean;
  npm_version: string | null;
  has_arkcli: boolean;
  arkcli_version: string | null;
  logged_in: boolean;
  user_name: string | null;
  account_id: string | null;
  active_profile: string | null;
  error_message: string | null;
}

export interface CommandResult {
  success: boolean;
  message: string;
  details: string | null;
}

export interface Period {
  label: 'session' | 'weekly' | 'monthly' | string;
  percent: number;
  reset_at?: string;
  used?: number;
  total?: number;
}

export interface PlanItem {
  product: string;
  edition: string;
  seat_id?: string;
  subscribed: boolean;
  periods?: Period[];
  error?: string;
}

export interface UsagePlanResponse {
  viewer?: {
    auth_method: string;
    user_id: string;
    user_name: string;
    account_id: string;
    profile: string;
    tenant: string;
    region: string;
    project_name: string;
  };
  items?: PlanItem[];
}

export interface UpdateInfo {
  has_update: boolean;
  current_version: string;
  latest_version: string;
  release_url: string;
  release_notes: string;
  download_url?: string;
}

export interface UpdateProgress {
  stage: 'preparing' | 'downloading' | 'extracting' | 'installing' | 'restarting' | 'error' | 'done';
  percent: number;
  current_bytes: number;
  total_bytes: number;
  message: string;
}

export type ProviderType = 'volcengine' | 'antigravity' | 'grok' | 'codex' | 'teamo';

export type NotchPosition = 'right' | 'left' | 'top' | 'hidden';
export type NotchMetric = 'session' | 'weekly' | 'monthly' | 'today_tokens' | 'balance';
export type NotchScale = 'compact' | 'normal' | 'large';
export type SettingsNavTab = 'accounts' | 'appearance' | 'notifications' | 'general';

export interface ProviderTabConfig {
  id: ProviderType;
  name: string;
  visible: boolean;
  notch_metric?: NotchMetric;
  model_filter?: string;
  notification_enabled?: boolean;
}

/** 菜单栏显示模式：始终显示配额百分比 / 仅告警(≥75%或断连) / 纯图标 / 今日 Token / 近5小时 Token */
export type TrayPercentMode = 'always' | 'alert' | 'never' | 'today_tokens' | 'session_tokens';

export interface DailyTokenRecord {
  date: string;
  total_tokens: number;
  input_tokens: number;
  output_tokens: number;
  cache_hit_tokens: number;
  request_count: number;
}

export interface ProviderTokenSummary {
  session_5h_tokens: number;
  today_tokens: number;
  this_week_tokens: number;
  this_month_tokens: number;
  total_tokens?: number;
  input_tokens: number;
  output_tokens: number;
  cache_hit_tokens: number;
  cache_hit_rate: number;
  this_month_cache_hit_tokens?: number;
  this_month_cache_hit_rate?: number;
  supports_cache_stats?: boolean;
  request_count: number;
  daily_history: DailyTokenRecord[];
  data_source_type: string;
  updated_at: string;
}

export interface ProviderAmount {
  value: number;
  currency: string;
}

export interface ProviderUsageDataExtension {
  balance?: ProviderAmount | null;
  today_cost?: ProviderAmount | null;
  month_cost?: ProviderAmount | null;
}

export interface ProviderQuotaPeriod {
  label: string;
  name: string;
  used_percent: number;
  remaining_percent: number;
  reset_at?: string;
  used?: number;
  total?: number;
  description?: string;
}

export interface ProviderPlanGroup {
  group_name: string;
  edition?: string;
  periods: ProviderQuotaPeriod[];
}

export interface ProviderAccountInfo {
  user_name?: string;
  email?: string;
  account_id?: string;
  plan_name?: string;
}

export interface ProviderUsageData {
  provider: ProviderType;
  provider_name: string;
  icon: string;
  is_connected: boolean;
  status_message?: string;
  error_message?: string;
  account_info?: ProviderAccountInfo;
  groups: ProviderPlanGroup[];
  primary_session_percent?: number;
  primary_reset_at?: string;
  console_url?: string;
  token_summary?: ProviderTokenSummary | null;
  extension?: ProviderUsageDataExtension;
}
