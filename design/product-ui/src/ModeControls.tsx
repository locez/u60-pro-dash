import { useState } from "react";
import {
  Check,
  ChevronRight,
  Gauge,
  Leaf,
  Plane,
  Radio,
  SlidersHorizontal,
  Thermometer,
  TrainFront,
  Scale,
} from "lucide-react";
import { useModel } from "./model";
import { Modal, Note } from "./controls";
import { Panel, PanelTitle, Toggle } from "./ui";
import {
  bandwidthModes,
  performanceModes,
  type PerformanceMode,
} from "./modes";
import "./modes.css";

export function ProfileIcon({
  profile,
  size = 16,
}: {
  profile: PerformanceMode;
  size?: number;
}) {
  const Icon =
    profile === "saving" ? Leaf : profile === "performance" ? Gauge : Scale;
  return <Icon size={size} />;
}
export function ModeOverview({ open }: { open: () => void }) {
  const { modes: v } = useModel();
  return (
    <div className="mode-overview" aria-label="设备运行模式">
      <button onClick={open} className={v.airplane ? "active" : ""}>
        {v.airplane ? <Plane size={16} /> : <Radio size={16} />}
        <span>
          {v.unknown
            ? "无线状态未知"
            : v.phase === "restoring"
              ? "蜂窝恢复中"
              : v.airplane
                ? "飞行模式"
                : "蜂窝已开启"}
        </span>
      </button>
      <button onClick={open}>
        <ProfileIcon profile={v.performance} />
        <span>{v.unknown ? "性能状态未知" : `${v.profile.label}模式`}</span>
        <ChevronRight size={13} />
      </button>
      {v.thermal ? (
        <span className="mode-status warning">
          <Thermometer size={15} /> 温度保护中
        </span>
      ) : v.rail ? (
        <span className="mode-status">
          <TrainFront size={15} /> 高铁优化中
        </span>
      ) : (
        <span className="mode-strategy">
          带宽策略 · {v.unknown ? "未知" : v.strategy.label}
        </span>
      )}
      <button className="mode-open" onClick={open}>
        <SlidersHorizontal size={15} /> 快捷控制
      </button>
    </div>
  );
}
export function ModeControls({
  compact = false,
  openDetails,
}: {
  compact?: boolean;
  openDetails?: () => void;
}) {
  const m = useModel(),
    v = m.modes;
  const [confirm, setConfirm] = useState(false);
  return (
    <div className={"mode-controls " + (compact ? "compact" : "")}>
      {v.unknown && (
        <Note warning>
          暂时无法读取模式状态，请稍后刷新。读数恢复后才能修改。
        </Note>
      )}
      <section className={"mode-flight " + (v.airplane ? "active" : "")}>
        <span className="mode-icon">
          <Plane size={22} />
        </span>
        <div>
          <b>飞行模式</b>
          <p>
            {v.phase === "restoring"
              ? "正在恢复蜂窝无线电与网络注册…"
              : v.phase === "applying"
                ? "正在关闭蜂窝无线电…"
                : "关闭蜂窝网络、通话与短信，保留 Wi-Fi 和本地管理。"}
          </p>
        </div>
        <span aria-busy={v.busy}>
          <Toggle
            label="飞行模式"
            on={v.airplane}
            disabled={v.busy || v.unknown}
            onChange={() => {
              if (!v.busy && !v.unknown)
                v.airplane ? v.changeAirplane(false) : setConfirm(true);
            }}
          />
        </span>
      </section>
      {v.error && (
        <div className="mode-error" role="alert">
          <Note warning>{v.error}</Note>
          {v.airplane && (
            <button
              className="btn"
              disabled={v.busy}
              onClick={() => v.changeAirplane(false)}
            >
              重试恢复蜂窝
            </button>
          )}
        </div>
      )}
      <section className="mode-performance">
        <div className="mode-section-title">
          <h3>性能模式</h3>
          <span>选择后立即应用</span>
        </div>
        <div className="performance-options" role="group" aria-label="性能模式">
          {performanceModes.map((p) => (
            <button
              key={p.id}
              aria-pressed={!v.unknown && v.performance === p.id}
              disabled={v.unknown}
              onClick={() => v.changePerformance(p.id)}
            >
              <ProfileIcon profile={p.id} size={21} />
              <b>{p.label}</b>
              <small>{p.hint}</small>
              {!v.unknown && v.performance === p.id && (
                <Check className="mode-check" size={15} />
              )}
            </button>
          ))}
        </div>
        <p className="mode-help">
          {v.unknown ? "当前模式未知" : v.profile.detail}{" "}
          各档位都保留温度保护，Wi-Fi、网络制式和组网服务独立设置。
        </p>
        {v.thermal && (
          <Note warning>
            温度保护中，设备正在限制处理能力。已选模式保留，温度恢复后继续使用。
          </Note>
        )}
      </section>
      {!compact && (
        <section className="mode-network-strategy">
          <div className="mode-section-title">
            <h3>应用带宽策略</h3>
            <span>原厂智能调度</span>
          </div>
          <div
            className="bandwidth-options"
            role="group"
            aria-label="应用带宽策略"
          >
            {bandwidthModes.map((p) => (
              <button
                key={p.id}
                aria-pressed={!v.unknown && v.bandwidth === p.id}
                disabled={v.unknown}
                onClick={() => v.changeBandwidth(p.id)}
              >
                <b>{p.label}</b>
                <small>{p.hint}</small>
                {!v.unknown && v.bandwidth === p.id && <Check size={14} />}
              </button>
            ))}
          </div>
          <p className="mode-help">
            {v.airplane
              ? "蜂窝已关闭，带宽策略暂停；原来的选择将在蜂窝恢复后继续使用。"
              : "“极速”让应用自由竞争带宽，可以与任一性能档位组合使用。"}
          </p>
        </section>
      )}
      <div className="mode-rail">
        <TrainFront size={21} />
        <div>
          <b>
            高铁优化{" "}
            <span className={v.rail ? "mode-on" : ""}>
              {v.unknown
                ? "状态未知"
                : v.airplane
                  ? "已暂停"
                  : v.rail
                    ? "正在工作"
                    : "未触发"}
            </span>
          </b>
          <p>由设备自动识别并优化高速移动场景，无需手动切换。</p>
        </div>
        <span className="mode-auto">自动</span>
      </div>
      {compact && (
        <button
          className="mode-detail-link"
          onClick={
            openDetails || (() => m.go({ page: "system", tab: "运行模式" }))
          }
        >
          查看全部运行模式与带宽策略 <ChevronRight size={14} />
        </button>
      )}
      {confirm && (
        <Modal title="开启飞行模式？" close={() => setConfirm(false)}>
          <div className="mode-confirm-icon">
            <Plane size={28} />
          </div>
          <p>蜂窝互联网、通话与短信将中断。依赖蜂窝连接的远程访问也会断开。</p>
          <Note>
            Wi-Fi 热点、USB 和本地管理保持可用。可在机身屏幕或本地 Web
            中恢复蜂窝网络。
          </Note>
          <div className="dialog-actions">
            <button className="btn" onClick={() => setConfirm(false)}>
              取消
            </button>
            <button
              className="btn primary"
              onClick={() => {
                setConfirm(false);
                v.changeAirplane(true);
              }}
            >
              开启飞行模式
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}
export function OperatingModesPage() {
  const m = useModel();
  return (
    <>
      <Panel>
        <PanelTitle
          title="运行模式"
          note="独立控制连接、性能与带宽分配，状态在所有界面同步。"
        />
        <ModeControls />
      </Panel>
      <div className="mode-related">
        <button
          className="btn"
          onClick={() => m.go({ page: "system", tab: "电源与电池" })}
        >
          电池与供电设置 <ChevronRight size={14} />
        </button>
        <button
          className="btn"
          onClick={() => m.go({ page: "internet", tab: "连接设置" })}
        >
          移动网络设置 <ChevronRight size={14} />
        </button>
      </div>
    </>
  );
}
