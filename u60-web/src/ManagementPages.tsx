import { useState } from "react";
import { api, type ObjectData } from "./api";
import { type Route, go } from "./catalog";
import { useJobPolling, useReading, useResource } from "./live";
import { useMutation } from "./mutations";
import { t } from "./i18n";
import { advanced, batteryModel, dnsModel, routingModel } from "./models";
import {
  CommandForm,
  field,
  flag,
  options,
  ReadingsPanel,
  SettingsForm,
} from "./forms";
import {
  autoSleep,
  charge,
  fastBoot,
  firewall,
  lan,
  powerSave,
} from "./settings";
import {
  bytes,
  Definition,
  ErrorState,
  Field,
  Note,
  Panel,
  PanelTitle,
  Refresh,
  scalar,
  Stats,
  Toggle,
} from "./ui";
export function LanPage({ route }: { route: Route }) {
  if (route.tab === "dns") return <DNS />;
  if (route.tab === "firewall")
    return (
      <>
        <div className="two-columns">
          {firewall.map((spec) => (
            <SettingsForm
              key={spec.id}
              spec={{ ...spec, decode: routingModel.normalizeSwitches }}
            />
          ))}
        </div>
        <Ports />
        <div className="two-columns">
          <SettingsForm
            spec={{
              id: "qos",
              title: "Traffic prioritization",
              read: "/api/router/qos",
              decode: routingModel.normalizeSwitches,
              fields: [flag("qos_switch", "Quality of service")],
            }}
          />
          <SettingsForm
            spec={{
              id: "ttl",
              title: "Outbound packet lifetime",
              read: "/api/router/ttl",
              fields: [
                flag("enabled", "TTL rewriting", "boolean"),
                field("ipv4_ttl", "IPv4 TTL", {
                  type: "number",
                  wire: "number",
                  min: 1,
                  max: 255,
                }),
                field("ipv6_hop_limit", "IPv6 hop limit", {
                  type: "number",
                  wire: "number",
                  min: 1,
                  max: 255,
                }),
              ],
            }}
          />
        </div>
      </>
    );
  return <SettingsForm spec={lan} />;
}
function DNS() {
  const r = useResource("/api/dns/status"),
    cache = useResource("/api/dns/cache");
  return (
    <>
      <Panel>
        <PanelTitle title="Resolver path" />
        <Definition
          items={[
            ["Mode", r.data?.mode],
            ["Resolver", dnsModel.endpoint(r.data)],
            ["Cached answers", r.data?.stats?.cache_entries],
          ]}
        />
        <ErrorState error={r.error} />
      </Panel>
      <SettingsForm
        onApplied={r.reload}
        spec={{
          id: "dns",
          title: "DNS settings",
          note: "Every change tests the candidate resolver first. The previous settings are restored if verification fails.",
          read: "/api/dns/status",
          write: "/api/dns/apply",
          fields: [
            options("mode", "Mode", [
              ["automatic", "Carrier automatic"],
              ["manual", "Manual DNS"],
              ["doh", "DNS over HTTPS"],
              ["dot", "DNS over TLS"],
            ]),
            field("primary", "Primary DNS"),
            field("secondary", "Secondary DNS"),
            field("doh_url", "DoH endpoint"),
            field("dot_endpoint", "DoT server"),
            field("dot_server_name", "DoT certificate name"),
          ],
          body: (_, v) => dnsModel.payload(v),
          timeout: 40000,
        }}
      />
      <Panel>
        <PanelTitle
          title="Local DNS cache"
          action={<Refresh onClick={cache.reload} />}
        />
        <ErrorState error={cache.error} />
        <div className="table-scroll">
          <table className="data-table">
            <thead>
              <tr>
                {["Domain", "Type", "Expires in"].map((s) => (
                  <th key={s}>{t(s)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {dnsModel
                .cacheEntries(cache.data?.entries)
                .map((row: ObjectData) => (
                  <tr key={row.key}>
                    <td>{row.domain}</td>
                    <td>{row.type}</td>
                    <td>{row.ttlSeconds} s</td>
                  </tr>
                ))}
            </tbody>
          </table>
        </div>
      </Panel>
      <CommandForm
        title="Clear DNS cache"
        endpoint="/api/dns/cache/clear"
        label="Clear cache"
        body={() => ({})}
        after={cache.reload}
      />
    </>
  );
}
function Ports() {
  const r = useResource("/api/router/firewall/port-forward"),
    lanState = useResource("/api/router/lan"),
    mutation = useMutation(),
    [error, setError] = useState<Error>();
  const rows = r.data
    ? (() => {
        try {
          return routingModel.rules(r.data);
        } catch {
          return [];
        }
      })()
    : [];
  return (
    <>
      <SettingsForm
        spec={{
          id: "ports",
          title: "Port forwarding",
          read: "/api/router/firewall",
          write: "/api/router/firewall/port-forward/switch",
          decode: routingModel.normalizeSwitches,
          fields: [flag("port_forward_switch", "Port forwarding")],
        }}
      />
      <Panel>
        <PanelTitle title="Forwarding rules" />
        <ErrorState error={error || r.error} />
        <div className="table-scroll">
          <table className="data-table">
            <thead>
              <tr>
                {[
                  "Name",
                  "Protocol",
                  "External port",
                  "Destination",
                  "Actions",
                ].map((s) => (
                  <th key={s}>{t(s)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row: ObjectData) => (
                <tr key={row.id}>
                  <td>{row.name}</td>
                  <td>{row.protocol}</td>
                  <td>{row.wan_port}</td>
                  <td>
                    {row.lan_ip}:{row.lan_port}
                  </td>
                  <td>
                    <button
                      className="text-link"
                      disabled={!row.removable || mutation.busy}
                      onClick={async () => {
                        try {
                          if (
                            !(await mutation.confirm({
                              message: "Delete this forwarding rule?",
                              danger: true,
                            }))
                          )
                            return;
                          await mutation.run(async () => {
                            await api("/api/router/firewall/port-forward", {
                              method: "POST",
                              body: {
                                action: "delete",
                                id: row.id,
                                expected_rules: r.data?.rule_list,
                              },
                            });
                            r.reload();
                          });
                        } catch (e) {
                          setError(e as Error);
                        }
                      }}
                    >
                      {t("Delete")}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Panel>
      <CommandForm
        title="Add forwarding rule"
        endpoint="/api/router/firewall/port-forward"
        fields={[
          field("name", "Name", { required: true }),
          options("protocol", "Protocol", [
            ["tcp", "TCP"],
            ["udp", "UDP"],
            ["tcp+udp", "TCP + UDP"],
          ]),
          field("wan_port", "External port", {
            type: "number",
            min: 1,
            max: 65535,
            required: true,
          }),
          field("lan_ip", "Destination IPv4", { required: true }),
          field("lan_port", "Internal port", {
            type: "number",
            min: 1,
            max: 65535,
            required: true,
          }),
          flag("enabled", "Enabled"),
        ]}
        body={(v) =>
          routingModel.portBody(
            v,
            routingModel.normalizeLAN(lanState.data),
            rows,
          )
        }
        label="Add rule"
        after={r.reload}
      />
    </>
  );
}
export function SystemPage({ route }: { route: Route }) {
  if (route.tab === "screen") return <ScreenSettings />;
  if (route.tab === "diagnostics") return <Diagnostics detail={route.detail} />;
  if (route.tab === "automation") return <Automation />;
  if (route.tab === "update") return <Updates />;
  return <Power />;
}
function Power() {
  const details = useResource("/api/device/battery-details");
  const control = useResource("/api/device/charge-control");
  useJobPolling(details, true, 5000);
  const b = batteryModel.parse(details.data || {});
  return (
    <>
      <Panel>
        <PanelTitle title="Battery & power" />
        <ErrorState error={details.error} />
        <Stats
          items={[
            ["Battery", b.capacity === null ? "—" : b.capacity + "%"],
            [
              "Temperature",
              b.temperature === null ? "—" : b.temperature + " °C",
            ],
            [
              "Battery power",
              b.power === null ? "—" : b.power.toFixed(2) + " W",
            ],
            [
              "USB input",
              b.usbPower === null ? "—" : b.usbPower.toFixed(2) + " W",
            ],
          ]}
        />
        <Definition
          items={[
            ["State", b.state],
            ["External power", scalar(b.usbOnline)],
            ["Direct power supply", scalar(b.bypass)],
            ["Time remaining", batteryModel.duration(b.empty.seconds)],
            ["USB supply type", b.usbType],
            ["Charge cycles", b.cycles],
          ]}
        />
      </Panel>
      <SettingsForm
        spec={{
          ...charge,
          fields: charge.fields.map((f) =>
            f.key === "charging_stopped"
              ? options(
                  "charging_stopped",
                  "Power supply mode",
                  [
                    ["false", "Charge battery"],
                    ["true", "Direct power supply"],
                  ],
                  { wire: "boolean" },
                )
              : f,
          ),
        }}
        onApplied={control.reload}
      />
      <div className="two-columns">
        <SettingsForm spec={powerSave} />
        <SettingsForm spec={fastBoot} />
      </div>
      <SettingsForm spec={autoSleep} />
      <SettingsForm
        spec={{
          id: "usb",
          title: "USB mode",
          read: "/api/usb/status",
          write: "/api/usb/mode",
          fields: [
            options("mode", "USB mode", [
              ["none", "Disable USB data"],
              ["rndis", "RNDIS network"],
              ["ecm", "ECM network"],
              ["mtp", "MTP file transfer"],
              ["debug", "ADB debugging"],
            ]),
          ],
          interrupt:
            "Changing USB mode can disconnect USB networking and ADB. Debug mode gives the connected computer root access.",
        }}
      />
      <SettingsForm
        spec={{
          id: "powerbank",
          title: "USB power bank",
          read: "/api/device/charger",
          write: "/api/usb/powerbank",
          fields: [
            options("otg_powerbank_state", "Reverse charging", [
              ["0", "Disabled"],
              ["1", "Enabled"],
            ]),
          ],
          body: (c) => ({ state: Number(c.otg_powerbank_state) }),
        }}
      />
    </>
  );
}
function ScreenSettings() {
  return (
    <>
      <SettingsForm
        spec={{
          id: "screen",
          title: "Screen appearance",
          read: "/api/screen/preferences",
          fields: [
            options("language", "Language", [
              ["zh-Hans", "简体中文"],
              ["zh-Hant", "繁體中文"],
              ["en", "English"],
              ["ja", "日本語"],
              ["ko", "한국어"],
            ]),
            options(
              "dark",
              "Appearance",
              [
                ["false", "Light"],
                ["true", "Dark"],
              ],
              { wire: "boolean" },
            ),
            field("accent", "Accent color"),
            field("brightness", "Brightness (%)", {
              type: "number",
              wire: "number",
              min: 5,
              max: 100,
            }),
            field("sleep_seconds", "Screen timeout (seconds)", {
              type: "number",
              wire: "number",
              min: 15,
              max: 300,
            }),
          ],
        }}
      />
      <ScreenLayout />
      <HTTPS />
    </>
  );
}
function ScreenLayout() {
  return (
    <SettingsForm
      spec={{
        id: "screen-layout",
        title: "Home layout",
        read: "/api/screen/preferences",
        decode: (d) => ({ ...d, ...d.home }),
        fields: [
          flag("show_wifi", "Share Wi-Fi", "boolean"),
          flag("show_radio", "Radio details", "boolean"),
          flag("show_services", "Service status", "boolean"),
          flag("show_tailscale", "Tailscale", "boolean"),
          flag("show_wireguard", "WireGuard", "boolean"),
          flag("show_mihomo", "Mihomo", "boolean"),
          ...[1, 2, 3].map((i) =>
            options("metric_" + i, t("Metric {0}", i), [
              ["clients", "Online devices"],
              ["month", "Monthly data"],
              ["temperature", "Temperature"],
              ["cpu", "CPU"],
              ["memory", "Memory"],
              ["uptime", "Uptime"],
              ["battery", "Battery"],
            ]),
          ),
        ],
        body: (c) => ({ home: c }),
      }}
    />
  );
}
function HTTPS() {
  const r = useResource("/api/https");
  return (
    <>
      <Panel>
        <PanelTitle title="Management HTTPS" />
        <ErrorState error={r.error} />
        <Definition
          items={[
            ["State", scalar(r.data?.state || r.data?.enabled)],
            ["Address", scalar(r.data?.url || r.data?.hostname)],
            [
              "Certificate",
              scalar(r.data?.certificate?.expires_at || r.data?.expires_at),
            ],
          ]}
        />
        <a className="text-link" href="/ui/https/u60-pro-ca.crt">
          {t("Download local CA certificate")}
        </a>
      </Panel>
      <CommandForm
        title="Enable HTTPS"
        endpoint="/api/https/on"
        body={() => ({})}
        label="Enable HTTPS"
        after={r.reload}
      />
    </>
  );
}
function Updates() {
  const r = useResource("/api/update/status");
  return (
    <>
      <Panel>
        <PanelTitle
          title="Installed software"
          action={<Refresh onClick={r.reload} />}
        />
        <ErrorState error={r.error} />
        <Definition
          items={[
            ["Version", r.data?.current?.version],
            ["Commit", r.data?.current?.commit],
            ["Deployment", r.data?.deployment_mode],
            ["State", r.data?.phase],
          ]}
        />
        {r.data?.deployment_mode === "paired" && (
          <Note>
            {t(
              "This installation updates the agent and screen together. Use the project deployment bundle to retain automatic recovery.",
            )}
          </Note>
        )}
      </Panel>
      <ReadingsPanel
        title="Device information"
        path="/api/device"
        rows={[
          ["hostname", "Device name"],
          ["kernel", "Kernel"],
          ["uptime_secs", "Uptime (seconds)"],
        ]}
      />
      <div className="two-columns">
        <CommandForm
          title="Restart router"
          endpoint="/api/device/reboot"
          label="Restart router"
          body={() => ({})}
          warning="Internet, Wi-Fi and this console will disconnect while the router starts up."
        />
        <CommandForm
          title="Factory reset"
          endpoint="/api/device/factory-reset"
          label="Erase and reset"
          body={() => ({})}
          warning="Permanently erase router settings and Wi-Fi credentials. Set up the device again after it restarts."
          danger
          phrase="RESET"
        />
      </div>
    </>
  );
}
function Diagnostics({ detail }: { detail?: string }) {
  const [result, setResult] = useState<ObjectData>();
  if (detail === "processes") return <Processes />;
  if (detail === "speedtest") return <Speedtest />;
  if (detail === "terminal")
    return (
      <>
        <CommandForm
          title="AT terminal"
          endpoint="/api/at/send"
          fields={[
            field("command", "AT command", { required: true, maxLength: 512 }),
            field("timeout", "Timeout (seconds)", {
              type: "number",
              min: 1,
              max: 30,
              required: true,
            }),
          ]}
          body={(v) => advanced.atCommand(v.command, v.timeout)}
          label="Send command"
          warning="This command runs directly on the modem. Commands that write settings may interrupt service or change persistent configuration."
          after={setResult}
        />
        {result && (
          <Panel>
            <pre className="log-view">
              {scalar(result.response || result.raw || result)}
            </pre>
          </Panel>
        )}
      </>
    );
  return (
    <>
      <Resources />
      <div className="entry-grid">
        {[
          ["processes", "Current processes"],
          ["speedtest", "Internet speed test"],
          ["terminal", "AT terminal"],
        ].map(([id, label]) => (
          <button
            key={id}
            className="setting-entry"
            onClick={() => go("system", "diagnostics", id)}
          >
            {t(label)} →
          </button>
        ))}
      </div>
    </>
  );
}
function Resources() {
  const cpu = useReading("health"),
    memory = useResource("/api/memory"),
    d = memory.data;
  return (
    <Panel>
      <PanelTitle title="System resources" />
      <ErrorState error={memory.error} />
      <Stats
        items={[
          [
            "CPU",
            cpu?.data?.cpu?.overall !== undefined
              ? Number(cpu.data.cpu.overall).toFixed(1) + "%"
              : "—",
          ],
          ["Available memory", d ? bytes(d.available_kb * 1024) : "—"],
          ["Total memory", d ? bytes(d.total_kb * 1024) : "—"],
        ]}
      />
      <div className="cpu-grid">
        {(cpu?.data?.cpu?.cores || []).map((core: ObjectData, i: number) => (
          <div key={i}>
            <span>CPU {i}</span>
            <progress
              max="100"
              value={
                typeof core === "number" ? core : (core.usage ?? core.percent)
              }
            />
          </div>
        ))}
      </div>
    </Panel>
  );
}
function Processes() {
  const r = useResource("/api/system/top"),
    live = useReading("processes"),
    d = live?.data || r.data;
  return (
    <Panel>
      <PanelTitle
        title="Current processes"
        action={<Refresh onClick={r.reload} />}
      />
      <ErrorState error={r.error} />
      <div className="table-scroll">
        <table className="data-table">
          <thead>
            <tr>
              {["PID", "Process", "CPU", "Memory"].map((s) => (
                <th key={s}>{t(s)}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {(d?.processes || []).map((p: ObjectData) => (
              <tr key={p.pid}>
                <td>{p.pid}</td>
                <td>{p.name}</td>
                <td>{scalar(p.cpu_pct)}%</td>
                <td>{bytes(p.rss_kb * 1024)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Panel>
  );
}
function Speedtest() {
  const servers = useResource<ObjectData[]>("/api/speedtest/servers"),
    progress = useResource("/api/speedtest/progress");
  useJobPolling(
    progress,
    ["ping", "download", "upload", "starting"].includes(progress.data?.phase),
  );
  return (
    <>
      <CommandForm
        title="Internet speed test"
        note="This test uses mobile data and may consume a large part of your allowance."
        endpoint="/api/speedtest/start"
        fields={[
          options(
            "server_id",
            "Test server",
            (servers.data || []).map((s) => [
              String(s.id),
              `${s.sponsor} · ${s.name}`,
            ]),
          ),
        ]}
        body={(v) => {
          const id = Number(v.server_id);
          if (!servers.data?.some((s) => s.id === id))
            throw new Error("Choose a test server.");
          return { server_id: id };
        }}
        label="Start speed test"
        after={progress.reload}
      />
      <Panel>
        <PanelTitle
          title="Test progress"
          action={<Refresh onClick={progress.reload} />}
        />
        <ErrorState error={servers.error || progress.error} />
        <Definition
          items={[
            ["Phase", progress.data?.phase],
            ["Download", scalar(progress.data?.download_mbps)],
            ["Upload", scalar(progress.data?.upload_mbps)],
            ["Latency", scalar(progress.data?.ping_ms)],
          ]}
        />
      </Panel>
      <CommandForm
        title="Stop speed test"
        endpoint="/api/speedtest/stop"
        body={() => ({})}
        label="Stop"
        after={progress.reload}
      />
    </>
  );
}
function Automation() {
  const r = useResource<ObjectData[]>("/api/scheduler/jobs"),
    mutation = useMutation(),
    [error, setError] = useState<Error>();
  return (
    <>
      <Panel>
        <PanelTitle
          title="Scheduled actions"
          action={<Refresh onClick={r.reload} />}
        />
        <ErrorState error={error || r.error} />
        <div className="table-scroll">
          <table className="data-table">
            <thead>
              <tr>
                {["Name", "Schedule", "Status", "Actions"].map((s) => (
                  <th key={s}>{t(s)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {(r.data || []).map((job) => (
                <tr key={job.id}>
                  <td>{job.name}</td>
                  <td>{scalar(job.schedule)}</td>
                  <td>
                    <Toggle
                      on={job.enabled}
                      label={job.name}
                      disabled={mutation.busy}
                      onChange={async () => {
                        try {
                          await mutation.run(async () => {
                            await api("/api/scheduler/jobs/toggle", {
                              method: "PUT",
                              body: { id: job.id, enabled: !job.enabled },
                            });
                            r.reload();
                          });
                        } catch (e) {
                          setError(e as Error);
                        }
                      }}
                    />
                  </td>
                  <td>
                    <button
                      className="text-link"
                      disabled={mutation.busy}
                      onClick={async () => {
                        try {
                          if (
                            !(await mutation.confirm({
                              message: "Delete this scheduled action?",
                              danger: true,
                            }))
                          )
                            return;
                          await mutation.run(async () => {
                            await api("/api/scheduler/jobs", {
                              method: "DELETE",
                              body: { id: job.id },
                            });
                            r.reload();
                          });
                        } catch (e) {
                          setError(e as Error);
                        }
                      }}
                    >
                      {t("Delete")}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Panel>
      <CommandForm
        title="Schedule a daily restart"
        note="Uses the router’s local time. Restarting interrupts every connection."
        endpoint="/api/scheduler/jobs"
        fields={[
          field("name", "Name", { required: true }),
          field("time", "Time (HH:MM)", { required: true }),
        ]}
        body={(v) => {
          if (!/^([01]\d|2[0-3]):[0-5]\d$/.test(v.time))
            throw new Error("Enter a time as HH:MM.");
          return {
            name: v.name,
            enabled: false,
            schedule: {
              type: "recurring",
              time: v.time,
              days: [0, 1, 2, 3, 4, 5, 6],
            },
            action: { method: "POST", path: "/api/device/reboot", body: {} },
          };
        }}
        label="Create disabled schedule"
        after={r.reload}
      />
    </>
  );
}
