// Integration tests only use an in-memory router, never a connected device.
const { chromium } = require("playwright");
const assert = require("node:assert/strict");
const token = "a".repeat(64);
const wifi = {
  wifi_onoff: "1",
  wifi6_switch: "0",
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
const operating = {
  airplane: { enabled: false, mode: "ONLINE", error: null },
  bandwidth: { supported: true, mode: "normal", error: null },
  rail: { active: false },
  performance: {
    supported: true,
    available: ["balanced", "performance", "saving"],
    requested: null,
    effective: "balanced",
    applied: true,
    thermal_limited: false,
  },
};
const fixtures = {
  "/api/device/operating": operating,
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
    accept_dns: false,
    exit_node: "",
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
  "/api/tailscale/peers": {
    exit_nodes: [{ address: "100.64.0.8", name: "Test gateway", online: true }],
  },
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
  "/api/router/vpn": {
    l2tp_passthrough: "1",
    pptp_passthrough: "0",
    ipsec_passthrough: "1",
    alg_sip_enable: "0",
  },
  "/api/router/domain-filter": { enable: 0, rule_list: [] },
  "/api/router/firewall/filter-rules": { rule_list: [] },
  "/api/cell/neighbors/nr": {
    cell_list: [{ band: "n78", pci: 101, earfcn: 632628, rsrp: -88 }],
  },
  "/api/cell/neighbors/lte": { cell_list: [] },
  "/api/cell/signal-detect/progress": { running: false, progress: 0 },
  "/api/cell/signal-detect/results": { record_list: [] },
  "/api/stk/menu": {
    supported: true,
    busy: false,
    command_id: "test-menu-1",
    type: "select",
    title: "Test SIM menu",
    items: [{ id: 1, label: "Test service" }],
  },
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
    follow_system: false,
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
  operating,
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
  const browser = await chromium.launch({
    headless: true,
    ...(process.env.U60_TEST_BROWSER_CHANNEL
      ? { channel: process.env.U60_TEST_BROWSER_CHANNEL }
      : {}),
  });
  try {
    const context = await browser.newContext({
      viewport: { width: 1440, height: 1000 },
      locale: "en-US",
    });
    const page = await context.newPage(),
      errors = [],
      writes = [];
    let ignoreWrite = false,
      modeFailure = "";
    page.on("pageerror", (e) => {
      errors.push(e.message);
      console.error("Browser:", e.message);
    });
    await context.addInitScript((token) => {
      sessionStorage.setItem("openu60.session", token);
      localStorage.setItem("openu60.language", "en");
      // Playwright fulfills finite responses. Model the device's persistent
      // event stream so controls can distinguish live state from disconnection.
      const fetchOriginal = window.fetch;
      window.fetch = async (input, init) => {
        const response = await fetchOriginal(input, init);
        if (!String(input).includes("/api/events?")) return response;
        const body = await response.text();
        return new Response(
          new ReadableStream({
            start(controller) {
              const enc = new TextEncoder();
              controller.enqueue(enc.encode(body));
              const timer = setInterval(
                () => controller.enqueue(enc.encode(": keepalive\n\n")),
                5000,
              );
              init?.signal?.addEventListener(
                "abort",
                () => {
                  clearInterval(timer);
                  controller.close();
                },
                { once: true },
              );
            },
          }),
          { headers: response.headers, status: response.status },
        );
      };
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
      if (path === "/api/lan/download")
        return route.fulfill({
          contentType: "application/octet-stream",
          body: Buffer.alloc(10 * 1024 * 1024),
        });
      if (path === "/api/lan/upload") {
        const size = req.postDataBuffer().byteLength;
        assert.equal(size, 10 * 1024 * 1024);
        return route.fulfill({ json: { ok: true, data: { bytes: size } } });
      }
      if (
        ["POST", "PUT", "DELETE"].includes(req.method()) &&
        !["/api/sms/list", "/api/device/power-save"].includes(path)
      ) {
        writes.push({ path, body: req.postDataJSON() });
        if (
          [
            "/api/device/performance",
            "/api/modem/airplane",
            "/api/modem/bandwidth",
          ].includes(path)
        ) {
          const body = req.postDataJSON();
          if (modeFailure !== "ignore") {
            if (path.endsWith("performance"))
              Object.assign(operating.performance, {
                requested: body.profile,
                effective: body.profile,
                applied: true,
              });
            if (path.endsWith("bandwidth")) {
              operating.bandwidth.mode = body.mode;
              if (operating.airplane.enabled)
                operating.airplane.mode = "LOW_POWER";
            }
            if (path.endsWith("airplane"))
              Object.assign(operating.airplane, {
                mode: body.operate_mode,
                enabled: body.operate_mode !== "ONLINE",
              });
          }
          if (modeFailure === "applied-error")
            return route.fulfill({
              status: 503,
              json: {
                ok: false,
                error: "Response interrupted; check current state",
              },
            });
        }
        if (path === "/api/proxy/mihomo/settings" && !ignoreWrite)
          Object.assign(
            fixtures["/api/proxy/mihomo/status"],
            req.postDataJSON(),
          );
        if (path === "/api/wifi/settings")
          Object.assign(wifi, req.postDataJSON());
        if (path === "/api/dns/apply")
          Object.assign(fixtures["/api/dns/status"], req.postDataJSON());
        if (path === "/api/tailscale/config")
          Object.assign(fixtures["/api/tailscale/status"], req.postDataJSON());
        if (path === "/api/screen/preferences") {
          const patch = req.postDataJSON(),
            state = fixtures[path];
          if (patch.home) {
            const key = Object.keys(patch.home).find((k) =>
              k.startsWith("metric_"),
            );
            if (key && patch.home[key] !== "none") {
              const other = ["metric_1", "metric_2", "metric_3"].find(
                (k) => k !== key && state.home[k] === patch.home[key],
              );
              if (other) state.home[other] = state.home[key];
            }
            Object.assign(state.home, patch.home);
            delete patch.home;
          }
          Object.assign(state, patch);
        }
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
    await page.goto(
      process.env.U60_WEB_TEST_URL || "http://127.0.0.1:5177/console/",
    );
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
      "system/diagnostics/lan-test",
      "system/automation",
    ];
    for (const path of paths) {
      await page.evaluate((path) => (location.hash = path), path);
      await page.waitForTimeout(100);
      assert.equal(await page.locator("main h1").count(), 1, path);
    }
    assert.deepEqual(writes, [], "navigation must never mutate router state");
    await page.evaluate(() => (location.hash = "lan/firewall"));
    await page.getByText("Advanced network policies", { exact: true }).click();
    await page.getByText("Blocked domains", { exact: true }).waitFor();
    await page.evaluate(() => (location.hash = "internet/connection/bands"));
    await page.getByText("Advanced radio tools", { exact: true }).click();
    await page.getByRole("cell", { name: "632628", exact: true }).waitFor();
    await page.evaluate(() => (location.hash = "internet/carrier"));
    await page.getByLabel("Menu item", { exact: true }).waitFor();
    await page
      .getByRole("button", { name: "Open menu item", exact: true })
      .click();
    await page.getByRole("button", { name: "Cancel change" }).click();
    assert.deepEqual(
      writes,
      [],
      "cancelled carrier service may not send a SIM command",
    );
    await page.evaluate(() => (location.hash = "system/diagnostics/lan-test"));
    await page.getByRole("button", { name: "Start test", exact: true }).click();
    await page.getByText("Completed", { exact: true }).waitFor();
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
    // A stale editor may not overwrite values changed by the other screen.
    await page.evaluate(() => (location.hash = "services/mihomo"));
    await page.getByLabel("Mode", { exact: true }).waitFor();
    fixtures["/api/proxy/mihomo/status"].mode = "global";
    await page.getByLabel("Mode", { exact: true }).selectOption("rule");
    await page
      .getByText(
        "Settings changed elsewhere. Review the current values before trying again.",
        { exact: true },
      )
      .waitFor();
    assert.equal(writes.length, 1);
    await page
      .getByText("Discard draft and read current values", { exact: true })
      .click();
    await page.waitForTimeout(100);
    assert.equal(
      await page.getByLabel("Mode", { exact: true }).inputValue(),
      "global",
    );
    // Accepted writes without matching readback must not report success or retry.
    ignoreWrite = true;
    await page.getByLabel("Mode", { exact: true }).selectOption("rule");
    await page
      .getByText("The router has not confirmed the change.", { exact: true })
      .waitFor();
    assert.equal(writes.length, 2);
    ignoreWrite = false;
    await page.evaluate(() => (location.hash = "lan/dns"));
    await page.getByLabel("Mode", { exact: true }).selectOption("doh");
    await page.waitForTimeout(100);
    assert.equal(writes.length, 2, "incomplete resolver may not be applied");
    await page
      .getByLabel("DoH endpoint", { exact: true })
      .fill("https://dns.example.test/dns-query");
    await page.getByLabel("DoH endpoint", { exact: true }).press("Tab");
    await page.getByText("Applied and verified", { exact: true }).waitFor();
    assert.equal(writes.at(-1).path, "/api/dns/apply");
    await page.evaluate(() => (location.hash = "system/screen"));
    await page.getByLabel("Metric 1", { exact: true }).selectOption("month");
    await page.getByText("Applied and verified", { exact: true }).waitFor();
    assert.equal(
      await page.getByLabel("Metric 2", { exact: true }).inputValue(),
      "clients",
    );
    await page.evaluate(
      () => (location.hash = "services/mihomo/configuration"),
    );
    await page
      .getByLabel("YAML / JSON", { exact: true })
      .fill("# private draft\nproxies: []\nrules: []\n");
    await page.evaluate(() => (location.hash = "overview"));
    await page.waitForTimeout(50);
    await page.evaluate(
      () => (location.hash = "services/mihomo/configuration"),
    );
    assert(
      (
        await page.getByLabel("YAML / JSON", { exact: true }).inputValue()
      ).startsWith("# private draft"),
    );
    await page.evaluate(() => (location.hash = "overview"));
    for (const [locale, name] of [
      ["zh-Hant", "中國聯通"],
      ["ja", "中国聯通"],
      ["ko", "차이나유니콤"],
      ["zh-Hans", "中国联通"],
      ["en", "China Unicom"],
    ]) {
      await page.locator(".language-select").selectOption(locale);
      await page.getByText(name, { exact: true }).waitFor();
    }
    // Real mode controls: one verified write, independent settings, safe
    // cancellation and recovery even when a response is lost after applying.
    const modeStart = writes.length;
    await page.evaluate(() => (location.hash = "system/power"));
    await page.getByRole("group", { name: "Performance mode" }).waitFor();
    assert.equal(
      await page
        .getByRole("switch", { name: "Airplane mode", exact: true })
        .count(),
      0,
      "flight belongs to mobile network",
    );
    await page.getByRole("button", { name: /^Power saving/ }).click();
    await page.waitForFunction(() =>
      document
        .querySelector('.performance-options button[aria-pressed="true"]')
        ?.textContent.includes("Power saving"),
    );
    assert.equal(writes.length, modeStart + 1);
    assert.deepEqual(writes.at(-1), {
      path: "/api/device/performance",
      body: { profile: "saving" },
    });
    assert.equal(operating.bandwidth.mode, "normal");
    assert.equal(
      await page.getByRole("dialog").count(),
      0,
      "profile selection needs no extra confirmation",
    );
    modeFailure = "ignore";
    await page.getByRole("button", { name: /^Performance/ }).click();
    await page
      .getByText(
        "The requested setting was not confirmed. Check current device status.",
      )
      .waitFor();
    assert.equal(operating.performance.effective, "saving");
    modeFailure = "";
    await page.evaluate(() => (location.hash = "internet/connection"));
    const flight = page.getByRole("switch", {
      name: "Airplane mode",
      exact: true,
    });
    await flight.waitFor();
    await flight.click();
    await page
      .getByRole("button", { name: "Cancel change", exact: true })
      .click();
    assert.equal(
      writes.length,
      modeStart + 2,
      "cancelled flight creates no write",
    );
    modeFailure = "applied-error";
    await flight.click();
    await page
      .getByRole("button", { name: "Continue and apply", exact: true })
      .click();
    await page.getByText("Response interrupted; check current state").waitFor();
    assert.equal(
      await flight.getAttribute("aria-checked"),
      "true",
      "readback wins over uncertain command result",
    );
    modeFailure = "";
    // B31 may change its radio-off enum after a bandwidth selection. The
    // boolean projection keeps recovery available in both settings surfaces.
    await page.getByRole("button", { name: /^AI smart/ }).click();
    await page.waitForFunction(() =>
      document
        .querySelector('.bandwidth-options button[aria-pressed="true"]')
        ?.textContent.includes("AI smart"),
    );
    assert.equal(operating.airplane.mode, "LOW_POWER");
    assert.equal(await flight.isEnabled(), true);
    assert.equal(await flight.getAttribute("aria-checked"), "true");
    await page.evaluate(() => (location.hash = "overview"));
    await page.waitForFunction(() =>
      document
        .querySelector(".mode-overview")
        ?.textContent.includes("Airplane mode"),
    );
    assert.equal(
      await page.locator(".live-rates strong").first().innerText(),
      "—",
      "stale cellular speed is hidden while radio is off",
    );
    await page
      .getByRole("button", { name: "Quick controls", exact: true })
      .first()
      .click();
    await page
      .getByRole("dialog")
      .getByRole("switch", { name: "Airplane mode", exact: true })
      .click();
    await page.waitForFunction(
      () =>
        document
          .querySelector('dialog [role="switch"]')
          ?.getAttribute("aria-checked") === "false",
    );
    assert.equal(
      await page.getByRole("dialog").count(),
      1,
      "restoring cellular needs no second dialog",
    );
    assert.equal(operating.airplane.mode, "ONLINE");
    await page
      .getByRole("dialog")
      .getByRole("button", { name: "Close", exact: true })
      .click();
    await page.evaluate(() => (location.hash = "internet/connection"));
    await page.getByRole("button", { name: /^Gaming/ }).click();
    await page.waitForFunction(() =>
      document
        .querySelector('.bandwidth-options button[aria-pressed="true"]')
        ?.textContent.includes("Gaming"),
    );
    assert.deepEqual(writes.at(-1), {
      path: "/api/modem/bandwidth",
      body: { mode: "game" },
    });
    assert.equal(
      operating.performance.effective,
      "saving",
      "bandwidth priority does not change CPU profile",
    );
    await page.evaluate(() => (location.hash = "wifi/radio"));
    const standard = page.getByLabel("Wi-Fi 7 (802.11be)", { exact: true });
    await standard.waitFor();
    assert.equal(
      await standard.count(),
      1,
      "one global Wi-Fi standard control for both radios",
    );
    assert.equal(await standard.inputValue(), "0");
    const beforeWifi = writes.length;
    await standard.selectOption("1");
    await page
      .getByRole("button", { name: "Cancel change", exact: true })
      .click();
    assert.equal(
      writes.length,
      beforeWifi,
      "cancelled wireless disruption does not write",
    );
    await standard.selectOption("1");
    await page
      .getByRole("button", { name: "Continue and apply", exact: true })
      .click();
    await page.getByText("Applied and verified", { exact: true }).waitFor();
    assert.deepEqual(writes.at(-1), {
      path: "/api/wifi/settings",
      body: { wifi6_switch: "1" },
    });
    await page.evaluate(() => (location.hash = "internet/connection"));
    await flight.waitFor();
    // Capture only public controls; no service names or real device values.
    for (const language of ["zh-Hans", "zh-Hant", "en", "ja", "ko"]) {
      await page.locator(".language-select").selectOption(language);
      assert.equal(
        await page.locator(".mode-flight b").innerText(),
        {
          en: "Airplane mode",
          "zh-Hans": "飞行模式",
          "zh-Hant": "飛航模式",
          ja: "機内モード",
          ko: "비행기 모드",
        }[language],
      );
    }
    await page.locator(".language-select").selectOption("zh-Hans");
    await page.screenshot({
      path: "/tmp/u60-operating-web-mobile-network.png",
      fullPage: true,
    });
    await page.evaluate(() => (location.hash = "system/power"));
    await page.getByText("性能模式", { exact: true }).waitFor();
    await page.screenshot({
      path: "/tmp/u60-operating-web-power.png",
      fullPage: true,
    });
    await page.locator(".language-select").selectOption("en");
    await page.setViewportSize({ width: 390, height: 844 });
    await page.evaluate(() => (location.hash = "overview"));
    await page.waitForTimeout(150);
    assert(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
      "mobile shell must fit viewport",
    );
    await page.addStyleTag({
      content: '[data-service="mihomo"] { display: none !important; }',
    });
    await page.screenshot({
      path: "/tmp/u60-react-home-mobile.png",
      fullPage: true,
    });
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.screenshot({
      path: "/tmp/u60-react-home-desktop.png",
      fullPage: true,
    });
    await page.getByRole("button", { name: "Dark mode", exact: true }).click();
    await page.locator(".language-select").selectOption("zh-Hans");
    await page.getByText("中国联通", { exact: true }).waitFor();
    await page.waitForTimeout(200);
    await page.screenshot({
      path: "/tmp/u60-react-home-dark.png",
      fullPage: true,
    });
    await page.setViewportSize({ width: 768, height: 1024 });
    assert(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
      "tablet layout fits",
    );
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
