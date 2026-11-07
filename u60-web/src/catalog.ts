import {
  LayoutDashboard,
  Wifi,
  Smartphone,
  Radio,
  MessageSquare,
  Network,
  Router,
  SlidersHorizontal,
} from "lucide-react";
export const navigation = [
  {
    id: "overview",
    label: "Overview",
    title: "Router overview",
    section: "Everyday",
    icon: LayoutDashboard,
    tabs: [],
  },
  {
    id: "wifi",
    label: "Wi-Fi",
    title: "Wi-Fi",
    section: "Everyday",
    icon: Wifi,
    tabs: [
      ["hotspot", "Hotspot settings"],
      ["guest", "Guest network"],
      ["radio", "Wireless parameters"],
    ],
  },
  {
    id: "devices",
    label: "Connected devices",
    title: "Connected devices",
    section: "Everyday",
    icon: Smartphone,
    tabs: [
      ["online", "Online devices"],
      ["all", "All devices"],
    ],
  },
  {
    id: "internet",
    label: "Mobile network",
    title: "Mobile network",
    section: "Everyday",
    icon: Radio,
    tabs: [
      ["signal", "Signal & cells"],
      ["connection", "Connection settings"],
      ["sim", "SIM & eSIM"],
      ["usage", "Data usage"],
      ["carrier", "Carrier services"],
    ],
  },
  {
    id: "messages",
    label: "SMS",
    title: "Messages",
    section: "Everyday",
    icon: MessageSquare,
    tabs: [
      ["inbox", "Inbox"],
      ["compose", "New message"],
      ["forward", "Forwarding rules"],
      ["storage", "Message storage"],
    ],
  },
  {
    id: "services",
    label: "Networking & services",
    title: "Networking & services",
    section: "Network management",
    icon: Network,
    tabs: [],
  },
  {
    id: "lan",
    label: "Local network",
    title: "Local network",
    section: "Network management",
    icon: Router,
    tabs: [
      ["lan", "LAN & DHCP"],
      ["dns", "DNS"],
      ["firewall", "Access & forwarding"],
    ],
  },
  {
    id: "system",
    label: "System settings",
    title: "System settings",
    section: "Device management",
    icon: SlidersHorizontal,
    tabs: [
      ["power", "Battery & power"],
      ["screen", "Screen & security"],
      ["update", "Updates & backup"],
      ["diagnostics", "Diagnostics"],
      ["automation", "Automation"],
    ],
  },
] as const;
export type Page = (typeof navigation)[number]["id"];
export type Route = { page: Page; tab?: string; detail?: string };
export function readRoute(): Route {
  const hash = location.hash.slice(1);
  const aliases: Record<string, string> = {
    "network/wifi": "wifi",
    "network/clients": "devices",
    "network/mobile": "internet/connection",
    "network/signal": "internet/signal",
    "network/bands": "internet/signal",
    "network/apn": "internet/connection",
    "network/esim": "internet/sim",
    "network/sim": "internet/sim",
    "network/lan": "lan/lan",
    "network/dns": "lan/dns",
    "network/firewall": "lan/firewall",
    "network/qos": "lan/firewall",
    "network/tailscale": "services/tailscale",
    "network/wireguard": "services/wireguard",
    "network/proxy": "services/mihomo",
    "sms/inbox": "messages/inbox",
    "sms/compose": "messages/compose",
    "sms/forwarding": "messages/forward",
    "tools/battery": "system/power",
    "tools/device": "system/update",
    "tools/traffic": "internet/usage",
    "tools/diagnostics": "system/diagnostics",
    "tools/terminal": "system/diagnostics/terminal",
    "tools/speedtest": "system/diagnostics/speedtest",
    "tools/lan-test": "system/diagnostics/lan-test",
    "settings/device": "system/update",
    "settings/appearance": "system/screen",
    "settings/https": "system/screen",
    "settings/connection": "system/screen",
  };
  const [p, tab, detail] = (aliases[hash] || hash).split("/");
  const page = navigation.some((n) => n.id === p) ? (p as Page) : "overview";
  return { page, tab, detail };
}
export function go(page: Page, tab?: string, detail?: string) {
  location.hash = [page, tab, detail].filter(Boolean).join("/");
}
export function topics(route: Route) {
  const base = ["operating", "service"];
  switch (route.page) {
    case "overview":
      return [
        ...base,
        "signal",
        "traffic",
        "health",
        "battery",
        "clients",
        "charger",
        "wan",
      ];
    case "devices":
      return [...base, "clients"];
    case "internet":
      return [
        ...base,
        "signal",
        "modem",
        "sim",
        ...(route.tab === "sim"
          ? ["sim_identity", "esim_switch", "esim_download"]
          : []),
      ];
    case "messages":
      return route.tab === "inbox" ? [...base, "sms"] : base;
    case "system":
      return route.tab === "power" || !route.tab
        ? [...base, "health", "battery", "charger", "battery_info", "usb"]
        : route.tab === "diagnostics"
          ? [
              ...base,
              "health",
              ...(route.detail === "processes" ? ["processes"] : []),
            ]
          : base;
    default:
      return base;
  }
}
