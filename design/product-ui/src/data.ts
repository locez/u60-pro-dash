import {
  LayoutDashboard,
  Wifi,
  Laptop,
  Signal,
  Network,
  MessageSquare,
  SlidersHorizontal,
  Settings2,
} from "lucide-react";
export type Page =
  | "overview"
  | "wifi"
  | "devices"
  | "internet"
  | "services"
  | "messages"
  | "lan"
  | "system";
export const navigation = [
  {
    id: "overview" as Page,
    label: "概览",
    icon: LayoutDashboard,
    section: "日常使用",
    description: "所有连接，尽在眼前。",
  },
  {
    id: "wifi" as Page,
    label: "Wi-Fi",
    icon: Wifi,
    section: "日常使用",
    description: "一个网络名称，连接你的每一天。",
  },
  {
    id: "devices" as Page,
    label: "连接设备",
    icon: Laptop,
    section: "日常使用",
    description: "了解谁在使用网络，以及怎样使用。",
  },
  {
    id: "internet" as Page,
    label: "移动网络",
    icon: Signal,
    section: "日常使用",
    description: "管理蜂窝连接、SIM 卡与流量套餐。",
  },
  {
    id: "messages" as Page,
    label: "短信",
    icon: MessageSquare,
    section: "日常使用",
    description: "收发信息、验证码与套餐通知。",
  },
  {
    id: "services" as Page,
    label: "组网与服务",
    icon: Network,
    section: "网络管理",
    description: "独立管理每个连接服务，让不同网络协同工作。",
  },
  {
    id: "lan" as Page,
    label: "局域网",
    icon: SlidersHorizontal,
    section: "网络管理",
    description: "管理地址分配、解析和网络访问规则。",
  },
  {
    id: "system" as Page,
    label: "系统设置",
    icon: Settings2,
    section: "设备管理",
    description: "让设备以适合你的方式运行。",
  },
];
export const clients = [
  {
    name: "MacBook Pro",
    kind: "laptop",
    ip: "192.0.2.12",
    band: "5 GHz",
    download: 64.2,
    upload: 5.8,
    online: true,
    tag: "当前设备",
  },
  {
    name: "iPhone 16",
    kind: "phone",
    ip: "192.0.2.18",
    band: "5 GHz",
    download: 28.7,
    upload: 2.4,
    online: true,
    tag: "",
  },
  {
    name: "iPad Air",
    kind: "tablet",
    ip: "192.0.2.21",
    band: "5 GHz",
    download: 12.6,
    upload: 1.2,
    online: true,
    tag: "",
  },
  {
    name: "客厅音箱",
    kind: "speaker",
    ip: "192.0.2.24",
    band: "2.4 GHz",
    download: 0.8,
    upload: 0.1,
    online: true,
    tag: "",
  },
  {
    name: "工作电脑",
    kind: "laptop",
    ip: "192.0.2.36",
    band: "USB",
    download: 18.3,
    upload: 2.9,
    online: true,
    tag: "",
  },
  {
    name: "阅读灯",
    kind: "bulb",
    ip: "192.0.2.40",
    band: "2.4 GHz",
    download: 0.0,
    upload: 0.0,
    online: true,
    tag: "",
  },
  {
    name: "访客手机",
    kind: "phone",
    ip: "192.0.2.42",
    band: "访客网络",
    download: 0,
    upload: 0,
    online: false,
    tag: "昨日连接",
  },
];
export const traffic = [
  25, 32, 29, 40, 38, 55, 48, 60, 58, 44, 50, 75, 69, 82, 67, 74, 90, 81, 102,
  86, 93, 117, 105, 98, 112, 106, 121, 103, 118, 111, 130, 124,
];
