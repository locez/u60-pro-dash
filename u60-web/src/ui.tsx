import {
  cloneElement,
  isValidElement,
  useId,
  useEffect,
  useRef,
  useState,
  type ReactNode,
  type ReactElement,
} from "react";
import { Check, Eye, EyeOff, Info, RefreshCw, X } from "lucide-react";
import QRCode from "qrcode";
import { stateText, t } from "./i18n";
const stateFields = new Set([
  "Mode",
  "Traffic scope",
  "State",
  "Phase",
  "Status",
  "Connection",
  "Routing",
]);
export function Panel({
  children,
  className = "",
}: {
  children: ReactNode;
  className?: string;
}) {
  return <section className={"panel " + className}>{children}</section>;
}
export function PanelTitle({
  title,
  note,
  action,
}: {
  title: string;
  note?: string;
  action?: ReactNode;
}) {
  return (
    <header className="panel-heading">
      <div>
        <h2>{t(title)}</h2>
        {note && <p>{t(note)}</p>}
      </div>
      {action}
    </header>
  );
}
export function Note({
  children,
  warning = false,
}: {
  children: ReactNode;
  warning?: boolean;
}) {
  return (
    <div
      className={"notice " + (warning ? "warning" : "")}
      role={warning ? "alert" : undefined}
    >
      <Info size={16} />
      <span>{children}</span>
    </div>
  );
}
export function Empty({ children }: { children: ReactNode }) {
  return <p className="empty-state">{children}</p>;
}
export function Disclosure({title,children}:{title:string;children:ReactNode}) {
  const [open,setOpen]=useState(false);
  return <details className="advanced-details" onToggle={e=>setOpen(e.currentTarget.open)}><summary>{t(title)}</summary>{open&&children}</details>;
}
export function ErrorState({
  error,
  retry,
}: {
  error?: Error;
  retry?: () => void;
}) {
  return error ? (
    <Note warning>
      {t(error.message)}{" "}
      {retry && (
        <button className="text-link" onClick={retry}>
          {t("Retry")}
        </button>
      )}
    </Note>
  ) : null;
}
export function Status({
  children,
  on = true,
}: {
  children: ReactNode;
  on?: boolean;
}) {
  return (
    <span className={"status " + (on ? "positive" : "muted")}>
      <i />
      {children}
    </span>
  );
}
export function Stats({ items }: { items: [string, ReactNode, string?][] }) {
  return (
    <div className="stat-grid">
      {items.map(([name, value, hint]) => (
        <div key={name}>
          <span>{t(name)}</span>
          <strong>
            {(stateFields.has(name)
              ? (stateText(value) as ReactNode)
              : value) ?? "—"}
          </strong>
          {hint && <small>{t(hint)}</small>}
        </div>
      ))}
    </div>
  );
}
export function Definition({ items }: { items: [string, ReactNode][] }) {
  return (
    <dl className="definition">
      {items.map(([name, value]) => (
        <div key={name}>
          <dt>{t(name)}</dt>
          <dd>
            {(stateFields.has(name)
              ? (stateText(value) as ReactNode)
              : value) ?? "—"}
          </dd>
        </div>
      ))}
    </dl>
  );
}
export function Tabs({
  items,
  value,
  onChange,
}: {
  items: [string, string][];
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <nav className="page-tabs" aria-label={t("Page sections")}>
      {items.map(([id, label]) => (
        <button
          key={id}
          aria-current={id === value ? "page" : undefined}
          onClick={() => onChange(id)}
        >
          {t(label)}
        </button>
      ))}
    </nav>
  );
}
export function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <label className="field" htmlFor={id}>
      <span id={id + "-label"}>{t(label)}</span>
      {isValidElement(children)
        ? cloneElement(children as ReactElement<Record<string, unknown>>, {
            id,
            "aria-labelledby": id + "-label",
          })
        : children}
      {hint && <small>{t(hint)}</small>}
    </label>
  );
}
export function Password({
  value,
  onChange,
  onComplete,
  disabled = false,
  placeholder,
  id,
  "aria-labelledby": labelledBy,
}: {
  value: string;
  onChange: (value: string) => void;
  onComplete?: () => void;
  disabled?: boolean;
  placeholder?: string;
  id?: string;
  "aria-labelledby"?: string;
}) {
  const [show, setShow] = useState(false);
  return (
    <span
      className="password-input"
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget)) onComplete?.();
      }}
    >
      <input
        id={id}
        aria-labelledby={labelledBy}
        disabled={disabled}
        type={show ? "text" : "password"}
        value={value}
        autoComplete="off"
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.nativeEvent.isComposing) {
            e.preventDefault();
            e.currentTarget.blur();
          }
        }}
      />
      <button
        type="button"
        aria-label={t(show ? "Hide password" : "Show password")}
        onClick={() => setShow((s) => !s)}
      >
        {show ? <EyeOff size={17} /> : <Eye size={17} />}
      </button>
    </span>
  );
}
export function Toggle({
  on,
  onChange,
  label,
  disabled = false,
}: {
  on: boolean;
  onChange: () => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      className={"toggle " + (on ? "on" : "")}
      role="switch"
      aria-checked={on}
      aria-label={t(label)}
      disabled={disabled}
      onClick={onChange}
    >
      <span />
    </button>
  );
}
export function Modal({
  title,
  children,
  close,
  wide = false,
}: {
  title: string;
  children: ReactNode;
  close: () => void;
  wide?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const node = ref.current;
    node?.showModal();
    return () => node?.close();
  }, []);
  return (
    <dialog
      ref={ref}
      className={"product-dialog " + (wide ? "wide" : "")}
      onCancel={(e) => {
        e.preventDefault();
        close();
      }}
      aria-label={t(title)}
    >
      <header>
        <h2>{t(title)}</h2>
        <button className="icon-btn" onClick={close} aria-label={t("Close")}>
          <X size={20} />
        </button>
      </header>
      <div className="dialog-body">{children}</div>
    </dialog>
  );
}
export function Refresh({
  onClick,
  disabled = false,
}: {
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button className="btn small" onClick={onClick} disabled={disabled}>
      <RefreshCw size={14} />
      {t("Read current state")}
    </button>
  );
}
export function AutoFeedback({
  busy,
  message,
  error,
}: {
  busy: boolean;
  message?: string;
  error?: Error;
}) {
  return (
    <footer className="settings-feedback" role="status">
      {error ? <Info size={14} /> : <Check size={14} />}
      <span>
        {t(
          error?.message ||
            (busy ? "Applying…" : message || "Settings apply automatically."),
        )}
      </span>
    </footer>
  );
}
export function QR({
  ssid,
  password,
  encryption,
  hidden = false,
  size = 140,
}: {
  ssid: string;
  password: string;
  encryption: string;
  hidden?: boolean;
  size?: number;
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const escape = (v: string) => v.replace(/[\\;,:\"]/g, "\\$&");
    const type =
      encryption === "none"
        ? "nopass"
        : /wep/i.test(encryption)
          ? "WEP"
          : "WPA";
    const text = `WIFI:T:${type};S:${escape(ssid)};P:${escape(password)};H:${hidden ? "true" : "false"};;`;
    if (ref.current)
      QRCode.toCanvas(ref.current, text, {
        width: size,
        margin: 4,
        errorCorrectionLevel: "M",
      }).catch(() => {});
  }, [ssid, password, encryption, hidden, size]);
  return (
    <canvas
      ref={ref}
      width={size}
      height={size}
      aria-label={t("Wi-Fi connection code")}
    />
  );
}
export function Chart({
  series,
  height = 160,
}: {
  series: { name: string; values: (number | null)[]; color: string }[];
  height?: number;
}) {
  const all = series
    .flatMap((s) => s.values)
    .filter((n): n is number => n !== null && Number.isFinite(n));
  const max = Math.max(1, ...all);
  return (
    <div className="real-chart">
      <svg
        viewBox={`0 0 600 ${height}`}
        role="img"
        aria-label={t("Traffic trend")}
      >
        {[0, 1, 2, 3].map((i) => (
          <line
            key={i}
            x1="0"
            x2="600"
            y1={(i * height) / 3}
            y2={(i * height) / 3}
            stroke="var(--line)"
          />
        ))}
        {series.map((s) => (
          <path
            key={s.name}
            fill="none"
            stroke={s.color}
            strokeWidth="2"
            vectorEffect="non-scaling-stroke"
            d={s.values
              .map((n, i) =>
                n === null
                  ? ""
                  : `${i === 0 || s.values[i - 1] === null ? "M" : "L"}${(i * 600) / Math.max(1, s.values.length - 1)},${height - 5 - (n / max) * (height - 10)}`,
              )
              .join(" ")}
          />
        ))}
      </svg>
      {series.every((s) => s.values.length < 2) && (
        <span className="chart-empty">{t("No samples yet")}</span>
      )}
      <div className="chart-legend">
        {series.map((s) => (
          <span key={s.name}>
            <i style={{ background: s.color }} />
            {t(s.name)}
          </span>
        ))}
      </div>
    </div>
  );
}
export const number = (v: unknown) =>
  v !== null &&
  v !== undefined &&
  String(v).trim() !== "" &&
  Number.isFinite(Number(v))
    ? Number(v)
    : null;
export function bytes(v: unknown) {
  const n = number(v);
  if (n === null) return "—";
  const unit =
    n >= 1e12
      ? ["TB", 1e12]
      : n >= 1e9
        ? ["GB", 1e9]
        : n >= 1e6
          ? ["MB", 1e6]
          : n >= 1e3
            ? ["KB", 1e3]
            : ["B", 1];
  return `${(n / Number(unit[1])).toFixed(unit[0] === "B" ? 0 : 2)} ${unit[0]}`;
}
export function scalar(v: unknown): string {
  if (v === null || v === undefined || v === "") return "—";
  if (typeof v === "boolean") return t(v ? "Enabled" : "Disabled");
  return Array.isArray(v)
    ? v.map(scalar).join(" · ")
    : typeof v === "object"
      ? Object.entries(v as object)
          .map(([k, value]) => `${k}: ${scalar(value)}`)
          .join(" · ")
      : String(v);
}
