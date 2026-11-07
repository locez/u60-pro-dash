import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import type { Page } from "./data";
import featureData from "./features.json";
import { serviceNames } from "./presentation";
import { readScreenAccent, rememberScreenAccent } from "./AccentPicker";
import { useOperatingModes } from "./modes";
import {
  readHomeLayout,
  rememberHomeLayout,
  readScreenTheme,
  rememberScreenTheme,
} from "./screenPreferences";
export { serviceNames } from "./presentation";

export const features = featureData.map((feature) => ({
  ...feature,
  tab: feature.tab === "规则代理" ? serviceNames[2] : feature.tab,
})) as {
  id: string;
  name: string;
  web: Page;
  tab: string;
  screen: string;
  section: string;
}[];
export type Route = { page: Page; tab?: string; service?: number };
export type Scenario = "normal" | "offline" | "euicc";
export const defaultTabs: Record<Page, string> = {
  overview: "",
  wifi: "热点设置",
  devices: "在线设备",
  internet: "信号与频段",
  services: "概览",
  messages: "收件箱",
  lan: "LAN 与 DHCP",
  system: "电源与电池",
};
export const pageTabs: Partial<Record<Page, string[]>> = {
  wifi: ["热点设置", "访客网络", "无线参数"],
  devices: ["在线设备", "历史设备"],
  internet: ["信号与频段", "连接设置", "SIM 与 eSIM", "流量套餐", "运营商服务"],
  messages: ["收件箱", "转发规则", "存储管理"],
  lan: ["LAN 与 DHCP", "DNS", "访问与转发"],
  system: ["电源与电池", "屏幕与安全", "更新与备份", "诊断工具", "自动化"],
};
export type Fields = Record<string, string | boolean>;
export const initialSettings: Record<string, Fields> = {
  wifi: {
    enabled: true,
    unified: true,
    ssid: "U60 Studio",
    ssid5: "U60 Studio 5G",
    password: "sample-pass-2026",
    security: "WPA2 / WPA3",
  },
  guest: {
    enabled: false,
    ssid: "U60 Guest",
    password: "guest-demo-2026",
    isolated: true,
    duration: "4 小时",
  },
  power: {
    mode: "智能充电",
    limit: "90%",
    sleep: "1 分钟",
    fast: true,
  },
  security: {
    pin: false,
    timeout: "1 分钟",
    brightness: "60%",
    theme: "深色",
    language: "简体中文",
    accent: "#2563eb",
  },
  tailscale: {
    routes: true,
    dns: false,
    lan: true,
    exit: "不使用退出节点",
    target: "192.0.2.80:443",
  },
  proxyRuntime: {
    mode: "规则",
    scope: "Wi-Fi 热点",
    port: "7890",
    ipv6: true,
    concurrent: true,
  },
};
function useModelState() {
  const modes = useOperatingModes();
  const params = new URLSearchParams(location.search);
  const validPages = [
    "overview",
    "wifi",
    "devices",
    "internet",
    "services",
    "messages",
    "lan",
    "system",
  ];
  const [route, setRoute] = useState<Route>({
    page:
      params.get("page") === "system" && params.get("tab") === "运行模式"
        ? "internet"
        : validPages.includes(params.get("page") || "")
          ? (params.get("page") as Page)
          : "overview",
    tab:
      params.get("page") === "system" && params.get("tab") === "运行模式"
        ? "连接设置"
        : params.get("tab") || undefined,
    service: params.has("service")
      ? Math.min(2, Math.max(0, Number(params.get("service")) || 0))
      : undefined,
  });
  const [settings, setSettings] = useState<Record<string, Fields>>(() => ({
    ...initialSettings,
    security: {
      ...initialSettings.security,
      accent: readScreenAccent(),
      theme: readScreenTheme(),
    },
  }));
  useEffect(() => {
    rememberScreenAccent(String(settings.security.accent));
  }, [settings.security.accent]);
  useEffect(
    () => rememberScreenTheme(String(settings.security.theme)),
    [settings.security.theme],
  );
  const [homeLayout, setHomeLayout] = useState(readHomeLayout);
  useEffect(() => rememberHomeLayout(homeLayout), [homeLayout]);
  const [enabled, setEnabled] = useState([true, true, true]);
  const [scenario, setScenario] = useState<Scenario>("normal");
  const [toast, setToast] = useState("");
  const [share, setShare] = useState(false);
  return {
    modes,
    route,
    go: setRoute,
    settings,
    homeLayout,
    setHomeLayout,
    save: (key: string, values: Fields) =>
      setSettings((s) => ({ ...s, [key]: values })),
    enabled,
    toggle: (index: number) =>
      setEnabled((s) => s.map((v, i) => (i === index ? !v : v))),
    scenario,
    setScenario,
    toast,
    notify: setToast,
    share,
    setShare,
  };
}
const Model = createContext<ReturnType<typeof useModelState> | null>(null);
export function ModelProvider({ children }: { children: ReactNode }) {
  return <Model.Provider value={useModelState()}>{children}</Model.Provider>;
}
export function useModel() {
  const model = useContext(Model);
  if (!model) throw new Error("Missing design model");
  return model;
}
export function featureRoute(feature: (typeof features)[number]): Route {
  if (feature.web === "services")
    return {
      page: "services",
      service:
        feature.tab === "Tailscale" ? 0 : feature.tab === "WireGuard" ? 1 : 2,
      tab: "概览",
    };
  const aliases: Record<string, string> = {
    SIM与eSIM: "SIM 与 eSIM",
    全部设备: "在线设备",
    高级设置: "连接设置",
    设备与维护: "更新与备份",
    转发记录: "转发规则",
    连接记录: "信号与频段",
  };
  return { page: feature.web, tab: aliases[feature.tab] || feature.tab };
}
