import type { Fields } from "./model";
import { serviceNames } from "./presentation";

export function wifiInterruption(next: Fields, previous: Fields) {
  if (
    previous.enabled &&
    Object.keys(next).some((key) => next[key] !== previous[key])
  )
    return "需要重新配置无线网络，Wi-Fi 连接会短暂中断。请使用修改后的名称和密码重新连接；USB 管理连接不受影响。";
}

// The prototype shares one interruption rule between web and screen.
// The real adapter must derive this from the running service's capabilities.
export function proxyRuntimeInterruption(
  next: Fields,
  previous: Fields,
  running: boolean,
) {
  if (running && ["scope", "port"].some((key) => next[key] !== previous[key])) {
    return `需要先停止 ${serviceNames[2]}，应用设置后重新启动。当前代理连接会短暂中断。`;
  }
}
