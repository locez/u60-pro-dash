import { useEffect, useRef, useState } from "react";
import { api, type ObjectData } from "./api";
import { useResource } from "./live";
import { useMutation } from "./mutations";
import { dnsModel } from "./models";
import { t } from "./i18n";
import { AutoFeedback, ErrorState, Field, Note, Panel, PanelTitle } from "./ui";

const keys = [
  "primary",
  "secondary",
  "doh_url",
  "dot_endpoint",
  "dot_server_name",
];
const labels: Record<string, string> = {
  primary: "Primary DNS",
  secondary: "Secondary DNS",
  doh_url: "DoH endpoint",
  dot_endpoint: "DoT server",
  dot_server_name: "DoT TLS certificate name",
};
const fingerprint = (v: ObjectData) => JSON.stringify(dnsModel.payload(v));
function ready(v: ObjectData) {
  if (v.mode === "automatic") return true;
  if (
    !v.primary ||
    (v.mode === "doh" && !v.doh_url) ||
    (v.mode === "dot" && (!v.dot_endpoint || !v.dot_server_name))
  )
    return false;
  if (!dnsModel.ipv4(v.primary) || (v.secondary && !dnsModel.ipv4(v.secondary)))
    throw new Error("This U60 firmware accepts IPv4 DNS addresses only.");
  if (v.mode === "doh") {
    const url = new URL(v.doh_url);
    if (url.protocol !== "https:" || url.username || url.password || url.hash)
      throw new Error("Must be an HTTPS DNS endpoint.");
  }
  return true;
}
export function DNSSettings({ onApplied }: { onApplied: () => void }) {
  const r = useResource("/api/dns/status"),
    mutation = useMutation();
  const [draft, setDraft] = useState<ObjectData>(),
    [error, setError] = useState<Error>(),
    [message, setMessage] = useState("");
  const current = useRef<ObjectData>({}),
    baseline = useRef<ObjectData>({}),
    dirty = useRef(false),
    mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  useEffect(() => {
    if (!r.data || dirty.current) return;
    baseline.current = r.data;
    current.current = { ...r.data };
    setDraft(current.current);
  }, [r.data]);
  const change = (key: string, value: string) => {
    current.current = { ...current.current, [key]: value };
    dirty.current = true;
    setDraft(current.current);
    setError(undefined);
    setMessage("Complete input to apply");
  };
  const apply = async (v: ObjectData) => {
    if (mutation.busy || !dirty.current) return;
    try {
      if (!ready(v)) {
        setMessage("Complete the required resolver fields to apply.");
        return;
      }
      if (fingerprint(v) === fingerprint(baseline.current)) {
        dirty.current = false;
        setMessage("");
        return;
      }
      const candidate = dnsModel.payload(v);
      await mutation.run(async () => {
        setMessage("Applying…");
        setError(undefined);
        const actual = await api("/api/dns/status");
        if (!mounted.current) return;
        if (fingerprint(actual) !== fingerprint(baseline.current))
          throw new Error(
            "Settings changed elsewhere. Review the current values before trying again.",
          );
        await api("/api/dns/apply", {
          method: "PUT",
          body: candidate,
          timeout: 40000,
        });
        const fresh = await api("/api/dns/status");
        if (!mounted.current) return;
        if (fingerprint(fresh) !== fingerprint(candidate))
          throw new Error("The router has not confirmed the change.");
        baseline.current = fresh;
        current.current = fresh;
        dirty.current = false;
        setDraft(fresh);
        setMessage("Applied and verified");
        onApplied();
      });
    } catch (e) {
      if (mounted.current) {
        setError(e as Error);
        setMessage("");
      }
    }
  };
  return (
    <Panel className="form-panel">
      <PanelTitle
        title="DNS settings"
        note="Every change tests the candidate resolver first. The previous settings are restored if verification fails."
      />
      <ErrorState error={r.error} retry={r.reload} />
      {draft && (
        <>
          <div className="form-grid">
            <Field label="Mode">
              <select
                value={draft.mode}
                disabled={mutation.busy}
                onChange={(e) => {
                  change("mode", e.target.value);
                  void apply(current.current);
                }}
              >
                {[
                  ["automatic", "Carrier automatic"],
                  ["manual", "Manual DNS"],
                  ["doh", "DNS over HTTPS"],
                  ["dot", "DNS over TLS"],
                ].map(([k, label]) => (
                  <option key={k} value={k}>
                    {t(label)}
                  </option>
                ))}
              </select>
            </Field>
            {keys
              .filter(
                (k) =>
                  draft.mode !== "automatic" &&
                  (k === "primary" ||
                    k === "secondary" ||
                    (draft.mode === "doh" && k === "doh_url") ||
                    (draft.mode === "dot" && k.startsWith("dot_"))),
              )
              .map((key) => (
                <Field key={key} label={labels[key]}>
                  <input
                    disabled={mutation.busy}
                    value={String(draft[key] || "")}
                    autoComplete="off"
                    spellCheck={false}
                    onChange={(e) => change(key, e.target.value)}
                    onBlur={() => void apply(current.current)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") e.currentTarget.blur();
                    }}
                  />
                </Field>
              ))}
          </div>
          <div className="button-row">
            {["cloudflare", "quad9"].map((key) => (
              <button
                key={key}
                className="btn"
                disabled={mutation.busy}
                onClick={() => {
                  current.current = {
                    ...current.current,
                    ...dnsModel.presets[key],
                    mode: "doh",
                  };
                  dirty.current = true;
                  setDraft(current.current);
                  void apply(current.current);
                }}
              >
                {dnsModel.presets[key].name} · DoH
              </button>
            ))}
          </div>
          <Note>{t("Settings apply automatically.")}</Note>
        </>
      )}
      <AutoFeedback busy={mutation.busy} error={error} message={message} />
      {error && (
        <button
          className="text-link"
          disabled={mutation.busy}
          onClick={() => {
            dirty.current = false;
            setError(undefined);
            setMessage("");
            r.reload();
          }}
        >
          {t("Discard draft and read current values")}
        </button>
      )}
    </Panel>
  );
}
