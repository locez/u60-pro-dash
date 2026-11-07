import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  Check,
  ChevronRight,
  Gauge,
  Leaf,
  Plane,
  Radio,
  Scale,
  SlidersHorizontal,
  Thermometer,
  TrainFront,
} from "lucide-react";
import { api, type ObjectData } from "./api";
import { go } from "./catalog";
import { t } from "./i18n";
import { useLive, useReading } from "./live";
import { useMutation } from "./mutations";
import { ErrorState, Modal, Note, Panel, PanelTitle, Toggle } from "./ui";
import "./operating.css";
export type Profile = "balanced" | "performance" | "saving";
type Control = "airplane" | "performance" | "bandwidth";
export const profiles = [
  {
    id: "balanced",
    label: "Balanced",
    hint: "Adjust to demand for everyday use.",
    detail: "Balance response time, temperature and power use.",
  },
  {
    id: "performance",
    label: "Performance",
    hint: "Prioritize response and processing capacity.",
    detail: "For heavier workloads. Heat and power use may increase.",
  },
  {
    id: "saving",
    label: "Power saving",
    hint: "Reduce power use while on the move.",
    detail: "Processing speed and forwarding throughput may decrease.",
  },
] as const;
const strategies = [
  [
    "normal",
    "Top speed",
    "Use bandwidth as needed without application priority.",
  ],
  ["ai", "AI smart", "Automatically prioritize active applications."],
  ["game", "Gaming", "Prioritize gaming traffic."],
  ["video", "Video", "Prioritize video traffic."],
  ["meeting", "Meetings", "Prioritize meeting and classroom traffic."],
  ["live", "Live streaming", "Prioritize live streaming traffic."],
] as const;
const endpoint = "/api/device/operating";
function useOperatingState() {
  const reading = useReading("operating"),
    live = useLive(),
    mutation = useMutation();
  const [data, setData] = useState<ObjectData | null>(null),
    [error, setError] = useState<Error>(),
    [errors, setErrors] = useState<Partial<Record<Control, Error>>>({}),
    [pending, setPending] = useState<Control | null>(null);
  const inFlight = useRef(false);
  useEffect(() => {
    const controller = new AbortController();
    api(endpoint, { signal: controller.signal }).then(
      (value) => {
        if (!controller.signal.aborted && !inFlight.current) setData(value);
      },
      (e) => {
        if (!controller.signal.aborted) setError(e);
      },
    );
    return () => controller.abort();
  }, []);
  useEffect(() => {
    if (inFlight.current) return;
    if (reading?.data && !reading.error) {
      setData(reading.data);
      setError(undefined);
    } else if (reading?.error) {
      setData(null);
      setError(new Error(reading.error));
    }
  }, [reading]);
  const available = !!data && !error && live.state === "live";
  const change = async (control: Control, value: boolean | string) => {
    if (!available || mutation.busy || inFlight.current) return;
    if (
      control === "airplane" &&
      value === true &&
      !(await mutation.confirm({
        title: "Turn on airplane mode?",
        message:
          "Mobile internet, calls and SMS will disconnect. Wi-Fi, USB and local management remain available. Remote access over mobile data will disconnect.",
      }))
    )
      return;
    try {
      await mutation.run(async () => {
        inFlight.current = true;
        setPending(control);
        setErrors((previous) => ({ ...previous, [control]: undefined }));
        try {
          const path =
            control === "airplane"
              ? "/api/modem/airplane"
              : control === "performance"
                ? "/api/device/performance"
                : "/api/modem/bandwidth";
          const body =
            control === "airplane"
              ? { operate_mode: value ? "LPM" : "ONLINE" }
              : control === "performance"
                ? { profile: value }
                : { mode: value };
          await api(path, {
            method: control === "airplane" ? "POST" : "PUT",
            body,
            timeout: 30000,
          });
          const actual = await api(endpoint);
          setData(actual);
          setError(undefined);
          const applied =
            control === "airplane"
              ? actual.airplane?.enabled
              : control === "performance"
                ? actual.performance?.effective
                : actual.bandwidth?.mode;
          if (applied !== value)
            throw new Error(
              "The requested setting was not confirmed. Check current device status.",
            );
        } catch (e) {
          setErrors((previous) => ({ ...previous, [control]: e as Error }));
          // A lost response can follow an applied write: read the device, never invent a rollback.
          try {
            setData(await api(endpoint));
          } catch (readError) {
            setData(null);
            setError(readError as Error);
          }
        } finally {
          inFlight.current = false;
          setPending(null);
        }
      });
    } catch (e) {
      setErrors((previous) => ({ ...previous, [control]: e as Error }));
    }
  };
  return {
    data,
    error,
    errors,
    pending,
    available,
    disabled: !available || mutation.busy,
    change,
  };
}
const Context = createContext<ReturnType<typeof useOperatingState> | null>(
  null,
);
export function OperatingProvider({ children }: { children: ReactNode }) {
  const state = useOperatingState();
  return <Context.Provider value={state}>{children}</Context.Provider>;
}
export function useOperating() {
  const value = useContext(Context);
  if (!value) throw new Error("Missing operating context");
  return value;
}
export function ProfileIcon({
  profile,
  size = 16,
}: {
  profile?: string | null;
  size?: number;
}) {
  const Icon =
    profile === "saving" ? Leaf : profile === "performance" ? Gauge : Scale;
  return <Icon size={size} />;
}
function label(profile?: string | null) {
  return t(profiles.find((p) => p.id === profile)?.label || "Unknown");
}
export function FlightModeControl() {
  const m = useOperating(),
    value = m.data?.airplane?.enabled;
  return (
    <div className="mode-controls">
      <section className={"mode-flight " + (value === true ? "active" : "")}>
        <span className="mode-icon">
          <Plane size={22} />
        </span>
        <div>
          <b>{t("Airplane mode")}</b>
          <p>
            {t(
              m.pending === "airplane"
                ? "Updating cellular radio…"
                : !m.available || typeof value !== "boolean"
                  ? "Cellular radio status is unavailable."
                  : "Disable mobile data, calls and SMS; keep Wi-Fi and local management.",
            )}
          </p>
        </div>
        <Toggle
          label="Airplane mode"
          on={value === true}
          disabled={m.disabled || typeof value !== "boolean"}
          onChange={() => void m.change("airplane", !value)}
        />
      </section>
      <ErrorState error={m.errors.airplane || m.error} />
    </div>
  );
}
export function PerformanceControls() {
  const m = useOperating(),
    p = m.data?.performance,
    current = p?.requested || p?.effective;
  return (
    <section className="mode-performance">
      <div className="mode-section-title">
        <h3>{t("Performance mode")}</h3>
        <span>
          {t(
            m.pending === "performance"
              ? "Applying…"
              : "Settings apply automatically.",
          )}
        </span>
      </div>
      <div
        className="performance-options"
        role="group"
        aria-label={t("Performance mode")}
      >
        {profiles.map((profile) => (
          <button
            key={profile.id}
            aria-pressed={m.available && current === profile.id}
            disabled={
              m.disabled || !p?.supported || !p?.available?.includes(profile.id)
            }
            onClick={() => void m.change("performance", profile.id)}
          >
            <ProfileIcon profile={profile.id} size={21} />
            <b>{t(profile.label)}</b>
            <small>{t(profile.hint)}</small>
            {m.available && current === profile.id && (
              <Check className="mode-check" size={15} />
            )}
          </button>
        ))}
      </div>
      <p className="mode-help">
        {t(
          !m.available || !p?.supported
            ? "Performance control is unavailable."
            : profiles.find((profile) => profile.id === current)?.detail ||
                "Current CPU settings are managed outside this profile.",
        )}
      </p>
      {m.available && p?.supported && p?.applied === false && (
        <Note warning>
          {t(
            "The selected profile is not currently applied. Check device status before retrying.",
          )}
        </Note>
      )}
      {m.available && p?.thermal_limited === true && (
        <Note warning>
          {t(
            "Temperature protection is limiting processing capacity. Your selected profile is retained.",
          )}
        </Note>
      )}
      <ErrorState error={m.errors.performance || m.error} />
    </section>
  );
}
function RailStatus() {
  const m = useOperating(),
    active = m.data?.rail?.active,
    airplane = m.data?.airplane?.enabled;
  return (
    <div className="mode-rail">
      <TrainFront size={21} />
      <div>
        <b>
          {t("High-speed rail optimization")}{" "}
          <span className={m.available && active && !airplane ? "mode-on" : ""}>
            {t(
              !m.available || typeof active !== "boolean"
                ? "Unknown"
                : airplane
                  ? "Paused"
                  : active
                    ? "Active"
                    : "Not triggered",
            )}
          </span>
        </b>
        <p>
          {t(
            "The device manages this automatically while travelling at high speed.",
          )}
        </p>
      </div>
      <span className="mode-auto">{t("Automatic")}</span>
    </div>
  );
}
export function NetworkOptimization() {
  const m = useOperating(),
    value = m.data?.bandwidth;
  return (
    <Panel>
      <PanelTitle title="Network optimization" />
      <div className="mode-controls">
        <section>
          <div className="mode-section-title">
            <h3>{t("Bandwidth allocation")}</h3>
            <span>{t("Choose which applications get priority.")}</span>
          </div>
          <div
            className="bandwidth-options"
            role="group"
            aria-label={t("Bandwidth allocation")}
          >
            {strategies.map(([id, name, hint]) => (
              <button
                key={id}
                disabled={m.disabled || !value?.supported}
                aria-pressed={m.available && value?.mode === id}
                onClick={() => void m.change("bandwidth", id)}
              >
                <b>{t(name)}</b>
                <small>{t(hint)}</small>
                {m.available && value?.mode === id && <Check size={14} />}
              </button>
            ))}
          </div>
          {!value?.supported && (
            <p className="mode-help">
              {t("Bandwidth control is unavailable on this firmware.")}
            </p>
          )}
          {m.available && m.data?.airplane?.enabled === true && (
            <p className="mode-help">
              {t("Mobile radio is off. Your bandwidth choice is retained.")}
            </p>
          )}
          <ErrorState error={m.errors.bandwidth || m.error} />
        </section>
        <RailStatus />
      </div>
    </Panel>
  );
}
export function ModeOverview({ open }: { open: () => void }) {
  const m = useOperating(),
    p = m.data?.performance,
    airplane = m.available ? m.data?.airplane?.enabled : null;
  return (
    <div className="mode-overview">
      <button onClick={open}>
        {airplane ? <Plane size={16} /> : <Radio size={16} />}
        <span>
          {t(
            airplane === true
              ? "Airplane mode"
              : airplane === false
                ? "Cellular radio on"
                : "Cellular status unknown",
          )}
        </span>
      </button>
      <button onClick={open}>
        <ProfileIcon profile={p?.effective} />
        <span>{m.available ? label(p?.effective) : t("Unknown")}</span>
        <ChevronRight size={13} />
      </button>
      {m.available && p?.thermal_limited === true ? (
        <span className="mode-status warning">
          <Thermometer size={15} />
          {t("Temperature protection")}
        </span>
      ) : m.available && airplane === false && m.data?.rail?.active === true ? (
        <span className="mode-status">
          <TrainFront size={15} />
          {t("High-speed rail optimization")}
        </span>
      ) : null}
      <button className="mode-open" onClick={open}>
        <SlidersHorizontal size={15} />
        {t("Quick controls")}
      </button>
    </div>
  );
}
export function QuickControls({ close }: { close: () => void }) {
  const open = (page: "internet" | "system") => {
    close();
    go(page, page === "internet" ? "connection" : "power");
  };
  return (
    <Modal title="Quick controls" close={close}>
      <div className="mode-controls compact">
        <FlightModeControl />
        <PerformanceControls />
        <RailStatus />
        <div className="mode-destinations">
          <button className="mode-detail-link" onClick={() => open("internet")}>
            {t("Mobile network")}
            <ChevronRight size={14} />
          </button>
          <button className="mode-detail-link" onClick={() => open("system")}>
            {t("Battery & power")}
            <ChevronRight size={14} />
          </button>
        </div>
      </div>
    </Modal>
  );
}
