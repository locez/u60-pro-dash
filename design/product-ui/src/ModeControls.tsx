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
import { Toggle } from "./ui";
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
    <div className="mode-overview" aria-label="设备运行状态">
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
          带宽分配 · {v.unknown ? "未知" : v.strategy.label}
        </span>
      )}
      <button className="mode-open" onClick={open}>
        <SlidersHorizontal size={15} /> 快捷控制
      </button>
    </div>
  );
}

export function FlightModeControl() {
  const { modes: v } = useModel();
  const [confirm, setConfirm] = useState(false);
  return (
    <div className="mode-controls">
      <section className={"mode-flight " + (v.airplane ? "active" : "")}>
        <span className="mode-icon">
          <Plane size={22} />
        </span>
        <div>
          <b>飞行模式</b>
          <p>
            {v.unknown
              ? "暂时无法读取蜂窝无线电状态。"
              : v.phase === "restoring"
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
            onChange={() =>
              v.airplane ? v.changeAirplane(false) : setConfirm(true)
            }
          />
        </span>
      </section>
      {v.errors.airplane && (
        <div className="mode-error" role="alert">
          <Note warning>{v.errors.airplane}</Note>
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

export function PerformanceControls() {
  const { modes: v } = useModel();
  return (
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
        {v.unknown
          ? "暂时无法读取性能模式，状态恢复后可修改。"
          : v.profile.detail}
      </p>
      {v.errors.performance && (
        <div role="alert">
          <Note warning>{v.errors.performance}</Note>
        </div>
      )}
      {v.thermal && (
        <Note warning>
          温度保护中，设备正在限制处理能力。已选模式保留，温度恢复后继续使用。
        </Note>
      )}
    </section>
  );
}

export function RailStatus() {
  const { modes: v } = useModel();
  return (
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
  );
}

export function NetworkOptimizationControls() {
  const { modes: v } = useModel();
  return (
    <div className="mode-controls">
      <section className="mode-network-strategy">
        <div className="mode-section-title">
          <h3>带宽分配</h3>
          <span>选择优先保障的应用</span>
        </div>
        <div className="bandwidth-options" role="group" aria-label="带宽分配">
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
        {v.unknown ? (
          <p className="mode-help">暂时无法读取带宽分配设置。</p>
        ) : (
          v.airplane && (
            <p className="mode-help">
              蜂窝已关闭。所选设置将在蜂窝恢复后继续使用。
            </p>
          )
        )}
        {v.errors.bandwidth && (
          <div role="alert">
            <Note warning>{v.errors.bandwidth}</Note>
          </div>
        )}
      </section>
      <RailStatus />
    </div>
  );
}

// Quick access composes controls without becoming a separate settings category.
export function ModeControls({
  openDetails,
}: {
  openDetails: (page: "internet" | "system") => void;
}) {
  return (
    <div className="mode-controls compact">
      <FlightModeControl />
      <PerformanceControls />
      <RailStatus />
      <div className="mode-destinations">
        <button
          className="mode-detail-link"
          onClick={() => openDetails("internet")}
        >
          移动网络设置 <ChevronRight size={14} />
        </button>
        <button
          className="mode-detail-link"
          onClick={() => openDetails("system")}
        >
          电源与电池 <ChevronRight size={14} />
        </button>
      </div>
    </div>
  );
}
