import { useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  Check,
  ChevronRight,
  CircleHelp,
  Eye,
  EyeOff,
  Laptop,
  Plus,
  QrCode,
  Radio,
  ShieldCheck,
  Signal,
  Smartphone,
  Wifi,
} from "lucide-react";
import { clients } from "./data";
import { useModel } from "./model";
import {
  Battery,
  DeviceIcon,
  Panel,
  PanelTitle,
  Periods,
  QR,
  Status,
  TrafficChart,
  Usage,
} from "./ui";
import {
  ConfigForm,
  Definition,
  Entry,
  Field,
  Modal,
  Note,
  Password,
  SearchBox,
  Stats,
  SwitchRow,
} from "./controls";

export function Select({
  value,
  options,
  onChange,
}: {
  value: string;
  options: string[];
  onChange: (v: string) => void;
}) {
  return (
    <select value={value} onChange={(e) => onChange(e.target.value)}>
      {options.map((v) => (
        <option key={v}>{v}</option>
      ))}
    </select>
  );
}
const wifiValidate = (v: Record<string, string | boolean>) =>
  !String(v.ssid).trim()
    ? "请填写网络名称。"
    : new TextEncoder().encode(String(v.ssid)).length > 32
      ? "网络名称不能超过 32 字节。"
      : String(v.password).length < 8 || String(v.password).length > 63
        ? "密码需要 8–63 个字符。"
        : "";
export function WifiPage({ tab }: { tab: string }) {
  const m = useModel();
  if (tab === "访客网络")
    return (
      <div className="two-columns">
        <ConfigForm
          id="guest"
          title="访客网络"
          note="为临时访客提供独立的连接。"
          validate={wifiValidate}
        >
          {(v, set) => (
            <>
              <SwitchRow
                title="允许访客连接"
                hint="访客设备使用单独的网络名称与密码"
                value={!!v.enabled}
                onChange={() => set("enabled", !v.enabled)}
              />
              <div className="form-grid">
                <Field label="网络名称">
                  <input
                    value={String(v.ssid)}
                    onChange={(e) => set("ssid", e.target.value)}
                  />
                </Field>
                <Field label="连接密码">
                  <Password
                    value={String(v.password)}
                    onChange={(s) => set("password", s)}
                  />
                </Field>
                <Field label="有效时间">
                  <Select
                    value={String(v.duration)}
                    options={["1 小时", "4 小时", "24 小时", "持续开放"]}
                    onChange={(s) => set("duration", s)}
                  />
                </Field>
              </div>
              <SwitchRow
                title="隔离主网络"
                hint="访客只能访问互联网，无法访问局域网设备"
                value={!!v.isolated}
                onChange={() => set("isolated", !v.isolated)}
              />
            </>
          )}
        </ConfigForm>
        <Panel>
          <PanelTitle title="访客连接" />
          <div className="empty-state">
            <Wifi size={30} />
            <h3>
              {m.settings.guest.enabled ? "等待访客连接" : "访客网络未开启"}
            </h3>
            <p>开启后，可单独分享访客网络。</p>
          </div>
        </Panel>
      </div>
    );
  if (tab === "无线参数")
    return (
      <ConfigForm
        id="radio"
        title="无线参数"
        note="双频独立调节，网络名称与密码在热点设置中管理。"
        initial={{
          channel2: "自动",
          channel5: "自动",
          width2: "40 MHz",
          width5: "80 MHz",
          power: "100%",
          hidden: false,
        }}
        confirm="更改无线参数会短暂中断 Wi-Fi，已连接设备需要重新建立连接。"
      >
        {(v, set) => (
          <>
            <div className="form-grid">
              {[2, 5].map((b) => (
                <div key={b}>
                  <h3>{b === 2 ? "2.4" : "5"} GHz</h3>
                  <Field label="信道">
                    <Select
                      value={String(v["channel" + b])}
                      options={
                        b === 2
                          ? ["自动", "1", "6", "11"]
                          : ["自动", "36", "44", "149", "157"]
                      }
                      onChange={(s) => set("channel" + b, s)}
                    />
                  </Field>
                  <Field label="带宽">
                    <Select
                      value={String(v["width" + b])}
                      options={
                        b === 2
                          ? ["20 MHz", "40 MHz"]
                          : ["40 MHz", "80 MHz", "160 MHz"]
                      }
                      onChange={(s) => set("width" + b, s)}
                    />
                  </Field>
                </div>
              ))}
            </div>
            <SwitchRow
              title="隐藏网络名称"
              hint="关闭广播后，需要手动填写网络名称"
              value={!!v.hidden}
              onChange={() => set("hidden", !v.hidden)}
            />
          </>
        )}
      </ConfigForm>
    );
  return (
    <>
      <div className="wifi-band-cards">
        {["2.4 GHz", "5 GHz"].map((band, i) => (
          <Panel key={band}>
            <span className="band-icon">
              <Wifi size={23} />
            </span>
            <div>
              <h3>{band}</h3>
              <p>{i ? "高速连接 · 3 台设备" : "更远覆盖 · 2 台设备"}</p>
            </div>
            <Status muted={!m.settings.wifi.enabled}>
              {m.settings.wifi.enabled ? "广播中" : "已关闭"}
            </Status>
          </Panel>
        ))}
      </div>
      <div className="two-columns">
        <ConfigForm
          id="wifi"
          title="主网络"
          note="名称、密码和连接方式集中在这里。"
          validate={wifiValidate}
          confirm="应用后，Wi-Fi 连接可能中断。请使用新的名称和密码重新连接；USB 管理连接不受影响。"
        >
          {(v, set) => (
            <>
              <SwitchRow
                title="开启 Wi-Fi"
                value={!!v.enabled}
                onChange={() => set("enabled", !v.enabled)}
              />
              <SwitchRow
                title="双频合一"
                hint="2.4 GHz 与 5 GHz 共用名称和密码，设备自动选择频段"
                value={!!v.unified}
                onChange={() => set("unified", !v.unified)}
              />
              <div className="form-grid">
                <Field label={v.unified ? "网络名称" : "2.4 GHz 名称"}>
                  <input
                    value={String(v.ssid)}
                    onChange={(e) => set("ssid", e.target.value)}
                  />
                </Field>
                {!v.unified && (
                  <Field label="5 GHz 名称">
                    <input
                      value={String(v.ssid5)}
                      onChange={(e) => set("ssid5", e.target.value)}
                    />
                  </Field>
                )}
                <Field label="连接密码">
                  <Password
                    value={String(v.password)}
                    onChange={(s) => set("password", s)}
                  />
                </Field>
                <Field label="安全方式">
                  <Select
                    value={String(v.security)}
                    options={["WPA2 / WPA3", "WPA2", "WPA3"]}
                    onChange={(s) => set("security", s)}
                  />
                </Field>
              </div>
            </>
          )}
        </ConfigForm>
        <div className="stack">
          <Panel className="share-preview">
            <PanelTitle title="扫码连接" note="分享已应用的网络设置" />
            <QR
              size={152}
              ssid={String(m.settings.wifi.ssid)}
              password={String(m.settings.wifi.password)}
            />
            <h3>{String(m.settings.wifi.ssid)}</h3>
            <p>
              {m.settings.wifi.unified
                ? "双频合一 · 一次连接"
                : "2.4 GHz · 5 GHz 单独分享"}
            </p>
            <button className="btn" onClick={() => m.setShare(true)}>
              <QrCode size={16} /> 打开连接卡片
            </button>
          </Panel>
          <Note>
            只有两路热点使用相同名称和密码时，才共享同一张连接二维码。
          </Note>
        </div>
      </div>
    </>
  );
}

export function DevicesPage({ tab }: { tab: string }) {
  const m = useModel(),
    [query, setQuery] = useState(""),
    [band, setBand] = useState("全部连接"),
    [selected, setSelected] = useState<number | null>(null),
    [blocked, setBlocked] = useState<string[]>([]);
  const list = clients.filter(
    (c) =>
      (tab === "在线设备" ? c.online : !c.online) &&
      (c.name + c.ip).toLowerCase().includes(query.toLowerCase()) &&
      (band === "全部连接" || c.band === band),
  );
  const c = selected !== null ? clients[selected] : null;
  return (
    <>
      <Stats
        items={[
          ["在线设备", "6", "5 台无线 · 1 台 USB"],
          ["总下载速率", "124.6 Mbps"],
          ["总上传速率", "14.2 Mbps"],
          ["已限制", String(blocked.length), "单独管理访问权限"],
        ]}
      />
      <Panel>
        <div className="table-toolbar">
          <SearchBox
            value={query}
            onChange={setQuery}
            placeholder="搜索设备名称或 IP"
          />
          <Select
            value={band}
            onChange={setBand}
            options={["全部连接", "2.4 GHz", "5 GHz", "USB"]}
          />
        </div>
        <div className="table-scroll">
          <table className="data-table">
            <thead>
              <tr>
                {["设备", "连接", "IP 地址", "下载 / 上传", "权限", ""].map(
                  (s, i) => (
                    <th key={i}>{s}</th>
                  ),
                )}
              </tr>
            </thead>
            <tbody>
              {list.map((c) => (
                <tr key={c.ip}>
                  <td>
                    <button
                      className="identity-button"
                      onClick={() => setSelected(clients.indexOf(c))}
                    >
                      <span className="device-avatar">
                        <DeviceIcon kind={c.kind} />
                      </span>
                      <span>
                        <b>{c.name}</b>
                        <small>{c.tag || "已连接 2 小时 14 分"}</small>
                      </span>
                    </button>
                  </td>
                  <td>{c.band}</td>
                  <td className="mono">{c.ip}</td>
                  <td>
                    <span className="rate-pair">
                      <ArrowDown size={12} /> {c.download} <ArrowUp size={12} />{" "}
                      {c.upload}
                      <small>Mbps</small>
                    </span>
                  </td>
                  <td>
                    <Status muted={blocked.includes(c.ip)}>
                      {blocked.includes(c.ip)
                        ? "已限制"
                        : c.online
                          ? "允许"
                          : "离线"}
                    </Status>
                  </td>
                  <td>
                    <button
                      className="icon-btn"
                      aria-label={`查看 ${c.name}`}
                      onClick={() => setSelected(clients.indexOf(c))}
                    >
                      <ChevronRight size={16} />
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {!list.length && (
            <div className="empty-state">
              <Laptop size={26} />
              <h3>没有符合条件的设备</h3>
              <p>尝试其他名称或连接方式。</p>
            </div>
          )}
        </div>
      </Panel>
      {c && (
        <Modal title={c.name} close={() => setSelected(null)}>
          <Stats
            items={[
              ["下载", c.download + " Mbps"],
              ["上传", c.upload + " Mbps"],
            ]}
          />
          <Definition
            items={[
              ["连接", c.band],
              ["IP 地址", c.ip],
              ["设备状态", c.online ? "在线" : "离线"],
            ]}
          />
          <SwitchRow
            title="允许访问网络"
            hint={
              c.tag === "当前设备"
                ? "限制当前管理设备会断开连接"
                : "仅影响这台设备"
            }
            value={!blocked.includes(c.ip)}
            onChange={() => {
              setBlocked((s) =>
                s.includes(c.ip) ? s.filter((ip) => ip !== c.ip) : [...s, c.ip],
              );
              m.notify("已模拟更新设备访问权限");
            }}
          />
        </Modal>
      )}
    </>
  );
}

export function InternetPage({ tab }: { tab: string }) {
  const m = useModel(),
    [period, setPeriod] = useState("30 分钟"),
    [advanced, setAdvanced] = useState(false),
    [scan, setScan] = useState(false),
    [simPanel, setSimPanel] = useState("");
  if (tab === "连接设置")
    return (
      <ConfigForm
        id="cellular"
        title="蜂窝连接"
        note="选择接入方式，管理 APN 与网络恢复策略。"
        initial={{
          data: true,
          mode: "5G / 4G 自动",
          operator: "自动选择",
          apn: "3gnet",
          roaming: false,
          watchdog: true,
          oos: true,
          ttl: "保持系统默认",
        }}
        confirm="应用蜂窝设置会重新建立移动网络连接，正在进行的下载与通话可能中断。"
      >
        {(v, set) => (
          <>
            <SwitchRow
              title="移动数据"
              value={!!v.data}
              onChange={() => set("data", !v.data)}
            />
            <div className="form-grid">
              <Field label="首选网络">
                <Select
                  value={String(v.mode)}
                  options={["5G / 4G 自动", "仅 5G", "仅 4G"]}
                  onChange={(s) => set("mode", s)}
                />
              </Field>
              <Field label="运营商">
                <Select
                  value={String(v.operator)}
                  options={["自动选择", "中国联通", "手动扫描"]}
                  onChange={(s) => set("operator", s)}
                />
              </Field>
              <Field label="APN" hint="优先使用运营商提供的接入点">
                <input
                  value={String(v.apn)}
                  onChange={(e) => set("apn", e.target.value)}
                />
              </Field>
            </div>
            <SwitchRow
              title="数据漫游"
              hint="漫游可能产生额外费用"
              value={!!v.roaming}
              onChange={() => set("roaming", !v.roaming)}
            />
            <details className="advanced-details">
              <summary>高级连接策略</summary>
              <SwitchRow
                title="断网自动恢复"
                value={!!v.watchdog}
                onChange={() => set("watchdog", !v.watchdog)}
              />
              <SwitchRow
                title="无服务省电"
                value={!!v.oos}
                onChange={() => set("oos", !v.oos)}
              />
              <Field label="TTL / 跳数">
                <Select
                  value={String(v.ttl)}
                  options={["保持系统默认", "64", "65", "128"]}
                  onChange={(s) => set("ttl", s)}
                />
              </Field>
            </details>
          </>
        )}
      </ConfigForm>
    );
  if (tab === "SIM 与 eSIM")
    return (
      <div className="two-columns">
        <Panel>
          <PanelTitle title="当前 SIM 卡" action={<Status>已就绪</Status>} />
          <div className="sim-identity">
            <Smartphone size={32} />
            <div>
              <h3>中国联通</h3>
              <p>
                {m.scenario === "euicc" ? "可管理 eUICC 卡" : "普通运营商 SIM"}
              </p>
            </div>
          </div>
          <Definition
            items={[
              ["卡槽", "SIM 1"],
              ["网络注册", "已注册 · 5G SA"],
              ["ICCID", "•••• •••• •••• 0000"],
              ["PIN 保护", "已关闭"],
            ]}
          />
          <Entry
            title="SIM 安全"
            description="PIN 保护、修改 PIN 与 PUK 解锁"
            onClick={() => setSimPanel("SIM 安全")}
          />
        </Panel>
        <Panel>
          <PanelTitle title="eSIM 配置文件" />
          {m.scenario !== "euicc" ? (
            <div className="empty-state">
              <Smartphone size={30} />
              <h3>当前 SIM 不支持下载配置</h3>
              <p>
                此功能需要支持管理的 eUICC 卡。普通 SIM 仍可正常使用移动网络。
              </p>
            </div>
          ) : (
            <>
              <div className="profile-row">
                <span className="profile-icon">
                  <Signal size={21} />
                </span>
                <div>
                  <b>旅行数据套餐</b>
                  <small>已启用 · 示例配置</small>
                </div>
                <Status>使用中</Status>
              </div>
              <button
                className="btn"
                onClick={() => setSimPanel("添加 eSIM 配置")}
              >
                <Plus size={16} /> 添加配置
              </button>
            </>
          )}
        </Panel>
        {simPanel && (
          <Modal title={simPanel} close={() => setSimPanel("")}>
            <Field label={simPanel === "SIM 安全" ? "当前 SIM PIN" : "激活码"}>
              <input
                type={simPanel === "SIM 安全" ? "password" : "text"}
                placeholder={
                  simPanel === "SIM 安全" ? "输入 PIN 后开启保护" : "LPA:1$…"
                }
              />
            </Field>
            <Note warning={simPanel === "SIM 安全"}>
              {simPanel === "SIM 安全"
                ? "PIN 连续输错会锁卡，请确认剩余尝试次数。"
                : "下载配置需要保持互联网连接。"}
            </Note>
            <div className="dialog-actions">
              <button
                className="btn primary"
                onClick={() => {
                  setSimPanel("");
                  m.notify("已演示提交步骤；不会操作真实 SIM");
                }}
              >
                继续
              </button>
            </div>
          </Modal>
        )}
      </div>
    );
  if (tab === "流量套餐")
    return (
      <div className="two-columns">
        <Panel>
          <PanelTitle
            title="本月使用情况"
            note="统计周期：11 月 1 日 — 11 月 30 日"
          />
          <Usage />
          <div className="space-top">
            <TrafficChart period="7 天" />
          </div>
          <Note>设备统计与运营商账单可能有差异，请以运营商为准。</Note>
        </Panel>
        <ConfigForm
          id="plan"
          title="套餐与提醒"
          initial={{ total: "100", day: "1 日", alert: "80%", enabled: true }}
        >
          {(v, set) => (
            <>
              <Field label="每月套餐 / GB">
                <input
                  type="number"
                  min="1"
                  value={String(v.total)}
                  onChange={(e) => set("total", e.target.value)}
                />
              </Field>
              <Field label="重置日期">
                <Select
                  value={String(v.day)}
                  options={["1 日", "5 日", "10 日", "15 日", "20 日", "25 日"]}
                  onChange={(s) => set("day", s)}
                />
              </Field>
              <SwitchRow
                title="流量提醒"
                value={!!v.enabled}
                onChange={() => set("enabled", !v.enabled)}
              />
              <Field label="提醒阈值">
                <Select
                  value={String(v.alert)}
                  options={["50%", "80%", "90%"]}
                  onChange={(s) => set("alert", s)}
                />
              </Field>
            </>
          )}
        </ConfigForm>
      </div>
    );
  if (tab === "运营商服务")
    return (
      <Panel>
        <PanelTitle
          title="运营商服务"
          note="可用能力取决于 SIM 与运营商网络。"
        />
        <div className="entry-grid">
          {["通话", "USSD 查询", "SIM 工具菜单"].map((name) => (
            <Entry
              key={name}
              title={name}
              description="查看服务与操作"
              onClick={() => setSimPanel(name)}
              icon={<Smartphone size={22} />}
            />
          ))}
        </div>
        {simPanel && (
          <Modal title={simPanel} close={() => setSimPanel("")}>
            <Field label={simPanel === "通话" ? "电话号码" : "服务代码"}>
              <input
                inputMode="tel"
                placeholder={
                  simPanel === "通话" ? "输入号码" : "输入运营商提供的代码"
                }
              />
            </Field>
            <Note>当前预览不会发起通话或扣费请求。</Note>
            <div className="dialog-actions">
              <button
                className="btn primary"
                onClick={() => m.notify("服务请求已模拟发送")}
              >
                发起请求
              </button>
            </div>
          </Modal>
        )}
      </Panel>
    );
  return (
    <>
      <div className="connection-summary">
        <span className="large-icon">
          <Signal size={30} />
        </span>
        <div>
          <span className="eyebrow">当前移动网络</span>
          <h2>
            中国联通 <span>5G SA</span>
          </h2>
          <p>
            {m.scenario === "offline"
              ? "正在重新建立蜂窝连接"
              : "连接正常 · 已持续连接 2 天 6 小时"}
          </p>
        </div>
        <Status muted={m.scenario === "offline"}>
          {m.scenario === "offline" ? "连接中断" : "已联网"}
        </Status>
      </div>
      <Stats
        items={[
          ["信号强度 · RSRP", "−86 dBm", "良好"],
          ["信噪比 · SINR", "24 dB", "良好"],
          ["信号质量 · RSRQ", "−10 dB", "正常"],
          ["当前频段", "n78", "100 MHz · PCI 216"],
        ]}
      />
      <div className="two-columns">
        <Panel>
          <PanelTitle
            title="链路流量"
            note="上下行速率 / Mbps"
            action={<Periods value={period} onChange={setPeriod} />}
          />
          <TrafficChart period={period} />
        </Panel>
        <Panel>
          <PanelTitle title="无线链路" />
          <Definition
            items={[
              ["接入模式", "5G 独立组网"],
              ["载波聚合", "单载波"],
              ["信道号", "627264"],
              ["网络注册", "本地网络"],
              ["IPv4", "198.51.100.24"],
              ["IPv6", "2001:db8::24"],
            ]}
          />
        </Panel>
      </div>
      <Panel>
        <PanelTitle title="频段与小区" note="先观察信号，再调整连接偏好。" />
        <div className="entry-grid">
          <Entry
            title="频段偏好与锁定"
            description="5G / 4G 频段、小区与 STC 策略"
            icon={<Radio size={22} />}
            onClick={() => setAdvanced(true)}
          />
          <Entry
            title="邻区扫描"
            description={
              scan ? "发现 3 个邻区 · 最强 −82 dBm" : "扫描附近可用小区"
            }
            icon={<Signal size={22} />}
            onClick={() => setScan(true)}
          />
        </div>
        {scan && (
          <Definition
            items={[
              ["n78 · PCI 218", "−82 dBm"],
              ["n78 · PCI 216", "−86 dBm"],
              ["B3 · PCI 118", "−94 dBm"],
            ]}
          />
        )}
      </Panel>
      {advanced && (
        <Modal title="频段与小区偏好" close={() => setAdvanced(false)} wide>
          <ConfigForm
            id="bands"
            title="连接偏好"
            initial={{ nr: "自动", lte: "自动", cell: "不锁定", stc: "自动" }}
            confirm="不合适的频段或小区锁定可能导致无服务。确认当前网络支持后再应用。"
          >
            {(v, set) => (
              <div className="form-grid">
                <Field label="5G 频段">
                  <Select
                    value={String(v.nr)}
                    options={["自动", "n41 / n78 / n79", "仅 n78"]}
                    onChange={(s) => set("nr", s)}
                  />
                </Field>
                <Field label="4G 频段">
                  <Select
                    value={String(v.lte)}
                    options={["自动", "B1 / B3 / B8", "仅 B3"]}
                    onChange={(s) => set("lte", s)}
                  />
                </Field>
                <Field label="小区锁定">
                  <Select
                    value={String(v.cell)}
                    options={["不锁定", "当前小区"]}
                    onChange={(s) => set("cell", s)}
                  />
                </Field>
                <Field label="STC 策略">
                  <Select
                    value={String(v.stc)}
                    options={["自动", "关闭"]}
                    onChange={(s) => set("stc", s)}
                  />
                </Field>
              </div>
            )}
          </ConfigForm>
        </Modal>
      )}
    </>
  );
}
