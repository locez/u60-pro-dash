// Integration tests only use an in-memory router, never a connected device.
const { chromium } = require("playwright");
const assert = require("node:assert/strict");
const token = "a".repeat(64);
const wifi = {
  wifi_onoff: "1",
  radio2_disabled: "0",
  radio5_disabled: "0",
  ssid_2g: "Test hotspot",
  ssid_5g: "Test hotspot",
  key_2g: "test-only-wifi",
  key_5g: "test-only-wifi",
  encryption_2g: "psk2",
  encryption_5g: "psk2",
  hidden_2g: "0",
  hidden_5g: "0",
};
const fixtures = {
  "/api/wifi/status": wifi,
  "/api/wifi/guest": {
    ssid: "Guest",
    key: "test-only-guest",
    disabled_2g: "1",
    disabled_5g: "1",
    encryption: "psk2",
    hidden: "0",
    isolate: "1",
    guest_active_time: "0",
  },
  "/api/device": { hostname: "u60-test", uptime_secs: 3600, kernel: "test" },
  "/api/network/client-details": {
    inventory_complete: true,
    devices: [],
    router_macs: [],
  },
  "/api/data-usage": {
    day: { rx_bytes: 2e9, tx_bytes: 3e8 },
    month: { rx_bytes: 50e9, tx_bytes: 3e9 },
    total: { rx_bytes: 100e9, tx_bytes: 3e9 },
    limit: {
      available: true,
      enabled: true,
      bytes: 100e9,
      warning_percent: 80,
      auto_disconnect: false,
    },
    cycle: { available: true, enabled: true, reset_day: 1 },
  },
  "/api/sms/list": {
    messages: [
      {
        id: "1",
        number: "Test sender",
        tag: "1",
        content: "A long message. ".repeat(300),
        date: "26,11,7,12,0,0,32",
      },
    ],
  },
  "/api/sms/capacity": {
    sms_nvused_total: 1,
    sms_nv_total: 100,
    sms_simused_total: 0,
    sms_sim_total: 50,
  },
  "/api/tailscale/status": {
    installed: true,
    enabled: true,
    backend_state: "Running",
    hostname: "test-node",
    tailscale_ips: ["100.64.0.1"],
    accept_routes: true,
    gateway_mode: true,
    accept_routes_effective: true,
  },
  "/api/vpn/wireguard": {
    supported: true,
    configured: false,
    enabled: false,
    active: false,
    interface: "wg-test",
    peers: [],
  },
  "/api/proxy/mihomo/status": {
    installed: true,
    configured: true,
    running: true,
    enabled: true,
    mode: "rule",
    capture: "local",
    mixed_port: 7890,
    ipv6: true,
    tcp_concurrent: false,
  },
  "/api/proxy/mihomo/groups": [
    {
      name: "Test group",
      kind: "Selector",
      selected: "DIRECT",
      selectable: true,
      choices: ["DIRECT", "REJECT"],
    },
  ],
  "/api/proxy/mihomo/config": {
    config: "proxies: []\nrules: [MATCH,DIRECT]\n",
    revision: "test-revision",
    configured: true,
  },
  "/api/proxy/mihomo/logs": { text: "test log" },
  "/api/proxy/mihomo/panel": { installed: false },
  "/api/tailscale/network": { routes: [] },
  "/api/tailscale/logs": { text: "test log" },
  "/api/router/lan": {
    lan_ipaddr: "192.0.2.1",
    lan_netmask: "255.255.255.0",
    dhcp_enable: "1",
    dhcp_start: "100",
    dhcp_end: "200",
    dhcp_lease_time: "12h",
  },
  "/api/dns/status": {
    mode: "automatic",
    carrier_servers: ["192.0.2.53"],
    primary: "192.0.2.53",
    secondary: "",
    doh_url: "",
    dot_endpoint: "",
    dot_server_name: "",
    stats: { cache_entries: 0 },
  },
  "/api/dns/cache": { entries: [] },
  "/api/router/firewall": {
    firewall_switch: "1",
    firewall_level: "medium",
    nat_switch: "1",
    dmz_enabled: "0",
    dmz_ip: "",
    port_forward_switch: "1",
  },
  "/api/router/firewall/port-forward": { rule_list: [] },
  "/api/scheduler/jobs": [],
  "/api/speedtest/servers": [],
  "/api/esim/profiles": { profiles: [], agent_instance: 1 },
  "/api/esim/download": { state: "idle" },
  "/api/esim/switch": { state: "idle" },
  "/api/sim/info": { sim_states: "sim ready", pinnumber: 3, puknumber: 10 },
  "/api/router/apn/profiles": { apnListArray: [] },
  "/api/router/apn/mode": { apn_mode: 0 },
  "/api/sms/forward/config": {
    config: {
      rules: [],
      enabled: false,
      poll_interval_secs: 30,
      mark_read_after_forward: false,
      delete_after_forward: false,
    },
  },
  "/api/device/charge-control": {
    charging_stopped: false,
    charge_limit_enabled: true,
    charge_limit: 80,
    hysteresis: 5,
  },
  "/api/device/power-save": { power_saver_mode: "0" },
  "/api/device/fast-boot": { fast_boot: "0" },
  "/api/device/auto-sleep": { enabled: false, timeout: "10" },
  "/api/screen/preferences": {
    language: "en",
    dark: true,
    accent: "#3a9b78",
    brightness: 50,
    sleep_seconds: 60,
    home: {
      show_wifi: true,
      show_radio: true,
      show_services: true,
      show_tailscale: true,
      show_wireguard: true,
      show_mihomo: true,
      metric_1: "clients",
      metric_2: "month",
      metric_3: "temperature",
    },
  },
  "/api/system/top": { processes: [] },
  "/api/memory": { available_kb: 100000, total_kb: 500000 },
  "/api/update/status": {
    current: { version: "test", commit: "test" },
    deployment_mode: "paired",
    phase: "idle",
  },
};
const streams = {
  signal: { network_provider: "UNICOM", network_type: "5G SA" },
  traffic: { real_rx_speed: 1250000, real_tx_speed: 125000 },
  battery: {
    capacity: 80,
    status: "Charging",
    voltage_uv: 4000000,
    current_ua: 1000000,
    temperature: 320,
  },
  health: {
    cpu: { overall: 12, cores: [10, 14] },
    thermal: { device_temp: 38 },
  },
  charger: { charger_connect: 1, direct_power_supply_mode: "disable" },
  clients: { hosts: {} },
  wan: { wan: { ipaddr: "192.0.2.2" } },
  service: { network_type: "5G SA" },
  sms: { messages: fixtures["/api/sms/list"].messages },
};
(async () => {
  const browser = await chromium.launch({ channel: "chrome", headless: true });
  try {
    const context = await browser.newContext({
      viewport: { width: 1440, height: 1000 },
      locale: "en-US",
    });
    const page = await context.newPage(),
      errors = [],
      writes = [];
    page.on("pageerror", (e) => {
      errors.push(e.message);
      console.error("Browser:", e.message);
    });
    await context.addInitScript((token) => {
      sessionStorage.setItem("openu60.session", token);
      localStorage.setItem("openu60.language", "en");
    }, token);
    await context.route("**/api/**", async (route) => {
      const req = route.request(),
        url = new URL(req.url()),
        path = url.pathname;
      assert.equal(req.headers().authorization, "Bearer " + token);
      if (path === "/api/events") {
        const topics = url.searchParams.get("topics").split(",");
        return route.fulfill({
          contentType: "text/event-stream",
          body:
            'event: hello\ndata: {"v":1,"agent_instance":1}\n\n' +
            topics
              .filter((t) => streams[t])
              .map(
                (t) =>
                  "event: " +
                  t +
                  "\ndata: " +
                  JSON.stringify({
                    seq: 1,
                    age_ms: 0,
                    error: null,
                    data: streams[t],
                  }) +
                  "\n\n",
              )
              .join(""),
        });
      }
      if (
        ["POST", "PUT", "DELETE"].includes(req.method()) &&
        !["/api/sms/list", "/api/device/power-save"].includes(path)
      ) {
        writes.push({ path, body: req.postDataJSON() });
        if (path === "/api/proxy/mihomo/settings")
          Object.assign(
            fixtures["/api/proxy/mihomo/status"],
            req.postDataJSON(),
          );
        if (path === "/api/wifi/settings")
          Object.assign(wifi, req.postDataJSON());
      }
      return route.fulfill({
        json: {
          ok: true,
          data:
            fixtures[path] ??
            (path.startsWith("/api/software/")
              ? { phase: "idle", busy: false }
              : {}),
        },
      });
    });
    await page.goto("http://127.0.0.1:5177/console/");
    await page.getByText("Network topology", { exact: true }).waitFor();
    await page.getByText("China Unicom", { exact: true }).waitFor();
    await page.locator(".language-select").selectOption("zh-Hans");
    await page.getByText("中国联通", { exact: true }).waitFor();
    assert.equal(
      await page.getByText("10.0", { exact: true }).count(),
      1,
      "rate is numeric, not translated protocol",
    );
    await page.locator(".language-select").selectOption("en");
    const paths = [
      "wifi/hotspot",
      "wifi/guest",
      "wifi/radio",
      "devices/online",
      "internet/signal",
      "internet/connection",
      "internet/connection/apn",
      "internet/connection/bands",
      "internet/connection/operators",
      "internet/connection/policies",
      "internet/sim",
      "internet/usage",
      "internet/carrier",
      "messages/inbox",
      "messages/compose",
      "messages/forward",
      "messages/storage",
      "services",
      "services/tailscale",
      "services/tailscale/network",
      "services/tailscale/routes",
      "services/tailscale/installation",
      "services/wireguard/configuration",
      "services/mihomo",
      "services/mihomo/configuration",
      "services/mihomo/groups",
      "services/mihomo/logs",
      "services/mihomo/panel",
      "lan/lan",
      "lan/dns",
      "lan/firewall",
      "system/power",
      "system/screen",
      "system/update",
      "system/diagnostics",
      "system/diagnostics/processes",
      "system/diagnostics/speedtest",
      "system/automation",
    ];
    for (const path of paths) {
      await page.evaluate((path) => (location.hash = path), path);
      await page.waitForTimeout(100);
      assert.equal(await page.locator("main h1").count(), 1, path);
    }
    assert.deepEqual(writes, [], "navigation must never mutate router state");
    await page.evaluate(() => (location.hash = "services/mihomo"));
    await page.getByLabel("Mode", { exact: true }).selectOption("direct");
    await page.getByText("Applied and verified", { exact: true }).waitFor();
    assert.deepEqual(writes.at(-1), {
      path: "/api/proxy/mihomo/settings",
      body: { mode: "direct" },
    });
    await page.evaluate(() => (location.hash = "wifi/hotspot"));
    await page
      .getByLabel("Network name", { exact: true })
      .first()
      .fill("Changed hotspot");
    await page.getByLabel("Network name", { exact: true }).first().press("Tab");
    await page.getByRole("button", { name: "Cancel change" }).click();
    assert.equal(
      writes.length,
      1,
      "cancelled disruptive setting must not be written",
    );
    await page.setViewportSize({ width: 390, height: 844 });
    await page.evaluate(() => (location.hash = "overview"));
    await page.waitForTimeout(150);
    assert(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
      "mobile shell must fit viewport",
    );
    await page.screenshot({
      path: "/tmp/u60-react-home-mobile.png",
      fullPage: true,
    });
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.screenshot({
      path: "/tmp/u60-react-home-desktop.png",
      fullPage: true,
    });
    assert.deepEqual(errors, []);
    console.log(
      "Passed: all main routes, localized carrier and rates, no navigation writes, immediate settings and disruptive cancellation, mobile width.",
    );
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
