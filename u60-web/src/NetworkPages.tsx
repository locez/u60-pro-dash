import {
  FlightModeControl,
  NetworkOptimization,
  ModeOverview,
  useOperating,
} from "./Operating";
import { useEffect, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  ArrowUpRight,
  Globe,
  Router,
  Smartphone,
  Wifi,
} from "lucide-react";
import { api, type ObjectData } from "./api";
import { useLive, useReading, useResource } from "./live";
import { carrier, t } from "./i18n";
import {
  bytes,
  Chart,
  Definition,
  Empty,
  ErrorState,
  Field,
  Modal,
  Note,
  number,
  Panel,
  PanelTitle,
  QR,
  Refresh,
  scalar,
  Stats,
  Status,
} from "./ui";
import { SettingsForm } from "./forms";
import { cellular, clientsModel, usageModel } from "./models";
import { guest, mobile, networkMode, wifiSpec } from "./settings";
import { go } from "./catalog";
import { useMutation } from "./mutations";
import { ServiceSummary } from "./ServicePages";
export function useClients() {
  const basic = useReading("clients"),
    detail = useResource("/api/network/client-details");
  useEffect(() => {
    if (basic?.seq) detail.reload();
  }, [basic?.seq]);
  return {
    rows: clientsModel.merge(basic?.data, detail.data) as ObjectData[],
    error: detail.error,
    reload: detail.reload,
    loading: detail.loading,
    complete: detail.data?.inventory_complete === true,
  };
}
function rate(data: ObjectData | null | undefined, direction: string) {
  const value = number(
    data?.["real_" + direction + "_speed"] ?? data?.[direction + "_speed"],
  );
  return value === null ? "—" : ((value * 8) / 1e6).toFixed(1);
}
export function Overview({ openControls }: { openControls: () => void }) {
  const operating = useOperating();
  const airplane = operating.available
    ? operating.data?.airplane?.enabled
    : null;
  const radioUnavailable = airplane !== false;
  const live = useLive(),
    signal = useReading("signal"),
    traffic = useReading("traffic"),
    health = useReading("health"),
    battery = useReading("battery"),
    charger = useReading("charger"),
    wan = useReading("wan");
  const device = useResource("/api/device"),
    usage = useResource("/api/data-usage"),
    clients = useClients();
  const model = cellular.parse(signal?.data || {});
  const data = !radioUnavailable && !traffic?.error ? traffic?.data : null;
  const budget = usageModel.parse(usage.data || {});
  return (
    <>
      {live.state === "offline" && (
        <Note warning>
          {t("Connection interrupted. Reconnecting automatically…")}
        </Note>
      )}
      <ModeOverview open={openControls} />
      <Panel>
        <PanelTitle title="Network topology" />
        <div className="connection-topology">
          <div>
            <Globe size={25} />
            <b>{carrier(signal?.data?.network_provider)}</b>
            <Status on={!radioUnavailable && model.hasSignal && !signal?.error}>
              {airplane === true
                ? t("Airplane mode")
                : radioUnavailable
                  ? t("Unknown")
                  : model.mode}
            </Status>
          </div>
          <span className="topology-link" />
          <div>
            <Router size={28} />
            <b>U60 Pro</b>
            <small>{location.host}</small>
          </div>
          <span className="topology-link" />
          <button onClick={() => go("devices")}>
            <Smartphone size={25} />
            <b>
              {clients.complete
                ? clients.rows.filter((c) => c.connected === true).length
                : "—"}{" "}
              {t("devices")}
            </b>
            <small>{t("Connected devices")}</small>
          </button>
        </div>
      </Panel>
      <div className="two-columns">
        <Panel>
          <PanelTitle title="Traffic trend" note="Latest 60 samples" />
          <div className="live-rates">
            <div>
              <span>
                <ArrowDown size={15} />
                {t("Download")} / Mbps
              </span>
              <strong>{rate(data, "rx")}</strong>
            </div>
            <div>
              <span>
                <ArrowUp size={15} />
                {t("Upload")} / Mbps
              </span>
              <strong>{rate(data, "tx")}</strong>
            </div>
          </div>
          <Chart
            series={[
              {
                name: "Download",
                color: "var(--accent)",
                values: live.history.map((d) =>
                  number(d.real_rx_speed ?? d.rx_speed),
                ),
              },
              {
                name: "Upload",
                color: "var(--upload)",
                values: live.history.map((d) =>
                  number(d.real_tx_speed ?? d.tx_speed),
                ),
              },
            ]}
          />
        </Panel>
        <div className="stack">
          <Panel>
            <PanelTitle
              title="Data usage"
              action={
                <button
                  className="text-link"
                  onClick={() => go("internet", "usage")}
                >
                  <ArrowUpRight size={16} />
                </button>
              }
            />
            <Stats
              items={[
                ["This month", bytes(budget.month.totalBytes)],
                ["Remaining", bytes(budget.remainingBytes)],
              ]}
            />
            {budget.percent !== null && (
              <progress max="100" value={Math.min(100, budget.percent)} />
            )}
            <small className="subtle">
              {t("Router statistics may differ from your carrier’s bill.")}
            </small>
          </Panel>
          <Panel>
            <PanelTitle title="Battery & power" />
            <Stats
              items={[
                [
                  "Battery",
                  battery?.data?.capacity !== undefined
                    ? `${battery.data.capacity}%`
                    : "—",
                ],
                [
                  "CPU",
                  health?.data?.cpu?.overall !== undefined
                    ? `${Number(health.data.cpu.overall).toFixed(0)}%`
                    : "—",
                ],
              ]}
            />
            <Status
              on={[1, "1", true].includes(charger?.data?.charger_connect)}
            >
              {t(
                ["enable", "1", 1, true].includes(
                  charger?.data?.direct_power_supply_mode,
                )
                  ? "Direct power"
                  : battery?.data?.status === "Charging"
                    ? "Charging"
                    : charger?.data?.charger_connect === undefined
                      ? "Not reported"
                      : [1, "1", true].includes(charger.data.charger_connect)
                        ? "External power"
                        : "Battery power",
              )}
            </Status>
          </Panel>
        </div>
      </div>
      <div className="two-columns">
        <WifiShare compact />
        <Panel>
          <PanelTitle title="Connection details" />
          <Definition
            items={[
              [
                "Network",
                airplane === true
                  ? t("Airplane mode")
                  : airplane === false
                    ? model.mode
                    : t("Unknown"),
              ],
              ["Band", model.primary?.band],
              [
                "RSRP",
                model.primary?.rsrp === null
                  ? null
                  : model.primary?.rsrp !== undefined
                    ? `${model.primary.rsrp} dBm`
                    : null,
              ],
              [
                "SINR",
                model.primary?.sinr === null
                  ? null
                  : model.primary?.sinr !== undefined
                    ? `${model.primary.sinr} dB`
                    : null,
              ],
              [
                "WAN IPv4",
                wan?.data?.wan?.ipv4_address || wan?.data?.wan?.ipaddr,
              ],
              [
                "Uptime",
                device.data?.uptime_secs !== undefined
                  ? `${Math.floor(device.data.uptime_secs / 3600)}h ${Math.floor((device.data.uptime_secs % 3600) / 60)}m`
                  : null,
              ],
            ]}
          />
        </Panel>
      </div>
      <ServiceSummary />
    </>
  );
}
export function WifiShare({ compact = false }: { compact?: boolean }) {
  const r = useResource("/api/wifi/status");
  const [band, setBand] = useState("2g"),
    [show, setShow] = useState(false);
  const d = r.data;
  const unified =
    d &&
    d.ssid_2g === d.ssid_5g &&
    d.key_2g === d.key_5g &&
    d.encryption_2g === d.encryption_5g;
  const available = ["2g", "5g"].filter(
    (b) =>
      String(d?.[b === "2g" ? "radio2_disabled" : "radio5_disabled"]) === "0" &&
      String(d?.wifi_onoff) !== "0",
  );
  const selected = available.includes(band) ? band : available[0],
    ssid = d?.["ssid_" + selected];
  return (
    <Panel>
      <PanelTitle title="Share Wi-Fi" />
      <ErrorState error={r.error} retry={r.reload} />
      {!d ? (
        <Empty>{t("Loading…")}</Empty>
      ) : !ssid ? (
        <Empty>{t("No enabled wireless network")}</Empty>
      ) : (
        <>
          <div className={"wifi-join " + (compact ? "compact" : "")}>
            <QR
              ssid={ssid}
              password={d["key_" + selected] || ""}
              encryption={d["encryption_" + selected] || "none"}
              hidden={d["hidden_" + selected] === "1"}
              size={compact ? 124 : 160}
            />
            <div>
              <b>{ssid}</b>
              <small>
                {unified
                  ? t("Dual band")
                  : selected === "2g"
                    ? "2.4 GHz"
                    : "5 GHz"}
              </small>
              <div className="join-password">
                <code>{show ? d["key_" + selected] : "••••••••••"}</code>
                <button
                  className="text-link"
                  onClick={() => setShow(!show)}
                  aria-pressed={show}
                >
                  {t(show ? "Hide" : "Show")}
                </button>
              </div>
            </div>
          </div>
          {!unified && available.length > 1 && (
            <div className="segmented">
              {available.map((b) => (
                <button
                  key={b}
                  className={selected === b ? "selected" : ""}
                  onClick={() => {
                    setBand(b);
                    setShow(false);
                  }}
                >
                  {b === "2g" ? "2.4 GHz" : "5 GHz"}
                </button>
              ))}
            </div>
          )}
        </>
      )}
    </Panel>
  );
}
export function WifiPage({ tab }: { tab: string }) {
  const mutation = useMutation(),
    [error, setError] = useState<Error>(),
    [revision, setRevision] = useState(0);
  if (tab === "guest") return <SettingsForm spec={guest} />;
  return (
    <>
      <div className="two-columns">
        <SettingsForm
          key={"2g" + tab + revision}
          spec={wifiSpec("2g", tab === "radio")}
        />
        <SettingsForm
          key={"5g" + tab + revision}
          spec={wifiSpec("5g", tab === "radio")}
        />
      </div>
      {tab !== "radio" && (
        <>
          <Panel>
            <PanelTitle
              title="Dual band"
              note="Use the same network name and password on both bands."
            />
            <ErrorState error={error} />
            <button
              className="btn"
              disabled={mutation.busy}
              onClick={async () => {
                try {
                  if (
                    !(await mutation.confirm({
                      message:
                        "Both radios will use the current 2.4 GHz name, security and password. Wi-Fi connections will briefly disconnect.",
                    }))
                  )
                    return;
                  await mutation.run(async () => {
                    const d = await api("/api/wifi/status");
                    await api("/api/wifi/settings", {
                      method: "PUT",
                      body: {
                        ssid_5g: d.ssid_2g,
                        key_5g: d.key_2g,
                        encryption_5g: d.encryption_2g,
                      },
                    });
                    const fresh = await api("/api/wifi/status");
                    if (
                      fresh.ssid_5g !== d.ssid_2g ||
                      fresh.key_5g !== d.key_2g
                    )
                      throw new Error(
                        "The router has not confirmed the change.",
                      );
                    setRevision((r) => r + 1);
                  });
                } catch (e) {
                  setError(e as Error);
                }
              }}
            >
              {t("Use one name for both bands")}
            </button>
          </Panel>
          <WifiShare key={revision} />
        </>
      )}
    </>
  );
}
export function DevicesPage({ tab }: { tab: string }) {
  const inventory = useClients(),
    [query, setQuery] = useState(""),
    [selected, setSelected] = useState<ObjectData>();
  const visible = inventory.rows.filter(
    (c) =>
      (tab === "all" || c.connected === true) && clientsModel.matches(c, query),
  );
  return (
    <>
      <Stats
        items={[
          [
            "Online devices",
            inventory.complete
              ? inventory.rows.filter((c) => c.connected === true).length
              : "—",
          ],
          [
            "Wi-Fi",
            inventory.complete
              ? inventory.rows.filter(
                  (c) => c.connected === true && c.connection_type === "wifi",
                ).length
              : "—",
          ],
          [
            "USB",
            inventory.complete
              ? inventory.rows.filter(
                  (c) => c.connected === true && c.connection_type === "usb",
                ).length
              : "—",
          ],
        ]}
      />
      <Panel>
        <PanelTitle
          title="Connected devices"
          action={
            <Refresh onClick={inventory.reload} disabled={inventory.loading} />
          }
        />
        <ErrorState error={inventory.error} />
        <input
          className="list-search"
          aria-label={t("Search devices")}
          placeholder={t("Search devices")}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="table-scroll">
          <table className="data-table">
            <thead>
              <tr>
                {[
                  "Device",
                  "Connection",
                  "IP address",
                  "Download",
                  "Upload",
                  "Status",
                ].map((n) => (
                  <th key={n}>{t(n)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {visible.map((c) => (
                <tr key={c.mac} onClick={() => setSelected(c)}>
                  <td>
                    <button className="text-link">
                      {c.name || t("Unnamed device")}
                    </button>
                  </td>
                  <td>{clientsModel.connection(c)}</td>
                  <td className="mono">{c.ipv4.join(" · ") || "—"}</td>
                  <td>{clientsModel.rate(c.traffic?.download_bps)}</td>
                  <td>{clientsModel.rate(c.traffic?.upload_bps)}</td>
                  <td>
                    <Status on={c.connected === true}>
                      {t(
                        c.connected === true
                          ? "Online"
                          : c.connected === false
                            ? "Offline"
                            : "Unknown",
                      )}
                    </Status>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {!visible.length && (
          <Empty>
            {t(inventory.loading ? "Loading…" : "No devices to display")}
          </Empty>
        )}
      </Panel>
      {selected && (
        <Modal
          title={selected.name || "Device"}
          close={() => setSelected(undefined)}
        >
          <Definition
            items={[
              ["IPv4", selected.ipv4.join(" · ")],
              ["IPv6", selected.ipv6.join(" · ")],
              [
                "MAC",
                "••:••:••:••:" + selected.mac.split(":").slice(-2).join(":"),
              ],
              ["Connection", clientsModel.connection(selected)],
              ["Signal", scalar(selected.rssi)],
              ["Download", clientsModel.rate(selected.traffic?.download_bps)],
              ["Upload", clientsModel.rate(selected.traffic?.upload_bps)],
            ]}
          />
        </Modal>
      )}
    </>
  );
}
export function SignalPage() {
  const operating = useOperating();
  const airplane = operating.available
    ? operating.data?.airplane?.enabled
    : null;
  const reading = useReading("signal"),
    model = cellular.parse(airplane === false ? reading?.data || {} : {});
  return (
    <>
      <Stats
        items={[
          ["Operator", carrier(reading?.data?.network_provider)],
          [
            "Network",
            airplane === true
              ? t("Airplane mode")
              : airplane === false
                ? model.mode
                : t("Unknown"),
          ],
          ["Band", model.primary?.band || "—"],
          [
            "Signal",
            model.primary?.rsrp !== null && model.primary
              ? `${model.primary.rsrp} dBm`
              : "—",
          ],
        ]}
      />
      {reading?.error && <Note warning>{reading.error}</Note>}
      {["nr", "lte"].map((rat) => (
        <Panel key={rat}>
          <PanelTitle
            title={rat === "nr" ? "5G NR" : "4G LTE"}
            note={model.explanation}
          />
          <div className="table-scroll">
            <table className="data-table">
              <thead>
                <tr>
                  {[
                    "Carrier",
                    "Band",
                    "PCI",
                    "Channel",
                    "Bandwidth",
                    "RSRP",
                    "SINR",
                    "RSRQ",
                  ].map((n) => (
                    <th key={n}>{t(n)}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {model[rat].carriers.map((c: ObjectData, i: number) => (
                  <tr key={[c.pci, c.channel, i].join("-")}>
                    {[
                      c.role,
                      c.band,
                      c.pci,
                      c.channel,
                      c.bandwidth ? `${c.bandwidth} MHz` : null,
                      c.rsrp !== null ? `${c.rsrp} dBm` : null,
                      c.sinr !== null ? `${c.sinr} dB` : null,
                      c.rsrq !== null ? `${c.rsrq} dB` : null,
                    ].map((v, j) => (
                      <td key={j}>{v ?? "—"}</td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {!model[rat].carriers.length && (
            <Empty>{t("No serving carrier reported")}</Empty>
          )}
        </Panel>
      ))}
      <button
        className="btn"
        onClick={() => go("internet", "connection", "bands")}
      >
        {t("Band and cell preferences")} <ArrowUpRight size={15} />
      </button>
    </>
  );
}
export function ConnectionSettings() {
  return (
    <>
      <Panel>
        <FlightModeControl />
      </Panel>
      <div className="two-columns">
        <SettingsForm spec={mobile} />
        <SettingsForm spec={networkMode} />
      </div>
      <NetworkOptimization />
      <div className="entry-grid">
        {[
          ["apn", "APN profiles"],
          ["operators", "Network selection"],
          ["bands", "Band and cell preferences"],
          ["policies", "Recovery policies"],
        ].map(([id, label]) => (
          <button
            className="setting-entry"
            key={id}
            onClick={() => go("internet", "connection", id)}
          >
            <span>
              <b>{t(label)}</b>
            </span>
            <ArrowUpRight size={16} />
          </button>
        ))}
      </div>
    </>
  );
}
