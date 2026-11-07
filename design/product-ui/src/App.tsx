import { useEffect, useState } from "react";
import {
  Check,
  ChevronRight,
  ListTree,
  Monitor,
  Moon,
  Smartphone,
  Sun,
  X,
} from "lucide-react";
import Explore from "./Explore";
import Product, { FeatureMap } from "./Product";
import ScreenProduct from "./ScreenProduct";
import { ModelProvider, useModel, type Scenario } from "./model";
import { modeScenarios, type ModeScenario } from "./modes";
import { ShareModal } from "./ui";
import { presentationQuery, publicPresentation } from "./presentation";
import "./product.css";
import "./screen-product.css";
const initial = new URLSearchParams(location.search);
export default function App() {
  return initial.has("explore") ? (
    <Explore />
  ) : (
    <ModelProvider>
      <SelectedDesign />
    </ModelProvider>
  );
}
function SelectedDesign() {
  const m = useModel(),
    [view, setView] = useState(
      initial.get("view") === "screen"
        ? "screen"
        : initial.get("view") === "map"
          ? "map"
          : "web",
    ),
    [dark, setDark] = useState(initial.get("theme") === "dark"),
    [shareBand, setShareBand] = useState(0);
  useEffect(() => {
    const query = new URLSearchParams({
      ...presentationQuery,
      view,
      page: m.route.page,
      ...(m.route.tab ? { tab: m.route.tab } : {}),
      ...(m.route.service !== undefined
        ? { service: String(m.route.service) }
        : {}),
      ...(dark ? { theme: "dark" } : {}),
    });
    history.replaceState(null, "", "?" + query.toString());
  }, [view, dark, m.route]);
  useEffect(() => {
    if (!m.toast) return;
    const timer = setTimeout(() => m.notify(""), 4000);
    return () => clearTimeout(timer);
  }, [m.toast]);
  return (
    <div className={"design-app " + (dark ? "dark-design" : "")}>
      <div className="design-bar">
        <div className="design-brand">
          <span className="design-dot" /> U60 <span>产品设计稿</span>
          <span className="design-stage">03 / 连接与性能</span>
          <a
            href={
              "?explore=1" + (publicPresentation ? "&presentation=public" : "")
            }
          >
            初版风格
          </a>
        </div>
        <div className="design-controls">
          <div className="view-switch" aria-label="预览界面">
            {(
              [
                ["web", "Web", Monitor],
                ["screen", "设备屏幕", Smartphone],
                ["map", "功能地图", ListTree],
              ] as const
            ).map(([id, label, Icon]) => (
              <button
                key={id}
                aria-label={label}
                aria-pressed={view === id}
                onClick={() => setView(id)}
              >
                <Icon size={15} />
                <span>{label}</span>
              </button>
            ))}
          </div>
          <button
            className="design-theme"
            aria-label={dark ? "切换浅色" : "切换深色"}
            onClick={() => setDark(!dark)}
          >
            {dark ? <Sun size={17} /> : <Moon size={17} />}
          </button>
        </div>
      </div>
      <div className="design-description">
        <span className="selected-pair">
          <b>Web · 克制专业</b>
          <span>屏幕 · 紧凑高密度</span>
        </span>
        <label className="mode-review-control">
          模式演示
          <select
            aria-label="模式演示场景"
            value={m.modes.scenario}
            onChange={(e) => m.modes.setScene(e.target.value as ModeScenario)}
          >
            {modeScenarios.map(([id, label]) => (
              <option key={id} value={id}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <label className="design-scenario">
          <span>交互原型 · 示例数据</span>
          <select
            value={m.scenario}
            aria-label="预览场景"
            onChange={(e) => m.setScenario(e.target.value as Scenario)}
          >
            <option value="normal">正常连接</option>
            <option value="offline">蜂窝断网</option>
            <option value="euicc">支持 eSIM 的卡</option>
          </select>
        </label>
      </div>
      <div
        id="product-preview"
        className="variant-a"
        data-theme={dark ? "dark" : "light"}
      >
        {view === "web" ? (
          <Product />
        ) : view === "screen" ? (
          <ScreenProduct />
        ) : (
          <FeatureMap openWeb={() => setView("web")} />
        )}
      </div>
      <div className="design-bottom">
        <span>功能归属与关键流程评审 · 操作仅改变示例状态，未连接设备。</span>
        <span>
          React + Tailwind <ChevronRight size={12} /> 本地预览
        </span>
      </div>
      {m.share && (
        <ShareModal
          close={() => {
            m.setShare(false);
            setShareBand(0);
          }}
          ssid={String(
            shareBand ? m.settings.wifi.ssid5 : m.settings.wifi.ssid,
          )}
          password={String(m.settings.wifi.password)}
          unified={!!m.settings.wifi.unified}
          alternate={
            !m.settings.wifi.unified
              ? {
                  label: shareBand ? "切换到 2.4 GHz" : "切换到 5 GHz",
                  action: () => setShareBand((s) => 1 - s),
                }
              : undefined
          }
        />
      )}
      {m.toast && (
        <div role="status" className="toast">
          <Check size={17} />
          <span>{m.toast}</span>
          <button aria-label="关闭提示" onClick={() => m.notify("")}>
            <X size={16} />
          </button>
        </div>
      )}
    </div>
  );
}
