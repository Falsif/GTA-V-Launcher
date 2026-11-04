import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, errText } from "./api";
import type { Config, Edition, GameInfo, RunningStatus } from "./types";
import { EDITION_TITLE, plural } from "./format";

export type ToastKind = "success" | "error" | "info" | "warn";
export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  text?: string;
  action?: { label: string; run: () => void };
}

export interface DialogButton {
  id: string;
  label: string;
  kind?: "primary" | "danger" | "ghost";
}
export interface DialogRequest {
  title: string;
  body: ReactNode;
  buttons: DialogButton[];
}

export type Page = "home" | "mods" | "settings" | "profiles" | "tools" | "launcher";

interface Ctx {
  config: Config | null;
  setConfig: (c: Config) => void;
  reload: () => Promise<void>;
  edition: Edition;
  info: Record<Edition, GameInfo | null>;
  refreshInfo: () => Promise<void>;
  running: RunningStatus;
  background: string | null;
  setBackground: (b: string | null) => void;
  page: Page;
  go: (p: Page) => void;
  toast: (kind: ToastKind, title: string, text?: string, action?: Toast["action"]) => void;
  toasts: Toast[];
  dismiss: (id: number) => void;
  ask: (req: DialogRequest) => Promise<string | null>;
  dialog: (DialogRequest & { resolve: (v: string | null) => void }) | null;
  play: () => Promise<void>;
  launching: boolean;
  detect: (overwrite: boolean, silent?: boolean) => Promise<void>;
  busy: <T>(fn: () => Promise<T>, okText?: string) => Promise<T | undefined>;
}

const AppCtx = createContext<Ctx | null>(null);

export function useApp(): Ctx {
  const c = useContext(AppCtx);
  if (!c) throw new Error("useApp вне AppProvider");
  return c;
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [config, setConfig] = useState<Config | null>(null);
  const [info, setInfo] = useState<Record<Edition, GameInfo | null>>({ legacy: null, enhanced: null });
  const [running, setRunning] = useState<RunningStatus>({ running: false, edition: null, session_seconds: 0 });
  const [background, setBackground] = useState<string | null>(null);
  const [page, setPage] = useState<Page>("home");
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [dialog, setDialog] = useState<Ctx["dialog"]>(null);
  const [launching, setLaunching] = useState(false);
  const toastId = useRef(1);
  const configRef = useRef<Config | null>(null);
  configRef.current = config;

  const toast = useCallback<Ctx["toast"]>((kind, title, text, action) => {
    const id = toastId.current++;
    setToasts((t) => [...t.slice(-4), { id, kind, title, text, action }]);
    const ttl = kind === "error" ? 8000 : action ? 9000 : 4500;
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), ttl);
  }, []);
  const dismiss = useCallback((id: number) => setToasts((t) => t.filter((x) => x.id !== id)), []);

  const ask = useCallback<Ctx["ask"]>(
    (req) => new Promise((resolve) => setDialog({ ...req, resolve: (v) => (setDialog(null), resolve(v)) })),
    [],
  );

  const busy = useCallback<Ctx["busy"]>(
    async (fn, okText) => {
      try {
        const r = await fn();
        if (okText) toast("success", okText);
        return r;
      } catch (e) {
        toast("error", "Ошибка", errText(e));
        return undefined;
      }
    },
    [toast],
  );

  const refreshInfo = useCallback(async () => {
    const [legacy, enhanced] = await Promise.all([api.gameInfo("legacy"), api.gameInfo("enhanced")]);
    setInfo({ legacy, enhanced });
  }, []);

  const reload = useCallback(async () => {
    setConfig(await api.getConfig());
    await refreshInfo();
  }, [refreshInfo]);

  const detect = useCallback<Ctx["detect"]>(
    async (overwrite, silent = false) => {
      try {
        const r = await api.autoDetect(overwrite);
        setConfig(r.config);
        await refreshInfo();
        if (silent && r.found.length === 0) return;
        if (r.found.length === 0) {
          toast("warn", "GTA V не найдена", "Укажите папку игры вручную — там, где лежит GTA5.exe или GTA5_Enhanced.exe.");
        } else {
          const names = r.found.map((f) => EDITION_TITLE[f.edition]).join(", ");
          toast("success", `Найдено: ${names}`, r.applied.length ? "Путь к игре сохранён." : "Пути уже были сохранены.");
        }
      } catch (e) {
        toast("error", "Ошибка поиска", errText(e));
      }
    },
    [refreshInfo, toast],
  );

  // Запуск игры с предупреждением о модах.
  const play = useCallback(async () => {
    const cfg = configRef.current;
    if (!cfg || launching) return;
    const edition = cfg.selected_edition;
    if (!cfg.installs[edition]) {
      toast("warn", `${EDITION_TITLE[edition]} не найдена`, "Нажмите «Найти GTA V» или укажите папку вручную.");
      setPage("home");
      return;
    }
    setLaunching(true);
    try {
      if (cfg.launcher.warn_mods_before_launch) {
        const pf = (await api.preflight(edition).catch(() => null)) ?? { managed_enabled: 0, external: 0 };
        const total = pf.managed_enabled + pf.external;
        if (total > 0) {
          const buttons: DialogButton[] = [{ id: "cancel", label: "Отмена", kind: "ghost" }];
          if (pf.managed_enabled > 0) buttons.push({ id: "vanilla", label: "Отключить моды лаунчера и играть" });
          buttons.push({ id: "play", label: "Играть с модами", kind: "primary" });
          const answer = await ask({
            title: "Обнаружены моды",
            body: (
              <>
                <p>
                  В папке игры {total} {plural(total, "модификация", "модификации", "модификаций")}
                  {pf.external > 0 ? ` (из них ${pf.external} установлено вручную)` : ""}.
                </p>
                <p className="dialog-warning">
                  Моды предназначены только для сюжетного режима. Не заходите с ними в GTA Online — это нарушает
                  правила Rockstar и может привести к блокировке.
                </p>
              </>
            ),
            buttons,
          });
          if (answer === "cancel" || answer === null) return;
          if (answer === "vanilla") {
            const n = await api.disableAllMods(edition);
            toast("info", `Отключено модов: ${n}`, pf.external ? "Моды, установленные вручную, остались на месте." : undefined);
            setConfig(await api.getConfig());
          }
        }
      }
      const msg = await api.launch(edition);
      toast("success", msg, "Если игра не открылась, проверьте окно клиента — возможно, нужен вход в аккаунт.");
      setConfig(await api.getConfig());
      if (cfg.launcher.minimize_on_launch) {
        setTimeout(() => getCurrentWindow().minimize(), 1200);
      }
    } catch (e) {
      toast("error", "Не удалось запустить игру", errText(e));
    } finally {
      setLaunching(false);
    }
  }, [ask, launching, toast]);

  const playRef = useRef(play);
  playRef.current = play;

  useEffect(() => {
    (async () => {
      try {
        const cfg = await api.getConfig();
        setConfig(cfg);
        if (cfg.launcher.has_background) setBackground(await api.getBackground());
        setRunning(await api.runningStatus());
        await refreshInfo();
        if (!cfg.first_run_done || (!cfg.installs.legacy && !cfg.installs.enhanced)) {
          await detect(false, cfg.first_run_done);
        }
        if (cfg.launcher.check_updates_on_start) {
          api
            .checkUpdate()
            .then((u) => {
              if (u.available)
                toast("info", `Доступна новая версия лаунчера ${u.latest}`, `Сейчас установлена ${u.current}.`, {
                  label: "Скачать",
                  run: () => api.openUrl(u.url),
                });
            })
            .catch(() => {});
        }
      } catch (e) {
        toast("error", "Ошибка загрузки настроек", errText(e));
      }
    })();
    const un: Promise<() => void>[] = [
      listen<RunningStatus>("game-status", (e) => {
        setRunning((prev) => {
          if (prev.running && !e.payload.running) {
            api.getConfig().then(setConfig);
          }
          return e.payload;
        });
      }),
      listen("tray-play", () => playRef.current()),
      listen<{ ok: boolean; path?: string; error?: string }>("screenshot", (e) => {
        if (e.payload.ok) toast("success", "Скриншот сохранён", e.payload.path);
        else toast("error", "Скриншот не сделан", e.payload.error);
      }),
    ];
    return () => un.forEach((p) => p.then((f) => f()));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Тема и анимации
  useEffect(() => {
    if (!config) return;
    document.documentElement.dataset.theme = config.launcher.theme;
    document.documentElement.classList.toggle("reduce-motion", config.launcher.reduce_animations);
  }, [config?.launcher.theme, config?.launcher.reduce_animations]);

  const value = useMemo<Ctx>(
    () => ({
      config,
      setConfig,
      reload,
      edition: config?.selected_edition ?? "legacy",
      info,
      refreshInfo,
      running,
      background,
      setBackground,
      page,
      go: setPage,
      toast,
      toasts,
      dismiss,
      ask,
      dialog,
      play,
      launching,
      detect,
      busy,
    }),
    [config, reload, info, refreshInfo, running, background, page, toast, toasts, dismiss, ask, dialog, play, launching, detect, busy],
  );

  return <AppCtx.Provider value={value}>{children}</AppCtx.Provider>;
}
