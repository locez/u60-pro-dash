import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  ArrowLeft,
  BatteryCharging,
  Check,
  ChevronRight,
  Cpu,
  Eye,
  EyeOff,
  Fingerprint,
  LayoutDashboard,
  LockKeyhole,
  MessageSquare,
  Network,
  Plane,
  SlidersHorizontal,
  Thermometer,
  TrainFront,
  Settings2,
  ShieldCheck,
  Signal,
  Smartphone,
  Wifi,
  X,
  Zap,
} from "lucide-react";
import ScreenModes from "./ScreenModes";
import { ProfileIcon } from "./ModeControls";
import { bandwidthModes } from "./modes";
import { clients } from "./data";
import { features, serviceNames, useModel } from "./model";
import { QR } from "./ui";
import { sampleMessages } from "./ManagementPages";
import AccentPicker, { screenAccentStyle } from "./AccentPicker";
import HomeLayoutSettings from "./HomeLayoutSettings";
import { metricOptions, screenThemes, serviceIds } from "./screenPreferences";
import { proxyRuntimeInterruption, wifiInterruption } from "./settingsBehavior";

type ScreenRoot = "状态" | "连接" | "管理";
function Row({
  title,
  value,
  onClick,
  icon,
}: {
  title: string;
  value?: string;
  onClick: () => void;
  icon?: ReactNode;
}) {
  return (
    <button className="native-row" onClick={onClick}>
      {icon}
      <span>
        <b>{title}</b>
        {value && <small>{value}</small>}
      </span>
      <ChevronRight size={14} />
    </button>
  );
}
function Facts({ values }: { values: [string, string][] }) {
  return (
    <div className="native-facts">
      {values.map(([key, value]) => (
        <div key={key}>
          <span>{key}</span>
          <b>{value}</b>
        </div>
      ))}
    </div>
  );
}
function NativeChart() {
  return (
    <svg
      className="native-chart"
      viewBox="0 0 270 55"
      role="img"
      aria-label="最近 30 分钟流量趋势"
    >
      <path d="M0 18H270M0 40H270" stroke="var(--screen-line)" />
      <path
        d="M0 44 16 36 32 39 48 29 64 34 80 21 96 30 112 22 128 25 144 15 160 21 176 12 192 18 208 13 224 5 240 12 256 8 270 11"
        fill="none"
        stroke="var(--screen-accent-ink)"
        strokeWidth="1.8"
      />
      <path
        d="M0 51 16 49 32 50 48 45 64 48 80 42 96 45 112 40 128 42 144 40 160 41 176 37 192 41 208 36 224 38 240 34 256 36 270 33"
        fill="none"
        stroke="var(--screen-good)"
        strokeWidth="1.5"
      />
    </svg>
  );
}
export default function ScreenProduct() {
  const m = useModel(),
    [root, setRoot] = useState<ScreenRoot>("状态"),
    [path, setPath] = useState<string[]>([]),
    [locked, setLocked] = useState(false),
    [quickControls, setQuickControls] = useState(false),
    [holding, setHolding] = useState(false),
    [pinEntry, setPinEntry] = useState(false),
    [pin, setPin] = useState(""),
    [pinError, setPinError] = useState(false),
    [visible, setVisible] = useState(false),
    [sheet, setSheet] = useState<{
      title: string;
      options: string[];
      selected?: string;
      choose: (s: string) => void;
    } | null>(null),
    [screenToast, setScreenToast] = useState(""),
    [systemDark, setSystemDark] = useState(
      () => matchMedia("(prefers-color-scheme: dark)").matches,
    ),
    [pendingChange, setPendingChange] = useState<{
      title: string;
      summary: string;
      message: string;
      apply: () => void;
    } | null>(null);
  const hold = useRef<ReturnType<typeof setTimeout> | null>(null),
    pane = useRef<HTMLDivElement>(null);
  const title = path.at(-1) || root;
  const v = m.modes;
  const cellularUnavailable =
    v.airplane || v.unknown || v.busy || m.scenario === "offline";
  const requestAirplane = () => {
    if (v.airplane) {
      v.changeAirplane(false);
      return;
    }
    setQuickControls(false);
    setPendingChange({
      title: "开启飞行模式？",
      summary: "蜂窝互联网、通话与短信将中断。",
      message:
        "Wi-Fi、USB 和本地管理保留；依赖蜂窝的远程访问会断开。可在机身或本地 Web 恢复蜂窝。",
      apply: () => v.changeAirplane(true),
    });
  };
  const chooseBandwidth = () =>
    setSheet({
      title: "应用带宽策略",
      selected: v.strategy.label,
      options: bandwidthModes.map((p) => p.label),
      choose: (label) => {
        v.changeBandwidth(bandwidthModes.find((p) => p.label === label)!.id);
        setSheet(null);
      },
    });
  const modeControls = (compact = false) => (
    <ScreenModes
      compact={compact}
      requestAirplane={requestAirplane}
      chooseBandwidth={chooseBandwidth}
      openDetails={() => {
        setQuickControls(false);
        quick("管理", ["运行模式"]);
      }}
    />
  );
  const accent = String(m.settings.security.accent);
  const dark =
    m.settings.security.theme === "跟随系统"
      ? systemDark
      : m.settings.security.theme !== "浅色";
  const setTheme = (theme: string) =>
    m.save("security", { ...m.settings.security, theme });
  const homeMetrics = m.homeLayout.metrics.flatMap((id) =>
    metricOptions.filter((metric) => metric.id === id),
  );
  const homeServices = serviceIds
    .map((id, index) => ({ id, index, name: serviceNames[index] }))
    .filter(({ id }) => m.homeLayout.serviceVisibility[id]);
  const setAccent = (color: string) =>
    m.save("security", { ...m.settings.security, accent: color });
  useEffect(() => {
    const query = matchMedia("(prefers-color-scheme: dark)");
    const update = () => setSystemDark(query.matches);
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);
  const open = (s: string) => {
    setPath((p) => [...p, s]);
    setVisible(false);
  };
  const quick = (r: ScreenRoot, p: string[] = []) => {
    setLocked(false);
    setPinEntry(false);
    setRoot(r);
    setPath(p);
    setVisible(false);
  };
  useEffect(() => {
    pane.current?.scrollTo(0, 0);
  }, [title, root]);
  useEffect(() => {
    if (!screenToast) return;
    const id = setTimeout(() => setScreenToast(""), 2500);
    return () => clearTimeout(id);
  }, [screenToast]);
  useEffect(
    () => () => {
      if (hold.current) clearTimeout(hold.current);
    },
    [],
  );
  const stopHold = () => {
    if (hold.current) clearTimeout(hold.current);
    hold.current = null;
    setHolding(false);
  };
  const startHold = () => {
    if (hold.current) return;
    setHolding(true);
    hold.current = setTimeout(() => {
      hold.current = null;
      setHolding(false);
      if (m.settings.security.pin) setPinEntry(true);
      else setLocked(false);
    }, 1000);
  };
  const choose = (
    label: string,
    options: string[],
    apply: (s: string) => void | boolean,
  ) =>
    setSheet({
      title: label,
      options,
      choose: (s) => {
        const applied = apply(s);
        setSheet(null);
        if (applied !== false) setScreenToast("已应用示例设置");
      },
    });
  const serviceIndex = path.findIndex((p) => serviceNames.includes(p));
  const serviceId =
    serviceIndex < 0 ? -1 : serviceNames.indexOf(path[serviceIndex]);
  const editRuntime = (key: "mode" | "scope") => {
    setSheet({
      title: key === "mode" ? "运行模式" : "流量范围",
      selected: String(m.settings.proxyRuntime[key]),
      options:
        key === "mode"
          ? ["规则", "全局", "直连"]
          : ["Wi-Fi 热点", "仅本机 HTTP / SOCKS"],
      choose: (value) => {
        setSheet(null);
        if (m.settings.proxyRuntime[key] === value) return;
        const next = { ...m.settings.proxyRuntime, [key]: value };
        const message = proxyRuntimeInterruption(
          next,
          m.settings.proxyRuntime,
          m.enabled[2],
        );
        const apply = () => {
          m.save("proxyRuntime", next);
          setScreenToast("已应用示例运行设置");
        };
        if (message) {
          setScreenToast("");
          setPendingChange({
            title: "应用运行设置",
            summary: `${key === "mode" ? "运行模式" : "流量范围"}：${value}`,
            message,
            apply,
          });
        } else apply();
      },
    });
  };
  const editWifi = (
    key: string,
    value: string | boolean,
    done?: () => void,
  ) => {
    if (m.settings.wifi[key] === value) {
      done?.();
      return false;
    }
    const next = { ...m.settings.wifi, [key]: value };
    const message = wifiInterruption(next, m.settings.wifi);
    const apply = () => {
      m.save("wifi", next);
      setScreenToast("已应用示例 Wi-Fi 设置");
      done?.();
    };
    if (message) {
      setScreenToast("");
      setPendingChange({
        title: "应用 Wi-Fi 设置",
        summary: "无线网络连接将受影响",
        message,
        apply,
      });
      return false;
    }
    apply();
    return true;
  };
  const renderService = () => {
    if (title === "网络设置")
      return (
        <>
          <Row
            title="接受子网路由"
            value={m.settings.tailscale.routes ? "已开启" : "已关闭"}
            onClick={() =>
              m.save("tailscale", {
                ...m.settings.tailscale,
                routes: !m.settings.tailscale.routes,
              })
            }
          />
          <Row
            title="热点访问远端子网"
            value={m.settings.tailscale.lan ? "已开启" : "已关闭"}
            onClick={() =>
              m.save("tailscale", {
                ...m.settings.tailscale,
                lan: !m.settings.tailscale.lan,
              })
            }
          />
          <Row
            title="网络 DNS"
            value={m.settings.tailscale.dns ? "已开启" : "已关闭"}
            onClick={() =>
              m.save("tailscale", {
                ...m.settings.tailscale,
                dns: !m.settings.tailscale.dns,
              })
            }
          />
        </>
      );
    if (title === "子网路由")
      return (
        <>
          <p className="native-note">其他节点发布的路由</p>
          <Facts
            values={[
              [
                "192.0.2.0/24",
                m.settings.tailscale.routes ? "已接受" : "未接受",
              ],
              ["198.51.100.0/24", "已接受"],
              ["203.0.113.0/24", "已接受"],
            ]}
          />
          <p className="native-note">
            路由已接受不代表目标服务可达，请单独检测。
          </p>
        </>
      );
    if (title === "连通性检测")
      return (
        <>
          <div className="native-hero">
            <Network size={28} />
            <h2>尚未检测</h2>
            <p>{String(m.settings.tailscale.target)}</p>
          </div>
          <button
            className="native-primary"
            disabled={!m.enabled[0]}
            onClick={() => setScreenToast("示例结果：目标可达 · 38 ms")}
          >
            检测目标
          </button>
          <Row
            title="修改检测目标"
            value={String(m.settings.tailscale.target)}
            onClick={() => open("编辑检测目标")}
          />
        </>
      );
    if (title === "编辑检测目标")
      return (
        <NativeEditor
          label="主机与 TCP 端口"
          value={String(m.settings.tailscale.target)}
          save={(value) => {
            m.save("tailscale", { ...m.settings.tailscale, target: value });
            setPath((p) => p.slice(0, -1));
          }}
        />
      );
    if (title === "日志")
      return (
        <div className="native-logs">
          14:08:32 INFO 已启动{"\n"}14:08:32 INFO 已读取配置{"\n"}14:08:33 INFO
          网络已连接{"\n"}14:08:34 INFO 路由已更新{"\n"}14:08:35 INFO
          服务运行正常
        </div>
      );
    if (title === "安装与更新")
      return (
        <>
          <Facts
            values={[
              ["当前版本", serviceId === 0 ? "1.104.4" : "稳定版"],
              ["设备平台", "Linux ARM64"],
            ]}
          />
          <button
            className="native-primary"
            onClick={() => setScreenToast("示例结果：已是最新版本")}
          >
            检查更新
          </button>
          <p className="native-note">自动选择适配本机的安装包。</p>
        </>
      );
    if (title === "策略组")
      return (
        <>
          {["默认流量", "工作流量", "视频服务", "本地网络"].map((s, i) => (
            <Row
              key={s}
              title={s}
              value={String(
                m.settings.screenGroups?.[s] ||
                  ["自动选择", "美国", "香港", "直连"][i],
              )}
              onClick={() =>
                choose(
                  s,
                  ["自动选择", "香港", "日本", "美国", "直连"],
                  (value) =>
                    m.save("screenGroups", {
                      ...m.settings.screenGroups,
                      [s]: value,
                    }),
                )
              }
            />
          ))}
        </>
      );
    if (title === "配置文件")
      return (
        <>
          <Facts values={[["已保存配置", "studio.yaml"]]} />
          <Row
            title="运行模式"
            value={String(m.settings.proxyRuntime.mode)}
            onClick={() => editRuntime("mode")}
          />
          <Row
            title="流量范围"
            value={String(m.settings.proxyRuntime.scope)}
            onClick={() => editRuntime("scope")}
          />
          <p className="native-note">
            选项修改后自动应用；需要停止服务时会提示。
          </p>
          <p className="native-note">
            长配置与密钥可在 Web 管理中编辑：
            <br />
            192.0.2.1:2026
          </p>
        </>
      );
    if (title === "隧道配置")
      return (
        <>
          <Facts
            values={[
              ["接口", "wg-office"],
              ["本地地址", "10.80.0.2/32"],
              ["对端", "1 个"],
              ["MTU", "1420"],
            ]}
          />
          <p className="native-note">
            长配置与密钥可在 Web 管理中编辑：
            <br />
            192.0.2.1:2026
          </p>
        </>
      );
    return (
      <>
        <div className="native-service-hero">
          <span className="native-service-mark">
            <Network size={23} />
          </span>
          <div>
            <b>{serviceNames[serviceId]}</b>
            <p>
              <i className={m.enabled[serviceId] ? "on" : ""} />
              {m.enabled[serviceId] ? "运行中" : "已停止"}
            </p>
          </div>
          <button
            className={"native-switch " + (m.enabled[serviceId] ? "on" : "")}
            aria-label={`屏幕 ${serviceNames[serviceId]} 服务`}
            role="switch"
            aria-checked={m.enabled[serviceId]}
            onClick={() => m.toggle(serviceId)}
          >
            <i />
          </button>
        </div>
        <Facts
          values={
            serviceId === 0
              ? [
                  ["节点", "u60-studio"],
                  ["节点地址", "100.64.0.10"],
                ]
              : serviceId === 1
                ? [
                    ["接口", "wg-office"],
                    ["最近握手", "32 秒前"],
                  ]
                : [
                    ["运行模式", String(m.settings.proxyRuntime.mode)],
                    ["当前配置", "studio.yaml"],
                  ]
          }
        />
        {(serviceId === 0
          ? ["网络设置", "子网路由", "连通性检测", "日志", "安装与更新"]
          : serviceId === 1
            ? ["隧道配置", "日志"]
            : ["配置文件", "策略组", "日志", "安装与更新"]
        ).map((s) => (
          <Row key={s} title={s} onClick={() => open(s)} />
        ))}
      </>
    );
  };
  const renderPage = (): ReactNode => {
    if (title === "运行模式") return modeControls();
    if (serviceId >= 0) return renderService();
    if (title === "短信")
      return (
        <>
          {sampleMessages.map((s) => (
            <Row
              key={s.id}
              title={s.from}
              value={s.preview}
              icon={<MessageSquare size={18} />}
              onClick={() => open(s.id === 2 ? "长短信" : "短信正文")}
            />
          ))}
          <Row title="发送短信" onClick={() => open("发送短信")} />
        </>
      );
    if (title === "长短信" || title === "短信正文")
      return (
        <article className="native-sms">
          <header>
            <b>{title === "长短信" ? "服务通知" : "中国联通"}</b>
            <span>{title === "长短信" ? "昨天 11:20" : "今天 14:02"}</span>
          </header>
          <p>{sampleMessages[title === "长短信" ? 1 : 0].text}</p>
          <button className="native-primary" onClick={() => open("发送短信")}>
            回复
          </button>
        </article>
      );
    if (title === "发送短信")
      return (
        <NativeEditor
          label="收件人与短信内容"
          value=""
          multiline
          save={() => {
            setScreenToast("已模拟发送");
            setPath((p) => p.slice(0, -1));
          }}
        />
      );
    if (title === "Wi-Fi")
      return (
        <>
          <Row
            title={String(m.settings.wifi.ssid)}
            value={
              m.settings.wifi.unified
                ? "双频合一 · 5 台无线设备"
                : "2.4 GHz 与 5 GHz 分别设置"
            }
            icon={<Wifi size={20} />}
            onClick={() => open("连接二维码")}
          />
          <Row
            title="开启 Wi-Fi"
            value={m.settings.wifi.enabled ? "已开启" : "已关闭"}
            onClick={() =>
              choose("开启 Wi-Fi", ["开启", "关闭"], (s) =>
                editWifi("enabled", s === "开启"),
              )
            }
          />
          <Row
            title="双频合一"
            value={m.settings.wifi.unified ? "已开启" : "已关闭"}
            onClick={() =>
              choose("双频合一", ["开启", "关闭"], (s) =>
                editWifi("unified", s === "开启"),
              )
            }
          />
          <Row
            title="网络名称"
            value={String(m.settings.wifi.ssid)}
            onClick={() => open("编辑网络名称")}
          />
          <Row
            title="访客网络"
            value={m.settings.guest.enabled ? "已开启" : "已关闭"}
            onClick={() =>
              choose("访客网络", ["开启", "关闭"], (s) =>
                m.save("guest", { ...m.settings.guest, enabled: s === "开启" }),
              )
            }
          />
          <Row
            title="无线参数"
            value="信道、带宽与发射功率"
            onClick={() => open("无线参数")}
          />
        </>
      );
    if (title === "编辑网络名称")
      return (
        <NativeEditor
          label="网络名称"
          value={String(m.settings.wifi.ssid)}
          save={(value) => {
            if (!value.trim()) return;
            editWifi("ssid", value, () => setPath((p) => p.slice(0, -1)));
          }}
        />
      );
    if (title === "连接二维码")
      return (
        <>
          {(m.settings.wifi.unified ? ["双频合一"] : ["2.4 GHz", "5 GHz"]).map(
            (band, i) => (
              <div className="native-share-card" key={band}>
                <QR
                  size={106}
                  ssid={String(
                    i === 1 ? m.settings.wifi.ssid5 : m.settings.wifi.ssid,
                  )}
                  password={String(m.settings.wifi.password)}
                />
                <div>
                  <small>{band}</small>
                  <b>
                    {String(
                      i === 1 ? m.settings.wifi.ssid5 : m.settings.wifi.ssid,
                    )}
                  </b>
                </div>
              </div>
            ),
          )}
          <p className="native-note">用手机相机或 WLAN 扫描连接。</p>
        </>
      );
    if (title === "连接设备")
      return (
        <>
          {clients
            .filter((c) => c.online)
            .map((c) => (
              <Row
                key={c.ip}
                title={c.name}
                value={`${c.band} · ↓ ${c.download} Mbps`}
                onClick={() => open(c.name)}
                icon={<Smartphone size={18} />}
              />
            ))}
        </>
      );
    const client = clients.find((c) => c.name === title);
    if (client)
      return (
        <Facts
          values={[
            ["设备", client.name],
            ["连接", client.band],
            ["地址", client.ip],
            ["下载", client.download + " Mbps"],
            ["上传", client.upload + " Mbps"],
          ]}
        />
      );
    if (title === "移动网络")
      return (
        <>
          <div className="native-network-title">
            <b>中国联通</b>
            <span>5G SA</span>
          </div>
          <div className="native-metric-grid">
            <div>
              <small>RSRP</small>
              <b>
                −86<em>dBm</em>
              </b>
            </div>
            <div>
              <small>SINR</small>
              <b>
                24<em>dB</em>
              </b>
            </div>
          </div>
          <NativeChart />
          <Facts
            values={[
              ["频段 / 带宽", "n78 / 100 MHz"],
              ["连接状态", "已注册"],
            ]}
          />
          <Row
            title="飞行模式"
            value={v.airplane ? "已开启" : "已关闭"}
            onClick={() => open("运行模式")}
          />
          {["连接设置", "信号与频段", "流量套餐"].map((s) => (
            <Row key={s} title={s} onClick={() => open(s)} />
          ))}
        </>
      );
    if (title === "SIM 卡")
      return (
        <>
          <Facts
            values={[
              ["运营商", "中国联通"],
              [
                "卡片类型",
                m.scenario === "euicc" ? "可管理 eUICC" : "普通 SIM",
              ],
              ["注册状态", "已注册"],
              ["PIN 保护", "已关闭"],
            ]}
          />
          <Row
            title="SIM 安全"
            value="PIN 与 PUK"
            onClick={() => open("SIM 安全")}
          />
          <Row
            title="eSIM 配置"
            value={m.scenario === "euicc" ? "1 个配置" : "当前卡片不支持下载"}
            onClick={() => open("eSIM 配置")}
          />
        </>
      );
    if (title === "电源与电池")
      return (
        <>
          <div className="native-power">
            <BatteryCharging size={34} />
            <b>
              82<small>%</small>
            </b>
            <span>
              {m.settings.power.mode === "直供电" ? "直供电" : "正在充电"}
            </span>
          </div>
          <Facts
            values={[
              ["电池温度", "35°C"],
              ["外部电源", "已接入"],
            ]}
          />
          <Row
            title="供电模式"
            value={String(m.settings.power.mode)}
            onClick={() =>
              choose("供电模式", ["智能充电", "直供电"], (mode) =>
                m.save("power", { ...m.settings.power, mode }),
              )
            }
          />
          <Row
            title="充电上限"
            value={String(m.settings.power.limit)}
            onClick={() =>
              choose("充电上限", ["80%", "90%", "100%"], (limit) =>
                m.save("power", { ...m.settings.power, limit }),
              )
            }
          />
          <Row
            title="性能模式"
            value={v.unknown ? "状态未知" : v.profile.label}
            onClick={() => open("运行模式")}
          />
          <Row
            title="自动熄屏"
            value={String(m.settings.power.sleep)}
            onClick={() =>
              choose("自动熄屏", ["30 秒", "1 分钟", "3 分钟"], (sleep) =>
                m.save("power", { ...m.settings.power, sleep }),
              )
            }
          />
        </>
      );
    if (title === "首页布局")
      return (
        <HomeLayoutSettings value={m.homeLayout} onChange={m.setHomeLayout} />
      );
    if (title === "强调色")
      return (
        <>
          <AccentPicker value={accent} onChange={setAccent} />
          <p className="native-note">
            选中后即时生效。主要操作与重点读数使用强调色。
          </p>
        </>
      );
    if (title === "屏幕与安全")
      return (
        <>
          <Row
            title="首页布局"
            value="模块、快捷指标与服务摘要"
            onClick={() => open("首页布局")}
            icon={<LayoutDashboard size={18} />}
          />
          <Row
            title="界面外观"
            value={String(m.settings.security.theme)}
            onClick={() => choose("界面外观", [...screenThemes], setTheme)}
          />
          <Row
            title="强调色"
            value={accent.toUpperCase()}
            onClick={() => open("强调色")}
            icon={<span className="native-color-chip" />}
          />
          <Row
            title="密码解锁"
            value={
              m.settings.security.pin
                ? "已开启 · 长按后输入 PIN"
                : "已关闭 · 长按 1 秒解锁"
            }
            onClick={() =>
              m.save("security", {
                ...m.settings.security,
                pin: !m.settings.security.pin,
              })
            }
          />
          <Row
            title="立即锁屏"
            onClick={() => {
              setLocked(true);
              setVisible(false);
            }}
            icon={<LockKeyhole size={18} />}
          />
          <Row
            title="界面语言"
            value={String(m.settings.security.language)}
            onClick={() =>
              choose(
                "语言偏好",
                ["简体中文", "繁體中文", "English", "日本語", "한국어"],
                (language) =>
                  m.save("security", { ...m.settings.security, language }),
              )
            }
          />
          <Row
            title="自动锁屏"
            value={String(m.settings.security.timeout)}
            onClick={() =>
              choose("自动锁屏", ["30 秒", "1 分钟", "3 分钟"], (timeout) =>
                m.save("security", { ...m.settings.security, timeout }),
              )
            }
          />
          <p className="native-note">
            长按解锁用来避免误触；开启 PIN 后才会校验身份。
          </p>
        </>
      );
    if (title === "诊断工具")
      return (
        <>
          <div className="native-three-stats">
            <span>
              CPU<b>12%</b>
            </span>
            <span>
              内存<b>44%</b>
            </span>
            <span>
              温度<b>39°</b>
            </span>
          </div>
          {["当前进程", "互联网测速", "局域网测速", "AT 终端"].map((s) => (
            <Row key={s} title={s} onClick={() => open(s)} />
          ))}
        </>
      );
    if (title === "当前进程")
      return (
        <>
          <div className="native-table-header">
            <span>进程</span>
            <span>CPU · 内存</span>
          </div>
          {[
            ["zte-agent", "0.8% · 18 MB"],
            ["u60-screen", "0.3% · 26 MB"],
            ["tailscaled", "1.2% · 42 MB"],
          ].map(([name, value]) => (
            <div className="native-process" key={name}>
              <b>{name}</b>
              <span>{value}</span>
            </div>
          ))}
        </>
      );
    if (title === "设备设置")
      return (
        <>
          <Facts
            values={[
              ["型号", "U60 Pro / MU5250"],
              ["固件", "B31"],
              ["运行时间", "2 天 6 小时"],
            ]}
          />
          {[
            "项目更新",
            "配置备份",
            "管理访问",
            "计划任务",
            "运营商服务",
            "高级网络",
          ].map((s) => (
            <Row key={s} title={s} onClick={() => open(s)} />
          ))}
        </>
      );
    if (title === "项目更新")
      return (
        <>
          <Facts
            values={[
              ["当前版本", "开发预览版"],
              ["更新范围", "后台 + 屏幕"],
            ]}
          />
          <button
            className="native-primary"
            onClick={() => setScreenToast("已完成示例检查")}
          >
            检查更新
          </button>
        </>
      );
    if (title === "流量趋势" || title === "流量套餐")
      return (
        <>
          <div className="native-metric-grid">
            <div>
              <small>
                {title === "流量套餐" ? "本月 / GB" : "下载 / Mbps"}
              </small>
              <b>{title === "流量套餐" ? "62.8" : "124.6"}</b>
            </div>
            <div>
              <small>
                {title === "流量套餐" ? "剩余 / GB" : "上传 / Mbps"}
              </small>
              <b>{title === "流量套餐" ? "37.2" : "14.2"}</b>
            </div>
          </div>
          <NativeChart />
          <Facts
            values={[
              ["统计周期", "11.01 — 11.30"],
              ["套餐总量", "100 GB"],
            ]}
          />
        </>
      );
    if (
      title === "无线参数" ||
      title === "连接设置" ||
      title === "信号与频段" ||
      title === "高级网络" ||
      title === "运营商服务"
    ) {
      const names =
        title === "无线参数"
          ? ["2.4 GHz 信道", "5 GHz 信道", "发射功率"]
          : title === "连接设置"
            ? ["蜂窝数据", "首选网络", "APN 配置", "运营商选择", "断网自动恢复"]
            : title === "信号与频段"
              ? ["5G 频段偏好", "4G 频段偏好", "小区锁定", "邻区扫描"]
              : title === "高级网络"
                ? ["LAN 与 DHCP", "DNS 解析", "访问与转发"]
                : ["通话", "USSD 服务", "SIM 工具菜单"];
      return (
        <>
          {names.map((s) => (
            <Row key={s} title={s} value="查看设置" onClick={() => open(s)} />
          ))}
        </>
      );
    }
    const f = features.find((f) => f.name === title || f.section === title);
    return (
      <>
        <div className="native-hero">
          <Settings2 size={26} />
          <h2>{title}</h2>
        </div>
        <p className="native-note">完整配置与长文本编辑可在 Web 管理中完成。</p>
        <div className="native-web-address">192.0.2.1:2026</div>
        <p className="native-note">{f ? `${f.tab} / ${f.name}` : title}</p>
      </>
    );
  };
  return (
    <main className="selected-screen-review">
      <div className="screen-review-heading">
        <span className="eyebrow">C · 紧凑高密度</span>
        <h1>掌心里的控制台。</h1>
        <p>320 × 480 原始尺寸 · 状态优先 · 按对象逐层展开</p>
      </div>
      <div className="screen-review-layout">
        <aside className="screen-presets">
          <h3>快速查看</h3>
          {[
            ["首页", "状态", []],
            ["运行模式", "管理", ["运行模式"]],
            ["首页布局", "管理", ["屏幕与安全", "首页布局"]],
            ["Wi-Fi 与连接码", "连接", ["Wi-Fi"]],
            ["Tailscale", "连接", ["Tailscale"]],
            ["长短信", "管理", ["短信", "长短信"]],
            ["电源与电池", "管理", ["电源与电池"]],
            ["当前进程", "管理", ["诊断工具", "当前进程"]],
          ].map(([name, r, p]) => (
            <button
              key={String(name)}
              onClick={() => quick(r as ScreenRoot, p as string[])}
            >
              {String(name)}
              <ChevronRight size={14} />
            </button>
          ))}
          <button
            onClick={() => {
              setQuickControls(false);
              setPendingChange(null);
              setSheet(null);
              setLocked(true);
              setPinEntry(false);
              setPin("");
              setVisible(false);
            }}
          >
            状态锁屏
            <LockKeyhole size={14} />
          </button>
        </aside>
        <div
          className="screen-frame screen-c interactive-screen"
          data-screen-theme={dark ? "dark" : "light"}
          style={screenAccentStyle(accent, dark)}
        >
          <div className="screen-hardware">
            <div className="screen-face" data-testid="native-screen">
              <div
                className="screen-statusbar"
                role={locked ? undefined : "button"}
                tabIndex={locked ? undefined : 0}
                aria-label={locked ? "锁屏状态栏" : "打开快捷控制"}
                onClick={() => {
                  if (!locked) setQuickControls(true);
                }}
                onKeyDown={(e) => {
                  if (!locked && (e.key === "Enter" || e.key === " ")) {
                    e.preventDefault();
                    setQuickControls(true);
                  }
                }}
              >
                <span className="native-brand">
                  {locked && <LockKeyhole size={12} />} U60
                </span>
                <span>
                  {v.airplane ? (
                    <Plane size={15} aria-label="飞行模式" />
                  ) : v.unknown ? (
                    <span>—</span>
                  ) : (
                    <>
                      <Signal size={14} />
                      <span>{m.scenario === "offline" ? "无服务" : "5G"}</span>
                    </>
                  )}
                  {v.thermal ? (
                    <Thermometer
                      size={14}
                      className="native-state-warning"
                      aria-label="温度保护"
                    />
                  ) : !v.unknown && v.performance !== "balanced" ? (
                    <span
                      className="native-profile-indicator"
                      aria-label={v.profile.label + "模式"}
                    >
                      <ProfileIcon profile={v.performance} size={14} />
                    </span>
                  ) : null}
                  {v.rail && <TrainFront size={14} aria-label="高铁优化中" />}
                  {m.settings.wifi.enabled && <Wifi size={14} />}
                  {m.settings.power.mode === "直供电" ? (
                    <Zap size={17} />
                  ) : (
                    <BatteryCharging size={20} />
                  )}
                  <b>82%</b>
                </span>
              </div>
              {locked ? (
                <div className="native-lock">
                  <div className="native-lock-clock">
                    <b>14:08</b>
                    <span>11 月 6 日 · 星期五</span>
                  </div>
                  <div className="native-lock-mode">
                    <ProfileIcon profile={v.performance} size={12} />
                    <span>
                      {v.unknown ? "模式未知" : v.profile.label + "模式"}
                    </span>
                    {v.thermal ? (
                      <span className="native-state-warning">· 温度保护中</span>
                    ) : v.rail ? (
                      <span>· 高铁优化中</span>
                    ) : null}
                  </div>
                  <div className="native-lock-carrier">
                    <span>
                      {v.unknown
                        ? "网络状态未知"
                        : v.phase === "restoring"
                          ? "正在恢复蜂窝"
                          : v.airplane
                            ? "飞行模式"
                            : cellularUnavailable
                              ? "蜂窝未连接"
                              : "中国联通 · 5G SA"}
                    </span>
                    <i className={cellularUnavailable ? "paused" : ""} />{" "}
                    {v.airplane
                      ? m.settings.wifi.enabled
                        ? "Wi-Fi 可用"
                        : "蜂窝关闭"
                      : cellularUnavailable
                        ? "未联网"
                        : "网络正常"}
                  </div>
                  <div className="native-metric-grid">
                    <div>
                      <small>↓ 下载 / Mbps</small>
                      <b>{cellularUnavailable ? "—" : "124.6"}</b>
                    </div>
                    <div>
                      <small>↑ 上传 / Mbps</small>
                      <b>{cellularUnavailable ? "—" : "14.2"}</b>
                    </div>
                  </div>
                  {cellularUnavailable ? (
                    <div className="native-lock-paused">
                      {v.airplane ? <Plane size={17} /> : <Signal size={17} />}
                      <span>
                        {v.airplane ? "蜂窝流量已暂停" : "等待蜂窝数据"}
                      </span>
                    </div>
                  ) : (
                    <NativeChart />
                  )}
                  <div className="native-lock-bottom">
                    <span>
                      6 台设备 · 82% ·{" "}
                      {m.settings.power.mode === "直供电"
                        ? "直供电"
                        : "正在充电"}
                    </span>
                    <button
                      className={"hold-unlock " + (holding ? "holding" : "")}
                      aria-label="长按一秒解锁"
                      onPointerDown={(e) => {
                        e.currentTarget.setPointerCapture(e.pointerId);
                        startHold();
                      }}
                      onPointerUp={stopHold}
                      onPointerCancel={stopHold}
                      onLostPointerCapture={stopHold}
                      onKeyDown={(e) => {
                        if ((e.key === " " || e.key === "Enter") && !e.repeat) {
                          e.preventDefault();
                          startHold();
                        }
                      }}
                      onKeyUp={stopHold}
                      onBlur={stopHold}
                    >
                      <Fingerprint size={30} />
                    </button>
                    <small>{holding ? "继续按住…" : "长按 1 秒解锁"}</small>
                  </div>
                  {pinEntry && (
                    <div className="native-pin">
                      <h2>输入解锁 PIN</h2>
                      <div className="pin-dots">
                        {[0, 1, 2, 3].map((i) => (
                          <i
                            key={i}
                            className={pin.length > i ? "filled" : ""}
                          />
                        ))}
                      </div>
                      <p>
                        {pinError ? "PIN 不正确，请重试" : "请输入 4 位 PIN"}
                      </p>
                      <div>
                        {[
                          "1",
                          "2",
                          "3",
                          "4",
                          "5",
                          "6",
                          "7",
                          "8",
                          "9",
                          "取消",
                          "0",
                          "删除",
                        ].map((key) => (
                          <button
                            key={key}
                            onClick={() => {
                              if (key === "取消") {
                                setPinEntry(false);
                                setPin("");
                                return;
                              }
                              if (key === "删除") {
                                setPin((p) => p.slice(0, -1));
                                return;
                              }
                              const next = pin + key;
                              if (next.length === 4) {
                                if (next === "1234") {
                                  setLocked(false);
                                  setPinEntry(false);
                                } else setPinError(true);
                                setPin("");
                              } else setPin(next);
                            }}
                          >
                            {key}
                          </button>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              ) : (
                <>
                  {path.length > 0 && (
                    <div className="native-page-head">
                      <button
                        aria-label="屏幕返回"
                        onClick={() => setPath((p) => p.slice(0, -1))}
                      >
                        <ArrowLeft size={18} />
                      </button>
                      <h2>{title}</h2>
                      <span />
                    </div>
                  )}
                  <div
                    ref={pane}
                    className={
                      "native-scroll " +
                      (!path.length && root === "状态" ? "native-home" : "")
                    }
                    data-testid="native-scroll"
                  >
                    {path.length ? (
                      renderPage()
                    ) : root === "状态" ? (
                      <>
                        <div className="screen-c-carrier">
                          <b>
                            {v.unknown
                              ? "状态未知"
                              : v.airplane
                                ? "飞行模式"
                                : "中国联通"}
                          </b>
                          <button
                            className="native-mode-shortcut"
                            aria-label="打开运行模式"
                            onClick={() => setQuickControls(true)}
                          >
                            <ProfileIcon profile={v.performance} size={12} />
                            {v.unknown ? "未知" : v.profile.label}
                          </button>
                          <span>
                            {v.unknown
                              ? "等待数据"
                              : v.phase === "restoring"
                                ? "恢复中"
                                : v.airplane
                                  ? m.settings.wifi.enabled
                                    ? "Wi-Fi 保留"
                                    : "蜂窝关闭"
                                  : v.busy
                                    ? "切换中"
                                    : cellularUnavailable
                                      ? "无服务"
                                      : "5G SA"}{" "}
                            <i
                              className={cellularUnavailable ? "paused" : ""}
                            />
                          </span>
                        </div>

                        <button
                          className="native-home-rates"
                          onClick={() => open("流量趋势")}
                        >
                          <div className="screen-c-metrics">
                            <div>
                              <span>↓ 下载 / Mbps</span>
                              <b>{cellularUnavailable ? "—" : "124.6"}</b>
                            </div>
                            <div>
                              <span>↑ 上传 / Mbps</span>
                              <b>{cellularUnavailable ? "—" : "14.2"}</b>
                            </div>
                          </div>
                        </button>
                        {m.homeLayout.signal && (
                          <div className="screen-c-signal">
                            <span>
                              {v.airplane ? "蜂窝无线电" : "n78"}{" "}
                              <b>{cellularUnavailable ? "—" : "−86 dBm"}</b>
                            </span>
                            <span>
                              {v.airplane ? "已关闭" : "SINR"}{" "}
                              <b>{cellularUnavailable ? "" : "24 dB"}</b>
                            </span>
                          </div>
                        )}
                        {homeMetrics.length > 0 && (
                          <div
                            className="screen-c-summary"
                            style={{
                              gridTemplateColumns: `repeat(${homeMetrics.length}, minmax(0, 1fr))`,
                            }}
                          >
                            {homeMetrics.map((metric) => (
                              <button
                                key={metric.id}
                                onClick={() =>
                                  quick(metric.root, [...metric.path])
                                }
                              >
                                {metric.label}
                                <b>{metric.value}</b>
                              </button>
                            ))}
                          </div>
                        )}
                        {m.homeLayout.wifi && (
                          <div className="screen-wifi-card">
                            <button
                              aria-label="展开连接二维码"
                              onClick={() =>
                                quick("连接", ["Wi-Fi", "连接二维码"])
                              }
                            >
                              <QR
                                size={88}
                                ssid={String(m.settings.wifi.ssid)}
                                password={String(m.settings.wifi.password)}
                              />
                            </button>
                            <div>
                              <b>{String(m.settings.wifi.ssid)}</b>
                              <span>
                                {m.settings.wifi.unified
                                  ? "双频合一"
                                  : "2.4 GHz"}{" "}
                                · 6 台在线
                              </span>
                              <div className="screen-password">
                                <span>
                                  {visible
                                    ? String(m.settings.wifi.password)
                                    : "••••••••••"}
                                </span>
                                <button
                                  aria-label={
                                    visible ? "屏幕隐藏密码" : "屏幕显示密码"
                                  }
                                  onClick={() => setVisible(!visible)}
                                >
                                  {visible ? (
                                    <EyeOff size={16} />
                                  ) : (
                                    <Eye size={16} />
                                  )}
                                </button>
                              </div>
                            </div>
                          </div>
                        )}
                        {m.homeLayout.services && homeServices.length > 0 && (
                          <div className="native-home-services">
                            {homeServices.map(({ id, index, name }) => (
                              <button
                                key={id}
                                onClick={() => quick("连接", [name])}
                              >
                                <i className={m.enabled[index] ? "on" : ""} />
                                {name}
                              </button>
                            ))}
                          </div>
                        )}
                      </>
                    ) : root === "连接" ? (
                      <>
                        <p className="native-section-label">无线与蜂窝</p>
                        {[
                          ["Wi-Fi", "双频合一 · 5 台无线设备", Wifi],
                          ["连接设备", "6 台在线", Smartphone],
                          ["移动网络", "中国联通 · 5G SA", Signal],
                          ["SIM 卡", "已就绪", Smartphone],
                        ].map(([s, value, Icon]) => {
                          const I = Icon as typeof Wifi;
                          return (
                            <Row
                              key={String(s)}
                              title={String(s)}
                              value={String(value)}
                              onClick={() => open(String(s))}
                              icon={<I size={18} />}
                            />
                          );
                        })}
                        <p className="native-section-label">组网与代理</p>
                        {serviceNames.map((s, i) => (
                          <Row
                            key={s}
                            title={s}
                            value={m.enabled[i] ? "运行中" : "已停止"}
                            onClick={() => open(s)}
                            icon={<Network size={18} />}
                          />
                        ))}
                      </>
                    ) : (
                      <>
                        <p className="native-section-label">常用管理</p>
                        {[
                          ["短信", "1 条未读", MessageSquare],
                          [
                            "运行模式",
                            v.unknown
                              ? "状态未知"
                              : v.profile.label + " · " + v.strategy.label,
                            SlidersHorizontal,
                          ],
                          ["电源与电池", "82% · 外部电源", BatteryCharging],
                          ["屏幕与安全", "外观、语言与锁屏", LockKeyhole],
                          ["诊断工具", "负载、进程与测试", Cpu],
                          ["设备设置", "信息、维护与更多设置", Settings2],
                        ].map(([s, value, Icon]) => {
                          const I = Icon as typeof Wifi;
                          return (
                            <Row
                              key={String(s)}
                              title={String(s)}
                              value={String(value)}
                              onClick={() => open(String(s))}
                              icon={<I size={18} />}
                            />
                          );
                        })}
                      </>
                    )}
                  </div>
                  <nav className="screen-nav">
                    {(
                      [
                        ["状态", LayoutDashboard],
                        ["连接", Network],
                        ["管理", Settings2],
                      ] as const
                    ).map(([name, Icon]) => (
                      <button
                        key={name}
                        className={root === name ? "active" : ""}
                        onClick={() => {
                          setRoot(name);
                          setPath([]);
                          setVisible(false);
                        }}
                      >
                        <Icon size={18} />
                        <span>{name}</span>
                      </button>
                    ))}
                  </nav>
                </>
              )}
              {quickControls && !locked && (
                <div className="native-sheet-backdrop">
                  <section
                    className="native-sheet native-mode-sheet"
                    role="dialog"
                    aria-label="快捷控制"
                  >
                    <header>
                      <b>快捷控制</b>
                      <button
                        aria-label="关闭快捷控制"
                        onClick={() => setQuickControls(false)}
                      >
                        <X size={18} />
                      </button>
                    </header>
                    {modeControls(true)}
                  </section>
                </div>
              )}
              {sheet && (
                <div className="native-sheet-backdrop">
                  <section
                    className="native-sheet"
                    role="dialog"
                    aria-label={sheet.title}
                  >
                    <header>
                      <b>{sheet.title}</b>
                      <button
                        aria-label="关闭屏幕选项"
                        onClick={() => setSheet(null)}
                      >
                        <X size={18} />
                      </button>
                    </header>
                    {sheet.options.map((s) => (
                      <button
                        key={s}
                        aria-pressed={
                          sheet.selected === undefined
                            ? undefined
                            : sheet.selected === s
                        }
                        onClick={() => sheet.choose(s)}
                      >
                        {s}
                        {sheet.selected === s ? (
                          <Check size={14} />
                        ) : (
                          <ChevronRight size={14} />
                        )}
                      </button>
                    ))}
                  </section>
                </div>
              )}
              {pendingChange && (
                <div className="native-sheet-backdrop">
                  <section
                    className="native-sheet"
                    role="dialog"
                    aria-label={pendingChange.title}
                  >
                    <header>
                      <b>{pendingChange.title}</b>
                    </header>
                    <p className="native-note">{pendingChange.summary}</p>
                    <p className="native-note">{pendingChange.message}</p>
                    <button
                      className="native-primary"
                      onClick={() => {
                        pendingChange.apply();
                        setPendingChange(null);
                      }}
                    >
                      继续应用
                    </button>
                    <button onClick={() => setPendingChange(null)}>
                      取消修改
                    </button>
                  </section>
                </div>
              )}
              {screenToast && (
                <div className="native-toast" role="status">
                  {screenToast}
                </div>
              )}
            </div>
          </div>
        </div>
        <aside className="screen-principles">
          <fieldset className="screen-theme-picker">
            <legend>屏幕外观</legend>
            <div>
              {screenThemes.map((theme) => (
                <button
                  key={theme}
                  aria-pressed={m.settings.security.theme === theme}
                  onClick={() => setTheme(theme)}
                >
                  {theme}
                </button>
              ))}
            </div>
          </fieldset>
          <details className="screen-layout-preview">
            <summary>首页布局</summary>
            <HomeLayoutSettings
              value={m.homeLayout}
              onChange={m.setHomeLayout}
            />
          </details>
          <AccentPicker value={accent} onChange={setAccent} />
          <span>阅读与操作</span>
          <h3>紧凑，但不拥挤。</h3>
          <p>
            状态页聚合关键指标；一级列表按连接对象组织；复杂操作在详情页展开。
          </p>
          <ul>
            <li>触点保留足够高度</li>
            <li>二维码固定尺寸，文字单独换行</li>
            <li>长内容独立滚动，导航始终可达</li>
            <li>锁屏保留状态，隐藏连接凭据</li>
          </ul>
          <div className="screen-design-note">
            此处为交互设计稿。PIN 演示值为
            1234；语言选项演示设置流程，正文暂以简体中文评审。未连接设备。
          </div>
        </aside>
      </div>
    </main>
  );
}
function NativeEditor({
  label,
  value,
  save,
  multiline = false,
}: {
  label: string;
  value: string;
  save: (s: string) => void;
  multiline?: boolean;
}) {
  const [draft, setDraft] = useState(value);
  return (
    <div className="native-editor">
      <label>
        {label}
        {multiline ? (
          <textarea
            rows={6}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
          />
        ) : (
          <input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (
                e.key === "Enter" &&
                !e.nativeEvent.isComposing &&
                draft.trim() &&
                draft !== value
              ) {
                e.preventDefault();
                save(draft);
              }
            }}
          />
        )}
      </label>
      <button
        className="native-primary"
        disabled={!draft.trim() || draft === value}
        onClick={() => save(draft)}
      >
        {multiline ? "发送" : "完成"}
      </button>
      <p className="native-note">屏幕输入框示意；设备版使用软键盘。</p>
    </div>
  );
}
