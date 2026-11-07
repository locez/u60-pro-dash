import { ChevronRight, Plane, Thermometer, TrainFront } from "lucide-react";
import { useModel } from "./model";
import { ProfileIcon } from "./ModeControls";
import { performanceModes } from "./modes";
export default function ScreenModes({
  compact = false,
  requestAirplane,
  chooseBandwidth,
  openDetails,
}: {
  compact?: boolean;
  requestAirplane: () => void;
  chooseBandwidth: () => void;
  openDetails: () => void;
}) {
  const { modes: v } = useModel();
  return (
    <div className="native-mode-controls">
      {v.unknown && (
        <p className="native-mode-alert">模式状态暂不可用，恢复后可修改。</p>
      )}
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
            {v.phase === "applying"
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
      {v.error && (
        <div className="native-mode-alert" role="alert">
          {v.error}
          {v.airplane && (
            <button disabled={v.busy} onClick={() => v.changeAirplane(false)}>
              重试恢复
            </button>
          )}
        </div>
      )}
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
        {v.unknown ? "当前运行策略未知" : v.profile.detail}
      </p>
      {v.thermal && (
        <p className="native-mode-alert">
          <Thermometer size={13} /> 温度保护中，已选模式保留。
        </p>
      )}
      {!compact && (
        <>
          <button
            className="native-mode-bandwidth"
            disabled={v.unknown}
            onClick={chooseBandwidth}
          >
            <span>
              <b>应用带宽策略</b>
              <small>
                {v.airplane ? "蜂窝恢复后继续使用" : v.strategy.hint}
              </small>
            </span>
            <strong>{v.unknown ? "未知" : v.strategy.label}</strong>
            <ChevronRight size={14} />
          </button>
          <p className="native-mode-description">
            极速是带宽分配策略，可与任一性能档位组合。
          </p>
        </>
      )}
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
      {compact ? (
        <button className="native-mode-details" onClick={openDetails}>
          全部模式与带宽策略 <ChevronRight size={13} />
        </button>
      ) : (
        <p className="native-mode-description">
          温度保护始终启用。网络制式、Wi-Fi 与组网服务保持独立。
        </p>
      )}
    </div>
  );
}
