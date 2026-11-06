import { useEffect, useState } from "react";
import { api, type ObjectData } from "./api";
import { useJobPolling, useResource } from "./live";
import { useMutation } from "./mutations";
import {
  CommandForm,
  field,
  flag,
  options,
  ReadingsPanel,
  SettingsForm,
} from "./forms";
import {
  Definition,
  ErrorState,
  Note,
  Panel,
  PanelTitle,
  Refresh,
  scalar,
} from "./ui";
import { routingModel } from "./models";
import { t } from "./i18n";
const interruption =
  "Changing these settings can interrupt existing network connections.";
export function Records({
  rows,
  columns,
}: {
  rows: ObjectData[];
  columns: [string, string][];
}) {
  return (
    <div className="table-scroll">
      <table className="data-table">
        <thead>
          <tr>
            {columns.map(([key, label]) => (
              <th key={key}>{t(label)}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, index) => (
            <tr key={row.id ?? index}>
              {columns.map(([key]) => (
                <td key={key}>{scalar(row[key])}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {!rows.length && <p className="empty-state">{t("No records")}</p>}
    </div>
  );
}
export function NetworkPolicies() {
  const rules = useResource("/api/router/domain-filter"),
    filters = useResource("/api/router/firewall/filter-rules"),
    mutation = useMutation(),
    [error, setError] = useState<Error>();
  const list = Array.isArray(rules.data?.rule_list) ? rules.data.rule_list : [];
  return (
    <>
      <SettingsForm
        spec={{
          id: "passthrough",
          title: "VPN passthrough & ALG",
          read: "/api/router/vpn",
          decode: routingModel.normalizeSwitches,
          fields: [
            flag("l2tp_passthrough", "L2TP passthrough"),
            flag("pptp_passthrough", "PPTP passthrough"),
            flag("ipsec_passthrough", "IPsec passthrough"),
            flag("alg_sip_enable", "SIP ALG"),
          ],
          interrupt: interruption,
        }}
      />
      <SettingsForm
        onApplied={rules.reload}
        spec={{
          id: "domain-filter",
          title: "Domain filtering",
          read: "/api/router/domain-filter",
          fields: [flag("enable", "Domain filtering", "number")],
          interrupt: interruption,
        }}
      />
      <Panel>
        <PanelTitle title="Blocked domains" />
        <ErrorState error={error || rules.error} />
        <div className="table-scroll">
          <table className="data-table">
            <tbody>
              {list.map((row: ObjectData, index: number) => {
                const id = String(row.id ?? row.section_id ?? index);
                return (
                  <tr key={id}>
                    <td>{row.domain || row.fqdn}</td>
                    <td>
                      <button
                        className="text-link"
                        disabled={mutation.busy}
                        onClick={async () => {
                          try {
                            if (
                              !(await mutation.confirm({
                                message: "Delete this domain rule?",
                              }))
                            )
                              return;
                            await mutation.run(async () => {
                              const fresh = await api(
                                "/api/router/domain-filter",
                              );
                              if (
                                routingModel.canonical(fresh.rule_list) !==
                                routingModel.canonical(list)
                              )
                                throw new Error(
                                  "Settings changed elsewhere. Review the current values before trying again.",
                                );
                              await api("/api/router/domain-filter", {
                                method: "PUT",
                                body: {
                                  action: "delete",
                                  id,
                                  section_id: [id],
                                },
                              });
                              rules.reload();
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
                );
              })}
            </tbody>
          </table>
        </div>
      </Panel>
      <CommandForm
        title="Add domain rule"
        endpoint="/api/router/domain-filter"
        method="PUT"
        fields={[field("domain", "Domain", { required: true, maxLength: 253 })]}
        label="Add rule"
        body={(v) => {
          const domain = String(v.domain).trim().toLowerCase();
          if (
            !domain.includes(".") ||
            !domain
              .split(".")
              .every((label) =>
                /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(label),
              )
          )
            throw new Error("Enter a valid domain name");
          if (
            list.some(
              (r: ObjectData) =>
                String(r.domain || r.fqdn).toLowerCase() === domain,
            )
          )
            throw new Error("This domain already exists");
          return {
            action: "add",
            domain,
            fqdn: domain,
            enabled: "1",
            enable: 1,
          };
        }}
        before={async () => {
          const fresh = await api("/api/router/domain-filter");
          if (
            !Array.isArray(fresh.rule_list) ||
            routingModel.canonical(fresh.rule_list) !==
              routingModel.canonical(list)
          )
            throw new Error(
              "Settings changed elsewhere. Review the current values before trying again.",
            );
        }}
        after={rules.reload}
      />
      <Panel>
        <PanelTitle
          title="Firewall filter rules"
          action={<Refresh onClick={filters.reload} />}
        />
        <ErrorState error={filters.error} />
        <Records
          rows={filters.data?.rule_list || []}
          columns={[
            ["name", "Name"],
            ["src_ip", "Source"],
            ["dest_ip", "Destination"],
            ["protocol", "Protocol"],
            ["target", "Action"],
          ]}
        />
      </Panel>
    </>
  );
}
const stcFields = [
  ["stc_lte_collect_timer", "lte_collect_timer", "LTE collection interval"],
  ["stc_nrsa_collect_timer", "nrsa_collect_timer", "NR SA collection interval"],
  ["stc_lte_white_list_num_max", "lte_whitelist_max", "LTE allowlist size"],
  ["stc_nrsa_white_list_num_max", "nrsa_whitelist_max", "NR SA allowlist size"],
];
export function RadioTools() {
  const nr = useResource("/api/cell/neighbors/nr"),
    lte = useResource("/api/cell/neighbors/lte"),
    progress = useResource("/api/cell/signal-detect/progress"),
    results = useResource("/api/cell/signal-detect/results");
  const [started, setStarted] = useState(0),
    [timedOut, setTimedOut] = useState(false);
  useJobPolling(progress, !!started || progress.data?.running === true, 2000);
  useEffect(() => {
    if (!started) return;
    if (Date.now() - started > 120000) {
      setStarted(0);
      setTimedOut(true);
      return;
    }
    if (progress.data?.running === false && Date.now() - started > 3000) {
      setStarted(0);
      results.reload();
    }
  }, [progress.data, started]);
  const columns: [string, string][] = [
    ["band", "Band"],
    ["pci", "PCI"],
    ["earfcn", "Channel"],
    ["rsrp", "RSRP"],
    ["rsrq", "RSRQ"],
    ["sinr", "SINR"],
  ];
  return (
    <>
      <CommandForm
        title="Neighbor cells"
        endpoint="/api/cell/neighbors/scan"
        body={() => ({})}
        label="Scan neighbor cells"
        warning="Scanning can briefly interrupt mobile data. Firmware may retain earlier results."
        after={() => {
          nr.reload();
          lte.reload();
        }}
      />
      <div className="two-columns">
        {[
          ["5G NR", nr],
          ["4G LTE", lte],
        ].map(([name, resource]) => {
          const r = resource as typeof nr;
          return (
            <Panel key={name as string}>
              <PanelTitle
                title={name as string}
                action={<Refresh onClick={r.reload} />}
              />
              <ErrorState error={r.error} />
              <Records rows={r.data?.cell_list || []} columns={columns} />
            </Panel>
          );
        })}
      </div>
      <SettingsForm
        spec={{
          id: "stc",
          title: "STC cell policy",
          read: "/api/cell/stc/params",
          decode: (d) => ({
            ...d,
            ...Object.fromEntries(
              stcFields.map(([key, alias]) => [key, d[key] ?? d[alias]]),
            ),
          }),
          fields: stcFields.map(([key, , label]) =>
            field(key, label, { type: "number", min: 0, max: 2147483647 }),
          ),
          body: (_, all) =>
            Object.fromEntries(
              stcFields.map(([key]) => [key, String(all[key])]),
            ),
          interrupt: interruption,
        }}
      />
      <ReadingsPanel
        title="STC status"
        path="/api/cell/stc/status"
        rows={[
          ["stc_enable", "Enabled"],
          ["status", "State"],
        ]}
      />
      <div className="three-columns">
        {[
          ["enable", "Enable STC"],
          ["disable", "Disable STC"],
          ["reset", "Reset STC"],
        ].map(([id, label]) => (
          <CommandForm
            key={id}
            title={label}
            label={label}
            endpoint={"/api/cell/stc/" + id}
            body={() => ({})}
            warning={interruption}
          />
        ))}
      </div>
      <div className="two-columns">
        <CommandForm
          title="Signal quality test"
          endpoint="/api/cell/signal-detect/start"
          body={() => ({})}
          label="Start test"
          warning={interruption}
          after={() => {
            setStarted(Date.now());
            setTimedOut(false);
            progress.reload();
          }}
        />
        <CommandForm
          title="Stop signal test"
          endpoint="/api/cell/signal-detect/stop"
          body={() => ({})}
          label="Stop"
          after={() => {
            setStarted(0);
            progress.reload();
            results.reload();
          }}
        />
      </div>
      <Panel>
        <PanelTitle
          title="Test result"
          action={
            <Refresh
              onClick={() => {
                progress.reload();
                results.reload();
              }}
            />
          }
        />
        <ErrorState error={progress.error || results.error} />
        {timedOut && (
          <Note>
            {t(
              "Waiting for results timed out. Read the current state before repeating the test.",
            )}
          </Note>
        )}
        <Definition
          items={[
            ["Progress", scalar(progress.data?.progress)],
            ["State", scalar(progress.data?.running)],
          ]}
        />
        <Records rows={results.data?.record_list || []} columns={columns} />
      </Panel>
    </>
  );
}
export function SimToolkit() {
  const r = useResource("/api/stk/menu"),
    d = r.data;
  useJobPolling(r, d?.busy === true, 2000);
  const guard = async () => {
    const fresh = await api("/api/stk/menu");
    if (!d?.command_id || fresh.command_id !== d.command_id || fresh.busy)
      throw new Error("The SIM menu changed. Read it again before responding.");
  };
  const ready =
    d?.supported === true && !d.busy && typeof d.command_id === "string";
  return (
    <>
      <Panel>
        <PanelTitle
          title="SIM toolkit"
          action={<Refresh onClick={r.reload} />}
        />
        <ErrorState error={r.error} />
        <Definition
          items={[
            ["State", d?.status],
            ["Title", d?.title],
          ]}
        />
        <p className="message-body">{d?.text || d?.reason}</p>
      </Panel>
      {ready && (
        <div key={d.command_id}>
          {!!d.items?.length && (
            <CommandForm
              title="SIM menu"
              endpoint="/api/stk/select"
              fields={[
                options(
                  "item_id",
                  "Menu item",
                  d.items.map((item: ObjectData) => [
                    String(item.id),
                    item.label,
                  ]),
                ),
              ]}
              body={(v) => ({
                command_id: d.command_id,
                item_id: Number(v.item_id),
              })}
              before={guard}
              label="Open menu item"
              warning="SIM services can change your subscription or incur carrier charges."
              after={r.reload}
            />
          )}
          {d.type === "input" && (
            <CommandForm
              title="Reply to SIM"
              endpoint="/api/stk/respond"
              fields={
                d.input?.yes_no
                  ? [flag("yes", "Response", "boolean")]
                  : [
                      field("text", "Response", {
                        required: d.input?.min > 0,
                        maxLength: d.input?.max,
                        type: d.input?.echo === false ? "password" : "text",
                      }),
                    ]
              }
              body={(v) => {
                if (d.input?.yes_no)
                  return { command_id: d.command_id, yes: v.yes === "true" };
                if (
                  [...v.text].length < d.input.min ||
                  [...v.text].length > d.input.max ||
                  (d.input.digits_only && !/^[0-9*#+]*$/.test(v.text))
                )
                  throw new Error(
                    "Response does not match the SIM input requirements",
                  );
                return { command_id: d.command_id, text: v.text };
              }}
              before={guard}
              label="Send response"
              after={r.reload}
            />
          )}
          {d.type === "display" && (
            <CommandForm
              title="Acknowledge message"
              endpoint="/api/stk/respond"
              body={() => ({ command_id: d.command_id })}
              before={guard}
              label="Continue"
              after={r.reload}
            />
          )}
          <div className="two-columns">
            {[
              ["back", "Back"],
              ["cancel", "End session"],
            ].map(([action, label]) => (
              <CommandForm
                key={action}
                title={label}
                label={label}
                endpoint={"/api/stk/" + action}
                body={() => ({ command_id: d.command_id })}
                before={guard}
                after={r.reload}
              />
            ))}
          </div>
        </div>
      )}
    </>
  );
}
export function LiveActivity() {
  const r = useResource("/api/live-activity");
  return (
    <>
      <Panel>
        <PanelTitle title="iPhone Live Activity" />
        <ErrorState error={r.error} />
        <Definition
          items={[
            ["Enabled", scalar(r.data?.enabled)],
            ["Registered", scalar(r.data?.registered)],
            ["Expires at", r.data?.expires_at],
            ["Error", r.data?.last_error],
          ]}
        />
      </Panel>
      {r.data?.registered && (
        <CommandForm
          title="Unbind Live Activity"
          endpoint="/api/live-activity"
          method="DELETE"
          body={() => ({})}
          label="Unbind"
          warning="Stop updates to the currently registered iPhone activity?"
          after={r.reload}
        />
      )}
    </>
  );
}
