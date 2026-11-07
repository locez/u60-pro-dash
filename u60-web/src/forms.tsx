import { useEffect, useRef, useState } from "react";
import { api, type ObjectData } from "./api";
import { useResource, useSession } from "./live";
import { useMutation } from "./mutations";
import {
  AutoFeedback,
  ErrorState,
  Field,
  Panel,
  PanelTitle,
  Password,
  Refresh,
  scalar,
} from "./ui";
import { t } from "./i18n";
export type Option = [string, string];
export type FormField = {
  key: string;
  label: string;
  readKey?: string;
  type?: "text" | "password" | "number" | "textarea" | "url";
  options?: Option[];
  optionsFor?: (current: ObjectData) => Option[];
  wire?: "number" | "boolean";
  required?: boolean;
  min?: number;
  max?: number;
  maxLength?: number;
  hint?: string;
  allowMissing?: boolean;
};
export type FormSpec = {
  id: string;
  title: string;
  note?: string;
  read: string;
  readMethod?: string;
  readBody?: unknown;
  write?: string;
  method?: string;
  fields: FormField[];
  decode?: (data: ObjectData) => ObjectData;
  body?: (
    changed: ObjectData,
    all: ObjectData,
    previous: ObjectData,
  ) => ObjectData;
  validate?: (
    changed: ObjectData,
    all: ObjectData,
    previous: ObjectData,
  ) => void;
  verify?: (data: ObjectData, changed: ObjectData) => boolean;
  interrupt?:
    | string
    | ((changed: ObjectData, previous: ObjectData) => string | undefined);
  timeout?: number;
};
export const options = (
  key: string,
  label: string,
  values: Option[] | NonNullable<FormField["optionsFor"]>,
  extra: Partial<FormField> = {},
): FormField => ({
  key,
  label,
  ...(typeof values === "function"
    ? { optionsFor: values }
    : { options: values }),
  ...extra,
});
export const flag = (
  key: string,
  label: string,
  wire?: "number" | "boolean",
): FormField =>
  options(
    key,
    label,
    wire === "boolean"
      ? [
          ["false", "Disabled"],
          ["true", "Enabled"],
        ]
      : [
          ["0", "Disabled"],
          ["1", "Enabled"],
        ],
    { wire },
  );
export const field = (
  key: string,
  label: string,
  extra: Partial<FormField> = {},
): FormField => ({ key, label, ...extra });
const string = (value: unknown) =>
  value === undefined || value === null ? "" : String(value);
const same = (a: unknown, b: unknown) => string(a) === string(b);
export function SettingsForm({
  spec,
  onApplied,
}: {
  spec: FormSpec;
  onApplied?: () => void;
}) {
  const resource = useResource(spec.read, {
    method: spec.readMethod,
    body: spec.readBody,
  });
  const { epoch } = useSession();
  const mutation = useMutation();
  const [base, setBase] = useState<ObjectData>(),
    [draft, setDraft] = useState<ObjectData>({}),
    [error, setError] = useState<Error>(),
    [message, setMessage] = useState("");
  const dirty = useRef(new Set<string>()),
    active = useRef(true);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  useEffect(() => {
    if (!resource.data) return;
    try {
      const decoded = spec.decode ? spec.decode(resource.data) : resource.data;
      setBase(decoded);
      setDraft((old) =>
        Object.fromEntries(
          spec.fields.map((f) => [
            f.key,
            dirty.current.has(f.key)
              ? old[f.key]
              : f.type === "password"
                ? ""
                : string(decoded[f.readKey || f.key]),
          ]),
        ),
      );
    } catch (e) {
      setError(e as Error);
      setBase(undefined);
    }
  }, [resource.data]);
  const apply = async (f: FormField, value: string) => {
    if (!base || mutation.busy) return;
    const original = base[f.readKey || f.key];
    if ((f.type === "password" && !value) || same(value, original)) {
      dirty.current.delete(f.key);
      return;
    }
    const previous = {
      ...base,
      ...Object.fromEntries(
        spec.fields.map((item) => [item.key, base[item.readKey || item.key]]),
      ),
    };
    const nextValue =
      f.wire === "number"
        ? Number(value)
        : f.wire === "boolean"
          ? value === "true"
          : value;
    const changed = { [f.key]: nextValue },
      all = { ...previous, ...changed };
    try {
      if (f.required && !value.trim())
        throw new Error("This field is required");
      if (
        f.type === "number" &&
        (!/^-?\d+$/.test(value) ||
          (f.min !== undefined && Number(value) < f.min) ||
          (f.max !== undefined && Number(value) > f.max))
      )
        throw new Error(
          t("Enter a number between {0} and {1}", f.min ?? 0, f.max ?? 65535),
        );
      const choices = f.optionsFor?.(base) || f.options;
      if (choices && !choices.some(([id]) => id === value))
        throw new Error("Choose a supported value");
      spec.validate?.(changed, all, previous);
      const interrupt =
        typeof spec.interrupt === "function"
          ? spec.interrupt(changed, previous)
          : spec.interrupt;
      if (interrupt && !(await mutation.confirm({ message: interrupt }))) {
        dirty.current.delete(f.key);
        setDraft((s) => ({
          ...s,
          [f.key]: f.type === "password" ? "" : string(original),
        }));
        return;
      }
      if (!active.current) return;
      await mutation.run(async () => {
        setError(undefined);
        setMessage("Applying…");
        const before = await api(spec.read, {
          method: spec.readMethod,
          body: spec.readBody,
        });
        if (!active.current) return;
        const current = spec.decode ? spec.decode(before) : before;
        if (
          spec.fields.some(
            (item) =>
              item.type !== "password" &&
              !same(
                current[item.readKey || item.key],
                base[item.readKey || item.key],
              ),
          )
        ) {
          setBase(current);
          throw new Error(
            "Settings changed elsewhere. Review the current values before trying again.",
          );
        }
        await api(spec.write || spec.read, {
          method: spec.method || "PUT",
          body: spec.body ? spec.body(changed, all, previous) : changed,
          timeout: spec.timeout || 30000,
        });
        if (!active.current) return;
        let decoded: ObjectData = {},
          verified = false;
        for (let attempt = 0; attempt < 3; attempt++) {
          if (attempt) await new Promise((resolve) => setTimeout(resolve, 900));
          if (!active.current) return;
          const fresh = await api(spec.read, {
            method: spec.readMethod,
            body: spec.readBody,
          });
          if (!active.current) return;
          decoded = spec.decode ? spec.decode(fresh) : fresh;
          verified = spec.verify
            ? spec.verify(decoded, changed)
            : Object.entries(changed).every(([k, v]) =>
                same(
                  decoded[
                    spec.fields.find((item) => item.key === k)?.readKey || k
                  ],
                  v,
                ),
              );
          if (verified) break;
        }
        setBase(decoded);
        if (!verified)
          throw new Error("The router has not confirmed the change.");
        dirty.current.delete(f.key);
        setDraft((old) =>
          Object.fromEntries(
            spec.fields.map((item) => [
              item.key,
              dirty.current.has(item.key)
                ? old[item.key]
                : item.type === "password"
                  ? ""
                  : string(decoded[item.readKey || item.key]),
            ]),
          ),
        );
        setMessage("Applied and verified");
        onApplied?.();
      });
    } catch (e) {
      if (active.current) {
        setError(e as Error);
        setMessage("");
      }
    }
  };
  const edit = (f: FormField, value: string) => {
    dirty.current.add(f.key);
    setDraft((s) => ({ ...s, [f.key]: value }));
    setError(undefined);
    setMessage("Complete input to apply");
  };
  return (
    <Panel className="form-panel" key={epoch}>
      <PanelTitle title={spec.title} note={spec.note} />
      <ErrorState error={resource.error} retry={resource.reload} />
      {!base ? (
        <p className="loading-state">
          {t(resource.loading ? "Loading…" : "Settings unavailable")}
        </p>
      ) : (
        <div className="form-body">
          <div className="form-grid">
            {spec.fields.map((f) => {
              const available =
                f.allowMissing || base[f.readKey || f.key] !== undefined;
              const disabled = mutation.busy || !available;
              const value = string(draft[f.key]);
              const choices = f.optionsFor?.(base) || f.options;
              return (
                <Field
                  key={f.key}
                  label={f.label}
                  hint={!available ? "Not supported by this firmware" : f.hint}
                >
                  {choices ? (
                    <select
                      disabled={disabled}
                      value={value}
                      onChange={(e) => {
                        edit(f, e.target.value);
                        void apply(f, e.target.value);
                      }}
                    >
                      {!choices.some(([v]) => v === value) && (
                        <option value={value}>
                          {value ? scalar(value) : t("Not reported")}
                        </option>
                      )}
                      {choices.map(([v, label]) => (
                        <option key={v} value={v}>
                          {t(label)}
                        </option>
                      ))}
                    </select>
                  ) : f.type === "password" ? (
                    <Password
                      disabled={disabled}
                      value={value}
                      onChange={(v) => edit(f, v)}
                      onComplete={() => void apply(f, value)}
                      placeholder={t("Leave blank to keep current password")}
                    />
                  ) : f.type === "textarea" ? (
                    <textarea
                      rows={4}
                      disabled={disabled}
                      value={value}
                      onChange={(e) => edit(f, e.target.value)}
                      onBlur={() => void apply(f, value)}
                    />
                  ) : (
                    <input
                      disabled={disabled}
                      type={f.type || "text"}
                      value={value}
                      min={f.min}
                      max={f.max}
                      maxLength={f.maxLength}
                      autoComplete="off"
                      onChange={(e) => edit(f, e.target.value)}
                      onBlur={() => void apply(f, value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" && !e.nativeEvent.isComposing) {
                          e.preventDefault();
                          e.currentTarget.blur();
                        }
                      }}
                    />
                  )}
                </Field>
              );
            })}
          </div>
        </div>
      )}
      <AutoFeedback busy={mutation.busy} message={message} error={error} />
      {error && (
        <button
          className="text-link"
          disabled={mutation.busy}
          onClick={() => {
            dirty.current.clear();
            setError(undefined);
            setMessage("");
            resource.reload();
          }}
        >
          {t("Discard draft and read current values")}
        </button>
      )}
    </Panel>
  );
}
export function CommandForm({
  title,
  note,
  fields = [],
  endpoint,
  method = "POST",
  body,
  warning,
  label,
  after,
  danger = false,
  phrase,
  before,
}: {
  title: string;
  note?: string;
  fields?: FormField[];
  endpoint: string;
  method?: string;
  body?: (values: ObjectData) => unknown;
  warning?: string;
  label: string;
  after?: (data: ObjectData) => void;
  danger?: boolean;
  phrase?: string;
  before?: () => Promise<boolean | void>;
}) {
  const [values, setValues] = useState<ObjectData>(() =>
      Object.fromEntries(fields.map((f) => [f.key, f.options?.[0]?.[0] || ""])),
    ),
    [error, setError] = useState<Error>(),
    [result, setResult] = useState("");
  const mutation = useMutation();
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  return (
    <Panel>
      <PanelTitle title={title} note={note} />
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          try {
            if (
              warning &&
              !(await mutation.confirm({
                title,
                message: warning,
                danger,
                phrase,
              }))
            )
              return;
            await mutation.run(async () => {
              setError(undefined);
              const payload = body ? body(values) : values;
              if (before && (await before()) === false) return;
              if (!mounted.current) return;
              const data = await api(endpoint, {
                method,
                body: payload,
                timeout: 150000,
              });
              if (data && typeof data === "object" && "result" in data) {
                const accepted =
                  endpoint === "/api/sms/send"
                    ? [3, "3"]
                    : [0, "0", true, "success", "SUCCESS"];
                if (!accepted.includes(data.result))
                  throw new Error(
                    t(
                      "The modem returned result {0}; the action is not confirmed",
                      data.result,
                    ),
                  );
              }
              setResult(
                endpoint === "/api/sms/send"
                  ? "Send request accepted. Delivery is not confirmed."
                  : "Request accepted.",
              );
              after?.(data);
              setValues((s) => ({
                ...s,
                ...Object.fromEntries(
                  fields
                    .filter((f) => f.type === "password")
                    .map((f) => [f.key, ""]),
                ),
              }));
            });
          } catch (error) {
            setError(error as Error);
          }
        }}
      >
        <div className="form-grid">
          {fields.map((f) => (
            <Field key={f.key} label={f.label} hint={f.hint}>
              {f.options ? (
                <select
                  disabled={mutation.busy}
                  value={string(values[f.key])}
                  onChange={(e) =>
                    setValues((v) => ({ ...v, [f.key]: e.target.value }))
                  }
                >
                  {f.options.map(([v, label]) => (
                    <option value={v} key={v}>
                      {t(label)}
                    </option>
                  ))}
                </select>
              ) : f.type === "textarea" ? (
                <textarea
                  required={f.required}
                  rows={6}
                  value={string(values[f.key])}
                  onChange={(e) =>
                    setValues((v) => ({ ...v, [f.key]: e.target.value }))
                  }
                />
              ) : (
                <input
                  type={f.type || "text"}
                  required={f.required}
                  min={f.min}
                  max={f.max}
                  maxLength={f.maxLength}
                  autoComplete="off"
                  disabled={mutation.busy}
                  value={string(values[f.key])}
                  onChange={(e) =>
                    setValues((v) => ({ ...v, [f.key]: e.target.value }))
                  }
                />
              )}
            </Field>
          ))}
        </div>
        <ErrorState error={error} />
        {result && (
          <p className="form-result" role="status">
            {t(result)}
          </p>
        )}
        <button
          className={"btn " + (danger ? "danger" : "primary")}
          disabled={mutation.busy}
          type="submit"
        >
          {t(mutation.busy ? "Applying…" : label)}
        </button>
      </form>
    </Panel>
  );
}
export function ReadingsPanel({
  title,
  path,
  rows,
}: {
  title: string;
  path: string;
  rows: [string, string][];
}) {
  const r = useResource(path);
  return (
    <Panel>
      <PanelTitle
        title={title}
        action={<Refresh onClick={r.reload} disabled={r.loading} />}
      />
      <ErrorState error={r.error} />
      <dl className="definition">
        {rows.map(([key, label]) => (
          <div key={key}>
            <dt>{t(label)}</dt>
            <dd>{scalar(r.data?.[key])}</dd>
          </div>
        ))}
      </dl>
    </Panel>
  );
}
