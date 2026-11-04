import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Calendar,
  Camera,
  Clock,
  ExternalLink,
  FolderOpen,
  FolderSearch,
  Newspaper,
  Play,
  Puzzle,
  RefreshCw,
  Search,
  ShieldCheck,
  SlidersHorizontal,
  Tag,
  Store,
  Rocket,
  Download,
} from "lucide-react";
import { api, errText } from "../api";
import { useApp } from "../state";
import { EditionSwitch } from "../components/ui";
import InstallDialog from "../components/InstallDialog";
import {
  EDITION_TITLE,
  SOURCE_TITLE,
  STEAM_APPID,
  formatClock,
  formatDate,
  formatDuration,
  formatUnixDate,
  plural,
} from "../format";
import type { Edition, NewsResult } from "../types";

function HeroImage({ edition }: { edition: Edition }) {
  // Официальная обложка из магазина Steam загружается по сети; без интернета — градиент.
  const [ok, setOk] = useState(true);
  useEffect(() => setOk(true), [edition]);
  if (!ok) return null;
  return (
    <img
      className="hero-img"
      src={`https://cdn.cloudflare.steamstatic.com/steam/apps/${STEAM_APPID[edition]}/library_hero.jpg`}
      onError={() => setOk(false)}
      alt=""
      draggable={false}
    />
  );
}

export default function HomePage() {
  const { config, setConfig, edition, info, running, play, launching, detect, toast, go, busy, refreshInfo, background } =
    useApp();
  const [news, setNews] = useState<NewsResult | null>(null);
  const [newsError, setNewsError] = useState<string | null>(null);
  const [newsLoading, setNewsLoading] = useState(false);
  const [detecting, setDetecting] = useState(false);
  const [showInstall, setShowInstall] = useState(false);
  const gi = info[edition];
  const stats = config?.stats[edition];
  const inst = config?.installs[edition];
  const activeMods = config?.mods.filter((m) => m.enabled && m.edition === edition).length ?? 0;

  const loadNews = async () => {
    if (!config?.launcher.load_news) return;
    setNewsLoading(true);
    setNewsError(null);
    try {
      setNews(await api.news(edition));
    } catch (e) {
      setNewsError(errText(e));
    } finally {
      setNewsLoading(false);
    }
  };

  useEffect(() => {
    loadNews();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [edition, config?.launcher.load_news]);

  const switchEdition = async (e: Edition) => {
    const c = await busy(() => api.setEdition(e));
    if (c) setConfig(c);
  };

  const chooseFolder = async () => {
    const dir = await open({ directory: true, title: "Выберите папку, где лежит GTA5.exe или GTA5_Enhanced.exe" });
    if (typeof dir !== "string") return;
    try {
      const c = await api.selectGameFolder(dir);
      setConfig(c);
      await refreshInfo();
      toast("success", `${EDITION_TITLE[c.selected_edition]}: путь сохранён`, dir);
    } catch (e) {
      toast("error", "Это не папка GTA V", errText(e));
    }
  };

  const runDetect = async () => {
    setDetecting(true);
    await detect(false);
    setDetecting(false);
  };

  const verify = async () => {
    const r = await busy(() => api.verify(edition));
    if (r) toast(r.started ? "success" : "info", "Проверка целостности", r.message);
  };

  const shot = async () => {
    const p = await busy(() => api.screenshot(true));
    if (p) toast("success", "Скриншот сохранён", p, { label: "Открыть", run: () => api.openFile(p) });
  };

  const isRunningThis = running.running && running.edition === edition;
  const status = !inst
    ? { cls: "bad", text: "Не найдена" }
    : gi?.installed
      ? { cls: "ok", text: "Установлена" }
      : { cls: "warn", text: "Нет файлов игры" };

  return (
    <div className="home">
      <section className={`hero ${background ? "hero-custom" : ""}`}>
        {!background && <HeroImage edition={edition} />}
        <div className="hero-shade" />
        <div className="hero-content">
          <div className="hero-top">
            <EditionSwitch value={edition} onChange={switchEdition} />
            <span className={`status-pill ${status.cls}`}>{status.text}</span>
          </div>
          <h1 className="hero-title">
            Grand Theft Auto V<span>{edition === "legacy" ? "Legacy" : "Enhanced"}</span>
          </h1>

          {inst ? (
            <>
              <div className="hero-meta">
                <span>
                  <Store size={15} /> {SOURCE_TITLE[inst.source]}
                </span>
                <span>
                  <Tag size={15} /> {gi?.version ? `Версия ${gi.version}` : "Версия не определена"}
                </span>
                <span className="path" title={inst.path}>
                  <FolderOpen size={15} /> {inst.path}
                </span>
              </div>
              {gi && gi.missing.length > 0 && (
                <div className="alert alert-warn">
                  Не найдены файлы: {gi.missing.join(", ")}. Проверьте папку или целостность файлов.
                </div>
              )}
              <div className="hero-actions">
                {isRunningThis ? (
                  <>
                    <div className="play-running">
                      <span className="pulse" /> В игре · {formatClock(running.session_seconds)}
                    </div>
                    <button className="btn btn-lg" onClick={shot} title="Сочетание в игре: Ctrl+Alt+S">
                      <Camera size={18} /> Сделать скриншот
                    </button>
                  </>
                ) : (
                  <button className="btn-play" disabled={launching || !gi?.installed || running.running} onClick={play}>
                    <Play size={26} fill="currentColor" />
                    {launching ? "ЗАПУСК…" : "ИГРАТЬ"}
                  </button>
                )}
                <button className="btn btn-glass" onClick={verify} disabled={running.running}>
                  <ShieldCheck size={17} /> Проверить файлы
                </button>
                <button className="btn btn-glass" onClick={() => busy(() => api.openLocation("game", edition))}>
                  <FolderOpen size={17} /> Папка игры
                </button>
              </div>
            </>
          ) : (
            <div className="hero-find">
              <p>
                Лаунчер ищет игру в Steam, Epic Games и Rockstar Games Launcher. Если игры ещё нет на компьютере, нажмите
                «Установить GTA V» — лаунчер проверит место на дисках и откроет установку в официальном клиенте.
              </p>
              <div className="hero-actions">
                <button className="btn-play" onClick={runDetect} disabled={detecting}>
                  <Search size={24} /> {detecting ? "ПОИСК…" : "НАЙТИ GTA V"}
                </button>
                <button className="btn btn-glass" onClick={() => setShowInstall(true)}>
                  <Download size={17} /> Установить GTA V
                </button>
                <button className="btn btn-glass" onClick={chooseFolder}>
                  <FolderSearch size={17} /> Указать папку
                </button>
              </div>
            </div>
          )}
        </div>
      </section>

      <div className="home-grid">
        <div className="home-left">
          <div className="stat-row">
            <div className="stat-card">
              <Clock size={20} />
              <div>
                <small>Время в игре</small>
                <b>{formatDuration(stats?.seconds ?? 0)}</b>
              </div>
            </div>
            <div className="stat-card">
              <Calendar size={20} />
              <div>
                <small>Последний запуск</small>
                <b>{formatDate(stats?.last_launch)}</b>
              </div>
            </div>
            <div className="stat-card">
              <Rocket size={20} />
              <div>
                <small>Запусков</small>
                <b>{stats?.launches ?? 0}</b>
              </div>
            </div>
          </div>
          <p className="hint">
            Время учитывается, пока лаунчер открыт или свёрнут в трей: он видит процесс игры, даже если игра запущена не
            через него.
          </p>

          <div className="quick-grid">
            <button className="quick-card" onClick={() => go("mods")}>
              <Puzzle size={24} />
              <div>
                <b>Моды</b>
                <small>
                  {activeMods > 0
                    ? `${activeMods} ${plural(activeMods, "включён", "включено", "включено")}`
                    : "Нет включённых модов"}
                </small>
              </div>
            </button>
            <button className="quick-card" onClick={() => go("settings")}>
              <SlidersHorizontal size={24} />
              <div>
                <b>Настройки игры</b>
                <small>{gi?.settings_exists ? "Графика, экран, профили" : "Появятся после первого запуска"}</small>
              </div>
            </button>
            {inst && (
              <button className="quick-card" onClick={chooseFolder}>
                <FolderSearch size={24} />
                <div>
                  <b>Другая папка</b>
                  <small>Выбрать установку вручную</small>
                </div>
              </button>
            )}
          </div>
        </div>

        <section className="card news">
          <div className="card-head">
            <h3>
              <Newspaper size={18} /> Новости GTA V
            </h3>
            {config?.launcher.load_news && (
              <button className="icon-btn" onClick={loadNews} title="Обновить">
                <RefreshCw size={15} className={newsLoading ? "spin" : ""} />
              </button>
            )}
          </div>
          {!config?.launcher.load_news && <p className="muted">Загрузка новостей отключена в настройках лаунчера.</p>}
          {config?.launcher.load_news && newsError && !news && (
            <p className="muted">Не удалось загрузить новости: {newsError}</p>
          )}
          {news?.offline && <p className="muted small">Нет соединения — показаны сохранённые новости.</p>}
          {config?.launcher.load_news && !news && !newsError && <div className="skeleton-list" />}
          <ul className="news-list">
            {news?.items.map((n) => (
              <li key={n.url}>
                <button onClick={() => api.openUrl(n.url).catch((e) => toast("error", errText(e)))}>
                  <small>
                    {formatUnixDate(n.date)} · {n.source}
                  </small>
                  <b>{n.title}</b>
                  {n.summary && <p>{n.summary}</p>}
                  <ExternalLink size={13} className="news-ext" />
                </button>
              </li>
            ))}
          </ul>
          {news && news.items.length === 0 && <p className="muted">Новостей пока нет.</p>}
        </section>
      </div>
      {showInstall && <InstallDialog onClose={() => setShowInstall(false)} />}
    </div>
  );
}
