import { invoke } from "@tauri-apps/api/core";
import type {
  Addons,
  BackupInfo,
  Config,
  DetectSummary,
  Edition,
  EnableResult,
  ExternalItem,
  GameInfo,
  GameSettings,
  LauncherSettings,
  ModInspect,
  ModWarning,
  NewsResult,
  RunningStatus,
  SystemCheck,
  Source,
  UpdateInfo,
  VerifyResult,
} from "./types";

/** Типизированные вызовы команд Rust-бэкенда. Ошибки приходят строкой на русском. */
export const api = {
  getConfig: () => invoke<Config>("get_config"),
  appVersion: () => invoke<string>("app_version"),
  gameInfo: (edition: Edition) => invoke<GameInfo>("get_game_info", { edition }),
  runningStatus: () => invoke<RunningStatus>("get_running_status"),

  autoDetect: (overwrite: boolean) => invoke<DetectSummary>("auto_detect", { overwrite }),
  selectGameFolder: (path: string) => invoke<Config>("select_game_folder", { path }),
  clearGamePath: (edition: Edition) => invoke<Config>("clear_game_path", { edition }),
  setGameSource: (edition: Edition, source: Source) => invoke<Config>("set_game_source", { edition, source }),
  setEdition: (edition: Edition) => invoke<Config>("set_selected_edition", { edition }),

  systemCheck: () => invoke<SystemCheck>("system_check"),
  installGame: (edition: Edition, store: string) =>
    invoke<{ opened: boolean; message: string }>("install_game", { edition, store }),

  launch: (edition: Edition) => invoke<string>("launch_game", { edition }),
  verify: (edition: Edition) => invoke<VerifyResult>("verify_game", { edition }),
  preflight: (edition: Edition) =>
    invoke<{ managed_enabled: number; external: number }>("mods_preflight", { edition }),

  inspectMod: (path: string, edition: Edition) => invoke<ModInspect>("inspect_mod", { path, edition }),
  installMod: (req: {
    edition: Edition;
    source_path: string;
    name: string;
    version: string;
    description: string;
    category: string;
    target_subdir: string;
    enable: boolean;
  }) => invoke<EnableResult>("install_mod", { req }),
  enableMod: (id: string, force: boolean) => invoke<EnableResult>("enable_mod", { id, force }),
  disableMod: (id: string) => invoke<void>("disable_mod", { id }),
  removeMod: (id: string) => invoke<void>("remove_mod", { id }),
  disableAllMods: (edition: Edition) => invoke<number>("disable_all_mods", { edition }),
  updateMod: (id: string, name: string, version: string, description: string, category: string) =>
    invoke<void>("update_mod", { id, name, version, description, category }),
  scanExternal: (edition: Edition) => invoke<ExternalItem[]>("scan_external_mods", { edition }),
  checkMods: (edition: Edition) => invoke<ModWarning[]>("check_mods", { edition }),

  readSettings: (edition: Edition) => invoke<GameSettings>("read_game_settings", { edition }),
  writeSettings: (edition: Edition, changes: Record<string, string>) =>
    invoke<number>("write_game_settings", { edition, changes }),
  readCommandline: (edition: Edition) => invoke<string>("read_commandline", { edition }),
  writeCommandline: (edition: Edition, text: string) => invoke<void>("write_commandline", { edition, text }),
  listSettingsProfiles: (edition: Edition) => invoke<string[]>("list_settings_profiles", { edition }),
  saveSettingsProfile: (edition: Edition, name: string) => invoke<void>("save_settings_profile", { edition, name }),
  applySettingsProfile: (edition: Edition, name: string) =>
    invoke<void>("apply_settings_profile", { edition, name }),
  deleteSettingsProfile: (edition: Edition, name: string) =>
    invoke<void>("delete_settings_profile", { edition, name }),
  createBackup: (edition: Edition) => invoke<string>("create_config_backup", { edition }),
  listBackups: (edition: Edition) => invoke<BackupInfo[]>("list_config_backups", { edition }),
  restoreBackup: (edition: Edition, id: string) => invoke<void>("restore_config_backup", { edition, id }),
  deleteBackup: (edition: Edition, id: string) => invoke<void>("delete_config_backup", { edition, id }),

  createProfile: (name: string, avatar: string, edition: Edition) =>
    invoke<Config>("create_profile", { name, avatar, edition }),
  updateProfile: (id: string, name: string, avatar: string, edition: Edition, theme: string) =>
    invoke<Config>("update_profile", { id, name, avatar, edition, theme }),
  deleteProfile: (id: string) => invoke<Config>("delete_profile", { id }),
  switchProfile: (id: string) => invoke<{ config: Config; notes: string[] }>("switch_profile", { id }),
  setProfileMods: (id: string, capture: boolean) => invoke<Config>("set_profile_mods", { id, capture }),
  loadAvatar: (path: string) => invoke<string>("load_avatar", { path }),

  updateLauncherSettings: (settings: LauncherSettings) => invoke<Config>("update_launcher_settings", { settings }),
  getBackground: () => invoke<string | null>("get_background"),
  setBackground: (path: string | null) => invoke<string | null>("set_background", { path }),
  news: (edition: Edition) => invoke<NewsResult>("fetch_news", { edition }),
  checkUpdate: () => invoke<UpdateInfo>("check_launcher_update"),
  screenshot: (hideWindow: boolean) => invoke<string>("take_screenshot", { hideWindow }),
  addons: (edition: Edition) => invoke<Addons>("list_addons", { edition }),
  openLocation: (kind: string, edition: Edition) => invoke<void>("open_location", { kind, edition }),
  openFile: (path: string) => invoke<void>("open_file", { path }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  quit: () => invoke<void>("quit_app"),
};

export function errText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
