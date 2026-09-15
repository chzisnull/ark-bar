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

export type ProviderType = 'volcengine' | 'antigravity' | 'grok' | 'codex';

/** 菜单栏百分比显示模式：始终显示 / 仅告警(≥75%或断连)时显示 / 纯图标 */
export type TrayPercentMode = 'always' | 'alert' | 'never';

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
}
