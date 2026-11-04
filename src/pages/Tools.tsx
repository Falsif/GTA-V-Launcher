import { useEffect, useState, type ReactNode } from "react";
import {
  Archive,
  Camera,
  Database,
  FileText,
  FolderOpen,
  Image,
  Package,
  RotateCcw,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { api } from "../api";
import { useApp } from "../state";
import { PageHeader } from "../components/ui";
import { EDITION_TITLE, backupLabel } from "../format";
import type { Addons, BackupInfo } from "../types";

export default function ToolsPage() {
  const { edition, info, busy, toast, ask, running } = useApp();
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [addons, setAddons] = useState<Addons | null>(null);
  const [showOfficial, setShowOfficial] = useState(false);
  const hasGame = !!info[edition]?.path;

  const load = async () => {
    setBackups(await api.listBackups(edition));
    if (hasGame) api.addons(edition).then(setAddons).catch(() => setAddons(null));
    else setAddons(null);
  };
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [edition, hasGame]);

  const verify = async () => {
    const r = await busy(() => api.verify(edition));
    if (r) toast(r.started ? "success" : "info", "Проверка целостности", r.message);
  };

  const restore = async (b: BackupInfo) => {
    const a = await ask({
      title: "Восстановить резервную копию?",
      body: (
        <p>
          Файлы {b.files.join(", ")} будут заменены версией от {backupLabel(b.id)}. Текущие файлы тоже сохранятся в
          резервную копию.
        </p>
      ),
      buttons: [
        { id: "yes", label: "Восстановить", kind: "primary" },
        { id: "no", label: "Отмена", kind: "ghost" },
      ],
    });
    if (a !== "yes") return;
    await busy(() => api.restoreBackup(edition, b.id), "Конфигурация восстановлена");
    load();
  };

  const shot = async () => {
    const p = await busy(() => api.screenshot(true));
    if (p) toast("success", "Скриншот сохранён", p, { label: "Открыть", run: () => api.openFile(p) });
  };

  const folders: [string, string, ReactNode, boolean][] = [
    ["game", "Папка игры", <FolderOpen size={18} />, hasGame],
    ["documents", "Документы → Rockstar Games", <FileText size={18} />, true],
    ["scripts", "Папка scripts", <Package size={18} />, hasGame],
    ["screenshots", "Скриншоты лаунчера", <Image size={18} />, true],
    ["data", "Данные лаунчера", <Database size={18} />, true],
  ];

  return (
    <div className="page">
      <PageHeader title="Инструменты" subtitle={EDITION_TITLE[edition]} />

      <div className="tools-grid">
        <section className="card">
          <h3>
            <ShieldCheck size={18} /> Целостность файлов
          </h3>
          <p className="muted small">
            Проверка выполняется официальным клиентом (Steam, Epic Games или Rockstar Games Launcher): он сам сверит и
            докачает повреждённые файлы.
          </p>
          <button className="btn btn-primary" onClick={verify} disabled={!hasGame || running.running}>
            Проверить файлы игры
          </button>
        </section>

        <section className="card">
          <h3>
            <Camera size={18} /> Скриншот
          </h3>
          <p className="muted small">
            Работает, когда игра запущена. Прямо в игре нажмите <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>S</kbd> (можно
            отключить в настройках лаунчера). Снимки сохраняются в «Изображения\GTA V Launcher». В эксклюзивном
            полноэкранном режиме снимок может быть чёрным — используйте режим «Окно без рамки».
          </p>
          <div className="row-start">
            <button className="btn" onClick={shot} disabled={!running.running}>
              <Camera size={15} /> Сделать скриншот
            </button>
            <button className="btn btn-ghost" onClick={() => busy(() => api.openLocation("screenshots", edition))}>
              <FolderOpen size={15} /> Открыть папку
            </button>
          </div>
        </section>

        <section className="card">
          <h3>
            <FolderOpen size={18} /> Быстрый доступ
          </h3>
          <div className="folder-buttons">
            {folders.map(([kind, label, icon, enabled]) => (
              <button key={kind} className="btn" disabled={!enabled} onClick={() => busy(() => api.openLocation(kind, edition))}>
                {icon} {label}
              </button>
            ))}
          </div>
        </section>

        <section className="card">
          <div className="card-head">
            <h3>
              <Archive size={18} /> Резервные копии конфигурации
            </h3>
            <button
              className="btn btn-sm btn-primary"
              onClick={async () => {
                await busy(() => api.createBackup(edition), "Резервная копия создана");
                load();
              }}
            >
              Создать
            </button>
          </div>
          <p className="muted small">
            settings.xml и commandline.txt. Автокопия делается перед каждым изменением настроек (хранятся последние 30).
          </p>
          <ul className="row-list scroll">
            {backups.map((b) => (
              <li key={b.id}>
                <div className="backup-name">
                  <span>{backupLabel(b.id)}</span>
                  <small className="muted">{b.files.join(", ")}</small>
                </div>
                <div>
                  <button className="btn btn-sm" disabled={running.running} onClick={() => restore(b)}>
                    <RotateCcw size={14} /> Восстановить
                  </button>
                  <button
                    className="icon-btn danger"
                    onClick={async () => {
                      await busy(() => api.deleteBackup(edition, b.id));
                      load();
                    }}
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              </li>
            ))}
            {backups.length === 0 && <li className="muted">Копий пока нет</li>}
          </ul>
        </section>

        <section className="card span2">
          <h3>
            <Package size={18} /> Установленные дополнения
          </h3>
          {!hasGame ? (
            <p className="muted">Укажите папку игры.</p>
          ) : !addons ? (
            <p className="muted">Загрузка…</p>
          ) : (
            <>
              <div className="addon-summary">
                <div>
                  <b>{addons.official.length}</b>
                  <small>DLC-пакетов игры (update\x64\dlcpacks)</small>
                </div>
                <div>
                  <b>{addons.modded.length}</b>
                  <small>дополнений в папке mods</small>
                </div>
                <div>
                  <b>{addons.mods_folder ? "есть" : "нет"}</b>
                  <small>папка mods (OpenIV)</small>
                </div>
              </div>
              {addons.modded.length > 0 && (
                <>
                  <h4>Дополнения в mods</h4>
                  <div className="tag-cloud">
                    {addons.modded.map((a) => (
                      <span key={a} className="tag">
                        {a}
                      </span>
                    ))}
                  </div>
                </>
              )}
              <button className="btn btn-sm btn-ghost" onClick={() => setShowOfficial(!showOfficial)}>
                {showOfficial ? "Скрыть официальные пакеты" : "Показать официальные пакеты"}
              </button>
              {showOfficial && (
                <div className="tag-cloud">
                  {addons.official.map((a) => (
                    <span key={a} className="tag tag-dim">
                      {a}
                    </span>
                  ))}
                </div>
              )}
            </>
          )}
        </section>
      </div>
    </div>
  );
}
