import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
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
  confirm?:
    | string
    | ((next: Fields, previous: Fields, key: string) => string | undefined);
  validate?: (v: Fields) => string;
  children: (
    v: Fields,
    set: (key: string, value: string | boolean) => void,
  ) => ReactNode;
  onSaved?: (v: Fields) => void;
}) {
  const model = useModel();
  // Defaults are a snapshot for this form, not a new object on every render.
  const initialValues = useMemo(() => initial || {}, [id]);
  const saved = model.settings[id] || initialValues;
  const applied = useRef(saved);
  const drafts = useRef(new Map<string, string | boolean>());
  const inputKeys = useRef(new WeakMap<HTMLElement, string>());
  const [values, setValues] = useState<Fields>(saved);
  const [pending, setPending] = useState<{
    key: string;
    value: string | boolean;
    message: string;
  } | null>(null);
  const [error, setError] = useState("");
  const [hasApplied, setHasApplied] = useState(false);
  useEffect(() => {
    drafts.current.clear();
    inputKeys.current = new WeakMap();
    setError("");
    setPending(null);
    setHasApplied(false);
  }, [id]);
  useEffect(() => {
    applied.current = saved;
    const next = { ...saved, ...Object.fromEntries(drafts.current) };
    setValues((current) =>
      JSON.stringify(current) === JSON.stringify(next) ? current : next,
    );
  }, [saved]);
  const apply = (key: string, value: string | boolean) => {
    const next = { ...applied.current, [key]: value };
    applied.current = next;
    drafts.current.delete(key);
    model.save(id, next);
    onSaved?.(next);
    setValues((current) => ({ ...current, [key]: value }));
    setPending(null);
    setHasApplied(true);
    setError("");
  };
  const commit = (key: string, value: string | boolean) => {
    if (value === applied.current[key]) {
      drafts.current.delete(key);
      return;
    }
    const next = { ...applied.current, [key]: value };
    const issue = validate?.(next);
    if (issue) {
      setError(issue);
      return;
    }
    const message =
      typeof confirm === "function"
        ? confirm(next, applied.current, key)
        : confirm;
    if (message) setPending({ key, value, message });
    else apply(key, value);
  };
  const cancel = () => {
    if (pending) {
      drafts.current.delete(pending.key);
      setValues((current) => ({
        ...current,
        [pending.key]: applied.current[pending.key],
      }));
    }
    setPending(null);
    setError("");
  };
  const change = (key: string, value: string | boolean) => {
    setValues((current) => ({ ...current, [key]: value }));
    setError("");
    drafts.current.set(key, value);
    const input = document.activeElement;
    const textInput =
      input instanceof HTMLTextAreaElement ||
      (input instanceof HTMLInputElement &&
        !["checkbox", "radio", "range", "color", "file", "button"].includes(
          input.type,
        ));
    if (textInput && key !== "accent") inputKeys.current.set(input, key);
    else commit(key, value);
  };
  const finishInput = (
    target: EventTarget,
    relatedTarget: EventTarget | null,
  ) => {
    if (!(target instanceof HTMLElement)) return;
    const password = target.closest(".password-input");
    if (
      password &&
      relatedTarget instanceof Node &&
      password.contains(relatedTarget)
    )
      return;
    const input =
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement
        ? target
        : password?.querySelector("input");
    if (!input) return;
    const key = inputKeys.current.get(input);
    if (!key || !drafts.current.has(key)) return;
    if (!input.validity.valid) {
      setError(input.validationMessage);
      return;
    }
    commit(key, drafts.current.get(key)!);
  };
  const dirty = JSON.stringify(values) !== JSON.stringify(saved);
  return (
    <Panel className="form-panel">
      <PanelTitle title={title} note={note} />
      <div
        className="form-body"
        onBlurCapture={(event) =>
          finishInput(event.target, event.relatedTarget)
        }
        onKeyDownCapture={(event) => {
          if (
            event.key === "Enter" &&
            !event.nativeEvent.isComposing &&
            event.target instanceof HTMLInputElement
          ) {
            event.preventDefault();
            event.target.blur();
          }
        }}
      >
        {children(values, change)}
      </div>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      <footer className="settings-feedback" role="status">
        {hasApplied && !dirty && !error ? (
          <Check size={14} />
        ) : (
          <Info size={14} />
        )}
        <span>
          {error
            ? "请修正输入，尚未应用此修改"
            : pending
              ? "等待确认后应用"
              : dirty
                ? "输入完成后自动应用"
                : hasApplied
                  ? "已自动应用"
                  : "开关与选项即时生效，文字输入完成后生效"}
        </span>
      </footer>
      {pending && (
        <Modal title="此更改会中断连接" close={cancel}>
          <p>{pending.message}</p>
          <div className="dialog-actions">
            <button className="btn" onClick={cancel}>
              取消修改
            </button>
            <button
              className="btn primary"
              onClick={() => apply(pending.key, pending.value)}
            >
              继续应用
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
