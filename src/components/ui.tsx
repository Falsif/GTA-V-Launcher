import { useEffect, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  AlertTriangle,
  CheckCircle2,
  Copy,
  Gamepad2,
  Home,
  Info,
  Minus,
  Puzzle,
  Settings,
  SlidersHorizontal,
  Square,
  Users,
  Wrench,
  X,
  XCircle,
} from "lucide-react";
import { useApp, type Page } from "../state";
import { EDITION_TITLE, formatClock } from "../format";
import type { Edition } from "../types";

export function TitleBar() {
  const win = getCurrentWindow();
  const { config } = useApp();
  const close = () => (config?.launcher.close_to_tray ? win.hide() : win.close());
  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="titlebar-brand" data-tauri-drag-region>
        <span className="brand-mark">V</span>
        <span data-tauri-drag-region>GTA V Launcher</span>
      </div>
      <div className="titlebar-buttons">
        <button title="Свернуть" onClick={() => win.minimize()}>
          <Minus size={15} />
        </button>
        <button title="Развернуть" onClick={() => win.toggleMaximize()}>
          <Square size={12} />
        </button>
        <button title={config?.launcher.close_to_tray ? "Свернуть в трей" : "Закрыть"} className="close" onClick={close}>
          <X size={16} />
        </button>
      </div>
    </div>
  );
}

const NAV: { id: Page; label: string; icon: ReactNode }[] = [
  { id: "home", label: "Главная", icon: <Home size={20} /> },
  { id: "mods", label: "Моды", icon: <Puzzle size={20} /> },
  { id: "settings", label: "Настройки игры", icon: <SlidersHorizontal size={20} /> },
  { id: "profiles", label: "Профили", icon: <Users size={20} /> },
  { id: "tools", label: "Инструменты", icon: <Wrench size={20} /> },
  { id: "launcher", label: "Лаунчер", icon: <Settings size={20} /> },
];

export function Sidebar() {
  const { page, go, config, running, play, launching, edition, info } = useApp();
  const profile = config?.profiles.find((p) => p.id === config.active_profile);
  const activeMods = config?.mods.filter((m) => m.enabled && m.edition === edition).length ?? 0;
  return (
    <aside className="sidebar">
      <nav>
        {NAV.map((n) => (
          <button key={n.id} className={`nav-item ${page === n.id ? "active" : ""}`} onClick={() => go(n.id)}>
            {n.icon}
            <span>{n.label}</span>
            {n.id === "mods" && activeMods > 0 && <em className="nav-badge">{activeMods}</em>}
          </button>
        ))}
      </nav>
      <div className="sidebar-bottom">
        {running.running ? (
          <div className="running-chip">
            <span className="pulse" />
            <div>
              <b>Игра запущена</b>
              <small>{formatClock(running.session_seconds)}</small>
            </div>
          </div>
        ) : page === "home" ? null : (
          <button className="btn btn-primary sidebar-play" disabled={launching || !info[edition]?.installed} onClick={play}>
            <Gamepad2 size={18} /> Играть
          </button>
        )}
        <button className="profile-chip" onClick={() => go("profiles")} title="Профили лаунчера">
          <Avatar value={profile?.avatar ?? "🎮"} size={34} />
          <div>
            <b>{profile?.name ?? "Без профиля"}</b>
            <small>{EDITION_TITLE[edition]}</small>
          </div>
        </button>
      </div>
    </aside>
  );
}

export function Avatar({ value, size = 40 }: { value: string; size?: number }) {
  if (value.startsWith("data:image")) {
    return <img className="avatar" src={value} style={{ width: size, height: size }} alt="" />;
  }
  return (
    <span className="avatar avatar-emoji" style={{ width: size, height: size, fontSize: size * 0.55 }}>
      {value || "🎮"}
    </span>
  );
}

export function Toasts() {
  const { toasts, dismiss } = useApp();
  const icon = {
    success: <CheckCircle2 size={20} />,
    error: <XCircle size={20} />,
    info: <Info size={20} />,
    warn: <AlertTriangle size={20} />,
  };
  return (
    <div className="toasts">
      {toasts.map((t) => (
        <div key={t.id} className={`toast toast-${t.kind}`}>
          <span className="toast-icon">{icon[t.kind]}</span>
          <div className="toast-body">
            <b>{t.title}</b>
            {t.text && <p>{t.text}</p>}
            {t.action && (
              <button
                className="link"
                onClick={() => {
                  t.action!.run();
                  dismiss(t.id);
                }}
              >
                {t.action.label}
              </button>
            )}
          </div>
          <button className="toast-close" onClick={() => dismiss(t.id)}>
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}

export function Modal({
  title,
  children,
  onClose,
  footer,
  wide,
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
  footer?: ReactNode;
  wide?: boolean;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={`modal ${wide ? "modal-wide" : ""}`}>
        <div className="modal-head">
          <h3>{title}</h3>
          <button className="icon-btn" onClick={onClose}>
            <X size={18} />
          </button>
        </div>
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-foot">{footer}</div>}
      </div>
    </div>
  );
}

export function DialogHost() {
  const { dialog } = useApp();
  if (!dialog) return null;
  return (
    <Modal
      title={dialog.title}
      wide={dialog.buttons.length > 2}
      onClose={() => dialog.resolve(null)}
      footer={dialog.buttons.map((b) => (
        <button
          key={b.id}
          className={`btn ${b.kind === "primary" ? "btn-primary" : b.kind === "danger" ? "btn-danger" : b.kind === "ghost" ? "btn-ghost" : ""}`}
          onClick={() => dialog.resolve(b.id)}
        >
          {b.label}
        </button>
      ))}
    >
      <div className="dialog-text">{dialog.body}</div>
    </Modal>
  );
}

export function Toggle({ checked, onChange, disabled }: { checked: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      className={`toggle ${checked ? "on" : ""}`}
      onClick={() => onChange(!checked)}
    >
      <span />
    </button>
  );
}

export function EditionSwitch({ value, onChange }: { value: Edition; onChange: (e: Edition) => void }) {
  const { info } = useApp();
  return (
    <div className="segmented">
      {(["legacy", "enhanced"] as Edition[]).map((e) => (
        <button key={e} className={value === e ? "active" : ""} onClick={() => onChange(e)}>
          {e === "legacy" ? "Legacy" : "Enhanced"}
          <span className={`dot ${info[e]?.installed ? "ok" : info[e]?.path ? "bad" : ""}`} />
        </button>
      ))}
    </div>
  );
}

export function PageHeader({ title, subtitle, children }: { title: string; subtitle?: ReactNode; children?: ReactNode }) {
  return (
    <header className="page-header">
      <div>
        <h1>{title}</h1>
        {subtitle && <p className="muted">{subtitle}</p>}
      </div>
      {children && <div className="page-actions">{children}</div>}
    </header>
  );
}

export function NoGame({ text }: { text?: string }) {
  const { go, edition } = useApp();
  return (
    <div className="empty">
      <Gamepad2 size={42} />
      <h3>{EDITION_TITLE[edition]} не найдена</h3>
      <p className="muted">{text ?? "Сначала найдите игру на главной странице или укажите папку вручную."}</p>
      <button className="btn btn-primary" onClick={() => go("home")}>
        Найти GTA V
      </button>
    </div>
  );
}

export function CopyText({ text }: { text: string }) {
  const { toast } = useApp();
  return (
    <button
      className="icon-btn"
      title="Скопировать путь"
      onClick={() => navigator.clipboard.writeText(text).then(() => toast("info", "Скопировано"))}
    >
      <Copy size={14} />
    </button>
  );
}
