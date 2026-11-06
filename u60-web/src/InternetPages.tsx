import { useEffect, useRef, useState } from "react";
import { RadioTools, SimToolkit } from "./AdvancedPages";
import { api, type ObjectData } from "./api";
import { type Route, go } from "./catalog";
import { useJobPolling, useResource } from "./live";
import { carrier, t } from "./i18n";
import { ConnectionSettings, SignalPage } from "./NetworkPages";
import {
  CommandForm,
  field,
  flag,
  options,
  ReadingsPanel,
  SettingsForm,
} from "./forms";
import {
  bytes,
  Definition,
  Disclosure,
  Empty,
  ErrorState,
  Field,
  Note,
  Panel,
  PanelTitle,
  Refresh,
  scalar,
  Stats,
} from "./ui";
import { actions, advanced, usageModel } from "./models";
import { useMutation } from "./mutations";
import { SimPage } from "./SimPage";
const radioWarning =
  "Changing radio settings may interrupt mobile data. Keep a local connection to the router.";
export function InternetPage({ route }: { route: Route }) {
  if (route.tab === "signal") return <SignalPage />;
  if (route.tab === "sim") return <SimPage />;
  if (route.tab === "usage") return <Usage />;
  if (route.tab === "carrier") return <CarrierServices />;
  return route.detail === "apn" ? (
    <APN />
  ) : route.detail === "operators" ? (
    <Operators />
  ) : route.detail === "bands" ? (
    <Bands />
  ) : route.detail === "policies" ? (
    <Policies />
  ) : (
    <ConnectionSettings />
  );
}
function Usage() {
  const r = useResource("/api/data-usage"),
    d = usageModel.parse(r.data);
  return (
    <>
      <Panel>
        <PanelTitle
          title="Data usage"
          action={<Refresh onClick={r.reload} />}
        />
        <ErrorState error={r.error} />
        <Stats
          items={[
            ["Today", bytes(d.day.totalBytes)],
            ["This month", bytes(d.month.totalBytes)],
            ["All time", bytes(d.total.totalBytes)],
            ["Remaining", bytes(d.remainingBytes)],
          ]}
        />
        <p className="subtle">
          {t("Router statistics may differ from your carrier’s bill.")}
        </p>
      </Panel>
      <SettingsForm
        onApplied={r.reload}
        spec={{
          id: "allowance",
          title: "Monthly allowance",
          read: "/api/data-usage",
          write: "/api/data-usage/limit",
          decode: (d) => ({
            ...d.limit,
            gb: usageModel.gigabytes(d.limit?.bytes),
          }),
          fields: [
            flag("enabled", "Monthly allowance", "boolean"),
            field("gb", "Allowance (GB)"),
            field("warning_percent", "Warning at (%)", {
              type: "number",
              wire: "number",
              min: 1,
              max: 100,
            }),
            flag("auto_disconnect", "Disconnect at limit", "boolean"),
          ],
          body: (_, v) => {
            const bytes = usageModel.bytesFromGigabytes(v.gb);
            if (bytes === null)
              throw new Error(
                "Enter a positive allowance in GB, with at most three decimal places.",
              );
            return {
              enabled: v.enabled,
              bytes,
              warning_percent: Number(v.warning_percent),
              auto_disconnect: v.auto_disconnect,
            };
          },
          interrupt: (c) =>
            c.auto_disconnect === true
              ? "At the data allowance, the router will disconnect mobile internet."
              : undefined,
        }}
      />
      <SettingsForm
        onApplied={r.reload}
        spec={{
          id: "cycle",
          title: "Billing cycle",
          read: "/api/data-usage",
          write: "/api/data-usage/cycle",
          decode: (d) => d.cycle,
          fields: [
            flag("enabled", "Monthly reset", "boolean"),
            field("reset_day", "Reset day", {
              type: "number",
              wire: "number",
              min: 1,
              max: 31,
            }),
          ],
          body: (_, v) => ({
            enabled: v.enabled,
            reset_day: Number(v.reset_day),
          }),
        }}
      />
    </>
  );
}
function Policies() {
  return (
    <div className="two-columns">
      <SettingsForm
        spec={{
          id: "oos",
          title: "No-service recovery",
          read: "/api/modem/oos-policy",
          fields: [
            flag("enabled", "No-service recovery", "boolean"),
            field("grace_min", "Grace period (minutes)", {
              type: "number",
              wire: "number",
              min: 1,
              max: 1440,
            }),
            field("probe_every_min", "Probe interval (minutes)", {
              type: "number",
              wire: "number",
              min: 1,
              max: 1440,
            }),
            field("probe_window_min", "Probe window (minutes)", {
              type: "number",
              wire: "number",
              min: 1,
              max: 1440,
            }),
          ],
        }}
      />
      <SettingsForm
        spec={{
          id: "watchdog",
          title: "Connection watchdog",
          read: "/api/modem/conn-watchdog",
          fields: [
            flag("enabled", "Automatic recovery", "boolean"),
            flag("allow_reboot", "Allow reboot", "boolean"),
          ],
          interrupt: (c) =>
            c.allow_reboot
              ? "Allow the watchdog to restart the device when connectivity recovery fails?"
              : undefined,
        }}
      />
    </div>
  );
}
function Bands() {
  return (
    <>
      <Note>{t(radioWarning)}</Note>
      <div className="two-columns">
        <CommandForm
          title="LTE bands"
          endpoint="/api/cell/band/lte"
          fields={[
            field("bands", "LTE bands", {
              required: true,
              hint: "Comma-separated band numbers.",
            }),
          ]}
          body={(v) => ({
            is_lte_band: "1",
            lte_band_mask: actions.bands(v.bands, 88),
            is_gw_band: "0",
            gw_band_mask: "",
          })}
          warning={radioWarning}
          label="Apply bands"
        />
        <CommandForm
          title="5G NR bands"
          endpoint="/api/cell/band/nr"
          fields={[
            options("mode", "5G mode", [
              ["sa", "Standalone"],
              ["nsa", "NSA"],
            ]),
            field("bands", "NR bands", { required: true }),
          ]}
          body={(v) => ({
            nr5g_type: v.mode,
            nr5g_band: actions.bands(v.bands, 1024),
          })}
          warning={radioWarning}
          label="Apply bands"
        />
      </div>
      <div className="two-columns">
        {["lte", "nr"].map((rat) => (
          <CommandForm
            key={rat}
            title={rat === "lte" ? "LTE cell lock" : "NR cell lock"}
            endpoint={"/api/cell/lock/" + rat}
            fields={[
              field("pci", "Physical cell ID", {
                type: "number",
                min: 0,
                max: rat === "lte" ? 503 : 1007,
                required: true,
              }),
              field("earfcn", rat === "lte" ? "EARFCN" : "NR-ARFCN", {
                type: "number",
                min: 0,
                max: rat === "lte" ? 262143 : 3279165,
                required: true,
              }),
              ...(rat === "nr"
                ? [
                    field("band", "NR band", {
                      type: "number",
                      min: 1,
                      max: 1024,
                    }),
                  ]
                : []),
            ]}
            body={(v) =>
              Object.fromEntries(Object.entries(v).filter(([, v]) => v !== ""))
            }
            warning={radioWarning}
            label="Lock cell"
          />
        ))}
      </div>
      <div className="two-columns">
        <CommandForm
          title="Reset band restrictions"
          endpoint="/api/cell/band/reset"
          label="Allow all bands"
          warning={radioWarning}
          body={() => ({})}
        />
        <CommandForm
          title="Reset cell locks"
          endpoint="/api/cell/lock/reset"
          label="Unlock cells"
          warning={radioWarning}
          body={() => ({})}
        />
      </div>
      <Disclosure title="Advanced radio tools">
        <RadioTools />
      </Disclosure>
    </>
  );
}
function Operators() {
  const r = useResource("/api/modem/scan/results"),
    phase = useResource("/api/modem/scan/status"),
    registration = useResource("/api/modem/register/result"),
    [pending, setPending] = useState(false),
    [registering, setRegistering] = useState(false),
    [scanStatus, setScanStatus] = useState("idle"),
    [registerStatus, setRegisterStatus] = useState("idle"),
    [error, setError] = useState<Error>(),
    mutation = useMutation();
  useJobPolling(phase, pending, 2500);
  useJobPolling(registration, registering, 2500);
  const scanJob = useRef(new advanced.Job("scan", 120000)),
    registerJob = useRef(new advanced.Job("registration", 60000));
  useEffect(() => {
    if (!pending || phase.loading) return;
    const state = scanJob.current.observe(phase.data?.m_netselect_status);
    setScanStatus(state);
    if (!scanJob.current.pending) {
      setPending(false);
      r.reload();
    }
  }, [phase.data, phase.loading, pending]);
  useEffect(() => {
    if (!registering || registration.loading) return;
    const state = registerJob.current.observe(
      registration.data?.m_netselect_result,
    );
    setRegisterStatus(state);
    if (!registerJob.current.pending) {
      setRegistering(false);
      r.reload();
    }
  }, [registration.data, registration.loading, registering]);
  const rows = advanced.operators(r.data);
  return (
    <>
      <Panel>
        <PanelTitle
          title="Network selection"
          action={
            <Refresh
              onClick={() => {
                r.reload();
                phase.reload();
                registration.reload();
              }}
            />
          }
        />
        <ErrorState error={error || r.error} />
        <Definition
          items={[
            ["Scan status", t(scanStatus)],
            ["Registration", t(registerStatus)],
          ]}
        />
        <button
          className="btn"
          disabled={mutation.busy || pending || registering}
          onClick={async () => {
            try {
              if (!(await mutation.confirm({ message: radioWarning }))) return;
              await mutation.run(async () => {
                await api("/api/modem/scan", { method: "POST", body: {} });
                scanJob.current.start();
                setPending(true);
                phase.reload();
                r.reload();
              });
            } catch (e) {
              setError(e as Error);
            }
          }}
        >
          {t("Scan networks")}
        </button>
        <div className="table-scroll">
          <table className="data-table">
            <thead>
              <tr>
                {["Operator", "PLMN", "Network", "Status", "Actions"].map(
                  (s) => (
                    <th key={s}>{t(s)}</th>
                  ),
                )}
              </tr>
            </thead>
            <tbody>
              {rows.map((n: ObjectData) => (
                <tr key={n.plmn + "/" + n.rat}>
                  <td>{carrier(n.name)}</td>
                  <td>{n.plmn}</td>
                  <td>{n.rat}</td>
                  <td>{n.status}</td>
                  <td>
                    <button
                      className="text-link"
                      disabled={
                        !n.selectable || mutation.busy || pending || registering
                      }
                      onClick={async () => {
                        try {
                          if (
                            !(await mutation.confirm({ message: radioWarning }))
                          )
                            return;
                          await mutation.run(async () => {
                            await api("/api/modem/register", {
                              method: "POST",
                              body: { m_mcc_mnc: n.plmn, m_rat: n.rat },
                            });
                            registerJob.current.start();
                            setRegistering(true);
                            registration.reload();
                          });
                        } catch (e) {
                          setError(e as Error);
                        }
                      }}
                    >
                      {t("Connect")}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Panel>
    </>
  );
}
const apnFields = [
  field("profilename", "Profile name", { required: true }),
  field("wanapn", "APN", { required: true }),
  options(
    "pdpType",
    "IP version",
    [
      ["3", "IPv4 + IPv6"],
      ["1", "IPv4"],
      ["2", "IPv6"],
    ],
    { wire: "number" },
  ),
  options(
    "pppAuthMode",
    "Authentication",
    [
      ["0", "None"],
      ["1", "PAP"],
      ["2", "CHAP"],
    ],
    { wire: "number" },
  ),
  field("username", "Username"),
  field("password", "New password", { type: "password" }),
];
function APN() {
  const r = useResource("/api/router/apn/profiles"),
    mutation = useMutation(),
    [error, setError] = useState<Error>(),
    [selected, setSelected] = useState("");
  const profiles = (r.data?.apnListArray || []).filter(
    (p: ObjectData) => p.profilename || p.wanapn,
  );
  const current = profiles.find(
    (p: ObjectData) => String(p.profileId) === selected,
  );
  const change = async (remove = false) => {
    try {
      if (!current) return;
      if (
        !(await mutation.confirm({
          message: remove ? "Delete this APN profile?" : radioWarning,
          danger: remove,
        }))
      )
        return;
      await mutation.run(async () => {
        const fresh = await api("/api/router/apn/profiles");
        if (
          remove &&
          fresh.apnListArray.some(
            (p: ObjectData) =>
              String(p.profileId) === selected && String(p.isEnable) === "1",
          )
        )
          throw new Error("The active profile cannot be deleted");
        await api(
          "/api/router/apn/profiles/" + (remove ? "delete" : "activate"),
          { method: "POST", body: { profileId: current.profileId } },
        );
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
          id: "apn-mode",
          title: "APN selection",
          read: "/api/router/apn/mode",
          fields: [
            options(
              "apn_mode",
              "Profile mode",
              [
                ["0", "Automatic"],
                ["1", "Manual"],
              ],
              { wire: "number" },
            ),
          ],
          interrupt: radioWarning,
        }}
      />
      <Panel>
        <PanelTitle title="APN profiles" />
        <ErrorState error={error || r.error} />
        <Field label="Profile">
          <select
            value={selected}
            onChange={(e) => setSelected(e.target.value)}
          >
            <option value="">{t("Create a profile")}</option>
            {profiles.map((p: ObjectData) => (
              <option key={p.profileId} value={p.profileId}>
                {p.profilename} · {p.wanapn}
                {String(p.isEnable) === "1" ? " ✓" : ""}
              </option>
            ))}
          </select>
        </Field>
        {current && (
          <div className="dialog-actions">
            <button
              className="btn"
              disabled={mutation.busy || String(current.isEnable) === "1"}
              onClick={() => void change()}
            >
              {t("Activate profile")}
            </button>
            <button
              className="btn"
              disabled={mutation.busy || String(current.isEnable) === "1"}
              onClick={() => void change(true)}
            >
              {t("Delete")}
            </button>
          </div>
        )}
      </Panel>
      {current ? (
        <SettingsForm
          key={selected}
          onApplied={r.reload}
          spec={{
            id: "apn-edit",
            title: "Edit profile",
            read: "/api/router/apn/profiles",
            decode: (d) =>
              d.apnListArray.find(
                (p: ObjectData) => String(p.profileId) === selected,
              ),
            write: "/api/router/apn/profiles",
            fields: apnFields,
            body: (c, v) => ({
              ...Object.fromEntries(
                apnFields.map((f) => [f.key, v[f.key] ?? ""]),
              ),
              profileId: current.profileId,
            }),
            interrupt: (_, b) =>
              String(b.isEnable) === "1" ? radioWarning : undefined,
          }}
        />
      ) : (
        <CommandForm
          title="Create a profile"
          fields={apnFields}
          endpoint="/api/router/apn/profiles"
          body={(v) => ({
            ...v,
            pdpType: Number(v.pdpType),
            pppAuthMode: Number(v.pppAuthMode),
          })}
          label="Create profile"
          after={r.reload}
        />
      )}
    </>
  );
}
function CarrierServices() {
  const [result, setResult] = useState<ObjectData>();
  return (
    <>
      <CommandForm
        title="USSD"
        endpoint="/api/ussd/send"
        fields={[
          field("code", "USSD code", { required: true, maxLength: 182 }),
        ]}
        label="Send USSD"
        after={setResult}
      />
      {result && (
        <Panel>
          <PanelTitle title="USSD response" />
          <p className="message-body">
            {scalar(result.text || result.response || result)}
          </p>
        </Panel>
      )}
      <div className="two-columns">
        <CommandForm
          title="Phone call"
          endpoint="/api/call/dial"
          fields={[
            field("number", "Phone number", { required: true, maxLength: 40 }),
          ]}
          label="Call"
        />
        <CommandForm
          title="Current call"
          endpoint="/api/call/hangup"
          label="Hang up"
          body={() => ({})}
        />
      </div>
      <ReadingsPanel
        title="Call status"
        path="/api/call/status"
        rows={[["calls", "Calls"]]}
      />
      <SimToolkit />
    </>
  );
}
