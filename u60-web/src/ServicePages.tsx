import { useEffect, useRef, useState } from "react";
import {
  ArrowLeft,
  ArrowUpRight,
  Network,
  Router,
  ShieldCheck,
} from "lucide-react";
import { api, type ObjectData } from "./api";
import { go, type Route } from "./catalog";
import { useJobPolling, useResource } from "./live";
import { useMutation } from "./mutations";
import {
  bytes,
  Definition,
  Empty,
  ErrorState,
  Field,
  Note,
  Panel,
  PanelTitle,
  Refresh,
  scalar,
  Stats,
  Status,
  Tabs,
} from "./ui";
import { CommandForm, field, flag, options, SettingsForm } from "./forms";
import { Installer } from "./Installer";
import { wireguardModel } from "./models";
import { t } from "./i18n";
const names: Record<string, string> = {
  tailscale: "Tailscale",
  wireguard: "WireGuard",
  mihomo: "Mihomo",
};
const endpoints: Record<string, string> = {
  tailscale: "/api/tailscale/status",
  wireguard: "/api/vpn/wireguard",
  mihomo: "/api/proxy/mihomo/status",
};
const active = (id: string, d?: ObjectData) =>
  id === "tailscale"
    ? d?.backend_state === "Running"
    : id === "wireguard"
      ? d?.active === true
      : d?.running === true;
export function ServicePages({ route }: { route: Route }) {
  const id = route.tab;
  if (!id || !names[id])
    return (
      <div className="service-cards">
        {Object.keys(names).map((id) => (
          <ServiceCard key={id} id={id} />
        ))}
      </div>
    );
  return <ServiceDetail key={id} id={id} tab={route.detail || "overview"} />;
}
function ServiceCard({ id }: { id: string }) {
  const r = useResource(endpoints[id]),
    d = r.data;
  const Icon =
    id === "tailscale" ? Network : id === "wireguard" ? ShieldCheck : Router;
  return (
    <Panel>
      <div className="service-card-top">
        <span className="service-icon">
          <Icon size={23} />
        </span>
        <Status on={active(id, d)}>
          {t(
            !d
              ? "Loading…"
              : active(id, d)
                ? "Running"
                : d.installed === false
                  ? "Not installed"
                  : "Stopped",
          )}
        </Status>
      </div>
      <h2>{names[id]}</h2>
      <ErrorState error={r.error} retry={r.reload} />
      <Definition
        items={
          id === "tailscale"
            ? [
                ["Node", d?.hostname],
                [
                  "Accept remote subnet routes",
                  d ? t(d.accept_routes ? "Enabled" : "Disabled") : null,
                ],
                [
                  "Routing mode",
                  d
                    ? t(
                        d.gateway_mode
                          ? "Kernel gateway"
                          : "Device access only",
                      )
                    : null,
                ],
              ]
            : id === "wireguard"
              ? [
                  ["Interface", d?.interface],
                  ["Peers", d?.peers?.length],
                  ["Routing", d?.mode],
                ]
              : [
                  ["Mode", d?.mode],
                  ["Traffic scope", d?.capture],
                  [
                    "Configuration",
                    d ? t(d.configured ? "Saved" : "Not imported") : null,
                  ],
                ]
        }
      />
      <button className="btn full-width" onClick={() => go("services", id)}>
        {t("Manage")} {names[id]} <ArrowUpRight size={15} />
      </button>
    </Panel>
  );
}
function ServiceDetail({ id, tab }: { id: string; tab: string }) {
  const r = useResource(endpoints[id]),
    mutation = useMutation(),
    [error, setError] = useState<Error>();
  const d = r.data;
  useJobPolling(
    r,
    !!d?.updating || d?.state === "starting" || d?.state === "respawning",
    3000,
  );
  const items: [string, string][] =
    id === "mihomo"
      ? [
          ["overview", "Overview"],
          ["configuration", "Configuration"],
          ["groups", "Proxy groups"],
          ["logs", "Logs"],
          ["panel", "MetaCubeXD"],
          ["installation", "Installation & updates"],
        ]
      : id === "tailscale"
        ? [
            ["overview", "Overview"],
            ["network", "Network settings"],
            ["routes", "Subnet routes"],
            ["probe", "Connectivity test"],
            ["logs", "Logs"],
            ["installation", "Installation & updates"],
          ]
        : [
            ["overview", "Overview"],
            ["configuration", "Tunnel configuration"],
            ["peers", "Peers"],
          ];
  const change = async () => {
    try {
      const stopping = active(id, d) || d?.enabled;
      if (
        stopping &&
        !(await mutation.confirm({
          message: t(
            "Stop {0}? Connections through this service will disconnect.",
            names[id],
          ),
        }))
      )
        return;
      await mutation.run(async () => {
        const base =
          id === "wireguard"
            ? "/api/vpn/wireguard"
            : id === "tailscale"
              ? "/api/tailscale"
              : "/api/proxy/mihomo";
        await api(base + (stopping ? "/disable" : "/enable"), {
          method: "POST",
          body: {},
          timeout: 150000,
        });
        r.reload();
      });
    } catch (e) {
      setError(e as Error);
    }
  };
  return (
    <>
      <button className="back-link" onClick={() => go("services")}>
        <ArrowLeft size={15} />
        {t("All services")}
      </button>
      <div className="service-detail-heading">
        <span className="large-icon">
          <Network size={28} />
        </span>
        <div>
          <h2>{names[id]}</h2>
          <p>
            {t(
              id === "tailscale"
                ? "Remote devices and subnet routes"
                : id === "wireguard"
                  ? "Encrypted tunnels"
                  : "Native configuration and routing policies",
            )}
          </p>
        </div>
        <Status on={active(id, d)}>
          {t(!d ? "Loading…" : active(id, d) ? "Running" : "Stopped")}
        </Status>
        <button
          className="btn"
          disabled={
            !d ||
            mutation.busy ||
            d?.updating ||
            d?.installed === false ||
            (id === "mihomo" && !d?.configured)
          }
          onClick={() => void change()}
        >
          {t(active(id, d) || d?.enabled ? "Stop" : "Start")}
        </button>
      </div>
      <ErrorState error={error || r.error} retry={r.reload} />
      {d?.last_error && <Note warning>{d.last_error}</Note>}
      <Tabs
        items={items}
        value={tab}
        onChange={(tab) => go("services", id, tab)}
      />
      {tab === "installation" ? (
        <Installer service={id} installed={d} onInstalled={r.reload} />
      ) : id === "mihomo" ? (
        <Mihomo tab={tab} data={d} reload={r.reload} />
      ) : id === "tailscale" ? (
        <Tailscale tab={tab} data={d} reload={r.reload} />
      ) : (
        <WireGuard tab={tab} data={d} reload={r.reload} />
      )}
    </>
  );
}
function Tailscale({
  tab,
  data: d,
  reload,
}: {
  tab: string;
  data?: ObjectData;
  reload: () => void;
}) {
  const [auth, setAuth] = useState("");
  if (tab === "network")
    return (
      <SettingsForm
        onApplied={reload}
        spec={{
          id: "tailscale",
          title: "Network settings",
          read: "/api/tailscale/status",
          write: "/api/tailscale/config",
          decode: (d) => ({
            ...d,
            advertise_routes: (d.advertise_routes || []).join(", "),
          }),
          fields: [
            field("hostname", "Device name", { required: true, maxLength: 63 }),
            field("advertise_routes", "Advertised IPv4 routes"),
            flag("accept_routes", "Accept remote subnet routes", "boolean"),
            flag("tailscale_ssh", "Tailscale SSH", "boolean"),
            flag("hold_wakelock", "Keep network awake", "boolean"),
          ],
          body: (c) => ({
            ...c,
            ...("advertise_routes" in c
              ? {
                  advertise_routes: String(c.advertise_routes)
                    .split(",")
                    .map((s) => s.trim())
                    .filter(Boolean),
                }
              : {}),
          }),
        }}
      />
    );
  if (tab === "routes" || tab === "probe" || tab === "logs")
    return <ServiceNetwork tab={tab} />;
  return (
    <>
      <Stats
        items={[
          ["Connection", d?.backend_state],
          ["Node", d?.hostname],
          ["Addresses", (d?.tailscale_ips || []).join(" · ")],
          [
            "Routing mode",
            d
              ? t(d.gateway_mode ? "Kernel gateway" : "Device access only")
              : "—",
          ],
        ]}
      />
      <Panel>
        <PanelTitle title="Network health" />
        <Definition
          items={[
            ["Network name", d?.dns_name],
            [
              "Accept remote subnet routes",
              d ? t(d.accept_routes_effective ? "Enabled" : "Not active") : "—",
            ],
            [
              "Health",
              (d?.health || []).join(" · ") || t("No reported warning"),
            ],
            ["Version", d?.client_version || d?.pinned_version],
            ["Remote web port", d?.management_port],
          ]}
        />
      </Panel>
      {d?.backend_state !== "Running" && (
        <>
          <CommandForm
            title="Connect Tailscale"
            endpoint="/api/tailscale/login-url"
            body={() => ({})}
            label="Get authorization link"
            after={(r) => {
              try {
                const u = new URL(r.login_url);
                if (
                  u.protocol === "https:" &&
                  (u.hostname === "login.tailscale.com" ||
                    u.hostname.endsWith(".tailscale.com"))
                )
                  setAuth(u.href);
              } catch {}
              reload();
            }}
          />
          {auth && (
            <a
              className="btn primary"
              href={auth}
              target="_blank"
              rel="noreferrer noopener"
            >
              {t("Open authorization page")} <ArrowUpRight size={15} />
            </a>
          )}
          <details className="advanced-details">
            <summary>{t("Enroll with an auth key")}</summary>
            <CommandForm
              title="Tailscale auth key"
              endpoint="/api/tailscale/setup"
              fields={[
                field("auth_key", "Auth key", {
                  type: "password",
                  required: true,
                }),
              ]}
              label="Enroll router"
              after={reload}
            />
          </details>
        </>
      )}
      <details className="advanced-details">
        <summary>{t("Account")}</summary>
        <CommandForm
          title="Sign out of Tailscale"
          endpoint="/api/tailscale/logout"
          body={() => ({})}
          label="Sign out"
          warning="Remove this router’s network identity. Remote access stops and another authorization is required to reconnect."
          after={reload}
        />
      </details>
    </>
  );
}
function ServiceNetwork({ tab }: { tab: string }) {
  const r = useResource(
    "/api/tailscale/" + (tab === "logs" ? "logs" : "network"),
  );
  const [target, setTarget] = useState(""),
    [probe, setProbe] = useState<ObjectData>();
  return (
    <>
      <ErrorState error={r.error} retry={r.reload} />
      {tab === "logs" ? (
        <Panel>
          <PanelTitle title="Logs" action={<Refresh onClick={r.reload} />} />
          <pre className="log-view">{r.data?.text || t("No log entries")}</pre>
        </Panel>
      ) : tab === "probe" ? (
        <>
          <CommandForm
            title="Connectivity test"
            note="Test an actual TCP service independently of the node status."
            endpoint="/api/tailscale/probe"
            fields={[
              field("target", "IP address and TCP port", { required: true }),
            ]}
            label="Test connectivity"
            after={setProbe}
          />
          {probe && (
            <Panel>
              <PanelTitle title="Test result" />
              <Definition
                items={[
                  ["Target", probe.target],
                  ["Route", probe.route],
                  ["TCP", probe.reachable ? t("Reachable") : t("Unreachable")],
                  [
                    "Latency",
                    probe.latency_ms === undefined
                      ? "—"
                      : probe.latency_ms + " ms",
                  ],
                  [
                    "Hotspot access",
                    probe.gateway_ready ? t("Ready") : t("Not verified"),
                  ],
                ]}
              />
            </Panel>
          )}
        </>
      ) : (
        <Panel>
          <PanelTitle
            title="Subnet routes"
            action={<Refresh onClick={r.reload} />}
          />
          <div className="table-scroll">
            <table className="data-table">
              <thead>
                <tr>
                  {["Destination", "Interface", "Source"].map((s) => (
                    <th key={s}>{t(s)}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {(r.data?.routes || []).map((row: ObjectData, i: number) => (
                  <tr key={i}>
                    <td>{row.destination}</td>
                    <td>{row.interface}</td>
                    <td>{row.source || "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <Note>
            {t(
              "A route does not guarantee that a destination service is reachable.",
            )}
          </Note>
        </Panel>
      )}
    </>
  );
}
function Mihomo({
  tab,
  data: d,
  reload,
}: {
  tab: string;
  data?: ObjectData;
  reload: () => void;
}) {
  if (tab === "configuration") return <ProxyEditor data={d} reload={reload} />;
  if (tab === "groups") return <ProxyGroups />;
  if (tab === "logs") return <ProxyLogs />;
  if (tab === "panel") return <ProxyPanel data={d} />;
  return (
    <>
      <Stats
        items={[
          ["Mode", d?.mode],
          ["Traffic scope", d?.capture],
          ["Local HTTP / SOCKS", d?.local_endpoint],
          [
            "Configuration",
            d ? t(d.configured ? "Saved" : "Not imported") : "—",
          ],
        ]}
      />
      <SettingsForm
        onApplied={reload}
        spec={{
          id: "proxy-runtime",
          title: "Runtime settings",
          read: "/api/proxy/mihomo/status",
          write: "/api/proxy/mihomo/settings",
          fields: [
            options("mode", "Mode", [
              ["rule", "Rule"],
              ["global", "Global"],
              ["direct", "Direct"],
            ]),
            options("capture", "Traffic scope", [
              ["local", "Local HTTP / SOCKS clients"],
              ["hotspot", "Wi-Fi hotspot traffic"],
            ]),
            field("mixed_port", "Mixed HTTP / SOCKS port", {
              type: "number",
              wire: "number",
              min: 1024,
              max: 65535,
            }),
            flag("ipv6", "IPv6", "boolean"),
            flag("tcp_concurrent", "Concurrent TCP connections", "boolean"),
          ],
          interrupt: (c, b) =>
            b.running && Object.keys(c).some((k) => k !== "mode")
              ? "This change needs to stop and restart this service. Its current connections will disconnect."
              : undefined,
          body: (c, _, b) => ({
            ...c,
            ...(b.running && Object.keys(c).some((k) => k !== "mode")
              ? { restart: true }
              : {}),
          }),
          timeout: 150000,
        }}
      />
      {d && !d.configured && (
        <Note>
          {t("Import a configuration before starting the service.")}{" "}
          <button
            className="text-link"
            onClick={() => go("services", "mihomo", "configuration")}
          >
            {t("Configuration")}
          </button>
        </Note>
      )}
    </>
  );
}
function ProxyEditor({
  data,
  reload,
}: {
  data?: ObjectData;
  reload: () => void;
}) {
  const r = useResource("/api/proxy/mihomo/config"),
    mutation = useMutation();
  const [yaml, setYaml] = useState(""),
    [revision, setRevision] = useState<string | null>(),
    [dirty, setDirty] = useState(false),
    [url, setURL] = useState(""),
    [error, setError] = useState<Error>(),
    [message, setMessage] = useState("");
  useEffect(() => {
    if (r.data && !dirty) {
      setYaml(r.data.config || "");
      setRevision(r.data.revision ?? null);
    }
  }, [r.data]);
  const apply = async (action: string) => {
    try {
      await mutation.run(async () => {
        setError(undefined);
        if (revision === undefined)
          throw new Error("Load the current configuration first");
        if (new TextEncoder().encode(yaml).length > 512 * 1024)
          throw new Error("Configuration exceeds 512 KiB.");
        const body =
          action === "subscription"
            ? { url, revision }
            : { config: yaml, revision };
        const result = await api("/api/proxy/mihomo/" + action, {
          method: "POST",
          body,
          timeout: 600000,
        });
        if (action === "validate" || action === "geodata") {
          if (result[action === "validate" ? "valid" : "prepared"] !== true)
            throw new Error(
              "The router did not confirm configuration validation.",
            );
          setMessage(
            action === "validate"
              ? "Configuration is valid."
              : "Rule databases are ready.",
          );
          return;
        }
        const saved = await api("/api/proxy/mihomo/config");
        if (!saved.configured || saved.revision !== result.config_revision)
          throw new Error(
            "The saved configuration could not be confirmed. Your draft is retained.",
          );
        setYaml(saved.config);
        setRevision(saved.revision);
        setDirty(false);
        setURL("");
        setMessage(
          result.running
            ? "Configuration saved and reloaded."
            : "Configuration saved. You can now start the service.",
        );
        reload();
      });
    } catch (e) {
      setError(e as Error);
    }
  };
  return (
    <Panel>
      <PanelTitle
        title="Current configuration"
        note="YAML / JSON"
        action={
          <span className="file-label">
            {t(dirty ? "Editing" : data?.configured ? "Saved" : "Not imported")}
          </span>
        }
      />
      <ErrorState error={error || r.error} />
      <div className="editor-toolbar">
        <Refresh
          onClick={r.reload}
          disabled={r.loading || dirty || mutation.busy}
        />
        <label className="btn file-button">
          {t("Import file")}
          <input
            type="file"
            accept=".yml,.yaml,.json"
            disabled={mutation.busy}
            onChange={async (e) => {
              const f = e.target.files?.[0];
              if (f) {
                if (f.size > 512 * 1024) {
                  setError(new Error("Configuration exceeds 512 KiB."));
                  return;
                }
                setYaml(await f.text());
                setDirty(true);
              }
            }}
          />
        </label>
      </div>
      <textarea
        className="yaml-editor"
        aria-label="YAML / JSON"
        autoComplete="off"
        spellCheck={false}
        value={yaml}
        disabled={mutation.busy || revision === undefined}
        onChange={(e) => {
          setYaml(e.target.value);
          setDirty(true);
          setMessage("");
        }}
      />
      <div className="editor-actions">
        <span role="status">
          {t(
            message ||
              "Complete the configuration, then finish editing to validate, save and reload.",
          )}
        </span>
        <button
          className="btn"
          disabled={!yaml.trim() || mutation.busy || !data?.installed}
          onClick={() => void apply("validate")}
        >
          {t("Validate")}
        </button>
        <button
          className="btn primary"
          disabled={!dirty || !yaml.trim() || mutation.busy || !data?.installed}
          onClick={() => void apply("config")}
        >
          {t("Finish editing")}
        </button>
      </div>
      <details className="advanced-details">
        <summary>{t("Import a subscription")}</summary>
        <Field label="Subscription / configuration URL">
          <input
            type="url"
            value={url}
            onChange={(e) => setURL(e.target.value)}
            autoComplete="off"
          />
        </Field>
        <button
          className="btn"
          disabled={
            !/^https:\/\//.test(url) || mutation.busy || !data?.installed
          }
          onClick={() => void apply("subscription")}
        >
          {t("Import and apply")}
        </button>
      </details>
      <details className="advanced-details">
        <summary>{t("Rule databases")}</summary>
        <Note>
          {t(
            "Missing databases are prepared during validation. Stop the service before forcing a database repair.",
          )}
        </Note>
        <button
          className="btn"
          disabled={data?.running || !yaml.trim() || mutation.busy}
          onClick={() => void apply("geodata")}
        >
          {t("Prepare / repair rule databases")}
        </button>
      </details>
    </Panel>
  );
}
function ProxyGroups() {
  const r = useResource<ObjectData[]>("/api/proxy/mihomo/groups"),
    mutation = useMutation(),
    [error, setError] = useState<Error>();
  return (
    <>
      <ErrorState error={error || r.error} retry={r.reload} />
      <div className="proxy-groups">
        {(r.data || []).map((g: ObjectData) => (
          <Panel key={g.name}>
            <PanelTitle title={g.name} note={g.kind} />
            <div className="proxy-options">
              {(g.choices || []).map((name: string) => (
                <button
                  key={name}
                  disabled={mutation.busy || !g.selectable}
                  aria-pressed={g.selected === name}
                  onClick={async () => {
                    try {
                      await mutation.run(async () => {
                        await api("/api/proxy/mihomo/select", {
                          method: "POST",
                          body: { group: g.name, choice: name },
                        });
                        r.reload();
                      });
                    } catch (e) {
                      setError(e as Error);
                    }
                  }}
                >
                  {name}
                </button>
              ))}
            </div>
          </Panel>
        ))}
      </div>
      {!r.loading && !r.data?.length && (
        <Empty>{t("No proxy groups reported")}</Empty>
      )}
    </>
  );
}
function ProxyLogs() {
  const r = useResource("/api/proxy/mihomo/logs"),
    [query, setQuery] = useState("");
  return (
    <Panel>
      <PanelTitle
        title="Logs"
        action={<Refresh onClick={r.reload} disabled={r.loading} />}
      />
      <ErrorState error={r.error} />
      <input
        aria-label={t("Filter logs")}
        placeholder={t("Filter logs")}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      <pre className="log-view">
        {String(r.data?.text || r.data?.log || "")
          .split("\n")
          .filter((l) => l.toLowerCase().includes(query.toLowerCase()))
          .join("\n") || t("No log entries")}
      </pre>
    </Panel>
  );
}
function ProxyPanel({ data }: { data?: ObjectData }) {
  const r = useResource("/api/proxy/mihomo/panel"),
    mutation = useMutation(),
    [error, setError] = useState<Error>();
  return (
    <>
      <Panel>
        <PanelTitle
          title="MetaCubeXD"
          note="Connections, rules and proxy groups."
        />
        <ErrorState error={error || r.error} />
        <button
          className="btn primary"
          disabled={!r.data?.installed || !data?.running || mutation.busy}
          onClick={async () => {
            const tab = window.open("about:blank", "_blank");
            if (tab) {
              tab.opener = null;
              try {
                tab.sessionStorage.removeItem("openu60.session");
              } catch {}
            }
            try {
              const result = await api("/api/proxy/mihomo/panel-session", {
                method: "POST",
                body: {},
              });
              if (
                result.path !== "/proxy-panel/mihomo/" ||
                result.endpoint !== "/proxy-panel/mihomo/api" ||
                !/^[a-f0-9]{64}$/.test(result.token || "")
              )
                throw new Error("Invalid dashboard connection details.");
              const url = new URL(result.path, location.origin),
                params = new URLSearchParams({
                  hostname: location.host + result.endpoint,
                  secret: result.token,
                  [location.protocol === "https:" ? "https" : "http"]: "true",
                });
              url.hash = "/setup?" + params.toString();
              if (tab) tab.location.replace(url.href);
              else throw new Error("Allow popups to open the dashboard");
            } catch (e) {
              tab?.close();
              setError(e as Error);
            }
          }}
        >
          {t("Open dashboard")} <ArrowUpRight size={15} />
        </button>
      </Panel>
      <Installer
        service="metacubexd"
        installed={r.data}
        onInstalled={r.reload}
      />
    </>
  );
}
function WireGuard({
  tab,
  data: d,
  reload,
}: {
  tab: string;
  data?: ObjectData;
  reload: () => void;
}) {
  const [config, setConfig] = useState(""),
    [error, setError] = useState<Error>();
  const mutation = useMutation();
  if (tab === "configuration")
    return (
      <Panel>
        <PanelTitle
          title="Tunnel configuration"
          note="Import a WireGuard .conf file. Private keys are never returned by the router."
        />
        <Field label="Configuration file">
          <input
            type="file"
            accept=".conf"
            disabled={mutation.busy}
            onChange={async (e) => {
              const file = e.target.files?.[0];
              if (file && file.size <= 65536) setConfig(await file.text());
            }}
          />
        </Field>
        <textarea
          className="yaml-editor"
          aria-label="WireGuard configuration"
          value={config}
          autoComplete="off"
          spellCheck={false}
          onChange={(e) => setConfig(e.target.value)}
        />
        <ErrorState error={error} />
        <button
          className="btn primary"
          disabled={!config.trim() || mutation.busy}
          onClick={async () => {
            try {
              if (
                d?.enabled &&
                !(await mutation.confirm({
                  message:
                    "Apply this configuration and reconnect the WireGuard tunnel?",
                }))
              )
                return;
              await mutation.run(async () => {
                await api("/api/vpn/wireguard", {
                  method: "PUT",
                  body: { config, enabled: !!d?.enabled },
                  timeout: 150000,
                });
                setConfig("");
                reload();
              });
            } catch (e) {
              setError(e as Error);
            }
          }}
        >
          {t("Finish editing")}
        </button>
      </Panel>
    );
  return (
    <>
      <Stats
        items={[
          ["Interface", d?.interface],
          ["Addresses", (d?.addresses || []).join(" · ")],
          ["Peers", d?.peers?.length],
          ["MTU", d?.mtu],
        ]}
      />
      <Panel>
        <PanelTitle title="Peers & handshakes" />
        <div className="table-scroll">
          <table className="data-table">
            <thead>
              <tr>
                {[
                  "Peer",
                  "Endpoint",
                  "Routes",
                  "Latest handshake",
                  "Received",
                  "Sent",
                ].map((n) => (
                  <th key={n}>{t(n)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {(d?.peers || []).map((p: ObjectData) => (
                <tr key={p.public_key}>
                  <td>{p.name || t("Peer")}</td>
                  <td>{p.endpoint || "—"}</td>
                  <td>{(p.allowed_ips || []).join(" · ")}</td>
                  <td>{wireguardModel.handshake(p.last_handshake_at)}</td>
                  <td>{bytes(p.rx_bytes)}</td>
                  <td>{bytes(p.tx_bytes)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Panel>
    </>
  );
}
