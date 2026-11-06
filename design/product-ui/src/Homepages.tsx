import { useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  ArrowRight,
  ArrowUpRight,
  BatteryCharging,
  Bell,
  Cable,
  Check,
  CheckCheck,
  ChevronDown,
  ChevronRight,
  CircleHelp,
  Ellipsis,
  Globe2,
  LayoutGrid,
  LockKeyhole,
  Monitor,
  Network,
  Plus,
  QrCode,
  Router,
  Search,
  Settings2,
  ShieldCheck,
  Signal,
  Smartphone,
  Wifi,
  Zap,
} from "lucide-react";
import { type Page, navigation, clients } from "./data";
import { serviceNames } from "./presentation";
import {
  ArrowLink,
  Battery,
  Brand,
  DeviceIcon,
  DeviceTable,
  Panel,
  PanelTitle,
  Periods,
  Rate,
  SectionEmptyAction,
  ServiceList,
  Status,
  Topology,
  TrafficChart,
  Usage,
  type Notify,
  type Variant,
} from "./ui";
type Props = {
  variant: Variant;
  share: () => void;
  notify: Notify;
  enabled: boolean[];
  toggle: (i: number) => void;
  navigate?: (page: Page, tab?: string, service?: number) => void;
  offline?: boolean;
  wifi?: { ssid: string; unified: boolean; enabled: boolean };
  directPower?: boolean;
};
const later = (notify: Notify) => () =>
  notify("本轮先确认首页风格，其余页面在选定后展开设计");
function Sidebar({ variant, notify }: { variant: Variant; notify: Notify }) {
  return (
    <aside
      className={"product-sidebar " + (variant === "c" ? "slim-sidebar" : "")}
    >
      <Brand compact={variant === "c"} />
      {variant !== "c" && (
        <div className="device-switch">
          <span className="device-mini">
            <Router size={18} />
          </span>
          <span>
            <strong>U60 Pro</strong>
            <small>我的移动路由器</small>
          </span>
          <ChevronDown size={14} />
        </div>
      )}
      <nav aria-label="产品导航">
        {navigation.map((item, i) => (
          <div key={item.id}>
            {variant !== "c" &&
              (i === 0 || item.section !== navigation[i - 1].section) && (
                <span className="nav-section">{item.section}</span>
              )}
            <button
              className={"nav-item " + (i === 0 ? "selected" : "")}
              aria-current={i === 0 ? "page" : undefined}
              onClick={i === 0 ? () => {} : later(notify)}
              title={item.label}
            >
              <item.icon size={variant === "c" ? 20 : 18} />
              {variant !== "c" && (
                <>
                  <span>{item.label}</span>
                  {item.id === "messages" && (
                    <small className="nav-count">1</small>
                  )}
                </>
              )}
            </button>
          </div>
        ))}
      </nav>
      <div className="sidebar-foot">
        <span className="account-avatar">L</span>
        {variant !== "c" && (
          <span>
            <b>本地管理员</b>
            <small>
              <LockKeyhole size={10} /> 安全会话
            </small>
          </span>
        )}
        <button className="icon-btn" aria-label="帮助" onClick={later(notify)}>
          <CircleHelp size={17} />
        </button>
      </div>
    </aside>
  );
}
function Header({
  dense = false,
  notify,
}: {
  dense?: boolean;
  notify: Notify;
}) {
  return (
    <header className="product-header">
      <div className="breadcrumb">
        <span>U60 Pro</span>
        <ChevronRight size={12} />
        <b>概览</b>
      </div>
      <div className="header-actions">
        <span className="local-badge">
          <span />
          本地连接 <b>192.0.2.1</b>
        </span>
        <button
          className="icon-btn"
          aria-label="搜索功能"
          onClick={later(notify)}
        >
          <Search size={dense ? 16 : 18} />
        </button>
        <button
          className="icon-btn notification"
          aria-label="消息通知"
          onClick={() => notify("1 条新通知：本月流量已使用 62.8 GB")}
        >
          <Bell size={dense ? 16 : 18} />
          <i />
        </button>
      </div>
    </header>
  );
}
function Heading({
  share,
  dense = false,
  offline = false,
}: {
  share: () => void;
  dense?: boolean;
  offline?: boolean;
}) {
  return (
    <div className="page-title">
      <div>
        <div className="eyebrow">
          {dense ? "NETWORK OVERVIEW" : "你的网络，此刻"}
        </div>
        <h1>
          {offline
            ? "移动网络连接中断。"
            : dense
              ? "网络概览"
              : "连接，一切就绪。"}
          <span className="title-dot" />
        </h1>
        {!dense && (
          <p>
            {offline
              ? "本地管理正常 · 检查移动网络连接"
              : "互联网正常 · 已持续连接 2 天 6 小时"}
          </p>
        )}
      </div>
      <button className="btn primary" onClick={share}>
        <QrCode size={16} /> 分享 Wi-Fi
      </button>
    </div>
  );
}
export function Professional(p: Props) {
  return (
    <div className="product-shell professional">
      <Sidebar variant="a" notify={p.notify} />
      <div className="product-main">
        <Header notify={p.notify} />
        <main>
          <ProfessionalContent {...p} />
        </main>
      </div>
    </div>
  );
}
export function ProfessionalContent(p: Props) {
  const [period, setPeriod] = useState("30 分钟");
  return (
    <>
      <Heading share={p.share} offline={p.offline} />
      <Panel className="network-panel">
        <div className="network-panel-top">
          <span>
            <Globe2 size={15} /> 网络连接
          </span>
          <Status muted={p.offline}>
            {p.offline ? "本地连接正常 · 蜂窝断开" : "所有连接正常"}
          </Status>
        </div>
        <Topology offline={p.offline} wifi={p.wifi} />
      </Panel>
      <div className="dashboard-grid">
        <Panel className="bandwidth-panel">
          <PanelTitle
            title={p.offline ? "连接中断前的速率" : "实时速率"}
            note={
              p.offline
                ? "当前蜂窝流量为 0，保留历史趋势"
                : "你的连接，正在发生什么"
            }
            action={<Periods value={period} onChange={setPeriod} />}
          />
          <div className="rates">
            <Rate value={p.offline ? "0" : "124.6"} />
            <Rate up value={p.offline ? "0" : "14.2"} />
            <span className="chart-legend">
              <i />
              下载 <i className="upload-dot" />
              上传
            </span>
          </div>
          <TrafficChart period={period} />
        </Panel>
        <Panel className="usage-panel">
          <PanelTitle
            title="本月流量"
            action={
              <ArrowLink
                onClick={() =>
                  p.navigate
                    ? p.navigate("internet", "流量套餐")
                    : later(p.notify)()
                }
              >
                套餐
              </ArrowLink>
            }
          />
          <Usage />
          <div className="power-divider" />
          <Battery direct={p.directPower} />
        </Panel>
        <Panel className="devices-panel">
          <PanelTitle
            title="连接设备"
            note="6 台在线 · 5 GHz 优先连接"
            action={
              <ArrowLink
                onClick={() =>
                  p.navigate ? p.navigate("devices") : later(p.notify)()
                }
              >
                全部设备
              </ArrowLink>
            }
          />
          <DeviceTable
            notify={p.notify}
            onOpen={p.navigate ? () => p.navigate!("devices") : undefined}
          />
        </Panel>
        <Panel className="services-panel">
          <PanelTitle
            title="组网与服务"
            action={
              <span className="tiny-counter">
                {p.enabled.filter(Boolean).length} 个运行中
              </span>
            }
          />
          <ServiceList
            enabled={p.enabled}
            setEnabled={p.toggle}
            notify={p.notify}
            onOpen={
              p.navigate ? (i) => p.navigate!("services", "概览", i) : undefined
            }
          />
          {p.navigate ? (
            <button
              className="add-service"
              onClick={() => p.navigate!("services")}
            >
              管理服务 <ChevronRight size={13} />
            </button>
          ) : (
            <SectionEmptyAction notify={p.notify} />
          )}
        </Panel>
      </div>
      <div className="overview-footer">
        <span>
          <ShieldCheck size={13} /> 设备运行正常 · CPU 12% · 39°C
        </span>
        <span>
          数据已更新 <i /> 刚刚
        </span>
      </div>
    </>
  );
}
export function Consumer(p: Props) {
  const [period, setPeriod] = useState("30 分钟");
  return (
    <div className="product-shell consumer">
      <header className="consumer-header">
        <Brand />
        <nav>
          {["概览", "Wi-Fi", "设备", "网络", "服务", "设置"].map((n, i) => (
            <button
              className={i === 0 ? "active" : ""}
              key={n}
              onClick={i === 0 ? () => {} : later(p.notify)}
            >
              {n}
            </button>
          ))}
        </nav>
        <div className="header-actions">
          <button
            className="icon-btn"
            aria-label="通知"
            onClick={() => p.notify("本月套餐剩余 37.2 GB")}
          >
            <Bell size={18} />
          </button>
          <span className="account-avatar">L</span>
        </div>
      </header>
      <main>
        <div className="consumer-greeting">
          <div>
            <span className="eyebrow">U60 PRO · 随身连接</span>
            <h1>你的随身网络。</h1>
            <p>6 台设备在线，所有连接正常。</p>
          </div>
          <Status>网络状态良好</Status>
        </div>
        <div className="consumer-hero">
          <Panel className="consumer-connection">
            <div className="consumer-connection-copy">
              <span className="soft-pill">
                <Signal size={15} /> 中国联通 · 5G SA
              </span>
              <h2>
                5G 网络
                <br />
                连接正常
              </h2>
              <p>
                已稳定运行 2 天 6 小时，
                <br />
                正在使用中国联通移动网络。
              </p>
              <button className="btn primary" onClick={p.share}>
                <QrCode size={17} /> 邀请设备连接
                <ArrowUpRight size={16} />
              </button>
            </div>
            <div className="router-scene" aria-label="U60 移动路由器示意图">
              <div className="orbit orbit-one" />
              <div className="orbit orbit-two" />
              <div className="router-object">
                <div className="object-top">
                  <i />
                  <span>U60</span>
                </div>
                <div className="object-screen">
                  <span>
                    <Signal size={12} /> 5G <BatteryCharging size={14} />
                  </span>
                  <Wifi size={35} />
                  <b>已连接</b>
                  <small>U60 Studio</small>
                  <div>6 台设备在线</div>
                </div>
                <div className="object-logo">ZTE</div>
              </div>
              <span className="floating-wifi">
                <Wifi size={20} />
              </span>
              <span className="floating-shield">
                <Check size={19} />
              </span>
            </div>
            <div className="consumer-network-foot">
              <span>
                <Wifi size={16} />
                <b>U60 Studio</b>
                <small>双频合一</small>
              </span>
              <button onClick={p.share} aria-label="分享网络">
                <ArrowRight size={18} />
              </button>
            </div>
          </Panel>
          <div className="consumer-side">
            <Panel className="consumer-speed">
              <span className="metric-symbol">
                <Zap size={20} />
              </span>
              <p>当前下载速度</p>
              <div className="consumer-rate">
                <strong>124.6</strong>
                <span>Mbps</span>
              </div>
              <div className="mini-wave" aria-hidden="true">
                <svg viewBox="0 0 240 36">
                  <path
                    d="M0 30L10 28L20 31L30 20L40 23L50 15L60 19L70 5L80 15L90 9L100 21L110 18L120 25L130 20L140 13L150 19L160 8L170 12L180 6L190 14L200 3L210 9L220 5L240 12"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                  />
                </svg>
              </div>
              <div className="consumer-speed-foot">
                <ArrowUp size={13} /> 上传 14.2 Mbps<span>很流畅</span>
              </div>
            </Panel>
            <Panel className="consumer-power">
              <Battery />
              <div className="battery-cells">
                {Array.from({ length: 20 }, (_, i) => (
                  <i className={i < 16 ? "charged" : ""} key={i} />
                ))}
              </div>
              <span className="muted">充至 90% 后自动暂停充电</span>
            </Panel>
          </div>
        </div>
        <div className="consumer-lower">
          <Panel className="consumer-devices">
            <PanelTitle
              title="你的设备"
              action={<ArrowLink onClick={later(p.notify)}>查看全部</ArrowLink>}
            />
            <div className="device-orbit-list">
              {clients.slice(0, 4).map((c) => (
                <button
                  key={c.name}
                  onClick={() => p.notify(c.name + " · " + c.band)}
                >
                  <span>
                    <DeviceIcon kind={c.kind} size={25} />
                    <i />
                  </span>
                  <b>{c.name}</b>
                  <small>{c.band}</small>
                </button>
              ))}
              <button onClick={p.share}>
                <span className="add-device-icon">
                  <Plus size={25} />
                </span>
                <b>连接设备</b>
                <small>扫码加入</small>
              </button>
            </div>
            <div className="consumer-service-strip">
              <span>
                <Network size={17} />
                <b>{p.enabled.filter(Boolean).length} 项网络服务</b>
                <small>{serviceNames.join(" · ")}</small>
              </span>
              <ChevronRight size={16} />
            </div>
          </Panel>
          <Panel className="consumer-plan">
            <PanelTitle title="本月流量" note="100 GB 月度套餐" />
            <Usage />
            <span className="plan-caption">路由器统计，仅供套餐管理参考。</span>
          </Panel>
        </div>
        <Panel className="consumer-history">
          <PanelTitle
            title="连接趋势"
            action={<Periods value={period} onChange={setPeriod} />}
          />
          <TrafficChart dense period={period} />
        </Panel>
        <div className="consumer-footer">
          <span>
            <LockKeyhole size={12} /> 仅在你的本地网络中管理
          </span>
          <span>U60 Dash</span>
        </div>
      </main>
    </div>
  );
}
export function Compact(p: Props) {
  const [period, setPeriod] = useState("30 分钟");
  return (
    <div className="product-shell compact-console">
      <Sidebar variant="c" notify={p.notify} />
      <div className="product-main">
        <Header dense notify={p.notify} />
        <main>
          <Heading dense share={p.share} />
          <div className="compact-status-line">
            <Status>系统正常</Status>
            <span>中国联通 · 5G SA</span>
            <span>
              运行时间 <b>2d 06h 14m</b>
            </span>
            <span>
              设备温度 <b>39°C</b>
            </span>
            <span>
              CPU <b>12%</b>
            </span>
            <span>
              内存 <b>43%</b>
            </span>
          </div>
          <div className="metric-strip">
            {[
              {
                title: "下载速率",
                value: "124.6",
                unit: "Mbps",
                detail: "↓ 过去 30 分钟峰值 156.2",
                icon: ArrowDown,
              },
              {
                title: "上传速率",
                value: "14.2",
                unit: "Mbps",
                detail: "↑ 过去 30 分钟峰值 21.8",
                icon: ArrowUp,
              },
              {
                title: "本月流量",
                value: "62.8",
                unit: "GB",
                detail: "套餐 100 GB · 剩余 37.2 GB",
                icon: Globe2,
              },
              {
                title: "在线设备",
                value: "6",
                unit: "台",
                detail: "5 GHz 3 · 2.4 GHz 2 · USB 1",
                icon: Monitor,
              },
              {
                title: "电池电量",
                value: "82",
                unit: "%",
                detail: "USB-C 已连接 · 充电中",
                icon: BatteryCharging,
              },
            ].map((item) => (
              <div key={item.title}>
                <span>
                  <item.icon size={14} />
                  {item.title}
                </span>
                <strong>
                  {item.value}
                  <small>{item.unit}</small>
                </strong>
                <p>{item.detail}</p>
              </div>
            ))}
          </div>
          <div className="compact-grid">
            <Panel className="compact-traffic">
              <PanelTitle
                title="吞吐量"
                action={<Periods value={period} onChange={setPeriod} />}
              />
              <div className="chart-subtitle">
                <span>
                  <i /> 下载
                </span>
                <span>
                  <i className="upload-dot" /> 上传
                </span>
                <small>单位 Mbps · {period}</small>
              </div>
              <TrafficChart dense period={period} />
              <div className="compact-interfaces">
                <span>
                  <Signal size={14} />
                  <b>蜂窝网络</b>
                  <Status>已连接</Status>
                </span>
                <span>
                  WAN <b>192.0.2.86</b>
                </span>
                <span>
                  DNS <b>加密解析</b>
                  <ShieldCheck size={12} />
                </span>
              </div>
            </Panel>
            <Panel className="compact-radio">
              <PanelTitle
                title="蜂窝链路"
                action={<span className="soft-pill">5G SA</span>}
              />
              <div className="radio-grid">
                {[
                  ["运营商", "中国联通"],
                  ["频段", "n78"],
                  ["RSRP", "−86 dBm"],
                  ["SINR", "24 dB"],
                  ["RSRQ", "−9 dB"],
                  ["PCI", "312"],
                ].map(([k, v]) => (
                  <div key={k}>
                    <span>{k}</span>
                    <strong>{v}</strong>
                  </div>
                ))}
              </div>
              <div className="signal-quality">
                <span>信号质量</span>
                <div>
                  {Array.from({ length: 18 }, (_, i) => (
                    <i key={i} className={i < 14 ? "active" : ""} />
                  ))}
                </div>
                <b>良好</b>
              </div>
            </Panel>
            <Panel className="compact-clients">
              <PanelTitle
                title="在线设备"
                action={
                  <ArrowLink onClick={later(p.notify)}>6 台设备</ArrowLink>
                }
              />
              <DeviceTable limit={6} dense notify={p.notify} />
            </Panel>
            <Panel className="compact-services">
              <PanelTitle
                title="网络服务"
                action={
                  <button
                    className="icon-btn"
                    aria-label="服务设置"
                    onClick={later(p.notify)}
                  >
                    <Settings2 size={15} />
                  </button>
                }
              />
              <ServiceList
                enabled={p.enabled}
                setEnabled={p.toggle}
                notify={p.notify}
              />
              <div className="route-summary">
                <span>
                  远端子网 <b>3</b>
                </span>
                <span>
                  隧道 <b>2</b>
                </span>
                <span>
                  路由策略 <b>分流</b>
                </span>
              </div>
              <PanelTitle title="本地网络" />
              <div className="compact-wifi">
                <Wifi size={18} />
                <span>
                  <b>U60 Studio</b>
                  <small>2.4 / 5 GHz · WPA2 / WPA3</small>
                </span>
                <Status>开启</Status>
              </div>
            </Panel>
          </div>
          <Panel className="event-strip">
            <span>
              <CheckCheck size={16} /> 最近事件
            </span>
            <b>14:02</b>
            <p>iPhone 16 已连接至 5 GHz 网络</p>
            <span className="event-secondary">
              13:56 · Tailscale 子网路由已就绪
            </span>
            <button
              className="icon-btn"
              aria-label="事件详情"
              onClick={later(p.notify)}
            >
              <ChevronRight size={16} />
            </button>
          </Panel>
          <div className="overview-footer">
            <span>
              <LockKeyhole size={12} /> 本地管理 · 服务独立运行
            </span>
            <span>
              <i /> 实时连接正常
            </span>
          </div>
        </main>
      </div>
    </div>
  );
}
