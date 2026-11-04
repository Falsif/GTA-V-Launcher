import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, ImagePlus, Pencil, Plus, Puzzle, Trash2, UserCheck } from "lucide-react";
import { api, errText } from "../api";
import { useApp } from "../state";
import { Avatar, Modal, PageHeader } from "../components/ui";
import { EDITION_TITLE, formatDate, formatDuration, plural } from "../format";
import { THEMES } from "./Launcher";
import type { Edition, Profile } from "../types";

const EMOJIS = ["🎮", "🚗", "🏎️", "🚁", "✈️", "🏍️", "💰", "🔫", "🌴", "🌆", "🦈", "🐺", "👑", "🔥", "⭐", "🎯"];

export default function ProfilesPage() {
  const { config, setConfig, busy, toast, ask, running } = useApp();
  const [editing, setEditing] = useState<Profile | "new" | null>(null);
  if (!config) return null;

  const activate = async (p: Profile) => {
    if (p.mod_ids && running.running) {
      toast("warn", "Закройте игру", "Профиль меняет набор модов — это нельзя делать, пока игра запущена.");
      return;
    }
    try {
      const r = await api.switchProfile(p.id);
      setConfig(r.config);
      toast("success", `Профиль «${p.name}» активен`, r.notes.join("\n") || undefined);
    } catch (e) {
      toast("error", "Не удалось переключить профиль", errText(e));
    }
  };

  const remove = async (p: Profile) => {
    const a = await ask({
      title: "Удалить профиль?",
      body: <p>Профиль «{p.name}» и его статистика будут удалены. Моды и настройки игры не изменятся.</p>,
      buttons: [
        { id: "yes", label: "Удалить", kind: "danger" },
        { id: "no", label: "Отмена", kind: "ghost" },
      ],
    });
    if (a !== "yes") return;
    const c = await busy(() => api.deleteProfile(p.id), "Профиль удалён");
    if (c) setConfig(c);
  };

  const captureMods = async (p: Profile, capture: boolean) => {
    const c = await busy(
      () => api.setProfileMods(p.id, capture),
      capture ? "Набор модов запомнен в профиле" : "Профиль больше не меняет моды",
    );
    if (c) setConfig(c);
  };

  return (
    <div className="page">
      <PageHeader
        title="Профили"
        subtitle="Локальные профили лаунчера. Они не заменяют учётную запись Rockstar Games и не связаны с ней."
      >
        <button className="btn btn-primary" onClick={() => setEditing("new")}>
          <Plus size={16} /> Новый профиль
        </button>
      </PageHeader>

      {config.profiles.length === 0 ? (
        <div className="empty">
          <UserCheck size={42} />
          <h3>Профилей пока нет</h3>
          <p className="muted">
            Профиль хранит имя, аватар, выбранную версию GTA V, тему, набор модов и собственную статистику запусков.
          </p>
          <button className="btn btn-primary" onClick={() => setEditing("new")}>
            Создать профиль
          </button>
        </div>
      ) : (
        <div className="profile-grid">
          {config.profiles.map((p) => {
            const active = p.id === config.active_profile;
            const modNames = (p.mod_ids ?? [])
              .map((id) => config.mods.find((m) => m.id === id)?.name)
              .filter(Boolean) as string[];
            return (
              <div key={p.id} className={`profile-card ${active ? "active" : ""}`}>
                {active && (
                  <span className="active-badge">
                    <Check size={13} /> Активен
                  </span>
                )}
                <Avatar value={p.avatar} size={64} />
                <h3>{p.name}</h3>
                <p className="muted small">
                  {EDITION_TITLE[p.edition]} · тема «{THEMES.find((t) => t.id === p.theme)?.label ?? "Красная"}»
                </p>
                <div className="profile-stats">
                  <div>
                    <b>{p.launches}</b>
                    <small>{plural(p.launches, "запуск", "запуска", "запусков")}</small>
                  </div>
                  <div>
                    <b>{formatDuration(p.seconds)}</b>
                    <small>в игре</small>
                  </div>
                </div>
                <p className="muted small">Последний запуск: {formatDate(p.last_launch)}</p>
                <div className="profile-mods">
                  <Puzzle size={14} />
                  {p.mod_ids === null ? (
                    <span>Моды не меняет</span>
                  ) : (
                    <span title={modNames.join(", ")}>
                      {modNames.length
                        ? `${modNames.length} ${plural(modNames.length, "мод", "мода", "модов")}: ${modNames.slice(0, 3).join(", ")}${modNames.length > 3 ? "…" : ""}`
                        : "Без модов (всё отключит)"}
                    </span>
                  )}
                </div>
                <div className="profile-actions">
                  {!active && (
                    <button className="btn btn-primary btn-sm" onClick={() => activate(p)}>
                      Выбрать
                    </button>
                  )}
                  <button className="btn btn-sm" onClick={() => captureMods(p, true)} title="Запомнить моды, которые включены сейчас">
                    Запомнить моды
                  </button>
                  {p.mod_ids !== null && (
                    <button className="btn btn-sm btn-ghost" onClick={() => captureMods(p, false)}>
                      Не трогать моды
                    </button>
                  )}
                  <button className="icon-btn" onClick={() => setEditing(p)} title="Изменить">
                    <Pencil size={15} />
                  </button>
                  <button className="icon-btn danger" onClick={() => remove(p)} title="Удалить">
                    <Trash2 size={15} />
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {editing && <ProfileDialog profile={editing === "new" ? null : editing} onClose={() => setEditing(null)} />}
    </div>
  );
}

function ProfileDialog({ profile, onClose }: { profile: Profile | null; onClose: () => void }) {
  const { config, setConfig, toast, edition: currentEdition } = useApp();
  const [name, setName] = useState(profile?.name ?? "");
  const [avatar, setAvatar] = useState(profile?.avatar ?? "🎮");
  const [edition, setEdition] = useState<Edition>(profile?.edition ?? currentEdition);
  const [theme, setTheme] = useState(profile?.theme ?? config?.launcher.theme ?? "red");

  const pickImage = async () => {
    const path = await open({ title: "Аватар", filters: [{ name: "Изображения", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }] });
    if (typeof path !== "string") return;
    try {
      setAvatar(await api.loadAvatar(path));
    } catch (e) {
      toast("error", "Аватар не загружен", errText(e));
    }
  };

  const save = async () => {
    try {
      const c = profile
        ? await api.updateProfile(profile.id, name, avatar, edition, theme)
        : await api.createProfile(name, avatar, edition);
      setConfig(c);
      toast("success", profile ? "Профиль сохранён" : "Профиль создан");
      onClose();
    } catch (e) {
      toast("error", "Ошибка", errText(e));
    }
  };

  return (
    <Modal
      title={profile ? "Изменить профиль" : "Новый профиль"}
      onClose={onClose}
      footer={
        <>
          <button className="btn btn-ghost" onClick={onClose}>
            Отмена
          </button>
          <button className="btn btn-primary" disabled={!name.trim()} onClick={save}>
            Сохранить
          </button>
        </>
      }
    >
      <div className="avatar-pick">
        <Avatar value={avatar} size={72} />
        <div className="emoji-grid">
          {EMOJIS.map((e) => (
            <button key={e} className={avatar === e ? "active" : ""} onClick={() => setAvatar(e)}>
              {e}
            </button>
          ))}
          <button title="Загрузить картинку (до 2 МБ)" onClick={pickImage}>
            <ImagePlus size={18} />
          </button>
        </div>
      </div>
      <div className="form-grid">
        <label className="span2">
          Имя игрока
          <input value={name} onChange={(e) => setName(e.target.value)} maxLength={32} autoFocus />
        </label>
        <label>
          Версия GTA V
          <select value={edition} onChange={(e) => setEdition(e.target.value as Edition)}>
            <option value="legacy">GTA V Legacy</option>
            <option value="enhanced">GTA V Enhanced</option>
          </select>
        </label>
        {profile && (
          <label>
            Тема лаунчера
            <select value={theme} onChange={(e) => setTheme(e.target.value)}>
              {THEMES.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.label}
                </option>
              ))}
            </select>
          </label>
        )}
      </div>
    </Modal>
  );
}
