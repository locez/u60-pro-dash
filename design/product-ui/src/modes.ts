import { useEffect, useRef, useState } from "react";

export const performanceModes = [
  {
    id: "balanced",
    label: "均衡",
    hint: "按负载调节，适合日常使用",
    detail: "在响应速度、温度与功耗之间保持平衡。",
  },
  {
    id: "performance",
    label: "性能",
    hint: "优先响应与持续处理能力",
    detail: "适合多设备与高负载，发热和耗电可能增加。",
  },
  {
    id: "saving",
    label: "省电",
    hint: "优先降低功耗，延长移动使用时间",
    detail: "高负载时的处理速度与转发吞吐可能下降。",
  },
] as const;
export type PerformanceMode = (typeof performanceModes)[number]["id"];
export const bandwidthModes = [
  { id: "normal", label: "极速", hint: "应用自由竞争带宽，不指定应用优先级" },
  { id: "ai", label: "AI 智能", hint: "由原厂策略按应用使用情况分配带宽" },
  { id: "game", label: "游戏优先", hint: "优先保障游戏流量" },
  { id: "video", label: "视频优先", hint: "优先保障视频应用流量" },
  { id: "meeting", label: "会议优先", hint: "优先保障会议与网课流量" },
  { id: "live", label: "直播优先", hint: "优先保障直播上行流量" },
] as const;
export type BandwidthMode = (typeof bandwidthModes)[number]["id"];
export const modeScenarios = [
  ["normal", "正常状态"],
  ["rail", "高铁自动优化中"],
  ["thermal", "温度保护中"],
  ["unknown", "状态暂不可用"],
  ["apply-error", "设置应用失败"],
  ["recovery-error", "蜂窝恢复失败"],
] as const;
export type ModeScenario = (typeof modeScenarios)[number][0];

// Design-only state shared by the Web and screen previews. No device API calls.
export function useOperatingModes() {
  const [airplane, setAirplane] = useState(false);
  const [performance, setPerformance] = useState<PerformanceMode>("balanced");
  const [bandwidth, setBandwidth] = useState<BandwidthMode>("normal");
  const [phase, setPhase] = useState<"idle" | "applying" | "restoring">("idle");
  const [scenario, setScenario] = useState<ModeScenario>("normal");
  const [error, setError] = useState("");
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );
  const setScene = (next: ModeScenario) => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    setPhase("idle");
    setError("");
    setScenario(next);
    if (next === "rail") setAirplane(false);
    if (next === "recovery-error") setAirplane(true);
  };
  const changeAirplane = (next: boolean) => {
    if (phase !== "idle" || scenario === "unknown") return;
    setError("");
    setPhase(next ? "applying" : "restoring");
    timer.current = setTimeout(() => {
      timer.current = null;
      setPhase("idle");
      if (!next && scenario === "recovery-error") {
        setError(
          "蜂窝网络尚未恢复。可以重试或进入移动网络诊断；Wi-Fi 管理仍可使用。",
        );
      } else if (scenario === "apply-error") {
        setError("设置未能应用，已保留原来的模式。请稍后重试。");
      } else {
        setAirplane(next);
      }
    }, 900);
  };
  const apply = (fn: () => void) => {
    if (scenario === "unknown") return;
    if (scenario === "apply-error")
      setError("设置未能应用，已保留原来的模式。请稍后重试。");
    else {
      fn();
      setError("");
    }
  };
  const profile = performanceModes.find((p) => p.id === performance)!;
  const strategy = bandwidthModes.find((p) => p.id === bandwidth)!;
  return {
    airplane,
    performance,
    bandwidth,
    profile,
    strategy,
    phase,
    scenario,
    error,
    busy: phase !== "idle",
    unknown: scenario === "unknown",
    rail: scenario === "rail" && !airplane,
    thermal: scenario === "thermal",
    changeAirplane,
    setScene,
    changePerformance: (value: PerformanceMode) =>
      apply(() => setPerformance(value)),
    changeBandwidth: (value: BandwidthMode) => apply(() => setBandwidth(value)),
  };
}
