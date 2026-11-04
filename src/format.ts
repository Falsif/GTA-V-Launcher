import type { Edition, Source } from "./types";

export const EDITION_TITLE: Record<Edition, string> = {
  legacy: "GTA V Legacy",
  enhanced: "GTA V Enhanced",
};

export const SOURCE_TITLE: Record<Source, string> = {
  steam: "Steam",
  epic: "Epic Games",
  rockstar: "Rockstar Games Launcher",
  unknown: "Не определён",
};

export const STEAM_APPID: Record<Edition, number> = { legacy: 271590, enhanced: 3240220 };

export const CATEGORIES: { id: string; label: string }[] = [
  { id: "vehicles", label: "Транспорт" },
  { id: "graphics", label: "Графика" },
  { id: "characters", label: "Персонажи" },
  { id: "maps", label: "Карты" },
  { id: "scripts", label: "Скрипты" },
  { id: "gameplay", label: "Геймплей" },
  { id: "other", label: "Другое" },
];

export const categoryLabel = (id: string) => CATEGORIES.find((c) => c.id === id)?.label ?? "Другое";

export function plural(n: number, one: string, few: string, many: string) {
  const m10 = n % 10;
  const m100 = n % 100;
  if (m10 === 1 && m100 !== 11) return one;
  if (m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14)) return few;
  return many;
}

export function formatDuration(seconds: number): string {
  if (!seconds || seconds < 60) return seconds > 0 ? "меньше минуты" : "0 мин";
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h === 0) return `${m} мин`;
  return `${h} ч ${m} мин`;
}

export function formatClock(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  const pad = (x: number) => String(x).padStart(2, "0");
  return `${pad(h)}:${pad(m)}:${pad(s)}`;
}

export function formatDate(iso: string | null | undefined): string {
  if (!iso) return "ещё не запускалась";
  const d = new Date(iso);
  if (isNaN(d.getTime())) return "—";
  return d.toLocaleString("ru-RU", { day: "numeric", month: "long", hour: "2-digit", minute: "2-digit" });
}

export function formatUnixDate(ts: number): string {
  return new Date(ts * 1000).toLocaleDateString("ru-RU", { day: "numeric", month: "long", year: "numeric" });
}

export function formatSize(bytes: number): string {
  const units = ["Б", "КБ", "МБ", "ГБ"];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return i === 0 ? `${bytes} Б` : `${v.toFixed(1)} ${units[i]}`;
}

/** «2026-11-04_10-15-30_manual» → «4 ноября 2026, 10:15 (вручную)» */
export function backupLabel(id: string): string {
  const m = id.match(/^(\d{4})-(\d{2})-(\d{2})_(\d{2})-(\d{2})-(\d{2})_(\w+)$/);
  if (!m) return id;
  const d = new Date(+m[1], +m[2] - 1, +m[3], +m[4], +m[5], +m[6]);
  const kind = m[7] === "manual" ? "вручную" : "авто";
  return `${d.toLocaleString("ru-RU", { day: "numeric", month: "long", year: "numeric", hour: "2-digit", minute: "2-digit" })} · ${kind}`;
}
