import { useState } from "react";
import {
  Bell,
  ChevronDown,
  ChevronRight,
  CircleHelp,
  LockKeyhole,
  Menu,
  Router,
  Search,
  SlidersHorizontal,
  X,
} from "lucide-react";
import { navigation } from "./data";
import { Brand } from "./ui";
import { ProfessionalContent } from "./Homepages";
import {
  defaultTabs,
  featureRoute,
  features,
  pageTabs,
  serviceNames,
  useModel,
} from "./model";
import { Entry, Modal, Note, SearchBox, Tabs } from "./controls";
import { WifiPage, DevicesPage, InternetPage } from "./ConnectionPages";
import { ServicesPage } from "./ServicePages";
import { MessagesPage, LanPage, SystemPage } from "./ManagementPages";
import { ModeControls, ModeOverview } from "./ModeControls";

export default function Product() {
  const m = useModel(),
    [search, setSearch] = useState(false),
    [query, setQuery] = useState(""),
    [mobileNav, setMobileNav] = useState(false),
    [notifications, setNotifications] = useState(false);
  const [modePanel, setModePanel] = useState(false);
  const page = m.route.page,
    nav = navigation.find((n) => n.id === page)!,
    tabs = pageTabs[page];
  const tab = tabs
    ? tabs.includes(m.route.tab || "")
      ? m.route.tab!
      : defaultTabs[page]
    : m.route.tab || defaultTabs[page];
  return (
    <div className="product-shell professional complete-product">
      {mobileNav && (
        <button
          className="nav-scrim"
          aria-label="关闭导航"
          onClick={() => setMobileNav(false)}
        />
      )}
      <aside className={"product-sidebar " + (mobileNav ? "mobile-open" : "")}>
        <Brand />
        <div className="device-switch">
          <span className="device-mini">
            <Router size={18} />
          </span>
          <span>
            <strong>U60 Pro</strong>
            <small>移动路由器</small>
          </span>
          <ChevronDown size={14} />
        </div>
        <nav aria-label="产品导航">
          {navigation.map((item, i) => (
            <div key={item.id}>
              {(i === 0 || navigation[i - 1].section !== item.section) && (
                <span className="nav-section">{item.section}</span>
              )}
              <button
                className={"nav-item " + (page === item.id ? "selected" : "")}
                aria-current={page === item.id ? "page" : undefined}
                title={item.label}
                onClick={() => {
                  m.go({ page: item.id });
                  setMobileNav(false);
                }}
              >
                <item.icon size={18} />
                <span>{item.label}</span>
                {item.id === "messages" && (
                  <small className="nav-count">1</small>
                )}
              </button>
            </div>
          ))}
        </nav>
        <div className="sidebar-foot">
          <span className="account-avatar">U</span>
          <span>
            <b>本地管理员</b>
            <small>
              <LockKeyhole size={10} /> 安全会话
            </small>
          </span>
          <button
            className="icon-btn"
            aria-label="管理访问"
            onClick={() => m.go({ page: "system", tab: "屏幕与安全" })}
          >
            <CircleHelp size={17} />
          </button>
        </div>
      </aside>
      <div className="product-main">
        <header className="product-header">
          <button
            className="icon-btn mobile-menu"
            aria-label="打开导航"
            onClick={() => setMobileNav(true)}
          >
            <Menu size={21} />
          </button>
          <div className="breadcrumb">
            <span>U60 Pro</span>
            <ChevronRight size={12} />
            <b>{nav.label}</b>
            {m.route.service !== undefined && page === "services" && (
              <>
                <ChevronRight size={12} />
                <b>{serviceNames[m.route.service]}</b>
              </>
            )}
          </div>
          <div className="header-actions">
            <button
              className="icon-btn"
              aria-label="快捷控制"
              onClick={() => setModePanel(true)}
            >
              <SlidersHorizontal size={18} />
            </button>
            <span className="local-badge">
              <span />
              本地连接 <b>192.0.2.1:2026</b>
            </span>
            <button
              className="icon-btn"
              aria-label="搜索功能"
              onClick={() => setSearch(true)}
            >
              <Search size={18} />
            </button>
            <button
              className="icon-btn notification"
              aria-label="消息通知"
              onClick={() => setNotifications(true)}
            >
              <Bell size={18} />
              <i />
            </button>
          </div>
        </header>
        <main key={page}>
          {m.scenario === "offline" && (
            <Note warning>
              移动网络已断开。本地管理连接正常，可继续查看和调整设备设置。
            </Note>
          )}
          {page === "overview" ? (
            <ProfessionalContent
              offline={
                m.scenario === "offline" ||
                m.modes.airplane ||
                m.modes.busy ||
                m.modes.unknown
              }
              airplane={m.modes.airplane}
              unknown={m.modes.unknown}
              restoring={m.modes.phase === "restoring"}
              modeBar={<ModeOverview open={() => setModePanel(true)} />}
              wifi={{
                ssid: String(m.settings.wifi.ssid),
                unified: !!m.settings.wifi.unified,
                enabled: !!m.settings.wifi.enabled,
              }}
              directPower={m.settings.power.mode === "直供电"}
              variant="a"
              share={() => m.setShare(true)}
              notify={m.notify}
              enabled={m.enabled}
              toggle={m.toggle}
              navigate={(page, tab, service) => m.go({ page, tab, service })}
            />
          ) : (
            <>
              <div className="page-title product-page-heading">
                <div>
                  <span className="eyebrow">{nav.section}</span>
                  <h1>
                    {nav.label}
                    <span className="title-dot" />
                  </h1>
                  <p>{nav.description}</p>
                </div>
              </div>
              {tabs && (
                <Tabs
                  items={tabs}
                  value={tab}
                  onChange={(tab) => m.go({ ...m.route, tab })}
                />
              )}
              <div
                className="page-content"
                key={`${page}:${tab}:${m.route.service ?? ""}`}
              >
                {page === "wifi" ? (
                  <WifiPage tab={tab} />
                ) : page === "devices" ? (
                  <DevicesPage tab={tab} />
                ) : page === "internet" ? (
                  <InternetPage tab={tab} />
                ) : page === "services" ? (
                  <ServicesPage tab={tab} />
                ) : page === "messages" ? (
                  <MessagesPage tab={tab} />
                ) : page === "lan" ? (
                  <LanPage tab={tab} />
                ) : (
                  <SystemPage tab={tab} />
                )}
              </div>
            </>
          )}
        </main>
      </div>
      {modePanel && (
        <Modal title="快捷控制" close={() => setModePanel(false)}>
          <ModeControls
            openDetails={(page) => {
              setModePanel(false);
              m.go({
                page,
                tab: page === "internet" ? "连接设置" : "电源与电池",
              });
            }}
          />
        </Modal>
      )}
      {search && (
        <Modal title="查找功能" close={() => setSearch(false)} wide>
          <SearchBox
            value={query}
            onChange={setQuery}
            placeholder="搜索功能，例如 APN、Tailscale、充电"
          />
          <div className="feature-search-results">
            {features
              .filter((f) =>
                (f.name + f.tab).toLowerCase().includes(query.toLowerCase()),
              )
              .slice(0, 15)
              .map((f) => (
                <Entry
                  key={f.id}
                  title={f.name}
                  description={`${navigation.find((n) => n.id === f.web)?.label} / ${f.tab}`}
                  onClick={() => {
                    m.go(featureRoute(f));
                    setSearch(false);
                  }}
                />
              ))}
          </div>
        </Modal>
      )}
      {notifications && (
        <Modal title="通知" close={() => setNotifications(false)}>
          <Entry
            title="1 条未读短信"
            description="中国联通 · 本月流量已使用 62.8 GB"
            onClick={() => {
              setNotifications(false);
              m.go({ page: "messages" });
            }}
          />
          <Entry
            title="网络运行正常"
            description="6 台设备在线，3 项服务运行中"
            onClick={() => {
              setNotifications(false);
              m.go({ page: "overview" });
            }}
          />
        </Modal>
      )}
    </div>
  );
}

export function FeatureMap({ openWeb }: { openWeb: () => void }) {
  const m = useModel(),
    [query, setQuery] = useState(""),
    [filter, setFilter] = useState("all");
  const list = features.filter(
    (f) =>
      (filter === "all" || f.web === filter) &&
      (f.name + f.tab + f.section).toLowerCase().includes(query.toLowerCase()),
  );
  return (
    <main className="feature-map">
      <div className="map-heading">
        <span className="eyebrow">信息架构 · 评审视图</span>
        <h1>同一套功能，两种使用节奏。</h1>
        <p>
          82
          个现有功能入口全部有归属。此表核对导航迁移，页面原型展示主要布局与操作流程；不代表已接入全部接口。
        </p>
      </div>
      <div className="map-roots">
        {["状态", "连接", "管理"].map((root) => (
          <div key={root}>
            <b>屏幕 · {root}</b>
            <p>
              {[
                ...new Set(
                  features
                    .filter((f) => f.screen === root)
                    .map((f) => f.section),
                ),
              ].join(" · ")}
            </p>
          </div>
        ))}
      </div>
      <div className="table-toolbar">
        <SearchBox
          value={query}
          onChange={setQuery}
          placeholder="查找功能归属"
        />
        <select
          aria-label="按 Web 分类筛选"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        >
          <option value="all">所有分类 · 82 项</option>
          {navigation.map((n) => (
            <option key={n.id} value={n.id}>
              {n.label}
            </option>
          ))}
        </select>
        <span className="muted">{list.length} 项</span>
      </div>
      <div className="table-scroll">
        <table className="data-table">
          <thead>
            <tr>
              <th>功能</th>
              <th>Web 归属</th>
              <th>设备屏幕归属</th>
              <th>页面预览</th>
            </tr>
          </thead>
          <tbody>
            {list.map((f) => (
              <tr key={f.id}>
                <td>
                  <b>{f.name}</b>
                  <small className="feature-id">{f.id}</small>
                </td>
                <td>
                  {navigation.find((n) => n.id === f.web)?.label}{" "}
                  <ChevronRight size={11} /> {f.tab}
                </td>
                <td>
                  {f.screen} <ChevronRight size={11} /> {f.section}
                </td>
                <td>
                  <button
                    className="text-link"
                    onClick={() => {
                      m.go(featureRoute(f));
                      openWeb();
                    }}
                  >
                    打开归属页面 <ChevronRight size={13} />
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </main>
  );
}
