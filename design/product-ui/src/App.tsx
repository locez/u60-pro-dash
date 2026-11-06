import { useEffect, useState } from "react";
import {
  ArrowLeftRight,
  Check,
  ChevronRight,
  Monitor,
  Moon,
  Smartphone,
  Sun,
  X,
} from "lucide-react";
import { Compact, Consumer, Professional } from "./Homepages";
import ScreenHomes, { directions } from "./ScreenHomes";
import { ShareModal, type Variant } from "./ui";
const initial = new URLSearchParams(location.search);
export default function App() {
  const [variant, setVariant] = useState<Variant>(
    ["a", "b", "c"].includes(initial.get("style") || "")
      ? (initial.get("style") as Variant)
      : "a",
  );
  const [view, setView] = useState(
    initial.get("view") === "screen" ? "screen" : "web",
  );
  const [dark, setDark] = useState(initial.get("theme") === "dark");
  const [share, setShare] = useState(false),
    [toast, setToast] = useState(""),
    [enabled, setEnabled] = useState([true, true, true]);
  const notify = (message: string) => setToast(message);
  useEffect(() => {
    const query = new URLSearchParams({
      style: variant,
      view,
      ...(dark ? { theme: "dark" } : {}),
    });
    history.replaceState(null, "", "?" + query.toString());
  }, [variant, view, dark]);
  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(""), 3600);
    return () => clearTimeout(timer);
  }, [toast]);
  const props = {
    variant,
    share: () => setShare(true),
    notify,
    enabled,
    toggle: (i: number) =>
      setEnabled((v) => v.map((on, index) => (index === i ? !on : on))),
  };
  const Homepage =
    variant === "a" ? Professional : variant === "b" ? Consumer : Compact;
  return (
    <div className={"design-app " + (dark ? "dark-design" : "")}>
      <div className="design-bar">
        <div className="design-brand">
          <span className="design-dot" /> U60 <span>首页风格预览</span>
          <span className="design-stage">01 / 设计探索</span>
        </div>
        <div className="design-controls">
          <div className="style-switch" aria-label="首页风格">
            {(["a", "b", "c"] as Variant[]).map((v) => (
              <button
                key={v}
                onClick={() => setVariant(v)}
                aria-pressed={variant === v}
              >
                <span>{v.toUpperCase()}</span>
                {directions[v].name}
              </button>
            ))}
          </div>
          <span className="design-separator" />
          <div className="view-switch" aria-label="预览界面">
            <button
              aria-pressed={view === "web"}
              onClick={() => setView("web")}
            >
              <Monitor size={15} />
              <span>Web</span>
            </button>
            <button
              aria-pressed={view === "screen"}
              onClick={() => setView("screen")}
            >
              <Smartphone size={15} />
              <span>设备屏幕</span>
            </button>
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
        <span>
          <b>
            {variant.toUpperCase()} / {directions[variant].name}
          </b>
          <span className="description-detail">
            {directions[variant].label}
          </span>
        </span>
        <span>
          首页设计稿 <i /> 示例数据
        </span>
      </div>
      <div
        id="product-preview"
        className={"variant-" + variant}
        data-theme={dark ? "dark" : "light"}
      >
        {view === "web" ? (
          <Homepage {...props} />
        ) : (
          <ScreenHomes notify={notify} />
        )}
      </div>
      {share && <ShareModal close={() => setShare(false)} />}
      <div className="design-bottom">
        <span>先确认视觉方向，再展开完整产品与两端层级。</span>
        <span>
          React + Tailwind <ChevronRight size={12} /> 纯本地预览
        </span>
      </div>
      {toast && (
        <div role="status" className="toast">
          <Check size={17} />
          <span>{toast}</span>
          <button aria-label="关闭提示" onClick={() => setToast("")}>
            <X size={16} />
          </button>
        </div>
      )}
    </div>
  );
}
