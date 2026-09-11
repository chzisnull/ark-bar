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
