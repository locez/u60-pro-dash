import { useState } from "react";
import { api, type ObjectData } from "./api";
import { useJobPolling, useResource } from "./live";
import { useMutation } from "./mutations";
import { CommandForm, field, options } from "./forms";
import {
  Definition,
  Empty,
  ErrorState,
  Field,
  Note,
  Panel,
  PanelTitle,
  Refresh,
  scalar,
  Tabs,
} from "./ui";
import { t, carrier } from "./i18n";
const pin = (v: unknown) => {
  const s = String(v);
  if (!/^\d{4,8}$/.test(s)) throw new Error("Use a SIM PIN of 4–8 digits.");
  return s;
};
export function SimPage() {
  const [tab, setTab] = useState("card");
  return (
    <>
      <Tabs
        items={[
          ["card", "SIM card"],
          ["security", "SIM security"],
          ["esim", "eSIM profiles"],
        ]}
        value={tab}
        onChange={setTab}
      />
      {tab === "esim" ? (
        <Esim />
      ) : (
        <SimSecurity security={tab === "security"} />
      )}
    </>
  );
}
function SimSecurity({ security }: { security: boolean }) {
  const r = useResource("/api/sim/info"),
    d = r.data;
  const mutation = useMutation();
  const mask = (v: unknown) => (v ? "••••••••" + String(v).slice(-4) : "—");
  const guard =
    (expected: string, puk = false) =>
    async () => {
      const keys = [
        "sim_states",
        "modem_main_state",
        "sim_iccid",
        "iccid",
        "sim_epoch",
        "sim_phase",
        "agent_instance",
        "pin_status",
        "pinnumber",
        "puknumber",
      ];
      const current = await api("/api/sim/info");
      if (
        !d ||
        keys.some(
          (key) => String(current[key] ?? "") !== String(d[key] ?? ""),
        ) ||
        current.sim_states !== expected
      ) {
        r.reload();
        throw new Error(
          "The SIM state changed. Read the current card before trying again.",
        );
      }
      const remaining = Number(current[puk ? "puknumber" : "pinnumber"]);
      if (!Number.isInteger(remaining) || remaining < 1)
        throw new Error("Remaining SIM attempts are unavailable or exhausted.");
      if (remaining === 1) {
        if (
          !(await mutation.confirm({
            title: "Last SIM attempt",
            message:
              "Only one attempt remains. A wrong code can block the SIM.",
            danger: true,
            phrase: puk ? "PUK" : "PIN",
          }))
        )
          return false;
        const fresh = await api("/api/sim/info");
        if (
          keys.some(
            (key) => String(current[key] ?? "") !== String(fresh[key] ?? ""),
          )
        )
          throw new Error(
            "The SIM state changed. Read the current card before trying again.",
          );
      }
    };
  return (
    <>
      <Panel>
        <PanelTitle title="SIM card" action={<Refresh onClick={r.reload} />} />
        <ErrorState error={r.error} />
        <Definition
          items={[
            ["State", d?.sim_states],
            ["PIN attempts remaining", d?.pinnumber],
            ["PUK attempts remaining", d?.puknumber],
            ["ICCID", mask(d?.sim_iccid || d?.iccid)],
            ["IMSI", mask(d?.sim_imsi || d?.imsi)],
          ]}
        />
      </Panel>
      {security && (
        <>
          <Note warning>
            {t(
              "Incorrect PIN or PUK codes consume attempts. Read the current SIM state before submitting.",
            )}
          </Note>
          {d?.sim_states === "wait pin" && (
            <CommandForm
              title="Unlock SIM"
              before={guard("wait pin")}
              endpoint="/api/sim/pin/verify"
              fields={[
                field("pin", "SIM PIN", {
                  type: "password",
                  required: true,
                  maxLength: 8,
                }),
              ]}
              body={(v) => ({
                pin_num: pin(v.pin),
                puk_num: "",
                pin_encode_flag: "0",
              })}
              label="Unlock SIM"
              after={r.reload}
            />
          )}
          {d?.sim_states === "wait puk" && (
            <CommandForm
              title="Unlock with PUK"
              before={guard("wait puk", true)}
              endpoint="/api/sim/pin/verify"
              fields={[
                field("puk", "PUK", {
                  type: "password",
                  required: true,
                  maxLength: 8,
                }),
                field("pin", "New PIN", {
                  type: "password",
                  required: true,
                  maxLength: 8,
                }),
                field("repeat", "Repeat new PIN", {
                  type: "password",
                  required: true,
                  maxLength: 8,
                }),
              ]}
              body={(v) => {
                if (!/^\d{8}$/.test(v.puk) || v.pin !== v.repeat)
                  throw new Error("Check the PUK and matching new PIN values.");
                return {
                  pin_num: pin(v.pin),
                  puk_num: v.puk,
                  pin_encode_flag: "0",
                };
              }}
              label="Unlock SIM"
              after={r.reload}
            />
          )}
          {d?.sim_states === "sim ready" && (
            <div className="two-columns">
              <CommandForm
                title="SIM PIN protection"
                before={guard("sim ready")}
                endpoint="/api/sim/pin/mode"
                fields={[
                  options("enabled", "PIN protection", [
                    ["1", "Enabled"],
                    ["0", "Disabled"],
                  ]),
                  field("pin", "Current SIM PIN", {
                    type: "password",
                    required: true,
                    maxLength: 8,
                  }),
                ]}
                body={(v) => ({
                  pin_num_m: pin(v.pin),
                  pin_mode: Number(v.enabled),
                  pin_encode_flag: "0",
                })}
                label="Apply PIN protection"
                after={r.reload}
              />
              <CommandForm
                title="Change SIM PIN"
                before={guard("sim ready")}
                endpoint="/api/sim/pin/change"
                fields={[
                  field("pin", "Current SIM PIN", {
                    type: "password",
                    required: true,
                    maxLength: 8,
                  }),
                  field("next", "New PIN", {
                    type: "password",
                    required: true,
                    maxLength: 8,
                  }),
                  field("repeat", "Repeat new PIN", {
                    type: "password",
                    required: true,
                    maxLength: 8,
                  }),
                ]}
                body={(v) => {
                  if (v.next !== v.repeat)
                    throw new Error("The new PIN values do not match.");
                  return {
                    pin_num: pin(v.pin),
                    new_pin_num: pin(v.next),
                    pin_encode_flag: "0",
                  };
                }}
                label="Change PIN"
                after={r.reload}
              />
            </div>
          )}
          <details className="advanced-details">
            <summary>{t("Carrier network unlock")}</summary>
            <CommandForm
              title="Carrier network unlock"
              endpoint="/api/sim/unlock"
              fields={[
                field("nck", "Network unlock code", {
                  type: "password",
                  required: true,
                  maxLength: 16,
                }),
              ]}
              label="Unlock network"
              after={r.reload}
            />
          </details>
        </>
      )}
    </>
  );
}
function Esim() {
  const r = useResource("/api/esim/profiles", { timeout: 45000 }),
    download = useResource("/api/esim/download"),
    switching = useResource("/api/esim/switch"),
    mutation = useMutation(),
    [error, setError] = useState<Error>(),
    [rename, setRename] = useState<{ id: string; value: string }>();
  const busy =
    ["running", "confirm", "cancelling"].includes(download.data?.state) ||
    ["card_busy", "reloading", "card_confirmed", "modem_confirmed"].includes(
      switching.data?.state,
    );
  useJobPolling(
    download,
    ["running", "cancelling"].includes(download.data?.state),
  );
  useJobPolling(
    switching,
    ["card_busy", "reloading", "card_confirmed", "modem_confirmed"].includes(
      switching.data?.state,
    ),
  );
  const profiles = r.data?.profiles || [];
  const freshProfile = async (id: string) => {
    const fresh = await api("/api/esim/profiles", { timeout: 45000 });
    if (
      fresh.stale ||
      fresh.agent_instance !== r.data?.agent_instance ||
      fresh.card_changed
    )
      throw new Error(
        "The card changed. Read its profiles before making changes.",
      );
    const profile = fresh.profiles.find((p: ObjectData) => p.iccid === id);
    if (!profile) throw new Error("The profile is no longer present.");
    return profile;
  };
  const change = async (p: ObjectData, remove = false) => {
    try {
      if (
        !(await mutation.confirm({
          message: remove
            ? "Permanently delete this disabled eSIM profile?"
            : "Switch the active SIM profile? Mobile data will disconnect while the modem reconnects.",
          danger: remove,
        }))
      )
        return;
      await mutation.run(async () => {
        const fresh = await freshProfile(p.iccid);
        if (remove && fresh.state !== "disabled")
          throw new Error("Only disabled profiles can be deleted.");
        await api("/api/esim/" + (remove ? "delete" : "enable"), {
          method: "POST",
          body: { iccid: p.iccid },
          timeout: 65000,
        });
        r.reload();
        switching.reload();
      });
    } catch (e) {
      setError(e as Error);
    }
  };
  const transition = async (action: string, accept?: boolean) => {
    try {
      const expected = {
        started_at: download.data?.started_at,
        agent_instance: download.data?.agent_instance,
      };
      await mutation.run(async () => {
        await api(
          "/api/esim/download" + (action === "dismiss" ? "" : "/" + action),
          {
            method: action === "dismiss" ? "DELETE" : "POST",
            body: { expected, ...(accept !== undefined ? { accept } : {}) },
          },
        );
        download.reload();
        r.reload();
      });
    } catch (e) {
      setError(e as Error);
    }
  };
  return (
    <>
      <Panel>
        <PanelTitle
          title="eSIM profiles"
          action={<Refresh onClick={r.reload} disabled={r.loading || busy} />}
        />
        <Note>
          {t(
            "Downloading profiles requires a supported eUICC card. A regular carrier SIM cannot download eSIM profiles.",
          )}
        </Note>
        <ErrorState error={error || r.error} />
        <div className="table-scroll">
          <table className="data-table">
            <thead>
              <tr>
                {["Profile", "Operator", "State", "Actions"].map((s) => (
                  <th key={s}>{t(s)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {profiles.map((p: ObjectData) => (
                <tr key={p.iccid}>
                  <td>{p.nickname || p.name || t("Unnamed profile")}</td>
                  <td>{carrier(p.provider)}</td>
                  <td>{t(p.state)}</td>
                  <td>
                    <button
                      className="text-link"
                      disabled={
                        mutation.busy ||
                        busy ||
                        r.data?.stale ||
                        p.state === "enabled"
                      }
                      onClick={() => void change(p)}
                    >
                      {t("Activate")}
                    </button>{" "}
                    ·{" "}
                    <button
                      className="text-link"
                      disabled={busy || mutation.busy}
                      onClick={() =>
                        setRename({ id: p.iccid, value: p.nickname || "" })
                      }
                    >
                      {t("Rename")}
                    </button>{" "}
                    ·{" "}
                    <button
                      className="text-link"
                      disabled={
                        mutation.busy ||
                        busy ||
                        r.data?.stale ||
                        p.state !== "disabled"
                      }
                      onClick={() => void change(p, true)}
                    >
                      {t("Delete")}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {rename && (
          <Field label="Profile nickname">
            <input
              value={rename.value}
              maxLength={64}
              disabled={mutation.busy}
              onChange={(e) => setRename({ ...rename, value: e.target.value })}
              onBlur={async () => {
                try {
                  await mutation.run(async () => {
                    await freshProfile(rename.id);
                    await api("/api/esim/nickname", {
                      method: "POST",
                      body: { iccid: rename.id, nickname: rename.value },
                      timeout: 45000,
                    });
                    r.reload();
                    setRename(undefined);
                  });
                } catch (e) {
                  setError(e as Error);
                }
              }}
            />
          </Field>
        )}
        {switching.data?.state && switching.data.state !== "idle" && (
          <p className="subtle">
            {t("Profile switch")}: {scalar(switching.data.state)} ·{" "}
            {scalar(switching.data.network_check)}
          </p>
        )}
      </Panel>
      {Array.isArray(r.data?.profiles) && !r.data?.stale && (
        <CommandForm
          title="Download profile"
          endpoint="/api/esim/download"
          fields={[
            field("activation_code", "Activation code", {
              required: true,
              maxLength: 512,
            }),
            field("confirmation_code", "Confirmation code", {
              type: "password",
              maxLength: 64,
            }),
          ]}
          body={(v) => {
            if (!/^LPA:1\$/i.test(v.activation_code))
              throw new Error(
                "Paste the complete activation code beginning with LPA:1$.",
              );
            return { ...v, preview: true };
          }}
          label="Read profile details"
          after={download.reload}
        />
      )}
      {download.data && download.data.state !== "idle" && (
        <Panel>
          <PanelTitle title="Profile download" />
          <Definition
            items={[
              ["State", download.data.state],
              ["Step", download.data.step],
              [
                "Profile",
                scalar(download.data.metadata || download.data.profile),
              ],
              ["Error", download.data.error || "—"],
            ]}
          />
          <div className="dialog-actions">
            {download.data.state === "confirm" ? (
              <>
                <button
                  className="btn"
                  disabled={mutation.busy}
                  onClick={() => void transition("confirm", false)}
                >
                  {t("Decline")}
                </button>
                <button
                  className="btn primary"
                  disabled={mutation.busy}
                  onClick={() => void transition("confirm", true)}
                >
                  {t("Install profile")}
                </button>
              </>
            ) : ["running", "cancelling"].includes(download.data.state) ? (
              <button
                className="btn"
                disabled={
                  mutation.busy ||
                  download.data.state === "cancelling" ||
                  String(download.data.step).startsWith(
                    "es10b_load_bound_profile_package",
                  )
                }
                onClick={() => void transition("cancel")}
              >
                {t("Cancel download")}
              </button>
            ) : (
              <button
                className="btn"
                disabled={mutation.busy}
                onClick={() => void transition("dismiss")}
              >
                {t("Close result")}
              </button>
            )}
          </div>
        </Panel>
      )}
      <Receipts />
    </>
  );
}
function Receipts() {
  const r = useResource("/api/esim/notifications", { timeout: 65000 });
  return (
    <details className="advanced-details">
      <summary>{t("Operator receipts")}</summary>
      <Panel>
        <PanelTitle
          title="Operator receipts"
          action={<Refresh onClick={r.reload} />}
        />
        <ErrorState error={r.error} />
        <Definition items={[["Pending", r.data?.count]]} />
      </Panel>
      {r.data?.count > 0 && (
        <CommandForm
          title="Send pending receipts"
          endpoint="/api/esim/notifications/process"
          body={() => ({})}
          label="Send receipts"
          after={r.reload}
        />
      )}
    </details>
  );
}
