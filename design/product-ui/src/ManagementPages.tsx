import { PerformanceControls } from "./ModeControls";
import { useState } from "react";
import {
  Activity,
  Archive,
  ArrowDownToLine,
  BatteryCharging,
  Check,
  ChevronRight,
  Clock,
  Cpu,
  Download,
  HardDrive,
  KeyRound,
  Mail,
  MessageSquare,
  MoreHorizontal,
  Plus,
  Power,
  RefreshCw,
  Send,
  Settings2,
  ShieldCheck,
  Smartphone,
  Terminal,
  Trash2,
  Zap,
} from "lucide-react";
import { useModel } from "./model";
import AccentPicker from "./AccentPicker";
import HomeLayoutSettings from "./HomeLayoutSettings";
import { Battery, Panel, PanelTitle, Status } from "./ui";
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
import { Select } from "./ConnectionPages";

export const sampleMessages = [
  {
    id: 1,
    from: "中国联通",
    number: "10010",
    time: "14:02",
    unread: true,
    text: "流量使用提醒\n\n截至今天 14:00，您本月已使用流量 62.8 GB，套餐内剩余流量 37.2 GB。\n\n您可在运营商客户端查询详细账单，或通过官方客服了解套餐内容。\n\n感谢您的使用。",
    preview: "本月流量已使用 62.8 GB…",
  },
  {
    id: 2,
    from: "服务通知",
    number: "106000",
    time: "昨天",
    unread: false,
    text: "网络服务已成功开通。请保持设备在线，配置将在连接后自动生效。\n\n如果暂时无法使用，可以检查移动数据开关、SIM 状态以及套餐余量。\n\n以下是详细使用说明：\n\n1. 首次连接请确认设备有良好的移动网络信号。\n\n2. 使用设备上显示的 Wi-Fi 名称连接，也可以扫描连接二维码。\n\n3. 连接后可通过管理页面查看流量和连接设备。\n\n4. 离开设备时请使用锁屏保护配置与连接信息。\n\n5. 遇到问题时可先查看诊断结果，再联系服务提供方。\n\n本条为长短信设计示例。\n\n—— 消息结束 ——",
    preview: "网络服务已开通，查看详细说明。",
  },
  {
    id: 3,
    from: "中国联通",
    number: "10010",
    time: "11 月 4 日",
    unread: false,
    text: "您的套餐已生效，有效期至本月底。感谢使用。",
    preview: "您的套餐已生效。",
  },
];
export function MessagesPage({ tab }: { tab: string }) {
  const m = useModel(),
    [messages, setMessages] = useState(sampleMessages),
    [selected, setSelected] = useState(1),
    [query, setQuery] = useState(""),
    [compose, setCompose] = useState(false),
    [recipient, setRecipient] = useState(""),
    [body, setBody] = useState(""),
    [confirmDelete, setConfirmDelete] = useState(false);
  const current = messages.find((x) => x.id === selected),
    list = messages.filter((x) => (x.from + x.text).includes(query));
  if (tab === "转发规则")
    return (
      <div className="two-columns">
        <ConfigForm
          id="sms-forward"
          title="短信自动转发"
          note="匹配规则与投递记录分开管理。"
          initial={{
            enabled: false,
            channel: "Webhook",
            url: "",
            filter: "全部短信",
          }}
        >
          {(v, set) => (
            <>
              <SwitchRow
                title="启用转发"
                value={!!v.enabled}
                onChange={() => set("enabled", !v.enabled)}
              />
              <Field label="转发方式">
                <Select
                  value={String(v.channel)}
                  options={["Webhook", "邮件"]}
                  onChange={(s) => set("channel", s)}
                />
              </Field>
              <Field label="接收地址">
                <input
                  value={String(v.url)}
                  placeholder="https://example.test/webhook"
                  onChange={(e) => set("url", e.target.value)}
                />
              </Field>
              <Field label="匹配条件">
                <Select
                  value={String(v.filter)}
                  options={["全部短信", "包含关键词", "指定发件人"]}
                  onChange={(s) => set("filter", s)}
                />
              </Field>
            </>
          )}
        </ConfigForm>
        <Panel>
          <PanelTitle title="转发记录" />
          <div className="empty-state">
            <Send size={26} />
            <h3>暂无转发记录</h3>
            <p>规则生效后，投递结果会显示在这里。</p>
          </div>
        </Panel>
      </div>
    );
  if (tab === "存储管理")
    return (
      <Panel>
        <PanelTitle title="短信存储" note="删除操作仅影响已选择的消息。" />
        <Stats
          items={[
            ["已使用", `${messages.length} 条`],
            ["容量", "500 条"],
            ["存储位置", "设备"],
          ]}
        />
        <Note>收件箱内容按需加载，阅读长短信时保留当前滚动位置。</Note>
      </Panel>
    );
  return (
    <Panel className="messages-panel">
      <div className="inbox-toolbar">
        <SearchBox value={query} onChange={setQuery} placeholder="搜索短信" />
        <button className="btn primary" onClick={() => setCompose(true)}>
          <Plus size={16} /> 新短信
        </button>
      </div>
      <div className="inbox-layout">
        <div className="message-list">
          {list.map((item) => (
            <button
              key={item.id}
              className={item.id === selected ? "active" : ""}
              onClick={() => {
                setSelected(item.id);
                setMessages((s) =>
                  s.map((x) =>
                    x.id === item.id ? { ...x, unread: false } : x,
                  ),
                );
              }}
            >
              <div>
                <b>
                  {item.from}
                  {item.unread && <i />}
                </b>
                <time>{item.time}</time>
              </div>
              <p>{item.preview}</p>
            </button>
          ))}
          {!list.length && <div className="empty-state">未找到短信</div>}
        </div>
        <article className="message-detail">
          {current ? (
            <>
              <header>
                <span className="message-avatar">
                  <MessageSquare size={23} />
                </span>
                <div>
                  <h2>{current.from}</h2>
                  <p>
                    {current.number} · {current.time}
                  </p>
                </div>
                <button
                  className="icon-btn"
                  aria-label="删除当前短信"
                  onClick={() => setConfirmDelete(true)}
                >
                  <Trash2 size={18} />
                </button>
              </header>
              <div className="message-body" key={current.id}>
                {current.text}
              </div>
              <footer>
                <button
                  className="btn"
                  onClick={() => {
                    setRecipient(current.number);
                    setCompose(true);
                  }}
                >
                  <Send size={14} /> 回复
                </button>
                <small>消息已保存在设备</small>
              </footer>
            </>
          ) : (
            <div className="empty-state">
              <Mail size={30} />
              <h3>选择一条消息</h3>
            </div>
          )}
        </article>
      </div>
      {compose && (
        <Modal title="新短信" close={() => setCompose(false)}>
          <Field label="收件人">
            <input
              type="tel"
              value={recipient}
              onChange={(e) => setRecipient(e.target.value)}
              placeholder="输入手机号码"
            />
          </Field>
          <Field label="短信内容">
            <textarea
              rows={6}
              value={body}
              onChange={(e) => setBody(e.target.value)}
            />
          </Field>
          <p className="muted">{body.length} 字 · 发送可能产生运营商费用</p>
          <div className="dialog-actions">
            <button
              className="btn primary"
              disabled={!recipient.trim() || !body.trim()}
              onClick={() => {
                setCompose(false);
                setBody("");
                m.notify("已模拟发送短信；未向任何人发送消息");
              }}
            >
              <Send size={15} /> 发送
            </button>
          </div>
        </Modal>
      )}
      {confirmDelete && (
        <Modal title="删除这条短信？" close={() => setConfirmDelete(false)}>
          <p>删除后无法恢复。仅删除当前选中的消息。</p>
          <div className="dialog-actions">
            <button className="btn" onClick={() => setConfirmDelete(false)}>
              取消
            </button>
            <button
              className="btn danger"
              onClick={() => {
                setMessages((s) => s.filter((x) => x.id !== selected));
                setConfirmDelete(false);
              }}
            >
              删除短信
            </button>
          </div>
        </Modal>
      )}
    </Panel>
  );
}

export function LanPage({ tab }: { tab: string }) {
  const m = useModel(),
    [ports, setPorts] = useState(["NAS · TCP 8443 → 192.0.2.80:443"]),
    [dialog, setDialog] = useState(""),
    [name, setName] = useState("");
  if (tab === "DNS")
    return (
      <div className="two-columns">
        <ConfigForm
          id="dns"
          title="DNS 解析"
          initial={{
            mode: "自动获取",
            primary: "223.5.5.5",
            secondary: "119.29.29.29",
            encrypted: false,
            endpoint: "https://dns.example.test/dns-query",
          }}
        >
          {(v, set) => (
            <>
              <Field label="上游 DNS">
                <Select
                  value={String(v.mode)}
                  options={["自动获取", "手动指定"]}
                  onChange={(s) => set("mode", s)}
                />
              </Field>
              {v.mode === "手动指定" && (
                <div className="form-grid">
                  <Field label="首选 DNS">
                    <input
                      value={String(v.primary)}
                      onChange={(e) => set("primary", e.target.value)}
                    />
                  </Field>
                  <Field label="备用 DNS">
                    <input
                      value={String(v.secondary)}
                      onChange={(e) => set("secondary", e.target.value)}
                    />
                  </Field>
                </div>
              )}
              <SwitchRow
                title="加密 DNS"
                hint="使用 HTTPS 加密解析请求"
                value={!!v.encrypted}
                onChange={() => set("encrypted", !v.encrypted)}
              />
              {v.encrypted && (
                <Field label="DoH 地址">
                  <input
                    type="url"
                    value={String(v.endpoint)}
                    onChange={(e) => set("endpoint", e.target.value)}
                  />
                </Field>
              )}
            </>
          )}
        </ConfigForm>
        <Panel>
          <PanelTitle title="解析缓存" />
          <Stats
            items={[
              ["缓存条目", "128"],
              ["命中率", "92%"],
            ]}
          />
          <button
            className="btn"
            onClick={() => m.notify("已模拟清空 DNS 缓存")}
          >
            <RefreshCw size={14} /> 清空缓存
          </button>
          <Note>DNS 设置影响路由器与使用此 DNS 的客户端。</Note>
        </Panel>
      </div>
    );
  if (tab === "访问与转发")
    return (
      <>
        <div className="two-columns">
          <Panel>
            <PanelTitle
              title="端口转发"
              action={
                <button
                  className="btn small"
                  onClick={() => setDialog("添加转发规则")}
                >
                  <Plus size={14} /> 添加规则
                </button>
              }
            />
            {ports.map((item) => (
              <div className="forward-rule" key={item}>
                <span>
                  <b>{item.split(" · ")[0]}</b>
                  <small className="mono">{item.split(" · ")[1]}</small>
                </span>
                <button
                  className="icon-btn"
                  aria-label={`删除 ${item}`}
                  onClick={() => setPorts((s) => s.filter((p) => p !== item))}
                >
                  <Trash2 size={16} />
                </button>
              </div>
            ))}
          </Panel>
          <ConfigForm
            id="firewall"
            title="访问控制"
            initial={{ firewall: true, upnp: false, alg: true }}
          >
            {(v, set) => (
              <>
                <SwitchRow
                  title="防火墙"
                  value={!!v.firewall}
                  onChange={() => set("firewall", !v.firewall)}
                />
                <SwitchRow
                  title="UPnP"
                  hint="允许应用自动申请端口映射"
                  value={!!v.upnp}
                  onChange={() => set("upnp", !v.upnp)}
                />
                <SwitchRow
                  title="VPN / ALG 穿透"
                  value={!!v.alg}
                  onChange={() => set("alg", !v.alg)}
                />
              </>
            )}
          </ConfigForm>
        </div>
        <Panel>
          <PanelTitle title="高级网络" />
          <div className="entry-grid">
            {["DMZ 主机", "NAT 类型", "QoS 流量优先级", "域名过滤"].map((s) => (
              <Entry
                key={s}
                title={s}
                description="查看与调整规则"
                onClick={() => setDialog(s)}
              />
            ))}
          </div>
        </Panel>
        {dialog && (
          <Modal title={dialog} close={() => setDialog("")}>
            <ConfigForm
              id={"lan-" + dialog}
              title={dialog}
              initial={{ enabled: false, value: "" }}
            >
              {(v, set) => (
                <>
                  <SwitchRow
                    title="启用此规则"
                    value={!!v.enabled}
                    onChange={() => set("enabled", !v.enabled)}
                  />
                  <Field
                    label={
                      dialog === "添加转发规则"
                        ? "规则名称与目标端口"
                        : dialog === "DMZ 主机"
                          ? "目标 IP"
                          : "规则内容"
                    }
                  >
                    <input
                      value={String(v.value)}
                      placeholder={
                        dialog === "DMZ 主机" ? "192.0.2.80" : "输入规则内容"
                      }
                      onChange={(e) => set("value", e.target.value)}
                    />
                  </Field>
                </>
              )}
            </ConfigForm>
          </Modal>
        )}
      </>
    );
  return (
    <div className="two-columns">
      <ConfigForm
        id="lan"
        title="LAN 与 DHCP"
        note="为连接到路由器的设备分配本地地址。"
        initial={{
          address: "192.0.2.1",
          mask: "255.255.255.0",
          dhcp: true,
          start: "192.0.2.10",
          end: "192.0.2.200",
          lease: "24 小时",
        }}
        confirm={(_, __, key) =>
          ["address", "mask"].includes(key)
            ? "修改 LAN 地址或子网掩码会影响当前管理连接。应用后，需要重新连接并使用新地址访问。"
            : undefined
        }
      >
        {(v, set) => (
          <>
            <div className="form-grid">
              <Field label="路由器地址">
                <input
                  value={String(v.address)}
                  onChange={(e) => set("address", e.target.value)}
                />
              </Field>
              <Field label="子网掩码">
                <input
                  value={String(v.mask)}
                  onChange={(e) => set("mask", e.target.value)}
                />
              </Field>
            </div>
            <SwitchRow
              title="DHCP 服务"
              value={!!v.dhcp}
              onChange={() => set("dhcp", !v.dhcp)}
            />
            <div className="form-grid">
              <Field label="地址池起始">
                <input
                  value={String(v.start)}
                  onChange={(e) => set("start", e.target.value)}
                />
              </Field>
              <Field label="地址池结束">
                <input
                  value={String(v.end)}
                  onChange={(e) => set("end", e.target.value)}
                />
              </Field>
            </div>
            <Field label="租期">
              <Select
                value={String(v.lease)}
                options={["1 小时", "12 小时", "24 小时", "7 天"]}
                onChange={(s) => set("lease", s)}
              />
            </Field>
          </>
        )}
      </ConfigForm>
      <Panel>
        <PanelTitle title="网络接口" />
        <Definition
          items={[
            ["无线热点", "192.0.2.1/24"],
            ["USB 管理", "独立连接"],
            ["移动数据", "198.51.100.24"],
            ["地址分配", "6 个租约"],
          ]}
        />
        <Note>这里管理本地地址。远端子网与隧道路由在对应组网服务内管理。</Note>
      </Panel>
    </div>
  );
}

function ResourceChart({ kind = "cpu" }: { kind?: "cpu" | "memory" | "temp" }) {
  const values =
    kind === "cpu"
      ? [12, 15, 18, 11, 10, 18, 22, 16, 12, 13, 10, 12, 17, 15, 12]
      : kind === "memory"
        ? [41, 42, 42, 44, 44, 42, 43, 44, 44, 44, 43, 43, 44, 44, 44]
        : [36, 37, 37, 38, 38, 39, 39, 38, 38, 39, 40, 40, 39, 39, 39];
  const points = values
    .map(
      (v, i) =>
        `${(i * 400) / (values.length - 1)},${90 - v * (kind === "cpu" ? 2 : 1.5)}`,
    )
    .join(" ");
  return (
    <div className="resource-chart">
      <svg
        viewBox="0 0 400 100"
        role="img"
        aria-label={
          kind === "cpu"
            ? "CPU 使用率趋势，当前 12%"
            : kind === "memory"
              ? "内存趋势，当前 44%"
              : "温度趋势，当前 39 摄氏度"
        }
      >
        <path d="M0 25H400M0 55H400M0 85H400" stroke="var(--line)" />
        <polyline
          points={points}
          fill="none"
          stroke={
            kind === "temp"
              ? "#b58b51"
              : kind === "memory"
                ? "#63a98e"
                : "#6a91da"
          }
          strokeWidth="2"
        />
      </svg>
      <div>
        <span>30 分钟前</span>
        <span>现在</span>
      </div>
    </div>
  );
}
export function SystemPage({ tab }: { tab: string }) {
  const m = useModel(),
    [dialog, setDialog] = useState(""),
    [checked, setChecked] = useState(false),
    [task, setTask] = useState(false);
  if (tab === "屏幕与安全")
    return (
      <div className="two-columns">
        <ConfigForm id="security" title="设备屏幕" note="屏幕显示与解锁方式。">
          {(v, set) => (
            <>
              <div className="form-grid">
                <Field label="界面语言">
                  <Select
                    value={String(v.language)}
                    options={[
                      "简体中文",
                      "繁體中文",
                      "English",
                      "日本語",
                      "한국어",
                    ]}
                    onChange={(s) => set("language", s)}
                  />
                </Field>
                <Field label="外观">
                  <Select
                    value={String(v.theme)}
                    options={["深色", "浅色", "跟随系统"]}
                    onChange={(s) => set("theme", s)}
                  />
                </Field>
                <Field label="屏幕亮度">
                  <Select
                    value={String(v.brightness)}
                    options={["30%", "60%", "100%"]}
                    onChange={(s) => set("brightness", s)}
                  />
                </Field>
                <Field label="自动锁屏">
                  <Select
                    value={String(v.timeout)}
                    options={["30 秒", "1 分钟", "3 分钟"]}
                    onChange={(s) => set("timeout", s)}
                  />
                </Field>
              </div>
              <AccentPicker
                value={String(v.accent)}
                onChange={(color) => set("accent", color)}
              />
              <SwitchRow
                title="密码解锁"
                hint="长按 1 秒后输入 PIN；关闭时仅长按解锁"
                value={!!v.pin}
                onChange={() => set("pin", !v.pin)}
              />
              {v.pin && (
                <Field label="设置 4 位 PIN">
                  <input
                    type="password"
                    inputMode="numeric"
                    maxLength={4}
                    placeholder="输入新 PIN"
                  />
                </Field>
              )}
              <Note>
                锁屏继续显示网络、速率、电量与充电状态，隐藏密码及管理操作。
              </Note>
            </>
          )}
        </ConfigForm>
        <div className="stack">
          <Panel>
            <PanelTitle
              title="屏幕首页布局"
              note="选择模块与指标，调整后即时保存。"
            />
            <HomeLayoutSettings
              value={m.homeLayout}
              onChange={m.setHomeLayout}
            />
          </Panel>
          <Panel>
            <PanelTitle title="管理访问" />
            <Definition
              items={[
                ["管理端口", "2026"],
                ["当前会话", "本地管理员"],
                ["HTTPS", "未启用"],
              ]}
            />
            <Entry
              title="管理员与会话"
              description="修改密码、管理登录状态"
              onClick={() => setDialog("管理员与会话")}
            />
            <Entry
              title="HTTPS 与证书"
              description="配置受信任的管理连接"
              onClick={() => setDialog("HTTPS 与证书")}
            />
          </Panel>
        </div>
        {dialog && (
          <Modal title={dialog} close={() => setDialog("")}>
            <Field
              label={dialog.includes("HTTPS") ? "证书文件" : "新的管理密码"}
            >
              <input type={dialog.includes("HTTPS") ? "file" : "password"} />
            </Field>
            <button
              className="btn primary"
              onClick={() => {
                setDialog("");
                m.notify("已演示管理访问设置");
              }}
            >
              保存
            </button>
          </Modal>
        )}
      </div>
    );
  if (tab === "更新与备份")
    return (
      <>
        <div className="two-columns">
          <Panel>
            <PanelTitle
              title="U60 Dash 项目更新"
              note="后台与设备屏幕作为一套版本发布。"
            />
            <div className="version-identity">
              <Download size={30} />
              <div>
                <small>当前安装</small>
                <h2>开发预览版</h2>
              </div>
              <Status>运行正常</Status>
            </div>
            <Definition
              items={[
                ["发布通道", "稳定版本"],
                ["更新范围", "后台 + 设备屏幕"],
                ["版本检查", checked ? "已完成示例检查" : "尚未检查"],
              ]}
            />
            <button className="btn primary" onClick={() => setChecked(true)}>
              <RefreshCw size={14} /> 检查更新
            </button>
          </Panel>
          <Panel>
            <PanelTitle title="配置备份" note="保留设置，迁移与恢复更从容。" />
            <div className="entry-grid single">
              <Entry
                title="导出配置"
                description="选择需要备份的配置范围"
                onClick={() => setDialog("导出配置")}
                icon={<Archive size={23} />}
              />
              <Entry
                title="恢复配置"
                description="先预览差异，再应用"
                onClick={() => setDialog("恢复配置")}
                icon={<ArrowDownToLine size={23} />}
              />
            </div>
          </Panel>
        </div>
        <Panel>
          <PanelTitle title="设备维护" />
          <Definition
            items={[
              ["型号", "ZTE U60 Pro / MU5250"],
              ["系统固件", "B31"],
              ["运行时间", "2 天 6 小时"],
              ["管理入口", "192.0.2.1:2026"],
            ]}
          />
          <div className="maintenance-actions">
            <button className="btn" onClick={() => setDialog("重启设备")}>
              <Power size={15} /> 重启设备
            </button>
            <button
              className="btn text-danger"
              onClick={() => setDialog("恢复出厂设置")}
            >
              恢复出厂设置
            </button>
          </div>
        </Panel>
        {dialog && (
          <Modal title={dialog} close={() => setDialog("")}>
            {dialog === "恢复配置" ? (
              <Field label="备份文件">
                <input type="file" />
              </Field>
            ) : (
              <p>
                {dialog === "重启设备"
                  ? "设备重启期间会暂时断开所有连接。"
                  : dialog === "恢复出厂设置"
                    ? "此操作会删除设备设置并中断连接。请先导出需要保留的配置。"
                    : "备份包含网络与服务设置，可能包含连接凭据。请妥善保存。"}
              </p>
            )}
            <div className="dialog-actions">
              <button className="btn" onClick={() => setDialog("")}>
                取消
              </button>
              <button
                className="btn primary"
                onClick={() => {
                  setDialog("");
                  m.notify("已模拟维护操作；真实设备未发生改变");
                }}
              >
                确认
              </button>
            </div>
          </Modal>
        )}
      </>
    );
  if (tab === "诊断工具")
    return (
      <>
        <div className="resource-grid">
          {(
            [
              ["cpu", "CPU 使用率", "12%"],
              ["memory", "内存", "44%"],
              ["temp", "设备温度", "39°C"],
            ] as const
          ).map(([kind, name, value]) => (
            <Panel key={kind}>
              <PanelTitle title={name} />
              <strong className="resource-value">{value}</strong>
              <ResourceChart kind={kind} />
            </Panel>
          ))}
        </div>
        <Panel>
          <PanelTitle
            title="诊断工具"
            note="需要时运行，结果保留在当前任务中。"
          />
          <div className="entry-grid">
            {[
              ["当前进程", "查看进程与资源占用", Cpu],
              ["互联网测速", "测试下载、上传与延迟", Activity],
              ["局域网测速", "在局域网内测量传输能力", HardDrive],
              ["AT 终端", "发送调制解调器命令", Terminal],
            ].map(([name, description, Icon]) => {
              const I = Icon as typeof Cpu;
              return (
                <Entry
                  key={String(name)}
                  title={String(name)}
                  description={String(description)}
                  icon={<I size={23} />}
                  onClick={() => setDialog(String(name))}
                />
              );
            })}
          </div>
        </Panel>
        {dialog && (
          <Modal
            title={dialog}
            close={() => {
              setDialog("");
              setTask(false);
            }}
            wide
          >
            {dialog === "当前进程" ? (
              <>
                <table className="data-table">
                  <thead>
                    <tr>
                      <th>进程</th>
                      <th>CPU</th>
                      <th>内存</th>
                    </tr>
                  </thead>
                  <tbody>
                    {[
                      ["zte-agent", "0.8%", "18 MB"],
                      ["u60-screen", "0.3%", "26 MB"],
                      ["tailscaled", "1.2%", "42 MB"],
                    ].map(([name, cpu, mem]) => (
                      <tr key={name}>
                        <td>{name}</td>
                        <td>{cpu}</td>
                        <td>{mem}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <Note>
                  当前进程是只读视图。结束进程应作为单独的操作明确确认。
                </Note>
              </>
            ) : dialog === "AT 终端" ? (
              <>
                <Field label="命令">
                  <input className="mono" defaultValue="AT" />
                </Field>
                <button className="btn" onClick={() => setTask(true)}>
                  执行
                </button>
                {task && <pre className="log-console">AT{"\n"}OK</pre>}
              </>
            ) : (
              <>
                <p>测速会消耗流量，运行前请确认当前套餐。</p>
                <button className="btn primary" onClick={() => setTask(true)}>
                  开始测速
                </button>
                {task && (
                  <Stats
                    items={[
                      ["下载", "186 Mbps"],
                      ["上传", "32 Mbps"],
                      ["延迟", "28 ms"],
                    ]}
                  />
                )}
              </>
            )}
          </Modal>
        )}
      </>
    );
  if (tab === "自动化")
    return (
      <div className="two-columns">
        <ConfigForm
          id="schedule"
          title="计划任务"
          initial={{
            enabled: false,
            action: "重启设备",
            time: "04:00",
            repeat: "每周一",
          }}
        >
          {(v, set) => (
            <>
              <SwitchRow
                title="启用计划"
                value={!!v.enabled}
                onChange={() => set("enabled", !v.enabled)}
              />
              <Field label="操作">
                <Select
                  value={String(v.action)}
                  options={["重启设备", "关闭 Wi-Fi", "开启 Wi-Fi"]}
                  onChange={(s) => set("action", s)}
                />
              </Field>
              <div className="form-grid">
                <Field label="时间">
                  <input
                    type="time"
                    value={String(v.time)}
                    onChange={(e) => set("time", e.target.value)}
                  />
                </Field>
                <Field label="重复">
                  <Select
                    value={String(v.repeat)}
                    options={["每天", "每周一", "工作日"]}
                    onChange={(s) => set("repeat", s)}
                  />
                </Field>
              </div>
            </>
          )}
        </ConfigForm>
        <ConfigForm
          id="activity"
          title="iPhone 实时活动"
          initial={{ enabled: false, token: "" }}
        >
          {(v, set) => (
            <>
              <SwitchRow
                title="推送设备状态"
                hint="在已配对设备上显示连接与电量"
                value={!!v.enabled}
                onChange={() => set("enabled", !v.enabled)}
              />
              <Field label="配对标识">
                <input
                  value={String(v.token)}
                  onChange={(e) => set("token", e.target.value)}
                />
              </Field>
            </>
          )}
        </ConfigForm>
      </div>
    );
  return (
    <>
      <div className="power-overview">
        <Panel>
          <PanelTitle title="电池与供电" />
          <Battery direct={m.settings.power.mode === "直供电"} />
          <Definition
            items={[
              [
                "供电方式",
                m.settings.power.mode === "直供电"
                  ? "外部电源直供"
                  : "USB-C 充电",
              ],
              ["电池温度", "35°C"],
              ["充电上限", String(m.settings.power.limit)],
            ]}
          />
        </Panel>
        <Panel>
          <PanelTitle title="设备负载" note="过去 30 分钟" />
          <div className="mini-resource-row">
            <span>
              CPU <b>12%</b>
            </span>
            <span>
              内存 <b>44%</b>
            </span>
            <span>
              温度 <b>39°C</b>
            </span>
          </div>
          <ResourceChart />
        </Panel>
      </div>
      <Panel>
        <PerformanceControls />
      </Panel>
      <ConfigForm
        id="power"
        title="电源策略"
        note="为移动使用与长期插电选择合适的方式。"
      >
        {(v, set) => (
          <>
            <div className="power-choices">
              {[
                ["智能充电", "兼顾续航与电池寿命", BatteryCharging],
                ["直供电", "接入外部电源时绕过电池供电", Zap],
              ].map(([mode, note, Icon]) => {
                const I = Icon as typeof Zap;
                return (
                  <button
                    key={String(mode)}
                    aria-pressed={v.mode === mode}
                    onClick={() => set("mode", String(mode))}
                  >
                    <I size={22} />
                    <b>{String(mode)}</b>
                    <span>{String(note)}</span>
                    {v.mode === mode && <Check size={16} />}
                  </button>
                );
              })}
            </div>
            <div className="form-grid">
              <Field label="充电上限">
                <Select
                  value={String(v.limit)}
                  options={["80%", "90%", "100%"]}
                  onChange={(s) => set("limit", s)}
                />
              </Field>
              <Field label="自动熄屏">
                <Select
                  value={String(v.sleep)}
                  options={["30 秒", "1 分钟", "3 分钟"]}
                  onChange={(s) => set("sleep", s)}
                />
              </Field>
            </div>
            <details className="advanced-details">
              <summary>启动与 USB 供电</summary>
              <SwitchRow
                title="快速启动"
                value={!!v.fast}
                onChange={() => set("fast", !v.fast)}
              />
              <Field label="USB 模式">
                <Select
                  value={String(v.usb || "数据与充电")}
                  options={["数据与充电", "仅充电", "反向供电"]}
                  onChange={(s) => set("usb", s)}
                />
              </Field>
            </details>
          </>
        )}
      </ConfigForm>
    </>
  );
}
