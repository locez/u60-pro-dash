export const screenThemes = ["深色", "浅色", "跟随系统"] as const;
export type ScreenTheme = (typeof screenThemes)[number];
export const metricOptions = [
  {
    id: "clients",
    name: "在线设备",
    label: "在线设备",
    value: "6",
    root: "连接",
    path: ["连接设备"],
  },
  {
    id: "traffic",
    name: "本月已用流量",
    label: "本月 / GB",
    value: "62.8",
    root: "状态",
    path: ["流量套餐"],
  },
  {
    id: "remaining",
    name: "套餐剩余流量",
    label: "剩余 / GB",
    value: "37.2",
    root: "状态",
    path: ["流量套餐"],
  },
  {
    id: "cpu",
    name: "CPU 使用率",
    label: "CPU",
    value: "12%",
    root: "管理",
    path: ["诊断工具"],
  },
  {
    id: "memory",
    name: "内存使用率",
    label: "内存",
    value: "44%",
    root: "管理",
    path: ["诊断工具"],
  },
  {
    id: "temperature",
    name: "设备温度",
    label: "温度",
    value: "39°",
    root: "管理",
    path: ["诊断工具"],
  },
  {
    id: "battery",
    name: "电池电量",
    label: "电量",
    value: "82%",
    root: "管理",
    path: ["电源与电池"],
  },
  {
    id: "uptime",
    name: "运行时间",
    label: "运行时间",
    value: "2天6时",
    root: "管理",
    path: ["设备设置"],
  },
] as const;
export type MetricId = (typeof metricOptions)[number]["id"] | "none";
export const serviceIds = ["tailscale", "wireguard", "proxy"] as const;
export type ScreenHomeLayout = {
  signal: boolean;
  wifi: boolean;
  services: boolean;
  serviceVisibility: Record<(typeof serviceIds)[number], boolean>;
  metrics: [MetricId, MetricId, MetricId];
};
export const defaultHomeLayout: ScreenHomeLayout = {
  signal: true,
  wifi: true,
  services: true,
  serviceVisibility: { tailscale: true, wireguard: true, proxy: true },
  metrics: ["clients", "traffic", "temperature"],
};
const homeKey = "u60-design-screen-home-v1";
const themeKey = "u60-design-screen-theme";
export function readHomeLayout(): ScreenHomeLayout {
  try {
    const data = JSON.parse(localStorage.getItem(homeKey) || "null");
    if (!data || typeof data !== "object")
      return structuredClone(defaultHomeLayout);
    const layout = structuredClone(defaultHomeLayout);
    for (const key of ["signal", "wifi", "services"] as const) {
      if (typeof data[key] === "boolean") layout[key] = data[key];
    }
    for (const key of serviceIds) {
      if (typeof data.serviceVisibility?.[key] === "boolean")
        layout.serviceVisibility[key] = data.serviceVisibility[key];
    }
    const used = new Set<string>();
    layout.metrics = layout.metrics.map((fallback, index) => {
      const candidate = data.metrics?.[index];
      const valid =
        candidate === "none" || metricOptions.some((m) => m.id === candidate);
      const metric = valid ? (candidate as MetricId) : fallback;
      if (metric !== "none" && used.has(metric)) return "none";
      used.add(metric);
      return metric;
    }) as ScreenHomeLayout["metrics"];
    return layout;
  } catch {
    return structuredClone(defaultHomeLayout);
  }
}
export function rememberHomeLayout(layout: ScreenHomeLayout) {
  try {
    localStorage.setItem(homeKey, JSON.stringify(layout));
  } catch {
    /* Optional preview preference. */
  }
}
export function replaceMetric(
  layout: ScreenHomeLayout,
  index: number,
  next: MetricId,
): ScreenHomeLayout {
  const metrics = [...layout.metrics] as ScreenHomeLayout["metrics"];
  const previousIndex = next === "none" ? -1 : metrics.indexOf(next);
  if (previousIndex !== -1) metrics[previousIndex] = metrics[index];
  metrics[index] = next;
  return { ...layout, metrics };
}
export function readScreenTheme(): ScreenTheme {
  try {
    const theme = localStorage.getItem(themeKey);
    if (screenThemes.includes(theme as ScreenTheme))
      return theme as ScreenTheme;
  } catch {
    /* Optional preview preference. */
  }
  return "深色";
}
export function rememberScreenTheme(theme: string) {
  if (!screenThemes.includes(theme as ScreenTheme)) return;
  try {
    localStorage.setItem(themeKey, theme);
  } catch {
    /* Optional preview preference. */
  }
}
