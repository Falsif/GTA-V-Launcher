import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  ChevronDown,
  FileArchive,
  FolderOpen,
  FolderPlus,
  Info,
  Pencil,
  PowerOff,
  RefreshCw,
  Search,
  ShieldAlert,
  Trash2,
  Upload,
  XCircle,
} from "lucide-react";
import { api, errText } from "../api";
import { useApp } from "../state";
import { Modal, NoGame, PageHeader, Toggle } from "../components/ui";
import { CATEGORIES, EDITION_TITLE, categoryLabel, formatDate, formatSize, plural } from "../format";
import type { ExternalItem, ModEntry, ModInspect, ModWarning } from "../types";

type Sort = "name" | "date" | "category" | "enabled";

export default function ModsPage() {
  const { config, setConfig, edition, info, busy, toast, ask, running } = useApp();
  const [cat, setCat] = useState("all");
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<Sort>("name");
  const [warnings, setWarnings] = useState<ModWarning[]>([]);
  const [external, setExternal] = useState<ExternalItem[]>([]);
  const [install, setInstall] = useState<{ path: string; data: ModInspect } | null>(null);
  const [edit, setEdit] = useState<ModEntry | null>(null);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [working, setWorking] = useState<string | null>(null);

  const hasGame = !!config?.installs[edition] && !!info[edition]?.path;

  const refresh = useCallback(async () => {
    setConfig(await api.getConfig());
    if (!config?.installs[edition]) return;
    api.checkMods(edition).then(setWarnings).catch(() => setWarnings([]));
    api.scanExternal(edition).then(setExternal).catch(() => setExternal([]));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [edition, config?.installs[edition]?.path]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const mods = useMemo(() => {
    let list = (config?.mods ?? []).filter((m) => m.edition === edition);
    if (cat !== "all") list = list.filter((m) => m.category === cat);
    if (query.trim()) {
      const q = query.toLowerCase();
      list = list.filter((m) => `${m.name} ${m.description} ${m.version}`.toLowerCase().includes(q));
    }
    const cmp: Record<Sort, (a: ModEntry, b: ModEntry) => number> = {
      name: (a, b) => a.name.localeCompare(b.name, "ru"),
      date: (a, b) => b.installed_at.localeCompare(a.installed_at),
      category: (a, b) => categoryLabel(a.category).localeCompare(categoryLabel(b.category), "ru") || a.name.localeCompare(b.name, "ru"),
      enabled: (a, b) => Number(b.enabled) - Number(a.enabled) || a.name.localeCompare(b.name, "ru"),
    };
    return [...list].sort(cmp[sort]);
  }, [config?.mods, edition, cat, query, sort]);

  const counts = useMemo(() => {
    const all = (config?.mods ?? []).filter((m) => m.edition === edition);
    const c: Record<string, number> = { all: all.length };
    all.forEach((m) => (c[m.category] = (c[m.category] ?? 0) + 1));
    return c;
  }, [config?.mods, edition]);

  if (!hasGame) {
    return (
      <div className="page">
        <PageHeader title="Моды" subtitle={EDITION_TITLE[edition]} />
        <NoGame />
      </div>
    );
  }

  const pick = async (directory: boolean) => {
    const path = await open(
      directory
        ? { directory: true, title: "Папка с файлами мода" }
        : {
            title: "Файл мода",
            filters: [
              { name: "Моды GTA V", extensions: ["zip", "asi", "dll", "cs", "vb", "lua", "ini", "xml", "oiv", "rar", "7z"] },
              { name: "Все файлы", extensions: ["*"] },
            ],
          },
    );
    if (typeof path !== "string") return;
    try {
      const data = await api.inspectMod(path, edition);
      setInstall({ path, data });
    } catch (e) {
      toast("error", "Этот мод нельзя установить", errText(e));
    }
  };

  const resolveConflicts = async (id: string, conflicts: { mod_name: string; file: string }[]) => {
    const names = [...new Set(conflicts.map((c) => c.mod_name))];
    const answer = await ask({
      title: "Конфликт модов",
      body: (
        <>
          <p>
            Этот мод заменяет те же файлы, что и {names.map((n) => `«${n}»`).join(", ")}:
          </p>
          <ul className="file-list small">
            {conflicts.slice(0, 8).map((c) => (
              <li key={c.file}>{c.file}</li>
            ))}
            {conflicts.length > 8 && <li>…и ещё {conflicts.length - 8}</li>}
          </ul>
          <p>Можно отключить конфликтующие моды и включить этот.</p>
        </>
      ),
      buttons: [
        { id: "force", label: "Отключить их и включить этот", kind: "primary" },
        { id: "cancel", label: "Оставить как есть", kind: "ghost" },
      ],
    });
    if (answer === "force") {
      await busy(() => api.enableMod(id, true), "Мод включён");
    }
  };

  const toggle = async (m: ModEntry) => {
    setWorking(m.id);
    try {
      if (m.enabled) {
        await api.disableMod(m.id);
        toast("info", `«${m.name}» отключён`, "Оригинальные файлы восстановлены.");
      } else {
        const r = await api.enableMod(m.id, false);
        if (!r.ok) await resolveConflicts(m.id, r.conflicts);
        else toast("success", `«${m.name}» включён`);
      }
    } catch (e) {
      toast("error", "Ошибка", errText(e));
    } finally {
      setWorking(null);
      refresh();
    }
  };

  const remove = async (m: ModEntry) => {
    const a = await ask({
      title: "Удалить мод?",
      body: (
        <p>
          «{m.name}» будет удалён из папки игры и из лаунчера. Заменённые им оригинальные файлы восстановятся из резервной
          копии.
        </p>
      ),
      buttons: [
        { id: "yes", label: "Удалить", kind: "danger" },
        { id: "no", label: "Отмена", kind: "ghost" },
      ],
    });
    if (a !== "yes") return;
    setWorking(m.id);
    await busy(() => api.removeMod(m.id), "Мод удалён");
    setWorking(null);
    refresh();
  };

  const disableAll = async () => {
    const n = await busy(() => api.disableAllMods(edition));
    if (n !== undefined) toast("info", `Отключено: ${n} ${plural(n, "мод", "мода", "модов")}`);
    refresh();
  };

  const levelIcon = { error: <XCircle size={16} />, warn: <AlertTriangle size={16} />, info: <Info size={16} /> };

  return (
    <div className="page">
      <PageHeader
        title="Моды"
        subtitle={
          <>
            {EDITION_TITLE[edition]} · только одиночный режим
            {running.running && <span className="tag tag-warn">Закройте игру, чтобы менять моды</span>}
          </>
        }
      >
        <button className="btn btn-primary" onClick={() => pick(false)} disabled={running.running}>
          <Upload size={16} /> Установить из файла
        </button>
        <button className="btn" onClick={() => pick(true)} disabled={running.running}>
          <FolderPlus size={16} /> Из папки
        </button>
        <button className="btn btn-ghost" onClick={disableAll} disabled={running.running} title="Отключить все моды лаунчера">
          <PowerOff size={16} />
        </button>
        <button className="btn btn-ghost" onClick={refresh} title="Проверить ещё раз">
          <RefreshCw size={16} />
        </button>
      </PageHeader>

      <div className="online-banner">
        <ShieldAlert size={18} />
        Лаунчер не запускает GTA Online с модами и не обходит античит. Перед заходом в Online отключите все моды.
      </div>

      {warnings.length > 0 && (
        <div className="warnings">
          {warnings.map((w, i) => (
            <div key={i} className={`warning warning-${w.level}`}>
              {levelIcon[w.level]} <span>{w.text}</span>
            </div>
          ))}
        </div>
      )}

      <div className="toolbar">
        <div className="chips">
          <button className={`chip ${cat === "all" ? "active" : ""}`} onClick={() => setCat("all")}>
            Все <em>{counts.all ?? 0}</em>
          </button>
          {CATEGORIES.map((c) => (
            <button key={c.id} className={`chip ${cat === c.id ? "active" : ""}`} onClick={() => setCat(c.id)}>
              {c.label} {counts[c.id] ? <em>{counts[c.id]}</em> : null}
            </button>
          ))}
        </div>
        <div className="toolbar-right">
          <label className="search">
            <Search size={15} />
            <input placeholder="Поиск модов" value={query} onChange={(e) => setQuery(e.target.value)} />
          </label>
          <select value={sort} onChange={(e) => setSort(e.target.value as Sort)}>
            <option value="name">По названию</option>
            <option value="date">Сначала новые</option>
            <option value="category">По категории</option>
            <option value="enabled">Сначала включённые</option>
          </select>
        </div>
      </div>

      {mods.length === 0 ? (
        <div className="empty small">
          <FileArchive size={36} />
          <h3>{counts.all ? "Ничего не найдено" : "Модов пока нет"}</h3>
          <p className="muted">
            Поддерживаются .zip-архивы, папки и отдельные файлы (.asi, .dll, скрипты .cs/.vb). Пакеты .oiv ставьте через
            OpenIV.
          </p>
        </div>
      ) : (
        <div className="mod-list">
          {mods.map((m) => (
            <div key={m.id} className={`mod-card ${m.enabled ? "enabled" : ""}`}>
              <div className="mod-main">
                <Toggle checked={m.enabled} onChange={() => toggle(m)} disabled={working === m.id || running.running} />
                <div className="mod-info">
                  <div className="mod-title">
                    <b>{m.name}</b>
                    {m.version && <span className="ver">v{m.version}</span>}
                    <span className={`tag cat-${m.category}`}>{categoryLabel(m.category)}</span>
                  </div>
                  {m.description && <p>{m.description}</p>}
                  <small className="muted">
                    {m.files.length} {plural(m.files.length, "файл", "файла", "файлов")} · {formatSize(m.size)} · установлен{" "}
                    {formatDate(m.installed_at)}
                  </small>
                </div>
                <div className="mod-actions">
                  <button className="icon-btn" title="Файлы мода" onClick={() => setExpanded(expanded === m.id ? null : m.id)}>
                    <ChevronDown size={17} className={expanded === m.id ? "rot" : ""} />
                  </button>
                  <button className="icon-btn" title="Изменить описание" onClick={() => setEdit(m)}>
                    <Pencil size={16} />
                  </button>
                  <button
                    className="icon-btn danger"
                    title="Удалить"
                    onClick={() => remove(m)}
                    disabled={working === m.id || running.running}
                  >
                    <Trash2 size={16} />
                  </button>
                </div>
              </div>
              {expanded === m.id && (
                <ul className="file-list">
                  {m.files.map((f) => (
                    <li key={f}>{f}</li>
                  ))}
                </ul>
              )}
            </div>
          ))}
        </div>
      )}

      {external.length > 0 && (
        <section className="card external">
          <div className="card-head">
            <h3>Установлено вручную (не через лаунчер)</h3>
            <button className="btn btn-sm" onClick={() => busy(() => api.openLocation("game", edition))}>
              <FolderOpen size={14} /> Открыть папку
            </button>
          </div>
          <p className="muted small">
            Эти файлы лаунчер только показывает: он не удаляет и не отключает то, что ставил не он.
          </p>
          <table className="table">
            <tbody>
              {external.map((x) => (
                <tr key={x.path}>
                  <td>{x.path}</td>
                  <td className="muted">{x.kind}</td>
                  <td className="muted right">{x.size ? formatSize(x.size) : ""}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {install && (
        <InstallDialog
          path={install.path}
          data={install.data}
          onClose={() => setInstall(null)}
          onDone={async (id, conflicts) => {
            setInstall(null);
            if (conflicts.length) await resolveConflicts(id, conflicts);
            refresh();
          }}
        />
      )}
      {edit && <EditDialog mod={edit} onClose={() => setEdit(null)} onSaved={refresh} />}
    </div>
  );
}

function InstallDialog({
  path,
  data,
  onClose,
  onDone,
}: {
  path: string;
  data: ModInspect;
  onClose: () => void;
  onDone: (id: string, conflicts: { mod_name: string; file: string }[]) => void;
}) {
  const { edition, toast } = useApp();
  const [name, setName] = useState(data.name);
  const [version, setVersion] = useState(data.version);
  const [description, setDescription] = useState(data.description);
  const [category, setCategory] = useState(data.category);
  const [target, setTarget] = useState(data.target_subdir);
  const [enable, setEnable] = useState(true);
  const [preview, setPreview] = useState<ModInspect>(data);
  const [saving, setSaving] = useState(false);
  const targetPreset = ["", "scripts", "plugins", "mods"].includes(target) ? target : "custom";

  // Пересчитать список файлов при смене папки установки.
  useEffect(() => {
    if (target === data.target_subdir) {
      setPreview(data);
      return;
    }
    const prefix = target.trim().replace(/^[\\/]+|[\\/]+$/g, "");
    const strip = data.target_subdir ? data.target_subdir + "/" : "";
    const files = data.files.map((f) => {
      const rel = strip && f.startsWith(strip) ? f.slice(strip.length) : f;
      return prefix ? `${prefix}/${rel}` : rel;
    });
    setPreview({ ...data, files });
  }, [target, data]);

  const submit = async () => {
    setSaving(true);
    try {
      const r = await api.installMod({
        edition,
        source_path: path,
        name,
        version,
        description,
        category,
        target_subdir: target,
        enable,
      });
      toast("success", `Мод «${name}» установлен`, r.ok && enable ? "Мод включён." : undefined);
      onDone(r.mod_id, r.ok ? [] : r.conflicts);
    } catch (e) {
      toast("error", "Установка не удалась", errText(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      title="Установка мода"
      wide
      onClose={onClose}
      footer={
        <>
          <button className="btn btn-ghost" onClick={onClose}>
            Отмена
          </button>
          <button className="btn btn-primary" onClick={submit} disabled={saving || !name.trim()}>
            {saving ? "Установка…" : "Установить"}
          </button>
        </>
      }
    >
      <div className="form-grid">
        <label>
          Название
          <input value={name} onChange={(e) => setName(e.target.value)} maxLength={80} />
        </label>
        <label>
          Версия
          <input value={version} onChange={(e) => setVersion(e.target.value)} placeholder="например, 1.2" maxLength={20} />
        </label>
        <label className="span2">
          Описание
          <textarea value={description} onChange={(e) => setDescription(e.target.value)} rows={2} maxLength={500} />
        </label>
        <label>
          Категория
          <select value={category} onChange={(e) => setCategory(e.target.value)}>
            {CATEGORIES.map((c) => (
              <option key={c.id} value={c.id}>
                {c.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          Куда установить
          <select
            value={targetPreset}
            onChange={(e) => setTarget(e.target.value === "custom" ? "scripts/" : e.target.value)}
          >
            <option value="">Корневая папка игры</option>
            <option value="scripts">scripts (скрипты ScriptHookVDotNet)</option>
            <option value="plugins">plugins</option>
            <option value="mods">mods (OpenIV)</option>
            <option value="custom">Другая папка…</option>
          </select>
        </label>
        {targetPreset === "custom" && (
          <label className="span2">
            Папка внутри игры
            <input value={target} onChange={(e) => setTarget(e.target.value)} placeholder="например, scripts/MyMod" />
          </label>
        )}
      </div>

      {data.warnings.map((w) => (
        <div key={w} className="warning warning-warn">
          <AlertTriangle size={16} /> <span>{w}</span>
        </div>
      ))}

      <div className="files-preview">
        <div className="files-head">
          <b>
            {preview.file_count} {plural(preview.file_count, "файл", "файла", "файлов")} · {formatSize(preview.total_size)}
          </b>
          <span className="muted small">так файлы лягут в папку игры</span>
        </div>
        <ul className="file-list">
          {preview.files.map((f) => (
            <li key={f}>{f}</li>
          ))}
          {preview.file_count > preview.files.length && <li>…и ещё {preview.file_count - preview.files.length}</li>}
        </ul>
      </div>
      <label className="check">
        <Toggle checked={enable} onChange={setEnable} /> Сразу включить мод
      </label>
      <p className="muted small">
        Заменяемые файлы игры автоматически сохраняются в резервную копию и восстанавливаются при отключении мода.
      </p>
    </Modal>
  );
}

function EditDialog({ mod, onClose, onSaved }: { mod: ModEntry; onClose: () => void; onSaved: () => void }) {
  const { busy } = useApp();
  const [name, setName] = useState(mod.name);
  const [version, setVersion] = useState(mod.version);
  const [description, setDescription] = useState(mod.description);
  const [category, setCategory] = useState(mod.category);
  const save = async () => {
    await busy(() => api.updateMod(mod.id, name, version, description, category), "Сохранено");
    onClose();
    onSaved();
  };
  return (
    <Modal
      title="Изменить мод"
      onClose={onClose}
      footer={
        <>
          <button className="btn btn-ghost" onClick={onClose}>
            Отмена
          </button>
          <button className="btn btn-primary" onClick={save} disabled={!name.trim()}>
            Сохранить
          </button>
        </>
      }
    >
      <div className="form-grid">
        <label>
          Название
          <input value={name} onChange={(e) => setName(e.target.value)} maxLength={80} />
        </label>
        <label>
          Версия
          <input value={version} onChange={(e) => setVersion(e.target.value)} maxLength={20} />
        </label>
        <label className="span2">
          Описание
          <textarea value={description} onChange={(e) => setDescription(e.target.value)} rows={3} maxLength={500} />
        </label>
        <label>
          Категория
          <select value={category} onChange={(e) => setCategory(e.target.value)}>
            {CATEGORIES.map((c) => (
              <option key={c.id} value={c.id}>
                {c.label}
              </option>
            ))}
          </select>
        </label>
      </div>
    </Modal>
  );
}
