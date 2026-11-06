import { useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  BatteryCharging,
  ChevronRight,
  CircleHelp,
  Eye,
  EyeOff,
  LayoutDashboard,
  LockKeyhole,
  Network,
  QrCode,
  Settings2,
  ShieldCheck,
  Signal,
  Wifi,
} from "lucide-react";
import { QR, type Notify, type Variant } from "./ui";
import { serviceNames } from "./presentation";
export const directions = {
  a: {
    name: "克制专业",
    label: "清晰的状态层级，安静的视觉秩序。",
    web: "拓扑 + 趋势 + 设备表",
    screen: "数据清楚，连接分享就近。",
  },
  b: {
    name: "圆润轻盈",
    label: "柔和、亲近，强调设备与连接体验。",
    web: "设备主视觉 + 常用操作",
    screen: "大状态，大触点，轻松扫读。",
  },
  c: {
    name: "紧凑高密度",
    label: "一眼掌握链路、负载与网络服务。",
    web: "指标条 + 链路 + 设备监控",
    screen: "优先信息密度，保留清晰分区。",
  },
};
export function ScreenHome({
  variant,
  notify,
}: {
  variant: Variant;
  notify: Notify;
}) {
  const [visible, setVisible] = useState(false);
  return (
    <div className={"screen-frame screen-" + variant}>
      <div className="screen-hardware">
        <div className="screen-face">
          <div className="screen-statusbar">
            <span>U60</span>
            <span>
              <Signal size={14} />
              <span>5G</span>
              <Wifi size={14} />
              <BatteryCharging size={20} />
              <b>82%</b>
            </span>
          </div>
          <div className="screen-content">
            {variant === "a" ? (
              <>
                <div className="screen-carrier">
                  <span>中国联通</span>
                  <span className="screen-badge">5G SA</span>
                </div>
                <h2>
                  连接正常
                  <span />
                </h2>
                <p className="screen-subtitle">6 台设备在线 · 信号良好</p>
                <div className="screen-rates">
                  <div>
                    <span>
                      <ArrowDown size={12} /> 下载
                    </span>
                    <b>
                      124.6<small>Mbps</small>
                    </b>
                  </div>
                  <div>
                    <span>
                      <ArrowUp size={12} /> 上传
                    </span>
                    <b>
                      14.2<small>Mbps</small>
                    </b>
                  </div>
                </div>
                <div className="screen-wifi-card">
                  <QR size={94} />
                  <div>
                    <small>扫码连接 Wi-Fi</small>
                    <b>U60 Studio</b>
                    <span>2.4 GHz / 5 GHz</span>
                    <div className="screen-password">
                      <span>{visible ? "sample-pass-2026" : "••••••••••"}</span>
                      <button
                        aria-label="屏幕显示演示密码"
                        onClick={() => setVisible(!visible)}
                      >
                        {visible ? <EyeOff size={16} /> : <Eye size={16} />}
                      </button>
                    </div>
                  </div>
                </div>
                <div className="screen-footer-stats">
                  <span>
                    <Network size={13} /> Tailscale 在线
                  </span>
                  <span>本月 62.8 GB</span>
                </div>
              </>
            ) : variant === "b" ? (
              <>
                <div className="screen-b-top">
                  <span>中国联通 · 5G SA</span>
                  <span className="screen-online">已联网</span>
                </div>
                <div className="screen-halo">
                  <Wifi size={35} />
                </div>
                <h2 className="screen-center-title">网络连接正常</h2>
                <div className="screen-b-rates">
                  <span>
                    <ArrowDown size={12} />
                    <b>124.6</b> Mbps
                  </span>
                  <span>
                    <ArrowUp size={12} />
                    <b>14.2</b> Mbps
                  </span>
                </div>
                <div className="screen-wifi-card">
                  <QR size={86} />
                  <div>
                    <small>你的 Wi-Fi</small>
                    <b>U60 Studio</b>
                    <div className="screen-password">
                      <span>{visible ? "sample-pass-2026" : "••••••••••"}</span>
                      <button
                        aria-label="屏幕显示演示密码"
                        onClick={() => setVisible(!visible)}
                      >
                        {visible ? <EyeOff size={16} /> : <Eye size={16} />}
                      </button>
                    </div>
                  </div>
                </div>
                <div className="screen-b-bottom">
                  <span>
                    <Wifi size={14} /> 6 台设备
                  </span>
                  <span>
                    <ShieldCheck size={14} /> 3 项服务
                  </span>
                </div>
              </>
            ) : (
              <>
                <div className="screen-c-carrier">
                  <b>中国联通</b>
                  <span>
                    5G SA <i />
                  </span>
                </div>
                <div className="screen-c-metrics">
                  <div>
                    <span>↓ 下载 / Mbps</span>
                    <b>124.6</b>
                  </div>
                  <div>
                    <span>↑ 上传 / Mbps</span>
                    <b>14.2</b>
                  </div>
                </div>
                <div className="screen-c-signal">
                  <span>
                    n78 <b>−86 dBm</b>
                  </span>
                  <span>
                    SINR <b>24 dB</b>
                  </span>
                </div>
                <div className="screen-c-summary">
                  <span>
                    在线设备<b>6</b>
                  </span>
                  <span>
                    本月 / GB<b>62.8</b>
                  </span>
                  <span>
                    温度<b>39°</b>
                  </span>
                </div>
                <div className="screen-wifi-card">
                  <QR size={88} />
                  <div>
                    <b>U60 Studio</b>
                    <span>双频合一 · 6 台在线</span>
                    <div className="screen-password">
                      <span>{visible ? "sample-pass-2026" : "••••••••••"}</span>
                      <button
                        aria-label="屏幕显示演示密码"
                        onClick={() => setVisible(!visible)}
                      >
                        {visible ? <EyeOff size={16} /> : <Eye size={16} />}
                      </button>
                    </div>
                  </div>
                </div>
                <div className="screen-c-services">
                  <span>
                    <i /> Tailscale
                  </span>
                  <span>
                    <i /> WireGuard
                  </span>
                  <span>
                    <i /> {serviceNames[2]}
                  </span>
                </div>
              </>
            )}
          </div>
          <nav className="screen-nav">
            {[
              { name: "状态", icon: LayoutDashboard },
              { name: "连接", icon: Network },
              { name: "管理", icon: Settings2 },
            ].map((item, i) => (
              <button
                key={item.name}
                className={i === 0 ? "active" : ""}
                onClick={
                  i === 0
                    ? () => {}
                    : () => notify("当前仅展示屏幕首页，层级将在风格确认后展开")
                }
              >
                <item.icon size={18} />
                <span>{item.name}</span>
              </button>
            ))}
          </nav>
        </div>
      </div>
    </div>
  );
}
export default function ScreenHomes({ notify }: { notify: Notify }) {
  return (
    <main className="screen-gallery">
      <div className="screen-gallery-title">
        <span className="eyebrow">同一块屏幕，三种表达</span>
        <h1>小屏，也有清楚的秩序。</h1>
        <p>320 × 480 原始比例 · 同一组演示数据 · 常驻信息无需动画</p>
      </div>
      <div className="screen-options">
        {(["a", "b", "c"] as Variant[]).map((v) => (
          <section key={v}>
            <div className="screen-option-heading">
              <span>{v.toUpperCase()}</span>
              <div>
                <h2>{directions[v].name}</h2>
                <p>{directions[v].screen}</p>
              </div>
            </div>
            <ScreenHome variant={v} notify={notify} />
          </section>
        ))}
      </div>
      <div className="screen-gallery-note">
        <LockKeyhole size={15} />
        <span>
          本轮展示解锁后的首页。锁屏只保留状态信息，Wi-Fi
          密码与配置操作需要解锁。
        </span>
      </div>
    </main>
  );
}
