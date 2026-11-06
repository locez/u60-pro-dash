import { Toggle } from "./ui";
import { serviceNames } from "./presentation";
import {
  defaultHomeLayout,
  metricOptions,
  replaceMetric,
  serviceIds,
  type MetricId,
  type ScreenHomeLayout,
} from "./screenPreferences";

export default function HomeLayoutSettings({
  value,
  onChange,
}: {
  value: ScreenHomeLayout;
  onChange: (value: ScreenHomeLayout) => void;
}) {
  return (
    <div className="home-layout-settings">
      <fieldset>
        <legend>首页模块</legend>
        {(
          [
            ["signal", "信号详情"],
            ["wifi", "Wi-Fi 连接卡片"],
            ["services", "组网服务摘要"],
          ] as const
        ).map(([key, label]) => (
          <div className="layout-switch-row" key={key}>
            <span>{label}</span>
            <Toggle
              on={value[key]}
              label={`首页显示${label}`}
              onChange={() => onChange({ ...value, [key]: !value[key] })}
            />
          </div>
        ))}
      </fieldset>
      {value.services && (
        <fieldset>
          <legend>显示哪些服务</legend>
          {serviceIds.map((id, i) => (
            <div className="layout-switch-row" key={id}>
              <span>{serviceNames[i]}</span>
              <Toggle
                on={value.serviceVisibility[id]}
                label={`首页显示 ${serviceNames[i]}`}
                onChange={() =>
                  onChange({
                    ...value,
                    serviceVisibility: {
                      ...value.serviceVisibility,
                      [id]: !value.serviceVisibility[id],
                    },
                  })
                }
              />
            </div>
          ))}
          <p>仅调整首页展示，服务继续运行。</p>
        </fieldset>
      )}
      <fieldset>
        <legend>快捷指标</legend>
        <p>从左到右排列；选择已显示的指标会交换位置。</p>
        {value.metrics.map((id, i) => (
          <label className="layout-metric-field" key={i}>
            <span>第 {i + 1} 格</span>
            <select
              aria-label={`第 ${i + 1} 格指标`}
              value={id}
              onChange={(e) =>
                onChange(replaceMetric(value, i, e.target.value as MetricId))
              }
            >
              <option value="none">不显示</option>
              {metricOptions.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name}
                </option>
              ))}
            </select>
          </label>
        ))}
      </fieldset>
      <button
        type="button"
        className="layout-reset"
        onClick={() => onChange(structuredClone(defaultHomeLayout))}
      >
        恢复默认布局
      </button>
      <p className="layout-save-note">调整即时保存。</p>
    </div>
  );
}
