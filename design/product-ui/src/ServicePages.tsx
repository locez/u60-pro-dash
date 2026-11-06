import { useState } from "react";
import {
  ArrowLeft,
  ArrowUpRight,
  Check,
  Download,
  FileCode2,
  Network,
  Play,
  RefreshCw,
  Router,
  Search,
  ShieldCheck,
  Square,
  Terminal,
  Upload,
} from "lucide-react";
import { useModel, serviceNames } from "./model";
import { Panel, PanelTitle, Status, Toggle } from "./ui";
import {
  ConfigForm,
  Definition,
  Field,
  Modal,
  Note,
  Password,
  SearchBox,
  Stats,
  SwitchRow,
  Tabs,
} from "./controls";
import { Select } from "./ConnectionPages";
const serviceDescriptions = [
  "连接远端设备与子网",
  "管理点对点加密隧道",
  "管理规则、策略组与流量范围",
];
export function ServicesPage({ tab }: { tab: string }) {
  const m = useModel(),
    id = m.route.service;
  if (id === undefined)
    return (
      <>
        <div className="services-intro">
          <div>
            <Network size={25} />
            <h2>每一种连接，各司其职。</h2>
            <p>按服务管理连接状态、配置和维护。已安装的服务可以同时运行。</p>
          </div>
          <span className="service-total">
            <b>{m.enabled.filter(Boolean).length}</b> / 3 运行中
          </span>
        </div>
        <div className="service-cards">
          {serviceNames.map((name, i) => (
            <Panel key={name}>
              <div className="service-card-top">
                <span
                  className={"service-icon " + ["tailnet", "wg", "proxy"][i]}
                >
                  {i === 0 ? (
                    <Network size={23} />
                  ) : i === 1 ? (
                    <ShieldCheck size={23} />
                  ) : (
                    <Router size={23} />
                  )}
                </span>
                <Toggle
                  on={m.enabled[i]}
                  label={`${name} 服务`}
                  onChange={() => m.toggle(i)}
                />
              </div>
              <h2>{name}</h2>
              <p>{serviceDescriptions[i]}</p>
              <Status muted={!m.enabled[i]}>
                {m.enabled[i] ? "运行中" : "已停止"}
              </Status>
              <Definition
                items={
                  i === 0
                    ? [
                        ["节点", "u60-studio"],
                        ["接受子网路由", "已开启"],
                        ["目标连通性", "尚未检测"],
                      ]
                    : i === 1
                      ? [
                          ["接口", "wg-office"],
                          ["对端", "1 个"],
                          ["最近握手", "32 秒前"],
                        ]
                      : [
                          ["模式", "规则"],
                          ["流量范围", "Wi-Fi 热点"],
                          ["已保存配置", "studio.yaml"],
                        ]
                }
              />
              <button
                className="btn full-width"
                onClick={() =>
                  m.go({ page: "services", service: i, tab: "概览" })
                }
              >
                管理 {name}
                <ArrowUpRight size={15} />
              </button>
            </Panel>
          ))}
        </div>
        <Note>
          服务分别控制自己的路由与监听端口。添加服务时保留现有连接，不自动停止其他服务。
        </Note>
      </>
    );
  const items =
    id === 0
      ? ["概览", "网络设置", "子网路由", "连通性检测", "日志", "安装与更新"]
      : id === 1
        ? ["概览", "隧道配置", "对端", "日志"]
        : ["概览", "配置", "策略组", "日志", "安装与更新"];
  const current = items.includes(tab) ? tab : "概览";
  return (
    <>
      <button className="back-link" onClick={() => m.go({ page: "services" })}>
        <ArrowLeft size={15} /> 所有服务
      </button>
      <div className="service-detail-heading">
        <span className="large-icon">
          {id === 0 ? (
            <Network size={27} />
          ) : id === 1 ? (
            <ShieldCheck size={27} />
          ) : (
            <Router size={27} />
          )}
        </span>
        <div>
          <h2>{serviceNames[id]}</h2>
          <p>{serviceDescriptions[id]}</p>
        </div>
        <Status muted={!m.enabled[id]}>
          {m.enabled[id] ? "运行中" : "已停止"}
        </Status>
        <button className="btn" onClick={() => m.toggle(id)}>
          {m.enabled[id] ? <Square size={13} /> : <Play size={14} />}
          {m.enabled[id] ? "停止" : "启动"}
        </button>
      </div>
      <Tabs
        items={items}
        value={current}
        onChange={(tab) => m.go({ ...m.route, tab })}
      />
      {current === "日志" ? (
        <ServiceLogs id={id} />
      ) : current === "安装与更新" ? (
        <ServiceUpdate id={id} />
      ) : id === 0 ? (
        <TailscaleDesign tab={current} />
      ) : id === 1 ? (
        <WireGuardDesign tab={current} />
      ) : (
        <ProxyDesign tab={current} />
      )}
    </>
  );
}
function TailscaleDesign({ tab }: { tab: string }) {
  const m = useModel(),
    [probe, setProbe] = useState(false);
  if (tab === "网络设置")
    return (
      <ConfigForm
        id="tailscale"
        title="网络设置"
        note="远端子网与本地热点访问集中管理。"
      >
        {(v, set) => (
          <>
            <SwitchRow
              title="接受子网路由"
              hint="使用其他节点发布并获批准的子网"
              value={!!v.routes}
              onChange={() => set("routes", !v.routes)}
            />
            <SwitchRow
              title="允许热点访问远端子网"
              hint="让连接此路由器的设备也能访问这些网段"
              value={!!v.lan}
              onChange={() => set("lan", !v.lan)}
            />
            <SwitchRow
              title="使用网络 DNS"
              value={!!v.dns}
              onChange={() => set("dns", !v.dns)}
            />
            <Field label="退出节点">
              <Select
                value={String(v.exit)}
                options={["不使用退出节点", "studio-gateway"]}
                onChange={(s) => set("exit", s)}
              />
            </Field>
          </>
        )}
      </ConfigForm>
    );
  if (tab === "子网路由")
    return (
      <Panel>
        <PanelTitle
          title="接受的子网路由"
          note="路由是否获批准与目标是否可达分别显示。"
        />
        <div className="table-scroll">
          <table className="data-table">
            <thead>
              <tr>
                {["网段", "发布节点", "路由状态", "目标检测"].map((s) => (
                  <th key={s}>{s}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {[
                ["192.0.2.0/24", "studio-gateway"],
                ["198.51.100.0/24", "lab-gateway"],
                ["203.0.113.0/24", "office-gateway"],
              ].map(([net, node]) => (
                <tr key={net}>
                  <td className="mono">{net}</td>
                  <td>{node}</td>
                  <td>
                    <Status muted={!m.settings.tailscale.routes}>
                      {m.settings.tailscale.routes ? "已接受" : "未接受"}
                    </Status>
                  </td>
                  <td>尚未检测</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <Note>
          节点在线不代表目标服务可访问。到“连通性检测”验证具体地址与端口。
        </Note>
      </Panel>
    );
  if (tab === "连通性检测")
    return (
      <div className="two-columns">
        <ConfigForm
          id="tailscale"
          title="检测目标"
          note="使用经常在线的服务，验证路由后的实际连通性。"
          validate={(v) =>
            /^[a-zA-Z0-9.-]+:\d{1,5}$/.test(String(v.target))
              ? ""
              : "请输入主机名或 IPv4 地址与端口，例如 192.0.2.80:443。"
          }
        >
          {(v, set) => (
            <Field label="主机与 TCP 端口">
              <input
                value={String(v.target)}
                onChange={(e) => set("target", e.target.value)}
              />
            </Field>
          )}
        </ConfigForm>
        <Panel>
          <PanelTitle title="检测结果" />
          <div className="probe-result">
            <span className={"probe-icon " + (probe ? "success" : "")}>
              {probe ? <Check size={28} /> : <Network size={28} />}
            </span>
            <h3>{probe ? "目标服务可达" : "尚未检测"}</h3>
            <p>
              {String(m.settings.tailscale.target)} {probe && "· TCP · 38 ms"}
            </p>
            <button
              className="btn primary"
              disabled={!m.enabled[0]}
              onClick={() => setProbe(true)}
            >
              立即检测
            </button>
            {!m.enabled[0] && <small>请先启动 Tailscale</small>}
          </div>
        </Panel>
      </div>
    );
  return (
    <>
      <Stats
        items={[
          ["连接状态", m.enabled[0] ? "已连接" : "已停止", "网络：studio"],
          ["节点名称", "u60-studio"],
          ["子网路由", m.settings.tailscale.routes ? "3 条" : "未接受"],
          ["目标服务", "尚未检测", "独立验证实际连通性"],
        ]}
      />
      <div className="two-columns">
        <Panel>
          <PanelTitle title="本机身份" />
          <Definition
            items={[
              ["节点名称", "u60-studio"],
              ["节点地址", "100.64.0.10"],
              ["连接路径", "直接连接"],
              ["当前版本", "1.104.4"],
            ]}
          />
        </Panel>
        <Panel>
          <PanelTitle title="网络能力" />
          <Definition
            items={[
              ["接受路由", m.settings.tailscale.routes ? "已开启" : "已关闭"],
              ["热点访问子网", m.settings.tailscale.lan ? "已开启" : "已关闭"],
              ["退出节点", String(m.settings.tailscale.exit)],
              ["网络 DNS", m.settings.tailscale.dns ? "已开启" : "已关闭"],
            ]}
          />
        </Panel>
      </div>
    </>
  );
}
function WireGuardDesign({ tab }: { tab: string }) {
  const m = useModel();
  if (tab === "隧道配置")
    return (
      <ConfigForm
        id="wireguard"
        title="隧道配置"
        note="每条隧道保留自己的地址、密钥与路由。"
        initial={{
          name: "wg-office",
          address: "10.80.0.2/32",
          private: "demo-private-key",
          mtu: "1420",
          route: true,
        }}
        confirm="应用隧道设置会重新建立此 WireGuard 连接。其他服务继续运行。"
      >
        {(v, set) => (
          <>
            <div className="form-grid">
              <Field label="接口名称">
                <input
                  value={String(v.name)}
                  onChange={(e) => set("name", e.target.value)}
                />
              </Field>
              <Field label="本地地址">
                <input
                  value={String(v.address)}
                  onChange={(e) => set("address", e.target.value)}
                />
              </Field>
              <Field label="私钥">
                <Password
                  value={String(v.private)}
                  onChange={(s) => set("private", s)}
                />
              </Field>
              <Field label="MTU">
                <input
                  inputMode="numeric"
                  value={String(v.mtu)}
                  onChange={(e) => set("mtu", e.target.value)}
                />
              </Field>
            </div>
            <SwitchRow
              title="安装隧道路由"
              value={!!v.route}
              onChange={() => set("route", !v.route)}
            />
          </>
        )}
      </ConfigForm>
    );
  if (tab === "对端")
    return (
      <ConfigForm
        id="wg-peer"
        title="office-gateway"
        initial={{
          endpoint: "gateway.example.test:51820",
          public: "demo-public-key",
          allowed: "10.80.0.0/24",
          keepalive: "25",
        }}
      >
        {(v, set) => (
          <div className="form-grid">
            {[
              ["endpoint", "对端地址"],
              ["public", "公钥"],
              ["allowed", "允许的网段"],
              ["keepalive", "保活间隔 / 秒"],
            ].map(([key, name]) => (
              <Field key={key} label={name}>
                <input
                  value={String(v[key])}
                  onChange={(e) => set(key, e.target.value)}
                />
              </Field>
            ))}
          </div>
        )}
      </ConfigForm>
    );
  return (
    <>
      <Stats
        items={[
          ["接口", "wg-office"],
          ["对端", "1 个"],
          ["最近握手", m.enabled[1] ? "32 秒前" : "服务已停止"],
          ["本次流量", "1.28 GB"],
        ]}
      />
      <Panel>
        <PanelTitle title="隧道状态" />
        <Definition
          items={[
            ["本地地址", "10.80.0.2/32"],
            ["对端端点", "gateway.example.test:51820"],
            ["允许的网段", "10.80.0.0/24"],
            ["MTU", "1420"],
          ]}
        />
      </Panel>
    </>
  );
}
const demoYaml = `mixed-port: 7890\nmode: rule\nlog-level: info\n\nproxy-groups:\n  - name: 工作流量\n    type: select\n    proxies: [自动选择, DIRECT]\n\nrules:\n  - DOMAIN-SUFFIX,example.test,DIRECT\n  - MATCH,工作流量\n`;
function ProxyDesign({ tab }: { tab: string }) {
  const m = useModel(),
    saved = String(m.settings.proxyYaml?.text || demoYaml);
  const applied = String(m.settings.proxyYaml?.applied || demoYaml);
  const [yaml, setYaml] = useState(saved),
    [validated, setValidated] = useState(false),
    [source, setSource] = useState(""),
    [dashboard, setDashboard] = useState(false);
  const [groups, setGroups] = useState<Record<string, string>>({
    默认流量: "自动选择",
    工作流量: "美国",
    视频服务: "香港",
    本地网络: "直连",
  });
  if (tab === "配置")
    return (
      <Panel>
        <PanelTitle
          title="当前配置"
          note="studio.yaml · 原生 YAML"
          action={
            <span className="file-label">
              <FileCode2 size={15} />{" "}
              {yaml !== saved
                ? "未保存"
                : applied !== saved
                  ? "已保存，待重载"
                  : "与运行配置一致"}
            </span>
          }
        />
        <div className="editor-toolbar">
          <button
            className="btn"
            onClick={() => {
              setYaml(saved);
              setValidated(false);
            }}
          >
            重新读取已保存配置
          </button>
          <label className="btn file-button">
            <Upload size={14} /> 导入文件
            <input
              type="file"
              accept=".yaml,.yml,.json"
              onChange={async (e) => {
                const file = e.target.files?.[0];
                if (file) {
                  setYaml(await file.text());
                  setValidated(false);
                }
              }}
            />
          </label>
        </div>
        <textarea
          className="yaml-editor"
          aria-label="配置 YAML"
          spellCheck={false}
          value={yaml}
          onChange={(e) => {
            setYaml(e.target.value);
            setValidated(false);
          }}
        />
        <div className="editor-actions">
          <span>
            {validated ? "已演示校验完成" : "保存前校验，保存后再应用到服务"}
          </span>
          <button
            className="btn"
            onClick={() => {
              setValidated(true);
              m.notify("已模拟配置校验；正式版本由服务内核校验");
            }}
          >
            校验
          </button>
          <button
            className="btn"
            disabled={!yaml.trim() || yaml === saved}
            onClick={() => {
              m.save("proxyYaml", { text: yaml, applied });
              setValidated(true);
              m.notify("配置已保存在预览内存中");
            }}
          >
            保存配置
          </button>
          <button
            className="btn primary"
            disabled={!m.enabled[2] || yaml !== saved || applied === saved}
            onClick={() => {
              m.save("proxyYaml", { text: saved, applied: saved });
              m.notify("已模拟重载当前配置");
            }}
          >
            <RefreshCw size={14} /> 应用 / reload
          </button>
        </div>
        <details className="advanced-details">
          <summary>从订阅 URL 导入</summary>
          <div className="inline-form">
            <Field label="订阅地址" hint="可直接返回 YAML，不遮盖输入内容">
              <input
                type="url"
                value={source}
                placeholder="https://example.test/config.yaml"
                onChange={(e) => setSource(e.target.value)}
              />
            </Field>
            <button
              className="btn"
              disabled={!/^https?:\/\//.test(source)}
              onClick={() => {
                setYaml(demoYaml + "\n# 订阅导入预览\n");
                setValidated(false);
                m.notify("已填入示例订阅内容；未访问输入的网址");
              }}
            >
              读取到编辑器
            </button>
          </div>
        </details>
      </Panel>
    );
  if (tab === "策略组")
    return (
      <div className="proxy-groups">
        {Object.entries(groups).map(([group, chosen]) => (
          <Panel key={group}>
            <PanelTitle title={group} note="手动选择" />
            <div className="proxy-options">
              {["自动选择", "香港", "日本", "美国", "直连"].map((name, i) => (
                <button
                  key={name}
                  aria-pressed={chosen === name}
                  onClick={() => setGroups((s) => ({ ...s, [group]: name }))}
                >
                  <span>{name}</span>
                  <small>
                    {["智能选择", "32 ms", "68 ms", "144 ms", "DIRECT"][i]}
                  </small>
                  {chosen === name && <Check size={15} />}
                </button>
              ))}
            </div>
          </Panel>
        ))}
      </div>
    );
  return (
    <>
      <Stats
        items={[
          ["运行模式", "规则"],
          ["流量范围", "Wi-Fi 热点"],
          ["活动连接", "28"],
          ["当前配置", "studio.yaml"],
        ]}
      />
      <div className="two-columns">
        <ConfigForm
          id="proxyRuntime"
          title="运行设置"
          note="明确显示会覆盖配置文件的选项。"
          initial={{
            mode: "规则",
            scope: "Wi-Fi 热点",
            port: "7890",
            ipv6: true,
            concurrent: true,
          }}
          confirm={
            m.enabled[2]
              ? "应用流量范围与监听端口会重启本服务，现有代理连接会短暂中断。Tailscale 与 WireGuard 保持运行。"
              : undefined
          }
        >
          {(v, set) => (
            <>
              <div className="form-grid">
                <Field label="运行模式">
                  <Select
                    value={String(v.mode)}
                    options={["规则", "全局", "直连"]}
                    onChange={(s) => set("mode", s)}
                  />
                </Field>
                <Field label="流量范围">
                  <Select
                    value={String(v.scope)}
                    options={["Wi-Fi 热点", "仅本机 HTTP / SOCKS"]}
                    onChange={(s) => set("scope", s)}
                  />
                </Field>
                <Field label="HTTP / SOCKS 混合端口">
                  <input
                    inputMode="numeric"
                    value={String(v.port)}
                    onChange={(e) => set("port", e.target.value)}
                  />
                </Field>
              </div>
              <SwitchRow
                title="IPv6"
                value={!!v.ipv6}
                onChange={() => set("ipv6", !v.ipv6)}
              />
              <SwitchRow
                title="并发 TCP 连接"
                value={!!v.concurrent}
                onChange={() => set("concurrent", !v.concurrent)}
              />
            </>
          )}
        </ConfigForm>
        <div className="stack">
          <Panel>
            <PanelTitle
              title="控制面板"
              note="查看连接、规则与策略组的完整视图"
            />
            <div className="dashboard-preview">
              <div>
                <i />
                <i />
                <i />
              </div>
              <span />
              <span />
              <span />
              <span />
            </div>
            <button
              className="btn full-width"
              disabled={!m.enabled[2]}
              onClick={() => setDashboard(true)}
            >
              打开独立面板 <ArrowUpRight size={15} />
            </button>
          </Panel>
          <Note>
            本服务的流量规则保留已存在的子网路由；USB 管理连接不纳入代理。
          </Note>
        </div>
      </div>
      {dashboard && (
        <Modal title="独立控制面板" close={() => setDashboard(false)}>
          <p>入口使用当前管理会话打开面板，无需手工输入控制器地址与密钥。</p>
          <Note>这是入口与跳转流程的设计预览，未加载外部面板。</Note>
        </Modal>
      )}
    </>
  );
}
function ServiceLogs({ id }: { id: number }) {
  const m = useModel(),
    [query, setQuery] = useState(""),
    [level, setLevel] = useState("全部级别");
  const lines = [
    ["14:08:32", "INFO", "Service ready"],
    ["14:08:32", "INFO", "Configuration loaded successfully"],
    [
      "14:08:33",
      "INFO",
      id === 0
        ? "Subnet routes accepted: 3"
        : id === 1
          ? "Peer handshake completed"
          : "Controller listening on loopback",
    ],
    ["14:08:34", "WARN", "Connectivity target has not been checked"],
    ["14:08:35", "INFO", "Network is available"],
  ].filter(
    (l) =>
      (level === "全部级别" || l[1] === level) &&
      l.join(" ").toLowerCase().includes(query.toLowerCase()),
  );
  return (
    <Panel>
      <PanelTitle
        title={`${serviceNames[id]} 日志`}
        note="保留阅读位置，只追加新的日志。"
      />
      <div className="table-toolbar">
        <SearchBox
          value={query}
          onChange={setQuery}
          placeholder="搜索日志内容"
        />
        <Select
          value={level}
          onChange={setLevel}
          options={["全部级别", "INFO", "WARN", "ERROR"]}
        />
        <button
          className="btn"
          onClick={() => m.notify("示例日志已是最新；没有启动轮询")}
        >
          <RefreshCw size={14} /> 读取最新
        </button>
      </div>
      <div className="log-console" role="log">
        {lines.length ? (
          lines.map(([time, level, text], i) => (
            <div key={i}>
              <time>{time}</time>
              <b className={level === "WARN" ? "warn" : ""}>{level}</b>
              <span>{text}</span>
            </div>
          ))
        ) : (
          <p>没有符合条件的日志。</p>
        )}
      </div>
    </Panel>
  );
}
function ServiceUpdate({ id }: { id: number }) {
  const m = useModel(),
    [checked, setChecked] = useState(false),
    [upload, setUpload] = useState("");
  return (
    <div className="two-columns">
      <Panel>
        <PanelTitle title="安装与更新" note="自动识别设备平台与对应安装包。" />
        <div className="version-identity">
          <Download size={29} />
          <div>
            <small>当前版本</small>
            <h2>{id === 0 ? "1.104.4" : "稳定版"}</h2>
          </div>
          <Status>已安装</Status>
        </div>
        <Definition
          items={[
            ["设备平台", "Linux · ARM64"],
            ["更新通道", "稳定版本"],
            ["版本检查", checked ? "当前已是最新版本（示例）" : "尚未检查"],
          ]}
        />
        <button className="btn primary" onClick={() => setChecked(true)}>
          <RefreshCw size={15} /> 检查更新
        </button>
      </Panel>
      <Panel>
        <PanelTitle title="手动安装" note="离线使用，或安装指定版本。" />
        <Field label="安装包">
          <input
            type="file"
            onChange={(e) => setUpload(e.target.files?.[0]?.name || "")}
          />
        </Field>
        <p className="muted">{upload || "选择适用于 Linux ARM64 的安装包"}</p>
        <button
          className="btn"
          disabled={!upload}
          onClick={() => m.notify("已模拟校验安装包；不会安装到设备")}
        >
          <Upload size={15} /> 校验安装包
        </button>
        <Note>升级前校验架构与文件完整性；失败时保留当前版本。</Note>
      </Panel>
    </div>
  );
}
