import { useEffect, useState } from "react";
import { api, type ObjectData } from "./api";
import { useReading, useResource } from "./live";
import { useMutation } from "./mutations";
import { actions, advanced } from "./models";
import { t } from "./i18n";
import {
  CommandForm,
  field,
  flag,
  options,
  SettingsForm,
  type FormField,
} from "./forms";
import {
  Definition,
  Empty,
  ErrorState,
  Field,
  Modal,
  Note,
  Panel,
  PanelTitle,
  Refresh,
  scalar,
  Stats,
  Toggle,
} from "./ui";
export function decodeMessage(raw: unknown) {
  const value = String(raw ?? "");
  return value.length % 4 || !/^[0-9a-f]+$/i.test(value)
    ? value
    : value
        .match(/.{4}/g)!
        .map((v) => String.fromCharCode(parseInt(v, 16)))
        .join("");
}
function messageDate(value: unknown) {
  const raw = String(value ?? ""),
    p = raw.split(",").map(Number);
  if (
    p.length < 6 ||
    p.some((n) => !Number.isFinite(n)) ||
    p[1] < 1 ||
    p[1] > 12 ||
    p[2] < 1 ||
    p[2] > 31 ||
    Math.abs(p[6] || 0) > 56
  )
    return raw;
  return new Date(
    Date.UTC(
      p[0] < 100 ? p[0] + 2000 : p[0],
      p[1] - 1,
      p[2],
      p[3],
      p[4],
      p[5],
    ) -
      (p[6] || 0) * 900000,
  ).toLocaleString();
}
export function MessagesPage({ tab }: { tab: string }) {
  if (tab === "compose")
    return (
      <CommandForm
        title="New message"
        note="Messages are sent from the router’s SIM. Carrier charges and multipart message limits apply."
        endpoint="/api/sms/send"
        fields={[
          field("number", "Recipient", { required: true, maxLength: 40 }),
          field("message", "Message", {
            required: true,
            type: "textarea",
            maxLength: 1530,
          }),
        ]}
        body={(v) => actions.smsBody(v.number, v.message)}
        label="Send message"
      />
    );
  if (tab === "storage") return <Storage />;
  if (tab === "forward") return <Forwarding />;
  return <Inbox />;
}
const listOptions = {
  method: "POST",
  body: {
    page: 0,
    data_per_page: 500,
    mem_store: 2,
    tags: 10,
    order_by: "order by id desc",
  },
};
function Inbox() {
  const r = useResource("/api/sms/list", listOptions),
    live = useReading("sms"),
    mutation = useMutation();
  const [selected, setSelected] = useState<string>(),
    [query, setQuery] = useState(""),
    [error, setError] = useState<Error>();
  useEffect(() => {
    if (live?.seq) r.reload();
  }, [live?.seq]);
  const messages: Array<ObjectData> = (r.data?.messages || []).map(
    (m: ObjectData) => ({ ...m, body: decodeMessage(m.content) }),
  );
  const current = messages.find((m) => String(m.id) === selected);
  const change = async (kind: string) => {
    if (!current) return;
    try {
      if (
        kind === "delete" &&
        !(await mutation.confirm({
          title: "Delete message",
          message:
            "Delete this message from the router? This cannot be undone.",
          danger: true,
        }))
      )
        return;
      await mutation.run(async () => {
        const id = current.id;
        await api("/api/sms/" + (kind === "delete" ? "delete" : "read"), {
          method: "POST",
          body: {
            id: actions.ids([id]),
            ...(kind === "delete" ? {} : { tag: kind === "read" ? 0 : 1 }),
          },
        });
        r.reload();
        if (kind === "delete") setSelected(undefined);
      });
    } catch (e) {
      setError(e as Error);
    }
  };
  return (
    <Panel>
      <PanelTitle
        title="Inbox"
        action={<Refresh onClick={r.reload} disabled={r.loading} />}
      />
      <ErrorState error={error || r.error} />
      <input
        className="list-search"
        aria-label={t("Search messages")}
        placeholder={t("Search messages")}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      <div className="message-workspace">
        <div className="message-list">
          {messages
            .filter((m) =>
              (m.number + " " + m.body)
                .toLowerCase()
                .includes(query.toLowerCase()),
            )
            .map((m) => (
              <button
                className={selected === String(m.id) ? "selected" : ""}
                key={m.id}
                onClick={() => setSelected(String(m.id))}
              >
                <b>
                  {String(m.tag) === "1" ? "● " : ""}
                  {m.number || t("Unknown sender")}
                </b>
                <span className="message-preview">{m.body}</span>
                <small>{messageDate(m.date)}</small>
              </button>
            ))}
          {!messages.length && (
            <Empty>
              {t(r.loading ? "Loading…" : "No messages to display.")}
            </Empty>
          )}
        </div>
        <article className="message-reader">
          {current ? (
            <>
              <div>
                <b>{current.number}</b>
                <p className="subtle">{messageDate(current.date)}</p>
              </div>
              <div className="message-body" key={current.id}>
                {current.body}
              </div>
              <div className="message-actions">
                <button
                  className="btn"
                  disabled={mutation.busy}
                  onClick={() =>
                    void change(String(current.tag) === "1" ? "read" : "unread")
                  }
                >
                  {t(String(current.tag) === "1" ? "Mark read" : "Mark unread")}
                </button>
                <button
                  className="btn"
                  disabled={mutation.busy}
                  onClick={() => void change("delete")}
                >
                  {t("Delete")}
                </button>
              </div>
            </>
          ) : (
            <Empty>{t("Select a message to read it.")}</Empty>
          )}
        </article>
      </div>
    </Panel>
  );
}
function Storage() {
  const r = useResource("/api/sms/capacity"),
    d = r.data;
  return (
    <Panel>
      <PanelTitle
        title="Message storage"
        action={<Refresh onClick={r.reload} />}
      />
      <ErrorState error={r.error} />
      <Stats
        items={[
          [
            "Internal storage",
            d ? `${d.sms_nvused_total ?? "—"} / ${d.sms_nv_total ?? "—"}` : "—",
          ],
          [
            "SIM",
            d
              ? `${d.sms_simused_total ?? "—"} / ${d.sms_sim_total ?? "—"}`
              : "—",
          ],
          ["Unread", d?.sms_dev_unread_num],
        ]}
      />
    </Panel>
  );
}
function Forwarding() {
  const r = useResource("/api/sms/forward/config"),
    logs = useResource("/api/sms/forward/log"),
    mutation = useMutation(),
    [editing, setEditing] = useState<ObjectData | null>(),
    [error, setError] = useState<Error>();
  const rules = r.data?.config?.rules || [];
  const change = async (rule: ObjectData, remove = false) => {
    try {
      if (
        remove &&
        !(await mutation.confirm({
          message: "Delete this forwarding rule?",
          danger: true,
        }))
      )
        return;
      await mutation.run(async () => {
        await api("/api/sms/forward/rules" + (remove ? "" : "/toggle"), {
          method: remove ? "DELETE" : "PUT",
          body: remove
            ? { id: rule.id }
            : { id: rule.id, enabled: !rule.enabled },
        });
        r.reload();
      });
    } catch (e) {
      setError(e as Error);
    }
  };
  return (
    <>
      <SettingsForm
        spec={{
          id: "forwarding",
          title: "Forwarding service",
          read: "/api/sms/forward/config",
          decode: (d) => d.config,
          fields: [
            flag("enabled", "Forwarding service", "boolean"),
            field("poll_interval_secs", "Fallback check interval (seconds)", {
              type: "number",
              wire: "number",
              min: 10,
              max: 86400,
            }),
            flag(
              "mark_read_after_forward",
              "Mark read after forwarding",
              "boolean",
            ),
            flag("delete_after_forward", "Delete after forwarding", "boolean"),
          ],
          interrupt: (c) =>
            c.enabled || c.delete_after_forward
              ? "Forwarding shares matching incoming messages with your chosen destinations. Deleting after forwarding permanently removes those messages."
              : undefined,
        }}
      />
      <Panel>
        <PanelTitle
          title="Forwarding rules"
          action={
            <button className="btn small" onClick={() => setEditing(null)}>
              {t("New rule")}
            </button>
          }
        />
        <ErrorState error={error || r.error} />
        <div className="table-scroll">
          <table className="data-table">
            <thead>
              <tr>
                {["Rule", "Destination", "Status", "Actions"].map((s) => (
                  <th key={s}>{t(s)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rules.map((rule: ObjectData) => (
                <tr key={rule.id}>
                  <td>{rule.name}</td>
                  <td>{rule.destination?.type}</td>
                  <td>
                    <Toggle
                      on={rule.enabled}
                      label={rule.name}
                      disabled={mutation.busy}
                      onChange={() => void change(rule)}
                    />
                  </td>
                  <td>
                    <button
                      className="text-link"
                      onClick={() => setEditing(rule)}
                    >
                      {t("Edit")}
                    </button>{" "}
                    ·{" "}
                    <button
                      className="text-link"
                      onClick={() => void change(rule, true)}
                    >
                      {t("Delete")}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {!rules.length && <Empty>{t("No forwarding rules")}</Empty>}
      </Panel>
      <Panel>
        <PanelTitle
          title="Delivery activity"
          action={<Refresh onClick={logs.reload} />}
        />
        <ErrorState error={logs.error} />
        <div className="table-scroll">
          <table className="data-table">
            <tbody>
              {(Array.isArray(logs.data)
                ? logs.data
                : logs.data?.entries || []
              ).map((row: ObjectData, i: number) => (
                <tr key={i}>
                  <td>{scalar(row.timestamp)}</td>
                  <td>{row.rule_name || row.rule_id}</td>
                  <td>{scalar(row.status || row.success)}</td>
                  <td>{row.error || "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Panel>
      {editing !== undefined && (
        <ForwardEditor
          saved={editing}
          close={() => setEditing(undefined)}
          done={() => {
            setEditing(undefined);
            r.reload();
          }}
        />
      )}
    </>
  );
}
function ForwardEditor({
  saved,
  close,
  done,
}: {
  saved: ObjectData | null;
  close: () => void;
  done: () => void;
}) {
  const dest = saved?.destination || {},
    filter = saved?.filter || {};
  const [v, setV] = useState<ObjectData>({
      name: saved?.name || "",
      enabled: String(saved?.enabled || false),
      filter: filter.type || "all",
      patterns: (filter.patterns || []).join(", "),
      keywords: (filter.keywords || []).join(", "),
      destination: dest.type || "telegram",
      chat_id: dest.chat_id || "",
      silent: String(dest.silent || false),
      method: dest.method || "POST",
      forward_number: dest.forward_number || "",
      ntfy_url: dest.type === "ntfy" ? dest.url : "https://ntfy.sh",
      topic: dest.topic || "",
    }),
    [error, setError] = useState<Error>();
  const mutation = useMutation();
  const fields: FormField[] = [
    field("name", "Rule name", { required: true, maxLength: 100 }),
    options("enabled", "Rule status", [
      ["false", "Disabled"],
      ["true", "Enabled"],
    ]),
    options("filter", "Match messages", [
      ["all", "All incoming messages"],
      ["sender", "Sender"],
      ["content", "Message content"],
      ["sender_and_content", "Sender and content"],
    ]),
    ...(["sender", "sender_and_content"].includes(v.filter)
      ? [field("patterns", "Sender patterns")]
      : []),
    ...(["content", "sender_and_content"].includes(v.filter)
      ? [field("keywords", "Message keywords")]
      : []),
    options(
      "destination",
      "Destination",
      ["telegram", "webhook", "sms", "ntfy", "discord", "slack"].map((s) => [
        s,
        s,
      ]),
    ),
    ...(
      {
        telegram: [
          field("bot_token", "New bot token", { type: "password" }),
          field("chat_id", "Chat ID or @channel"),
          flag("silent", "Silent notification", "boolean"),
        ],
        webhook: [
          field("url", "Webhook URL", { type: "url" }),
          options("method", "Request method", [
            ["POST", "POST"],
            ["PUT", "PUT"],
          ]),
          field("headers", "Replacement request headers", { type: "textarea" }),
        ],
        sms: [field("forward_number", "Forwarding phone number")],
        ntfy: [
          field("ntfy_url", "ntfy server address", { type: "url" }),
          field("topic", "ntfy topic"),
          field("token", "New access token", { type: "password" }),
        ],
        discord: [field("webhook_url", "Webhook URL", { type: "url" })],
        slack: [field("webhook_url", "Webhook URL", { type: "url" })],
      } as Record<string, FormField[]>
    )[v.destination],
  ];
  return (
    <Modal title={saved ? "Edit rule" : "New rule"} close={close} wide>
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          try {
            const body = advanced.rule(v, saved);
            await mutation.run(async () => {
              if (saved) {
                const fresh = await api("/api/sms/forward/config");
                if (
                  advanced.canonical(
                    fresh.config.rules.find(
                      (r: ObjectData) => r.id === saved.id,
                    ),
                  ) !== advanced.canonical(saved)
                )
                  throw new Error(
                    "The rule changed elsewhere. Read current rules before editing.",
                  );
              }
              await api("/api/sms/forward/rules", {
                method: saved ? "PUT" : "POST",
                body,
              });
              done();
            });
          } catch (e) {
            setError(e as Error);
          }
        }}
      >
        <div className="form-grid">
          {fields.map((f) => (
            <Field key={f.key} label={f.label}>
              {f.options ? (
                <select
                  value={v[f.key] || ""}
                  disabled={mutation.busy}
                  onChange={(e) =>
                    setV((s) => ({ ...s, [f.key]: e.target.value }))
                  }
                >
                  {f.options.map(([id, label]) => (
                    <option key={id} value={id}>
                      {t(label)}
                    </option>
                  ))}
                </select>
              ) : f.type === "textarea" ? (
                <textarea
                  value={v[f.key] || ""}
                  onChange={(e) =>
                    setV((s) => ({ ...s, [f.key]: e.target.value }))
                  }
                />
              ) : (
                <input
                  required={f.required}
                  type={f.type || "text"}
                  value={v[f.key] || ""}
                  autoComplete="off"
                  maxLength={f.maxLength}
                  disabled={mutation.busy}
                  onChange={(e) =>
                    setV((s) => ({ ...s, [f.key]: e.target.value }))
                  }
                />
              )}
            </Field>
          ))}
        </div>
        <p className="subtle">
          {t("Leave secret fields blank to keep the saved values.")}
        </p>
        <ErrorState error={error} />
        <button className="btn primary" disabled={mutation.busy}>
          {t("Finish editing")}
        </button>
      </form>
    </Modal>
  );
}
