import { useEffect, useMemo, useState } from "react";
import { FileCode2, FolderOpen, Gauge, Info, RotateCcw, Save, Trash2, Upload } from "lucide-react";
import { api, errText } from "../api";
import { useApp } from "../state";
import { NoGame, PageHeader, Toggle } from "../components/ui";
import { EDITION_TITLE } from "../format";
import type { GameSettings } from "../types";

type Opt = [string, string];
interface Field {
  key: string;
  label: string;
  options?: Opt[];
  hint?: string;
}

const Q3: Opt[] = [["0", "Обычное"], ["1", "Высокое"], ["2", "Очень высокое"]];
const Q4: Opt[] = [["0", "Обычное"], ["1", "Высокое"], ["2", "Очень высокое"], ["3", "Ультра"]];

const SCREEN: Field[] = [
  { key: "Windowed", label: "Режим экрана", options: [["0", "Полноэкранный"], ["1", "Оконный"], ["2", "Окно без рамки"]] },
  { key: "VSync", label: "Вертикальная синхронизация", options: [["0", "Выключена"], ["1", "Включена"], ["2", "Половина частоты"]] },
  { key: "RefreshRate", label: "Частота обновления, Гц" },
];

const GRAPHICS: Field[] = [
  { key: "TextureQuality", label: "Качество текстур", options: Q3 },
  { key: "ShadowQuality", label: "Качество теней", options: [["1", "Обычное"], ["2", "Высокое"], ["3", "Очень высокое"]] },
  { key: "ShaderQuality", label: "Качество шейдеров", options: Q3 },
  { key: "ReflectionQuality", label: "Качество отражений", options: Q4 },
  { key: "WaterQuality", label: "Качество воды", options: Q3 },
  { key: "ParticleQuality", label: "Качество частиц", options: Q3 },
  { key: "GrassQuality", label: "Качество травы", options: Q4 },
  { key: "PostFX", label: "Постобработка", options: Q4 },
  { key: "Tessellation", label: "Тесселяция", options: [["0", "Выкл."], ["1", "Обычная"], ["2", "Высокая"], ["3", "Очень высокая"]] },
  { key: "SSAO", label: "Затенение (SSAO)", options: [["0", "Выкл."], ["1", "Обычное"], ["2", "Высокое"]] },
  { key: "AnisotropicFiltering", label: "Анизотропная фильтрация", options: [["0", "Выкл."], ["2", "x2"], ["4", "x4"], ["8", "x8"], ["16", "x16"]] },
  { key: "MSAA", label: "Сглаживание MSAA", options: [["0", "Выкл."], ["2", "x2"], ["4", "x4"], ["8", "x8"]] },
  { key: "FXAA", label: "Сглаживание FXAA" },
  { key: "DX_Version", label: "Версия DirectX", options: [["0", "DirectX 10"], ["1", "DirectX 10.1"], ["2", "DirectX 11"]] },
];

const RESOLUTIONS: Opt[] = [
  ["1280x720", "1280 × 720"],
  ["1366x768", "1366 × 768"],
  ["1600x900", "1600 × 900"],
  ["1920x1080", "1920 × 1080 (Full HD)"],
  ["2560x1080", "2560 × 1080"],
  ["2560x1440", "2560 × 1440 (2K)"],
  ["3440x1440", "3440 × 1440"],
  ["3840x2160", "3840 × 2160 (4K)"],
];

const isBool = (v: string) => v === "true" || v === "false";

export default function GameSettingsPage() {
  const { edition, info, busy, toast, ask, running } = useApp();
  const [data, setData] = useState<GameSettings | null>(null);
  const [draft, setDraft] = useState<Record<string, string>>({});
  const [profiles, setProfiles] = useState<string[]>([]);
  const [profileName, setProfileName] = useState("");
  const [cmd, setCmd] = useState("");
  const [cmdSaved, setCmdSaved] = useState("");
  const [showAll, setShowAll] = useState(false);
  const hasGame = !!info[edition]?.path;

  const load = async () => {
    try {
      const s = await api.readSettings(edition);
      setData(s);
      setDraft(s.values);
      setProfiles(await api.listSettingsProfiles(edition));
      if (hasGame) {
        const c = await api.readCommandline(edition).catch(() => "");
        setCmd(c);
        setCmdSaved(c);
      }
    } catch (e) {
      toast("error", "Не удалось прочитать настройки", errText(e));
    }
  };

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [edition, hasGame]);

  const changes = useMemo(() => {
    const c: Record<string, string> = {};
    if (!data) return c;
    for (const [k, v] of Object.entries(draft)) if (data.values[k] !== v) c[k] = v;
    return c;
  }, [draft, data]);
  const changed = Object.keys(changes).length;

  const set = (k: string, v: string) => setDraft((d) => ({ ...d, [k]: v }));

  const save = async () => {
    const n = await busy(() => api.writeSettings(edition, changes));
    if (n !== undefined) {
      toast("success", `Сохранено параметров: ${n}`, "Предыдущий settings.xml сохранён в резервную копию.");
      load();
    }
  };

  const known = new Set([...SCREEN, ...GRAPHICS].map((f) => f.key).concat(["ScreenWidth", "ScreenHeight"]));
  const others = Object.keys(draft).filter((k) => !known.has(k));

  const renderField = (f: Field) => {
    if (!(f.key in draft)) return null;
    const v = draft[f.key];
    let control;
    if (isBool(v) || (f.key === "FXAA" && (v === "0" || v === "1"))) {
      const on = v === "true" || v === "1";
      control = <Toggle checked={on} onChange={(x) => set(f.key, isBool(v) ? String(x) : x ? "1" : "0")} />;
    } else if (f.options) {
      const opts = f.options.some(([o]) => o === v) ? f.options : [...f.options, [v, `Текущее: ${v}`] as Opt];
      control = (
        <select value={v} onChange={(e) => set(f.key, e.target.value)}>
          {opts.map(([o, l]) => (
            <option key={o} value={o}>
              {l}
            </option>
          ))}
        </select>
      );
    } else {
      control = <input className="num" value={v} onChange={(e) => set(f.key, e.target.value.trim())} />;
    }
    return (
      <div key={f.key} className={`setting-row ${changes[f.key] !== undefined ? "changed" : ""}`}>
        <span>{f.label}</span>
        {control}
      </div>
    );
  };

  const resolution = `${draft.ScreenWidth}x${draft.ScreenHeight}`;
  const resOptions = RESOLUTIONS.some(([r]) => r === resolution)
    ? RESOLUTIONS
    : [...RESOLUTIONS, [resolution, resolution.replace("x", " × ") + " (текущее)"] as Opt];

  const applyProfile = async (name: string) => {
    const a = await ask({
      title: `Применить профиль «${name}»?`,
      body: <p>Текущий settings.xml будет заменён. Его копия сохранится в резервных копиях.</p>,
      buttons: [
        { id: "yes", label: "Применить", kind: "primary" },
        { id: "no", label: "Отмена", kind: "ghost" },
      ],
    });
    if (a !== "yes") return;
    await busy(() => api.applySettingsProfile(edition, name), `Профиль «${name}» применён`);
    load();
  };

  return (
    <div className="page">
      <PageHeader title="Настройки игры" subtitle={`${EDITION_TITLE[edition]} · settings.xml`}>
        {data?.exists && (
          <>
            <button className="btn btn-ghost" disabled={!changed} onClick={() => setDraft(data.values)}>
              <RotateCcw size={16} /> Сбросить
            </button>
            <button className="btn btn-primary" disabled={!changed || running.running} onClick={save}>
              <Save size={16} /> Сохранить{changed ? ` (${changed})` : ""}
            </button>
          </>
        )}
      </PageHeader>

      {running.running && (
        <div className="alert alert-warn">
          Игра запущена. Закройте её перед изменением настроек — при выходе игра перезаписывает settings.xml.
        </div>
      )}

      {!data ? null : !data.exists ? (
        <div className="empty small">
          <FileCode2 size={36} />
          <h3>settings.xml ещё не создан</h3>
          <p className="muted">
            Игра создаёт файл настроек при первом запуске: {data.path}. Запустите GTA V один раз, затем вернитесь сюда.
          </p>
        </div>
      ) : (
        <div className="settings-grid">
          <section className="card">
            <h3>Экран</h3>
            {"ScreenWidth" in draft && "ScreenHeight" in draft && (
              <div className={`setting-row ${changes.ScreenWidth || changes.ScreenHeight ? "changed" : ""}`}>
                <span>Разрешение</span>
                <select
                  value={resolution}
                  onChange={(e) => {
                    const [w, h] = e.target.value.split("x");
                    setDraft((d) => ({ ...d, ScreenWidth: w, ScreenHeight: h }));
                  }}
                >
                  {resOptions.map(([o, l]) => (
                    <option key={o} value={o}>
                      {l}
                    </option>
                  ))}
                </select>
              </div>
            )}
            {SCREEN.map(renderField)}
            <div className="setting-note">
              <Gauge size={16} />
              <div>
                <b>Ограничение FPS</b>
                <p>
                  В settings.xml нет параметра ограничения кадров, поэтому лаунчер его не меняет. Варианты: «Половина
                  частоты» у вертикальной синхронизации; пункт ограничения частоты кадров в меню графики игры (если он
                  есть в вашей версии); Max Frame Rate в панели NVIDIA или Frame Rate Target Control в AMD Adrenalin.
                </p>
              </div>
            </div>
          </section>

          <section className="card">
            <h3>Графика</h3>
            {GRAPHICS.map(renderField)}
            {GRAPHICS.every((f) => !(f.key in draft)) && (
              <p className="muted">Параметры графики в этом файле называются иначе — смотрите «Все параметры» ниже.</p>
            )}
          </section>

          {others.length > 0 && (
            <section className="card span2">
              <div className="card-head">
                <h3>Все параметры файла</h3>
                <button className="btn btn-sm" onClick={() => setShowAll(!showAll)}>
                  {showAll ? "Скрыть" : `Показать (${others.length})`}
                </button>
              </div>
              {showAll && (
                <>
                  <p className="muted small">
                    Изменяйте, только если знаете, за что отвечает параметр. Перед записью делается резервная копия.
                  </p>
                  <div className="raw-grid">
                    {others.map((k) => (
                      <label key={k} className={changes[k] !== undefined ? "changed" : ""}>
                        <span>{k}</span>
                        <input value={draft[k]} onChange={(e) => set(k, e.target.value.trim())} />
                      </label>
                    ))}
                  </div>
                </>
              )}
            </section>
          )}

          <section className="card">
            <h3>Профили настроек</h3>
            <p className="muted small">Сохраните текущие настройки (например, «Качество» и «Производительность») и переключайтесь одним кликом.</p>
            <div className="inline-form">
              <input placeholder="Название профиля" value={profileName} maxLength={60} onChange={(e) => setProfileName(e.target.value)} />
              <button
                className="btn"
                disabled={!profileName.trim()}
                onClick={async () => {
                  if (changed) {
                    toast("warn", "Сначала сохраните изменения", "Профиль сохраняет то, что записано в settings.xml.");
                    return;
                  }
                  await busy(() => api.saveSettingsProfile(edition, profileName), "Профиль сохранён");
                  setProfileName("");
                  load();
                }}
              >
                <Save size={15} /> Сохранить текущие
              </button>
            </div>
            <ul className="row-list">
              {profiles.map((p) => (
                <li key={p}>
                  <span>{p}</span>
                  <div>
                    <button className="btn btn-sm" disabled={running.running} onClick={() => applyProfile(p)}>
                      <Upload size={14} /> Применить
                    </button>
                    <button
                      className="icon-btn danger"
                      onClick={async () => {
                        await busy(() => api.deleteSettingsProfile(edition, p), "Профиль удалён");
                        load();
                      }}
                    >
                      <Trash2 size={15} />
                    </button>
                  </div>
                </li>
              ))}
              {profiles.length === 0 && <li className="muted">Профилей пока нет</li>}
            </ul>
          </section>

          <section className="card">
            <h3>Параметры запуска (commandline.txt)</h3>
            {hasGame ? (
              <>
                <p className="muted small">
                  Файл в папке игры, который GTA V читает при запуске. Например: <code>-windowed</code>,{" "}
                  <code>-borderless</code>. Поддержка параметров зависит от версии игры.
                </p>
                <textarea className="mono" rows={4} value={cmd} onChange={(e) => setCmd(e.target.value)} placeholder="Пусто — без параметров" />
                <div className="row-end">
                  <button
                    className="btn"
                    disabled={cmd === cmdSaved || running.running}
                    onClick={async () => {
                      await busy(() => api.writeCommandline(edition, cmd), "commandline.txt сохранён");
                      setCmdSaved(cmd);
                    }}
                  >
                    <Save size={15} /> Сохранить
                  </button>
                </div>
              </>
            ) : (
              <NoGame text="Укажите папку игры, чтобы редактировать commandline.txt." />
            )}
          </section>

          <section className="card span2 file-info">
            <Info size={16} />
            <span className="muted small">{data.path}</span>
            <button className="btn btn-sm" onClick={() => busy(() => api.openLocation("documents", edition))}>
              <FolderOpen size={14} /> Открыть папку
            </button>
          </section>
        </div>
      )}
    </div>
  );
}
