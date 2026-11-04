import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Download, FolderSearch, ImagePlus, Info, RefreshCw, Search, Trash2, X } from "lucide-react";
import { api, errText } from "../api";
import { useApp } from "../state";
import { CopyText, PageHeader, Toggle } from "../components/ui";
import { EDITION_TITLE, SOURCE_TITLE } from "../format";
import type { Edition, LauncherSettings, Source, UpdateInfo } from "../types";

export const THEMES = [
  { id: "red", label: "Красная", color: "#e01e26" },
  { id: "blue", label: "Синяя", color: "#2f7bff" },
  { id: "green", label: "Зелёная", color: "#22b45a" },
  { id: "purple", label: "Фиолетовая", color: "#9b4dff" },
  { id: "orange", label: "Оранжевая", color: "#ff7a1a" },
  { id: "mono", label: "Графит", color: "#c9c9cf" },
];

const TOGGLES: [keyof LauncherSettings, string, string][] = [
  ["minimize_on_launch", "Сворачивать лаунчер при запуске игры", "Окно уйдёт в панель задач, время игры продолжит считаться."],
  ["close_to_tray", "Закрывать в системный трей", "Крестик скрывает окно в трей, выход — через меню значка."],
  ["warn_mods_before_launch", "Предупреждать о модах перед запуском", "Напоминание, что с модами нельзя заходить в GTA Online."],
  ["load_news", "Загружать новости", "Новости из Steam. Единственное, для чего главной странице нужен интернет."],
  ["check_updates_on_start", "Проверять обновления лаунчера при старте", "Через GitHub Releases, без установки в фоне."],
  ["screenshot_hotkey", "Скриншот по Ctrl+Alt+S в игре", "Работает, пока лаунчер открыт или свёрнут."],
  ["reduce_animations", "Меньше анимаций", "Для слабых компьютеров."],
];

export default function LauncherPage() {
  const { config, setConfig, info, busy, toast, background, setBackground, detect, refreshInfo } = useApp();
  const [version, setVersion] = useState("");
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [repo, setRepo] = useState(config?.launcher.update_repo ?? "");

  useEffect(() => {
    api.appVersion().then(setVersion);
  }, []);
  if (!config) return null;
  const s = config.launcher;

  const save = async (patch: Partial<LauncherSettings>) => {
    const c = await busy(() => api.updateLauncherSettings({ ...s, ...patch }));
    if (c) setConfig(c);
  };

  const pickBackground = async () => {
    const path = await open({ title: "Фоновое изображение", filters: [{ name: "Изображения", extensions: ["png", "jpg", "jpeg", "webp"] }] });
    if (typeof path !== "string") return;
    try {
      setBackground(await api.setBackground(path));
      setConfig(await api.getConfig());
    } catch (e) {
      toast("error", "Фон не установлен", errText(e));
    }
  };

  const removeBackground = async () => {
    await busy(() => api.setBackground(null));
    setBackground(null);
    setConfig(await api.getConfig());
  };

  const changeFolder = async () => {
    const dir = await open({ directory: true, title: "Папка GTA V" });
    if (typeof dir !== "string") return;
    try {
      setConfig(await api.selectGameFolder(dir));
      await refreshInfo();
      toast("success", "Путь сохранён", dir);
    } catch (e) {
      toast("error", "Это не папка GTA V", errText(e));
    }
  };

  const checkUpdate = async () => {
    setChecking(true);
    if (repo !== s.update_repo) await save({ update_repo: repo.trim() });
    try {
      const u = await api.checkUpdate();
      setUpdate(u);
      if (!u.available) toast("info", "У вас последняя версия", `Версия ${u.current}`);
    } catch (e) {
      toast("error", "Не удалось проверить обновления", errText(e));
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className="page">
      <PageHeader title="Настройки лаунчера" subtitle="Сохраняются автоматически" />
      <div className="settings-grid">
        <section className="card">
          <h3>Оформление</h3>
          <div className="setting-row">
            <span>Тема</span>
            <div className="swatches">
              {THEMES.map((t) => (
                <button
                  key={t.id}
                  className={`swatch ${s.theme === t.id ? "active" : ""}`}
                  style={{ background: t.color }}
                  title={t.label}
                  onClick={() => save({ theme: t.id })}
                />
              ))}
            </div>
          </div>
          <div className="setting-row">
            <span>Фоновое изображение</span>
            <div className="row-start">
              <button className="btn btn-sm" onClick={pickBackground}>
                <ImagePlus size={14} /> Выбрать
              </button>
              {background && (
                <button className="btn btn-sm btn-ghost" onClick={removeBackground}>
                  <X size={14} /> Убрать
                </button>
              )}
            </div>
          </div>
          {background && <img className="bg-preview" src={background} alt="" />}
          <div className="setting-row">
            <span>Затемнение фона: {s.background_dim}%</span>
            <input
              type="range"
              min={20}
              max={95}
              value={s.background_dim}
              onChange={(e) => setConfig({ ...config, launcher: { ...s, background_dim: +e.target.value } })}
              onMouseUp={(e) => save({ background_dim: +(e.target as HTMLInputElement).value })}
            />
          </div>
          <p className="muted small">
            Без своего фона на главной показывается официальная обложка игры из магазина Steam (нужен интернет).
          </p>
        </section>

        <section className="card">
          <h3>Поведение</h3>
          {TOGGLES.map(([key, label, hint]) => (
            <div key={key} className="setting-row toggle-row">
              <div>
                <span>{label}</span>
                <small className="muted">{hint}</small>
              </div>
              <Toggle checked={!!s[key]} onChange={(v) => save({ [key]: v } as Partial<LauncherSettings>)} />
            </div>
          ))}
        </section>

        <section className="card span2">
          <div className="card-head">
            <h3>Установки игры</h3>
            <div className="row-start">
              <button className="btn btn-sm" onClick={() => detect(true)}>
                <Search size={14} /> Найти заново
              </button>
              <button className="btn btn-sm" onClick={changeFolder}>
                <FolderSearch size={14} /> Указать папку
              </button>
            </div>
          </div>
          <table className="table">
            <thead>
              <tr>
                <th>Версия</th>
                <th>Папка</th>
                <th>Запуск через</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {(["legacy", "enhanced"] as Edition[]).map((e) => {
                const inst = config.installs[e];
                return (
                  <tr key={e}>
                    <td>
                      <b>{EDITION_TITLE[e]}</b>
                      <br />
                      <small className={info[e]?.installed ? "ok-text" : "muted"}>
                        {!inst ? "не найдена" : info[e]?.installed ? `найдена${inst.auto ? " автоматически" : " вручную"}` : "файлы не найдены"}
                      </small>
                    </td>
                    <td className="path-cell">
                      {inst ? (
                        <>
                          <span title={inst.path}>{inst.path}</span> <CopyText text={inst.path} />
                        </>
                      ) : (
                        "—"
                      )}
                    </td>
                    <td>
                      {inst && (
                        <select
                          value={inst.source}
                          onChange={async (ev) => {
                            const c = await busy(() => api.setGameSource(e, ev.target.value as Source));
                            if (c) setConfig(c);
                          }}
                        >
                          {(["steam", "epic", "rockstar"] as Source[]).map((src) => (
                            <option key={src} value={src}>
                              {SOURCE_TITLE[src]}
                            </option>
                          ))}
                          {inst.source === "unknown" && <option value="unknown">Не определён (PlayGTAV.exe)</option>}
                        </select>
                      )}
                    </td>
                    <td className="right">
                      {inst && (
                        <button
                          className="icon-btn danger"
                          title="Забыть путь"
                          onClick={async () => {
                            const c = await busy(() => api.clearGamePath(e));
                            if (c) {
                              setConfig(c);
                              refreshInfo();
                            }
                          }}
                        >
                          <Trash2 size={15} />
                        </button>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          <p className="muted small">
            Источник определяет, какой официальный клиент запустит игру. Лицензия и вход в аккаунт проверяются самим
            клиентом — лаунчер их не обходит.
          </p>
        </section>

        <section className="card">
          <h3>Обновления лаунчера</h3>
          <div className="setting-row">
            <span>Установленная версия</span>
            <b>{version}</b>
          </div>
          <label className="stack">
            <small className="muted">Репозиторий GitHub (владелец/репозиторий)</small>
            <input value={repo} onChange={(e) => setRepo(e.target.value)} onBlur={() => repo !== s.update_repo && save({ update_repo: repo.trim() })} />
          </label>
          <div className="row-start">
            <button className="btn" onClick={checkUpdate} disabled={checking}>
              <RefreshCw size={15} className={checking ? "spin" : ""} /> Проверить обновления
            </button>
            {update?.available && (
              <button className="btn btn-primary" onClick={() => api.openUrl(update.url)}>
                <Download size={15} /> Скачать {update.latest}
              </button>
            )}
          </div>
          {update?.available && update.notes && <pre className="notes">{update.notes}</pre>}
        </section>

        <section className="card">
          <h3>
            <Info size={18} /> О программе
          </h3>
          <p className="muted small">
            GTA V Launcher — неофициальная бесплатная программа без рекламы. Не связана с Rockstar Games и Take-Two
            Interactive. Grand Theft Auto и Grand Theft Auto V — товарные знаки Take-Two Interactive.
          </p>
          <p className="muted small">
            Лаунчер запускает игру только через официальные клиенты, не изменяет защиту игры и не работает с GTA Online.
          </p>
          <div className="row-start">
            <button className="btn btn-sm" onClick={() => busy(() => api.openLocation("data", config.selected_edition))}>
              Папка данных лаунчера
            </button>
            <button className="btn btn-sm btn-ghost" onClick={() => api.quit()}>
              Выйти из лаунчера
            </button>
          </div>
        </section>
      </div>
    </div>
  );
}
