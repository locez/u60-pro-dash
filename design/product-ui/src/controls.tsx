import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  Check,
  ChevronRight,
  Eye,
  EyeOff,
  Info,
  Search,
  X,
} from "lucide-react";
import { Panel, PanelTitle, Toggle } from "./ui";
import { useModel, type Fields } from "./model";

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
    ref.current?.showModal();
    return () => ref.current?.close();
  }, []);
  return (
    <dialog
      ref={ref}
      className={"product-dialog " + (wide ? "wide" : "")}
      onCancel={close}
      onClick={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <header>
        <h2>{title}</h2>
        <button className="icon-btn" aria-label="关闭" onClick={close}>
          <X size={20} />
        </button>
      </header>
      <div className="dialog-body">{children}</div>
    </dialog>
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
    <div className={"notice " + (warning ? "warning" : "")}>
      <Info size={16} />
      <span>{children}</span>
    </div>
  );
}
export function Stats({ items }: { items: [string, string, string?][] }) {
  return (
    <div className="stat-grid">
      {items.map(([name, value, hint]) => (
        <div key={name}>
          <span>{name}</span>
          <strong>{value}</strong>
          {hint && <small>{hint}</small>}
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
          <dt>{name}</dt>
          <dd>{value}</dd>
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
  items: string[];
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <nav className="page-tabs" aria-label="页面分类">
      {items.map((item) => (
        <button
          key={item}
          aria-current={item === value ? "page" : undefined}
          onClick={() => onChange(item)}
        >
          {item}
        </button>
      ))}
    </nav>
  );
}
export function SearchBox({
  value,
  onChange,
  placeholder,
}: {
  value: string;
  onChange: (s: string) => void;
  placeholder: string;
}) {
  return (
    <label className="search-field">
      <Search size={16} />
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        aria-label={placeholder}
      />
    </label>
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
  return (
    <label className="field">
      <span>{label}</span>
      {children}
      {hint && <small>{hint}</small>}
    </label>
  );
}
export function Password({
  value,
  onChange,
}: {
  value: string;
  onChange: (v: string) => void;
}) {
  const [show, setShow] = useState(false);
  return (
    <span className="password-input">
      <input
        type={show ? "text" : "password"}
        autoComplete="off"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
      <button
        type="button"
        aria-label={show ? "隐藏密码" : "显示密码"}
        onClick={() => setShow(!show)}
      >
        {show ? <EyeOff size={17} /> : <Eye size={17} />}
      </button>
    </span>
  );
}
export function SwitchRow({
  title,
  hint,
  value,
  onChange,
}: {
  title: string;
  hint?: string;
  value: boolean;
  onChange: () => void;
}) {
  return (
    <div className="switch-row">
      <div>
        <b>{title}</b>
        {hint && <p>{hint}</p>}
      </div>
      <Toggle on={value} onChange={onChange} label={title} />
    </div>
  );
}
export function ConfigForm({
  id,
  title,
  note,
  initial,
  confirm,
  validate,
  children,
  onSaved,
}: {
  id: string;
  title: string;
  note?: string;
  initial?: Fields;
  confirm?: string;
  validate?: (v: Fields) => string;
  children: (
    v: Fields,
    set: (key: string, value: string | boolean) => void,
  ) => ReactNode;
  onSaved?: (v: Fields) => void;
}) {
  const model = useModel();
  const saved = model.settings[id] || initial || {};
  const [values, setValues] = useState<Fields>(saved),
    [confirming, setConfirming] = useState(false),
    [error, setError] = useState("");
  const dirty = JSON.stringify(values) !== JSON.stringify(saved);
  const apply = () => {
    model.save(id, values);
    onSaved?.(values);
    setConfirming(false);
    model.notify("示例设置已应用；未连接真实设备");
  };
  return (
    <Panel className="form-panel">
      <PanelTitle title={title} note={note} />
      <div className="form-body">
        {children(values, (key, value) => {
          setValues((v) => ({ ...v, [key]: value }));
          setError("");
        })}
      </div>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      <footer className="save-bar">
        <span>
          {dirty ? (
            "有未应用的更改"
          ) : (
            <>
              <Check size={14} /> 设置已同步
            </>
          )}
        </span>
        <button
          className="btn"
          disabled={!dirty}
          onClick={() => {
            setValues(saved);
            setError("");
          }}
        >
          撤销
        </button>
        <button
          className="btn primary"
          disabled={!dirty}
          onClick={() => {
            const issue = validate?.(values);
            if (issue) return setError(issue);
            if (confirm) setConfirming(true);
            else apply();
          }}
        >
          应用更改
        </button>
      </footer>
      {confirming && (
        <Modal title="应用连接设置" close={() => setConfirming(false)}>
          <p>{confirm}</p>
          <div className="dialog-actions">
            <button className="btn" onClick={() => setConfirming(false)}>
              继续编辑
            </button>
            <button className="btn primary" onClick={apply}>
              确认应用
            </button>
          </div>
        </Modal>
      )}
    </Panel>
  );
}
export function Entry({
  title,
  description,
  onClick,
  icon,
}: {
  title: string;
  description?: string;
  onClick: () => void;
  icon?: ReactNode;
}) {
  return (
    <button className="setting-entry" onClick={onClick}>
      {icon}
      <span>
        <b>{title}</b>
        {description && <small>{description}</small>}
      </span>
      <ChevronRight size={16} />
    </button>
  );
}
