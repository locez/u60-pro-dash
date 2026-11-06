import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import QRCode from "qrcode";
import {
  ArrowDown,
  ArrowUp,
  ArrowUpRight,
  BatteryCharging,
  Check,
  ChevronRight,
  Laptop,
  Smartphone,
  Tablet,
  Speaker,
  Lightbulb,
  ShieldCheck,
  Signal,
  Wifi,
  X,
  Eye,
  EyeOff,
  Copy,
  Cable,
  Router,
  Plus,
} from "lucide-react";
import { traffic, clients } from "./data";
export type Variant = "a" | "b" | "c";
export type Notify = (text: string) => void;
export function Brand({ compact = false }: { compact?: boolean }) {
  return (
    <div className="brand">
      <span className="brand-glyph">
        <svg viewBox="0 0 28 28" fill="none" aria-hidden="true">
          <path
            d="M5 8v8a5 5 0 0 0 10 0V6m8 1v9a9 9 0 0 1-9 9"
            stroke="currentColor"
            strokeWidth="3"
            strokeLinecap="round"
          />
        </svg>
      </span>
      {!compact && (
        <span>
          U60<span className="brand-light"> Dash</span>
        </span>
      )}
    </div>
  );
}
export function Status({
  children = "已连接",
  muted = false,
}: {
  children?: ReactNode;
  muted?: boolean;
}) {
  return (
    <span className={"status " + (muted ? "status-muted" : "")}>
      <i />
      {children}
    </span>
  );
}
export function ArrowLink({
  children,
  onClick,
}: {
  children: ReactNode;
  onClick?: () => void;
}) {
  return (
    <button className="text-link" onClick={onClick}>
      {children}
      <ArrowUpRight size={14} />
    </button>
  );
}
export function Panel({
  children,
  className = "",
}: {
  children: ReactNode;
  className?: string;
}) {
  return <section className={"panel " + className}>{children}</section>;
}
export function PanelTitle({
  title,
  note,
  action,
}: {
  title: string;
  note?: string;
  action?: ReactNode;
}) {
  return (
    <div className="panel-heading">
      <div>
        <h2>{title}</h2>
        {note && <p>{note}</p>}
      </div>
      {action}
    </div>
  );
}
export function Toggle({
  on,
  label,
  onChange,
}: {
  on: boolean;
  label: string;
  onChange: () => void;
}) {
  return (
    <button
      role="switch"
      aria-checked={on}
      aria-label={label}
      className={"toggle " + (on ? "on" : "")}
      onClick={onChange}
    >
      <i />
    </button>
  );
}
export function DeviceIcon({
  kind,
  size = 19,
}: {
  kind: string;
  size?: number;
}) {
  const Icon =
    (
      {
        laptop: Laptop,
        phone: Smartphone,
        tablet: Tablet,
        speaker: Speaker,
        bulb: Lightbulb,
      } as Record<string, typeof Laptop>
    )[kind] || Laptop;
  return <Icon size={size} />;
}
export function Rate({
  up = false,
  value = "124.6",
  small = false,
}: {
  up?: boolean;
  value?: string;
  small?: boolean;
}) {
  return (
    <div
      className={"rate " + (up ? "rate-up" : "") + (small ? " rate-small" : "")}
    >
      <span className="rate-name">
        {up ? <ArrowUp size={14} /> : <ArrowDown size={14} />}{" "}
        {up ? "上传" : "下载"}
      </span>
      <div>
        <strong>{value}</strong>
        <span>Mbps</span>
      </div>
    </div>
  );
}
export function QR({ size = 112 }: { size?: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    if (ref.current)
      QRCode.toCanvas(
        ref.current,
        "WIFI:T:WPA;S:U60 Studio;P:sample-pass-2026;;",
        {
          width: size,
          margin: 2,
          color: { dark: "#18263d", light: "#ffffff" },
          errorCorrectionLevel: "M",
        },
      ).catch(() => {});
  }, [size]);
  return (
    <canvas
      ref={ref}
      width={size}
      height={size}
      role="img"
      aria-label="演示 Wi-Fi 二维码"
      className="wifi-qr"
    />
  );
}
export function TrafficChart({
  dense = false,
  period = "30 分钟",
}: {
  dense?: boolean;
  period?: string;
}) {
  const id = useId().replace(/:/g, "");
  const [hover, setHover] = useState<number | null>(null);
  const values = traffic.map((v, i) =>
    period === "24 小时"
      ? v * (0.65 + (i % 4) * 0.12)
      : period === "7 天"
        ? v * (0.5 + (i % 7) * 0.09)
        : v,
  );
  const width = 700,
    height = dense ? 126 : 154;
  const line = (scale: number) =>
    values
      .map(
        (v, i) =>
          `${i ? "L" : "M"}${(i / (values.length - 1)) * width},${height - (v / 160) * height * scale}`,
      )
      .join(" ");
  const path = line(1);
  return (
    <div
      className={"traffic-chart " + (dense ? "dense" : "")}
      onMouseLeave={() => setHover(null)}
    >
      <div className="chart-axis">
        <span>160</span>
        <span>80</span>
        <span>0</span>
      </div>
      <div className="chart-plot">
        <svg
          role="img"
          aria-label={`${period}下载与上传趋势，示例数据`}
          viewBox={`0 0 ${width} ${height}`}
          preserveAspectRatio="none"
          onMouseMove={(e) => {
            const r = e.currentTarget.getBoundingClientRect();
            setHover(
              Math.min(
                values.length - 1,
                Math.max(
                  0,
                  Math.round(
                    ((e.clientX - r.left) / r.width) * (values.length - 1),
                  ),
                ),
              ),
            );
          }}
        >
          <defs>
            <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="var(--accent)" stopOpacity=".17" />
              <stop offset="100%" stopColor="var(--accent)" stopOpacity="0" />
            </linearGradient>
          </defs>
          {[0, 0.5, 1].map((v) => (
            <line
              key={v}
              x1="0"
              x2={width}
              y1={height * v}
              y2={height * v}
              stroke="var(--line)"
              strokeDasharray="3 5"
            />
          ))}
          <path
            d={`${path} L${width},${height} L0,${height} Z`}
            fill={`url(#${id})`}
          />
          <path
            d={path}
            fill="none"
            stroke="var(--accent)"
            strokeWidth="2.5"
            strokeLinejoin="round"
            vectorEffect="non-scaling-stroke"
          />
          <path
            d={line(0.17)}
            fill="none"
            stroke="var(--upload)"
            strokeWidth="2"
            strokeLinejoin="round"
            vectorEffect="non-scaling-stroke"
          />
          {hover !== null && (
            <g>
              <line
                x1={(hover / (values.length - 1)) * width}
                x2={(hover / (values.length - 1)) * width}
                y1="0"
                y2={height}
                stroke="var(--muted)"
                strokeDasharray="3 3"
              />
              <circle
                cx={(hover / (values.length - 1)) * width}
                cy={height - (values[hover] / 160) * height}
                r="4"
                fill="var(--accent)"
              />
            </g>
          )}
        </svg>
        {hover !== null && (
          <span
            className="chart-tooltip"
            style={{
              left: `${Math.min(80, (hover / (values.length - 1)) * 100)}%`,
            }}
          >
            {values[hover].toFixed(1)} Mbps
          </span>
        )}
        <div className="chart-labels">
          <span>
            {period === "30 分钟"
              ? "13:38"
              : period === "24 小时"
                ? "昨天 14:08"
                : "10 月 31 日"}
          </span>
          <span>
            {period === "30 分钟"
              ? "13:48"
              : period === "24 小时"
                ? "02:08"
                : "11 月 2 日"}
          </span>
          <span>
            {period === "30 分钟"
              ? "13:58"
              : period === "24 小时"
                ? "08:08"
                : "11 月 4 日"}
          </span>
          <span>现在</span>
        </div>
      </div>
    </div>
  );
}
export function Periods({
  value,
  onChange,
}: {
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <div className="periods" aria-label="图表时间范围">
      {["30 分钟", "24 小时", "7 天"].map((p) => (
        <button key={p} aria-pressed={value === p} onClick={() => onChange(p)}>
          {p}
        </button>
      ))}
    </div>
  );
}
export function DeviceTable({
  limit = 4,
  dense = false,
  notify,
}: {
  limit?: number;
  dense?: boolean;
  notify: Notify;
}) {
  return (
    <div className={"device-table " + (dense ? "dense" : "")}>
      <div className="table-labels">
        <span>设备</span>
        <span>连接方式</span>
        <span>下载</span>
        <span>上传</span>
      </div>
      {clients.slice(0, limit).map((c) => (
        <button
          className="device-row"
          key={c.name}
          onClick={() =>
            notify(`${c.name} · ${c.ip} · ${c.band}，当前为首页风格预览`)
          }
        >
          <span className="device-identity">
            <span className="device-avatar">
              <DeviceIcon kind={c.kind} />
            </span>
            <span>
              <b>{c.name}</b>
              <small>{c.tag || c.ip}</small>
            </span>
          </span>
          <span className="device-band">
            {c.band === "USB" ? <Cable size={13} /> : <Wifi size={13} />}{" "}
            {c.band}
          </span>
          <span className="number">
            {c.download.toFixed(1)}
            <small> Mbps</small>
          </span>
          <span className="number dim">
            {c.upload.toFixed(1)}
            <small> Mbps</small>
          </span>
        </button>
      ))}
    </div>
  );
}
export function Usage({ compact = false }: { compact?: boolean }) {
  return (
    <div className={"usage " + (compact ? "usage-compact" : "")}>
      <div className="usage-numbers">
        <strong>
          62.8<span>GB</span>
        </strong>
        <span>/ 100 GB</span>
      </div>
      <div className="usage-track">
        <i />
      </div>
      <div className="usage-foot">
        <span>剩余 37.2 GB</span>
        <span>12 月 1 日重置</span>
      </div>
    </div>
  );
}
export function Battery({ compact = false }: { compact?: boolean }) {
  return (
    <div className={"battery-detail " + (compact ? "compact" : "")}>
      <div className="battery-reading">
        <BatteryCharging size={compact ? 24 : 30} />
        <strong>
          82<small>%</small>
        </strong>
      </div>
      <div>
        <b>正在充电</b>
        <p>USB-C 电源已接入</p>
      </div>
    </div>
  );
}
export function Topology({ compact = false }: { compact?: boolean }) {
  return (
    <div className={"topology " + (compact ? "compact" : "")}>
      <div className="topology-node">
        <span className="node-icon">
          <Signal size={23} />
        </span>
        <div>
          <small>互联网</small>
          <b>
            中国联通 <em>5G SA</em>
          </b>
          <span>n78 · −86 dBm</span>
        </div>
      </div>
      <div className="topology-wire">
        <i />
        <small>已联网</small>
      </div>
      <div className="topology-node router-node">
        <span className="node-icon">
          <Router size={25} />
        </span>
        <div>
          <small>移动路由器</small>
          <b>U60 Pro</b>
          <span>正常运行</span>
        </div>
      </div>
      <div className="topology-wire">
        <i />
        <small>双频合一</small>
      </div>
      <div className="topology-node">
        <span className="node-icon">
          <Wifi size={23} />
        </span>
        <div>
          <small>Wi-Fi</small>
          <b>U60 Studio</b>
          <span>6 台设备在线</span>
        </div>
      </div>
    </div>
  );
}
export function ShareModal({ close }: { close: () => void }) {
  const [visible, setVisible] = useState(false),
    [copied, setCopied] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    dialog.current?.showModal();
    return () => dialog.current?.close();
  }, []);
  return (
    <dialog
      ref={dialog}
      className="share-dialog"
      onCancel={close}
      onClick={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <div className="dialog-head">
        <span className="dialog-icon">
          <Wifi size={24} />
        </span>
        <button className="icon-btn" aria-label="关闭" onClick={close}>
          <X size={20} />
        </button>
      </div>
      <h2>连接，从扫描开始</h2>
      <p>用手机相机或 WLAN 扫一扫。</p>
      <QR size={210} />
      <h3>U60 Studio</h3>
      <span className="muted">2.4 GHz 与 5 GHz 共用名称和密码</span>
      <div className="password-line">
        <span>{visible ? "sample-pass-2026" : "••••••••••••••••"}</span>
        <button
          className="icon-btn"
          aria-label={visible ? "隐藏密码" : "显示密码"}
          onClick={() => setVisible(!visible)}
        >
          {visible ? <EyeOff size={18} /> : <Eye size={18} />}
        </button>
        <button
          className="icon-btn"
          aria-label="复制演示密码"
          onClick={() => {
            navigator.clipboard
              ?.writeText("sample-pass-2026")
              .then(() => setCopied(true))
              .catch(() => setCopied(false));
          }}
        >
          {copied ? <Check size={18} /> : <Copy size={18} />}
        </button>
      </div>
      <small className="demo-caption">
        设计预览 · 二维码与密码均为虚构示例
      </small>
    </dialog>
  );
}
export function ServiceList({
  enabled,
  setEnabled,
  notify,
}: {
  enabled: boolean[];
  setEnabled: (index: number) => void;
  notify: Notify;
}) {
  return (
    <div className="service-list">
      {[
        {
          name: "Tailscale",
          description: "团队网络 · 3 条子网路由",
          kind: "tailnet",
        },
        { name: "WireGuard", description: "工作网络 · wg-office", kind: "wg" },
        {
          name: "规则代理",
          description: "规则模式 · 仅热点流量",
          kind: "proxy",
        },
      ].map((s, i) => (
        <div className="service-row" key={s.name}>
          <span className={"service-icon " + s.kind}>
            {i === 0 ? (
              <span className="tailnet-dots">
                {Array.from({ length: 9 }, (_, n) => (
                  <i key={n} />
                ))}
              </span>
            ) : i === 1 ? (
              <ShieldCheck size={19} />
            ) : (
              <Router size={19} />
            )}
          </span>
          <button
            className="service-copy"
            onClick={() =>
              notify(
                `${s.name} 的配置、日志与更新将在同一服务内管理；此处为首页预览`,
              )
            }
          >
            <b>{s.name}</b>
            <span>{enabled[i] ? s.description : "已停止"}</span>
          </button>
          <Toggle
            on={enabled[i]}
            label={`${s.name} 服务`}
            onChange={() => setEnabled(i)}
          />
        </div>
      ))}
    </div>
  );
}
export function SectionEmptyAction({ notify }: { notify: Notify }) {
  return (
    <button
      className="add-service"
      onClick={() => notify("所有服务独立启停，可同时运行；本轮只评审首页")}
    >
      <Plus size={14} /> 管理服务 <ChevronRight size={13} />
    </button>
  );
}
