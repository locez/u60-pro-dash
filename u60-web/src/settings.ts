import { field, flag, options, type FormSpec } from "./forms";
import { actions, routingModel } from "./models";
const wifiWarning =
  "Applying wireless settings may disconnect this device. Reconnect using the new network name or password, then reopen the console.";
const mobileWarning =
  "Changing mobile settings can interrupt internet access. Roaming may incur carrier charges. Keep a local connection to the router.";
export const wifiStandard: FormSpec = {
  id: "wifi-standard",
  title: "Wireless standard",
  note: "Applies to both bands. Enables Wi-Fi 6 and Wi-Fi 7 for compatible clients. Turn off only if older devices cannot connect.",
  read: "/api/wifi/status",
  write: "/api/wifi/settings",
  fields: [flag("wifi6_switch", "Wi-Fi 7 (802.11be)")],
  interrupt: wifiWarning,
};
export function wifiSpec(band: "2g" | "5g", radio = false): FormSpec {
  const name = band === "2g" ? "2.4 GHz" : "5 GHz";
  return {
    id: "wifi-" + band + (radio ? "-radio" : ""),
    title: name,
    read: "/api/wifi/status",
    write: "/api/wifi/settings",
    fields: radio
      ? [
          options(
            "channel_" + band,
            "Channel",
            [
              "auto",
              ...(band === "2g"
                ? Array.from({ length: 13 }, (_, i) => String(i + 1))
                : [
                    "36",
                    "40",
                    "44",
                    "48",
                    "52",
                    "56",
                    "60",
                    "64",
                    "100",
                    "104",
                    "108",
                    "112",
                    "116",
                    "120",
                    "124",
                    "128",
                    "132",
                    "136",
                    "140",
                    "144",
                    "149",
                    "153",
                    "157",
                    "161",
                    "165",
                  ]),
            ].map((v) => [v, v === "auto" ? "Automatic" : v]),
          ),
          options(
            "htmode_" + band,
            "Channel width",
            [
              "auto",
              "EHT20",
              "EHT40",
              ...(band === "5g" ? ["EHT80", "EHT160"] : []),
            ].map((v) => [v, v === "auto" ? "Automatic" : v.slice(3) + " MHz"]),
          ),
          field("txpower_" + band, "Transmit power (%)", {
            type: "number",
            min: 1,
            max: 100,
          }),
          flag("hidden_" + band, "Hidden network"),
        ]
      : [
          options(
            band === "2g" ? "radio2_disabled" : "radio5_disabled",
            "Radio",
            [
              ["0", "Enabled"],
              ["1", "Disabled"],
            ],
          ),
          field("ssid_" + band, "Network name", { required: true }),
          options("encryption_" + band, "Security", [
            ["psk2", "WPA2"],
            ["sae-mixed", "WPA2 / WPA3"],
            ["sae", "WPA3"],
            ["none", "Open network"],
          ]),
          field("key_" + band, "New password", { type: "password" }),
        ],
    interrupt: wifiWarning,
    validate: (changed) => {
      for (const [k, v] of Object.entries(changed)) {
        if (k.startsWith("ssid_")) actions.wifiText(v, "SSID");
        if (k.startsWith("key_")) actions.wifiPassword(v);
      }
    },
  };
}
export const guest: FormSpec = {
  id: "guest",
  title: "Guest network",
  read: "/api/wifi/guest",
  fields: [
    options(
      "guest_disabled_2g",
      "2.4 GHz",
      [
        ["0", "Enabled"],
        ["1", "Disabled"],
      ],
      { readKey: "disabled_2g" },
    ),
    options(
      "guest_disabled_5g",
      "5 GHz",
      [
        ["0", "Enabled"],
        ["1", "Disabled"],
      ],
      { readKey: "disabled_5g" },
    ),
    field("guest_ssid", "Network name", { readKey: "ssid", required: true }),
    options(
      "guest_encryption",
      "Security",
      [
        ["psk2", "WPA2"],
        ["sae-mixed", "WPA2 / WPA3"],
        ["none", "Open network"],
      ],
      { readKey: "encryption" },
    ),
    field("guest_key", "New password", { readKey: "key", type: "password" }),
    { ...flag("guest_isolate", "Isolate guest devices"), readKey: "isolate" },
    { ...flag("guest_hidden", "Hidden network"), readKey: "hidden" },
    options(
      "guest_active_time",
      "Automatically turn off",
      [
        [0, "No time limit"],
        [30, "After 30 minutes"],
        [60, "After 1 hour"],
        [240, "After 4 hours"],
      ].map(([v, l]) => [String(v), String(l)]),
    ),
  ],
  interrupt: wifiWarning,
  validate: (c) => {
    if (c.guest_ssid) actions.wifiText(c.guest_ssid, "SSID");
    if (c.guest_key) actions.wifiPassword(c.guest_key);
  },
};
export const mobile: FormSpec = {
  id: "mobile-data",
  title: "Mobile data",
  read: "/api/modem/data",
  fields: [
    flag("enable", "Mobile data", "number"),
    flag("roam_enable", "Data roaming", "number"),
    options(
      "connect_mode",
      "Connection mode",
      [
        ["1", "Automatic"],
        ["0", "Manual"],
      ],
      { wire: "number" },
    ),
  ],
  body: (_, v) => ({
    cid: 1,
    enable: Number(v.enable),
    roam_enable: Number(v.roam_enable),
    connect_mode: Number(v.connect_mode),
    ...(Number(v.enable) === 0 ? { connect_status: "disconnected" } : {}),
  }),
  interrupt: mobileWarning,
};
export const networkMode: FormSpec = {
  id: "network-mode",
  title: "Preferred mobile network",
  read: "/api/network/signal",
  write: "/api/modem/network-mode",
  fields: [
    options("net_select", "Network preference", [
      ["WL_AND_5G", "Automatic · 5G / 4G / 3G"],
      ["LTE_AND_5G", "5G NSA · LTE + NR"],
      ["Only_5G", "5G SA only"],
      ["Only_LTE", "4G LTE only"],
      ["WCDMA_AND_LTE", "4G / 3G"],
      ["Only_WCDMA", "3G only"],
    ]),
  ],
  interrupt: mobileWarning,
};
export const lan: FormSpec = {
  id: "lan",
  title: "LAN & DHCP",
  read: "/api/router/lan",
  decode: (d) => routingModel.normalizeLAN(d),
  body: (_, v) => routingModel.lanBody(v),
  fields: [
    field("lan_ipaddr", "Router address", { required: true }),
    field("lan_netmask", "Subnet mask", { required: true }),
    flag("dhcp_enable", "DHCP service"),
    field("dhcp_start", "DHCP start"),
    field("dhcp_end", "DHCP end"),
    field("dhcp_lease_time", "Lease time"),
  ],
  interrupt: (c) =>
    "lan_ipaddr" in c || "lan_netmask" in c
      ? "Changing the LAN address will interrupt management connections. Reconnect using the new address."
      : undefined,
  validate: (_, v) => {
    routingModel.ipv4(v.lan_ipaddr, "Router address");
    routingModel.ipv4(v.lan_netmask, "Subnet mask");
  },
};
export const charge: FormSpec = {
  id: "charge",
  title: "Battery charge limit",
  read: "/api/device/charge-control",
  fields: [
    flag("charge_limit_enabled", "Charge limit", "boolean"),
    field("charge_limit", "Stop at (%)", {
      type: "number",
      wire: "number",
      min: 50,
      max: 100,
    }),
    field("hysteresis", "Resume below limit by (%)", {
      type: "number",
      wire: "number",
      min: 1,
      max: 20,
    }),
    options(
      "charging_stopped",
      "Charging",
      [
        ["false", "Allow charging"],
        ["true", "Stop charging"],
      ],
      { wire: "boolean" },
    ),
  ],
};
export const fastBoot: FormSpec = {
  id: "fast-boot",
  title: "Fast boot",
  read: "/api/device/fast-boot",
  fields: [flag("fast_boot", "Fast boot")],
};
export const powerSave: FormSpec = {
  id: "power-save",
  title: "Power saver",
  read: "/api/device/power-save",
  readMethod: "POST",
  readBody: { deviceInfoList: ["power_saver_mode"] },
  fields: [flag("power_saver_mode", "Power saver")],
  body: (c) => ({ deviceInfoList: c }),
};
export const autoSleep: FormSpec = {
  id: "auto-sleep",
  title: "Automatic sleep",
  read: "/api/device/auto-sleep",
  fields: [
    flag("enabled", "Automatic sleep", "boolean"),
    field("timeout", "Inactivity timeout (minutes)", {
      type: "number",
      min: -1,
      max: 1440,
      wire: "number",
    }),
  ],
  body: (_, v) => ({
    enabled: v.enabled === true || v.enabled === "true",
    timeout: String(v.timeout),
  }),
  interrupt:
    "Automatic sleep may disconnect clients and make the console unavailable. Wake the device using its physical controls if needed.",
};
export const firewall: FormSpec[] = [
  {
    id: "firewall",
    title: "Firewall protection",
    read: "/api/router/firewall",
    write: "/api/router/firewall/switch",
    fields: [flag("firewall_switch", "Firewall protection")],
  },
  {
    id: "firewall-level",
    title: "Protection level",
    read: "/api/router/firewall",
    write: "/api/router/firewall/level",
    fields: [
      options("firewall_level", "Protection level", [
        ["low", "Low"],
        ["medium", "Medium"],
        ["high", "High"],
      ]),
    ],
  },
  {
    id: "nat",
    title: "NAT",
    read: "/api/router/firewall",
    write: "/api/router/firewall/nat",
    fields: [flag("nat_switch", "NAT")],
  },
  {
    id: "upnp",
    title: "UPnP",
    read: "/api/router/firewall/upnp",
    fields: [flag("upnp_switch", "UPnP")],
  },
  {
    id: "dmz",
    title: "DMZ host",
    read: "/api/router/firewall",
    write: "/api/router/firewall/dmz",
    fields: [
      flag("dmz_enabled", "DMZ"),
      field("dmz_ip", "Destination IPv4 address"),
    ],
    body: (_, v) => ({ dmz_enabled: String(v.dmz_enabled), dmz_ip: v.dmz_ip }),
    validate: (_, v) => {
      if (String(v.dmz_enabled) === "1") routingModel.ipv4(v.dmz_ip, "DMZ");
    },
  },
];
