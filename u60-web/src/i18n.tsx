import { useSyncExternalStore } from "react";
import productMessages from "./messages.json";
export const languages = {
  "zh-Hans": "简体中文",
  "zh-Hant": "繁體中文",
  en: "English",
  ja: "日本語",
  ko: "한국어",
};
export type Locale = keyof typeof languages;
const messages: Record<string, string[]> = {
  Overview: ["概览", "概覽", "概要", "개요"],
  Connections: ["连接", "連線", "接続", "연결"],
  "Connected devices": ["连接设备", "連線裝置", "接続端末", "연결된 기기"],
  "Mobile network": ["移动网络", "行動網路", "モバイル回線", "모바일 네트워크"],
  "Networking & services": [
    "组网与服务",
    "組網與服務",
    "ネットワークとサービス",
    "네트워크 및 서비스",
  ],
  "Local network": [
    "局域网",
    "區域網路",
    "ローカルネットワーク",
    "로컬 네트워크",
  ],
  "System settings": ["系统设置", "系統設定", "システム設定", "시스템 설정"],
  Everyday: ["日常使用", "日常使用", "日常の管理", "일상 관리"],
  "Network management": [
    "网络管理",
    "網路管理",
    "ネットワーク管理",
    "네트워크 관리",
  ],
  "Device management": ["设备管理", "裝置管理", "端末管理", "기기 관리"],
  "Router overview": [
    "路由器概览",
    "路由器概覽",
    "ルーターの概要",
    "라우터 개요",
  ],
  "A clear view of your connection.": [
    "掌握连接与设备状态。",
    "掌握連線與裝置狀態。",
    "接続と端末の状態を確認します。",
    "연결 및 기기 상태를 확인하세요.",
  ],
  "Home layout": ["首页布局", "首頁配置", "ホームの配置", "홈 레이아웃"],
  "Screen appearance": ["屏幕外观", "螢幕外觀", "画面の外観", "화면 모양"],
  Connecting: ["正在连接", "正在連線", "接続中", "연결 중"],
  Reconnecting: ["正在重连", "正在重新連線", "再接続中", "다시 연결 중"],
  Live: ["已连接", "已連線", "接続済み", "연결됨"],
  Paused: ["已暂停", "已暫停", "一時停止", "일시 중지"],
  "Network topology": ["连接拓扑", "連線拓撲", "接続構成", "연결 구성"],
  "Traffic trend": ["速率趋势", "速率趨勢", "通信速度の推移", "속도 추이"],
  "No samples yet": [
    "尚无采样数据",
    "尚無取樣資料",
    "データ待ち",
    "샘플 대기 중",
  ],
  "Settings apply automatically.": [
    "修改后自动应用。",
    "修改後自動套用。",
    "変更は自動的に適用されます。",
    "변경 사항이 자동 적용됩니다.",
  ],
  "Applying…": ["正在应用…", "正在套用…", "適用中…", "적용 중…"],
  "Applied and verified": [
    "已生效并回读确认",
    "已生效並讀回確認",
    "適用と確認が完了しました",
    "적용 및 확인 완료",
  ],
  "Complete input to apply": [
    "完成输入后应用",
    "完成輸入後套用",
    "入力完了時に適用",
    "입력 완료 시 적용",
  ],
  "Cancel change": ["取消修改", "取消修改", "変更を取り消す", "변경 취소"],
  "Continue and apply": ["继续应用", "繼續套用", "適用する", "계속 적용"],
  "This change interrupts connections": [
    "此修改会中断连接",
    "此修改會中斷連線",
    "接続が中断されます",
    "연결이 끊길 수 있습니다",
  ],
  "The router has not confirmed the change.": [
    "设备尚未确认修改已生效。",
    "裝置尚未確認修改已生效。",
    "端末が変更を確認していません。",
    "기기가 변경을 확인하지 않았습니다.",
  ],
  "Session expired": [
    "会话已过期，请重新登录",
    "工作階段已過期，請重新登入",
    "再度ログインしてください",
    "세션이 만료되었습니다",
  ],
  "Cannot reach the router": [
    "无法连接路由器",
    "無法連線路由器",
    "ルーターに接続できません",
    "라우터에 연결할 수 없습니다",
  ],
  "Read current state": [
    "读取当前状态",
    "讀取目前狀態",
    "現在の状態を確認",
    "현재 상태 읽기",
  ],
  "Finish editing": ["完成编辑", "完成編輯", "編集を完了", "편집 완료"],
  "Battery & power": [
    "电源与电池",
    "電源與電池",
    "電源とバッテリー",
    "전원 및 배터리",
  ],
  "Screen & security": [
    "屏幕与安全",
    "螢幕與安全",
    "画面とセキュリティ",
    "화면 및 보안",
  ],
  "Updates & backup": [
    "更新与备份",
    "更新與備份",
    "更新とバックアップ",
    "업데이트 및 백업",
  ],
  Diagnostics: ["诊断工具", "診斷工具", "診断ツール", "진단 도구"],
  Automation: ["自动化", "自動化", "自動化", "자동화"],
  "Hotspot settings": [
    "热点设置",
    "熱點設定",
    "アクセスポイント",
    "핫스팟 설정",
  ],
  "Wireless parameters": ["无线参数", "無線參數", "無線設定", "무선 설정"],
  "Connection settings": ["连接设置", "連線設定", "接続設定", "연결 설정"],
  "Signal & cells": ["信号与频段", "訊號與頻段", "電波とセル", "신호 및 셀"],
  "SIM & eSIM": ["SIM 与 eSIM", "SIM 與 eSIM", "SIM と eSIM", "SIM 및 eSIM"],
  "Carrier services": [
    "运营商服务",
    "電信業者服務",
    "通信事業者サービス",
    "통신사 서비스",
  ],
  "Access & forwarding": [
    "访问与转发",
    "存取與轉發",
    "アクセスと転送",
    "액세스 및 포워딩",
  ],
  "Log out": ["退出登录", "登出", "ログアウト", "로그아웃"],
  "China Unicom": ["中国联通", "中國聯通", "中国聯通", "차이나유니콤"],
  "China Mobile": ["中国移动", "中國移動", "中国移動", "차이나모바일"],
  "China Telecom": ["中国电信", "中國電信", "中国電信", "차이나텔레콤"],
  "China Broadnet": ["中国广电", "中國廣電", "中国広電", "차이나브로드넷"],
};
function resolve(code: string): Locale {
  if (/^zh/i.test(code))
    return /hant|tw|hk|mo/i.test(code) ? "zh-Hant" : "zh-Hans";
  return /^ja/i.test(code) ? "ja" : /^ko/i.test(code) ? "ko" : "en";
}
let locale: Locale = resolve(navigator.language);
try {
  const value = localStorage.getItem("openu60.language");
  if (value && value in languages) locale = value as Locale;
} catch {}
const listeners = new Set<() => void>();
export function setLocale(value: Locale) {
  locale = value;
  document.documentElement.lang = value;
  try {
    localStorage.setItem("openu60.language", value);
  } catch {}
  listeners.forEach((fn) => fn());
}
export function useLocale() {
  return useSyncExternalStore(
    (fn) => {
      listeners.add(fn);
      return () => {
        listeners.delete(fn);
      };
    },
    () => locale,
  );
}
export function t(source: string, ...values: unknown[]) {
  if (source == null) return "—";
  const index = ["zh-Hans", "zh-Hant", "ja", "ko"].indexOf(locale);
  const text =
    (index >= 0 ? messages[source]?.[index] : undefined) ||
    (productMessages as Record<string, Record<string, string>>)[source]?.[
      locale
    ] ||
    source;
  return text.replace(/\{(\d+)\}/g, (match, i) =>
    Number(i) < values.length ? String(values[Number(i)]) : match,
  );
}
const states: Record<string, string> = {
  Running: "Running",
  Stopped: "Stopped",
  NeedsLogin: "Sign in required",
  NeedsMachineAuth: "Admin approval required",
  NoState: "Not configured",
  Starting: "Starting",
  idle: "Idle",
  running: "Running",
  starting: "Starting",
  disabled: "Disabled",
  enabled: "Enabled",
  not_installed: "Not installed",
  updating: "Updating",
  respawning: "Starting",
  connected: "Connected",
  disconnected: "Disconnected",
  succeeded: "Completed",
  failed: "Failed",
  checking: "Checking",
  downloading: "Downloading",
  installing: "Installing",
  verifying: "Verifying",
  queued: "Queued",
  rule: "Rule",
  global: "Global",
  direct: "Direct",
  local: "Local HTTP / SOCKS clients",
  hotspot: "Wi-Fi hotspot traffic",
  automatic: "Automatic",
  manual: "Manual",
  doh: "DNS over HTTPS",
  dot: "DNS over TLS",
  kernel: "Kernel gateway",
  userspace: "Device access only",
  "sim ready": "Ready",
  "wait pin": "PIN required",
  "wait puk": "PUK required",
  pending: "Pending",
  success: "Completed",
  unknown: "Unknown",
};
export function stateText(value: unknown) {
  return typeof value === "string" ? t(states[value] || value) : value;
}
export function carrier(value: unknown) {
  const raw = typeof value === "string" ? value.trim() : "";
  const key = raw.replace(/[\s_-]/g, "").toUpperCase();
  return (
    t(
      (
        {
          UNICOM: "China Unicom",
          CUCC: "China Unicom",
          CHINAUNICOM: "China Unicom",
          中国联通: "China Unicom",
          中國聯通: "China Unicom",
          CMCC: "China Mobile",
          CHINAMOBILE: "China Mobile",
          中国移动: "China Mobile",
          中國移動: "China Mobile",
          CTCC: "China Telecom",
          CHINATELECOM: "China Telecom",
          中国电信: "China Telecom",
          中國電信: "China Telecom",
          CBN: "China Broadnet",
          CHINABROADNET: "China Broadnet",
          中国广电: "China Broadnet",
          中國廣電: "China Broadnet",
        } as Record<string, string>
      )[key] || raw,
    ) || t("Not reported")
  );
}
