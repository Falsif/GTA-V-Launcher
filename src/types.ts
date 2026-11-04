export type Edition = "legacy" | "enhanced";
export type Source = "steam" | "epic" | "rockstar" | "unknown";

export interface GameInstall {
  path: string;
  source: Source;
  epic_id: string | null;
  auto: boolean;
}

export interface EditionStats {
  seconds: number;
  launches: number;
  last_launch: string | null;
}

export interface LauncherSettings {
  theme: string;
  has_background: boolean;
  background_dim: number;
  close_to_tray: boolean;
  minimize_on_launch: boolean;
  warn_mods_before_launch: boolean;
  check_updates_on_start: boolean;
  load_news: boolean;
  screenshot_hotkey: boolean;
  reduce_animations: boolean;
  update_repo: string;
}

export interface Profile {
  id: string;
  name: string;
  avatar: string;
  edition: Edition;
  theme: string;
  mod_ids: string[] | null;
  launches: number;
  seconds: number;
  last_launch: string | null;
  created_at: string;
}

export interface ModEntry {
  id: string;
  name: string;
  version: string;
  description: string;
  category: string;
  edition: Edition;
  enabled: boolean;
  files: string[];
  size: number;
  installed_at: string;
  source_name: string;
}

export interface Config {
  selected_edition: Edition;
  installs: { legacy: GameInstall | null; enhanced: GameInstall | null };
  stats: { legacy: EditionStats; enhanced: EditionStats };
  launcher: LauncherSettings;
  profiles: Profile[];
  active_profile: string | null;
  mods: ModEntry[];
  first_run_done: boolean;
}

export interface GameInfo {
  edition: Edition;
  installed: boolean;
  path: string | null;
  source: Source | null;
  auto: boolean;
  version: string | null;
  missing: string[];
  documents: string;
  settings_exists: boolean;
}

export interface RunningStatus {
  running: boolean;
  edition: Edition | null;
  session_seconds: number;
}

export interface Found {
  edition: Edition;
  install: GameInstall;
}

export interface DetectSummary {
  found: Found[];
  applied: Edition[];
  config: Config;
}

export interface ModInspect {
  kind: "zip" | "file" | "folder";
  name: string;
  version: string;
  description: string;
  category: string;
  target_subdir: string;
  files: string[];
  file_count: number;
  total_size: number;
  warnings: string[];
}

export interface Conflict {
  file: string;
  mod_id: string;
  mod_name: string;
}

export interface EnableResult {
  ok: boolean;
  conflicts: Conflict[];
  mod_id: string;
}

export interface ExternalItem {
  path: string;
  kind: string;
  size: number;
}

export interface ModWarning {
  level: "info" | "warn" | "error";
  text: string;
}

export interface GameSettings {
  path: string;
  exists: boolean;
  values: Record<string, string>;
}

export interface BackupInfo {
  id: string;
  files: string[];
}

export interface NewsItem {
  title: string;
  url: string;
  summary: string;
  date: number;
  source: string;
}

export interface NewsResult {
  items: NewsItem[];
  offline: boolean;
  fetched_at: number;
}

export interface UpdateInfo {
  current: string;
  latest: string;
  available: boolean;
  url: string;
  notes: string;
}

export interface Addons {
  official: string[];
  modded: string[];
  mods_folder: boolean;
}

export interface VerifyResult {
  started: boolean;
  message: string;
}
