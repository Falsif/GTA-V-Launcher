import { useEffect, useState } from "react";
import { AlertTriangle, CheckCircle2, Cpu, Download, HardDrive, MemoryStick, MonitorSmartphone, RefreshCw, Search, XCircle } from "lucide-react";
import { api, errText } from "../api";
import { useApp } from "../state";
import { Modal } from "./ui";
import type { Edition, SystemCheck } from "../types";

/** Минимальные требования (официальные, Rockstar). */
const REQ: Record<Edition, { ram: number; disk: number; note: string }> = {
  legacy: { ram: 4, disk: 110, note: "Лучший выбор для слабого ПК: идёт на видеокартах уровня GeForce 9800 GT с 1 ГБ памяти." },
  enhanced: { ram: 8, disk: 110, note: "Нужен мощный ПК: видеокарта с DirectX 12 и 4 ГБ памяти, рекомендуется SSD." },
};

const STORES = [
  { id: "steam", label: "Steam", key: "steam" as const },
  { id: "epic", label: "Epic Games", key: "epic" as const },
  { id: "rockstar", label: "Rockstar Games Launcher", key: "rockstar" as const },
];

export default function InstallDialog({ onClose }: { onClose: () => void }) {
  const { toast, detect, edition: current } = useApp();
  const [edition, setEdition] = useState<Edition>(current);
  const [check, setCheck] = useState<SystemCheck | null>(null);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);

  const runCheck = async () => {
    setLoading(true);
    try {
      setCheck(await api.systemCheck());
    } catch (e) {
      toast("error", "Не удалось проверить компьютер", errText(e));
    } finally {
      setLoading(false);
    }
  };
  useEffect(() => {
    runCheck();
  }, []);

  const req = REQ[edition];
  const goodDrives = check?.drives.filter((d) => d.enough) ?? [];
  const ramOk = check ? check.ram_gb >= req.ram - 0.5 : true;

  const install = async (store: string) => {
    try {
      const r = await api.installGame(edition, store);
      setMessage(r.message);
      toast(r.opened ? "success" : "info", r.opened ? "Установка открыта в клиенте" : "Нужно установить клиент", r.message);
    } catch (e) {
      toast("error", "Не удалось начать установку", errText(e));
    }
  };

  return (
    <Modal
      title="Установка GTA V"
      wide
      onClose={onClose}
      footer={
        <>
          <button className="btn btn-ghost" onClick={onClose}>
            Закрыть
          </button>
          <button
            className="btn btn-primary"
            onClick={async () => {
              await detect(false);
              onClose();
            }}
          >
            <Search size={15} /> Игра установилась — найти её
          </button>
        </>
      }
    >
      <p className="muted small">
        Игру скачивает официальный клиент — нужен аккаунт, на котором GTA V куплена. Лаунчер открывает окно установки и
        заранее проверяет, хватит ли компьютеру места и памяти.
      </p>

      <div className="install-editions">
        {(["legacy", "enhanced"] as Edition[]).map((e) => (
          <button key={e} className={`install-edition ${edition === e ? "active" : ""}`} onClick={() => setEdition(e)}>
            <b>{e === "legacy" ? "GTA V Legacy" : "GTA V Enhanced"}</b>
            <small>{REQ[e].note}</small>
          </button>
        ))}
      </div>

      <div className="card-head" style={{ marginTop: 16 }}>
        <h3>Проверка компьютера</h3>
        <button className="icon-btn" onClick={runCheck} title="Проверить ещё раз">
          <RefreshCw size={15} className={loading ? "spin" : ""} />
        </button>
      </div>
      {loading && !check ? (
        <div className="skeleton-list" style={{ height: 120 }} />
      ) : check ? (
        <div className="syscheck">
          <div className={`syscheck-row ${ramOk ? "ok" : "bad"}`}>
            <MemoryStick size={17} />
            <span>Оперативная память</span>
            <b>{check.ram_gb} ГБ</b>
            {ramOk ? <CheckCircle2 size={16} /> : <XCircle size={16} />}
          </div>
          {!ramOk && (
            <p className="syscheck-hint">
              Нужно минимум {req.ram} ГБ. При нехватке памяти игра может не загрузиться — увеличьте файл подкачки до 16 ГБ.
            </p>
          )}
          <div className="syscheck-row">
            <Cpu size={17} />
            <span>Процессор</span>
            <b title={check.cpu}>
              {check.cpu} · {check.cores} потоков
            </b>
          </div>
          <div className="syscheck-row">
            <MonitorSmartphone size={17} />
            <span>Видеокарта</span>
            <b>{check.gpus.length ? check.gpus.join(", ") : "не определена"}</b>
          </div>
          <div className={`syscheck-row ${goodDrives.length ? "ok" : "bad"}`}>
            <HardDrive size={17} />
            <span>Свободное место (нужно ~{req.disk}+ ГБ)</span>
            <b />
            {goodDrives.length ? <CheckCircle2 size={16} /> : <XCircle size={16} />}
          </div>
          <div className="drives">
            {check.drives.map((d) => (
              <div key={d.mount} className={`drive ${d.enough ? "ok" : ""}`}>
                <b>{d.mount}</b>
                <div className="drive-bar">
                  <span style={{ width: `${Math.min(100, 100 - (d.free_gb / Math.max(d.total_gb, 1)) * 100)}%` }} />
                </div>
                <small>
                  свободно {d.free_gb} из {d.total_gb} ГБ{d.removable ? " · съёмный" : ""}
                </small>
              </div>
            ))}
          </div>
          {!goodDrives.length && (
            <p className="syscheck-hint">
              Ни на одном диске нет {req.disk} ГБ свободного места — из-за этого установка сразу выдаёт ошибку. Освободите
              место (удалите ненужные игры, очистите «Загрузки» и корзину) и нажмите обновить.
            </p>
          )}
          {goodDrives.length > 0 && (
            <p className="muted small">
              При установке выберите диск: {goodDrives.map((d) => d.mount).join(", ")}.
            </p>
          )}
        </div>
      ) : null}

      <h3 style={{ marginTop: 16 }}>Где куплена игра?</h3>
      <div className="store-buttons">
        {STORES.map((s) => (
          <button key={s.id} className={`btn ${s.id === "steam" ? "btn-primary" : ""}`} onClick={() => install(s.id)}>
            <Download size={15} /> {s.label}
            {check && !check[s.key] && <small className="store-missing">не установлен</small>}
          </button>
        ))}
      </div>
      {message && (
        <div className="warning warning-info">
          <AlertTriangle size={16} />
          <span>{message}</span>
        </div>
      )}
    </Modal>
  );
}
