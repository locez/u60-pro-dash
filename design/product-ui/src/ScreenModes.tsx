import { ChevronRight, Plane, Thermometer, TrainFront } from "lucide-react";
import { useModel } from "./model";
import { ProfileIcon } from "./ModeControls";
import { performanceModes } from "./modes";

export function ScreenFlightMode({
  requestAirplane,
}: {
  requestAirplane: () => void;
}) {
  const { modes: v } = useModel();
  return (
    <>
      <button
        className="native-mode-flight"
        role="switch"
        aria-label="飞行模式"
        aria-checked={v.airplane}
        disabled={v.busy || v.unknown}
        onClick={requestAirplane}
      >
        <Plane size={20} />
        <span>
          <b>飞行模式</b>
          <small>
            {v.unknown
              ? "蜂窝状态暂不可用"
              : v.phase === "applying"
                ? "正在关闭蜂窝…"
                : v.phase === "restoring"
                  ? "正在恢复蜂窝…"
                  : v.airplane
                    ? "蜂窝已关闭 · Wi-Fi 保留"
                    : "仅关闭蜂窝，保留 Wi-Fi"}
          </small>
        </span>
        <span
          aria-hidden="true"
          className={"native-switch " + (v.airplane ? "on" : "")}
        >
          <i />
        </span>
      </button>
      {v.errors.airplane && (
        <div className="native-mode-alert" role="alert">
          {v.errors.airplane}
          {v.airplane && (
            <button disabled={v.busy} onClick={() => v.changeAirplane(false)}>
              重试恢复
            </button>
          )}
        </div>
      )}
    </>
  );
}

export function ScreenPerformanceControls() {
  const { modes: v } = useModel();
  return (
    <div className="native-mode-controls">
      <div className="native-mode-heading">
        <b>性能模式</b>
        <small>选择即应用</small>
      </div>
      <div
        className="native-profile-options"
        role="group"
        aria-label="性能模式"
      >
        {performanceModes.map((p) => (
          <button
            key={p.id}
            aria-pressed={!v.unknown && v.performance === p.id}
            disabled={v.unknown}
            onClick={() => v.changePerformance(p.id)}
          >
            <ProfileIcon profile={p.id} size={19} />
            <span>{p.label}</span>
          </button>
        ))}
      </div>
      <p className="native-mode-description">
        {v.unknown ? "暂时无法读取性能模式，恢复后可修改。" : v.profile.detail}
      </p>
      {v.errors.performance && (
        <p className="native-mode-alert" role="alert">
          {v.errors.performance}
        </p>
      )}
      {v.thermal && (
        <p className="native-mode-alert">
          <Thermometer size={13} /> 温度保护中，已选模式保留。
        </p>
      )}
    </div>
  );
}

export function ScreenRailStatus() {
  const { modes: v } = useModel();
  return (
    <div className="native-mode-rail">
      <TrainFront size={17} />
      <span>
        <b>高铁优化</b>
        <small>
          {v.unknown
            ? "状态未知"
            : v.airplane
              ? "飞行模式期间暂停"
              : v.rail
                ? "正在自动优化"
                : "自动识别 · 当前未触发"}
        </small>
      </span>
      <em>{v.rail ? "工作中" : "自动"}</em>
    </div>
  );
}

export function ScreenNetworkOptimization({
  chooseBandwidth,
}: {
  chooseBandwidth: () => void;
}) {
  const { modes: v } = useModel();
  return (
    <>
      <button
        className="native-mode-bandwidth"
        disabled={v.unknown}
        onClick={chooseBandwidth}
      >
        <span>
          <b>带宽分配</b>
          <small>{v.unknown ? "当前设置未知" : v.strategy.hint}</small>
        </span>
        <strong>{v.unknown ? "未知" : v.strategy.label}</strong>
        <ChevronRight size={14} />
      </button>
      {v.airplane && (
        <p className="native-mode-description">蜂窝恢复后继续使用所选设置。</p>
      )}
      {v.errors.bandwidth && (
        <p className="native-mode-alert" role="alert">
          {v.errors.bandwidth}
        </p>
      )}
      <ScreenRailStatus />
    </>
  );
}

export default function ScreenModes({
  requestAirplane,
  openDetails,
}: {
  requestAirplane: () => void;
  openDetails: (page: "internet" | "system") => void;
}) {
  return (
    <div className="native-mode-controls">
      <ScreenFlightMode requestAirplane={requestAirplane} />
      <ScreenPerformanceControls />
      <ScreenRailStatus />
      <div className="native-mode-destinations">
        <button
          className="native-mode-details"
          onClick={() => openDetails("internet")}
        >
          移动网络设置 <ChevronRight size={13} />
        </button>
        <button
          className="native-mode-details"
          onClick={() => openDetails("system")}
        >
          电源与电池 <ChevronRight size={13} />
        </button>
      </div>
    </div>
  );
}
