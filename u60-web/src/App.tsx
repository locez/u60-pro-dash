import { useEffect, useState } from "react";
import {
  ChevronRight,
  LockKeyhole,
  LogOut,
  Menu,
  Moon,
  Router,
  Search,
  Sun,
} from "lucide-react";
import { login, logout } from "./api";
import { go, navigation, readRoute, topics, type Route } from "./catalog";
import { languages, setLocale, t, useLocale, type Locale } from "./i18n";
import { LiveProvider, useLive, useSession } from "./live";
import { MutationProvider } from "./mutations";
import { DevicesPage, Overview, WifiPage } from "./NetworkPages";
import { ServicePages } from "./ServicePages";
import { InternetPage } from "./InternetPages";
import { MessagesPage } from "./MessagesPage";
import { LanPage, SystemPage } from "./ManagementPages";
import { ErrorState, Modal, Status, Tabs } from "./ui";
function Language() {
  const locale = useLocale();
  return (
    <select
      className="language-select"
      aria-label={t("Language")}
      value={locale}
      onChange={(e) => setLocale(e.target.value as Locale)}
    >
      {Object.entries(languages).map(([id, label]) => (
        <option key={id} value={id}>
          {label}
        </option>
      ))}
    </select>
  );
}
export default function App() {
  const session = useSession();
  useLocale();
  const [dark, setDark] = useState(() => {
    try {
      return localStorage.getItem("u60.web.theme") === "dark";
    } catch {
      return false;
    }
  });
  useEffect(() => {
    document.documentElement.style.colorScheme = dark ? "dark" : "light";
    try {
      localStorage.setItem("u60.web.theme", dark ? "dark" : "light");
    } catch {}
  }, [dark]);
  return (
    <div className={"design-app production-app " + (dark ? "dark-design" : "")}>
      <div className="global-controls">
        <Language />
        <button
          className="icon-btn"
          aria-label={t(dark ? "Light mode" : "Dark mode")}
          onClick={() => setDark((v) => !v)}
        >
          {dark ? <Sun size={18} /> : <Moon size={18} />}
        </button>
      </div>
      {session.token ? (
        <MutationProvider key={session.epoch}>
          <Workspace />
        </MutationProvider>
      ) : (
        <Login />
      )}
    </div>
  );
}
function Login() {
  const [password, setPassword] = useState(""),
    [busy, setBusy] = useState(false),
    [error, setError] = useState<Error>();
  return (
    <main className="login-page">
      <section className="login-card">
        <Brand />
        <span className="eyebrow">U60 PRO</span>
        <h1>{t("Your router.\nUnder control.")}</h1>
        <p>{t("Sign in to manage this router.")}</p>
        <form
          onSubmit={async (e) => {
            e.preventDefault();
            setBusy(true);
            setError(undefined);
            try {
              await login(password);
              setPassword("");
            } catch (e) {
              setError(e as Error);
            } finally {
              setBusy(false);
            }
          }}
        >
          <label className="field">
            <span>{t("Administrator password")}</span>
            <input
              type="password"
              autoComplete="current-password"
              required
              autoFocus
              value={password}
              disabled={busy}
              onChange={(e) => setPassword(e.target.value)}
            />
          </label>
          <ErrorState error={error} />
          <button className="btn primary full-width" disabled={busy}>
            {t(busy ? "Signing in…" : "Sign in")} <ChevronRight size={16} />
          </button>
        </form>
        <small className="subtle mono">{location.host}</small>
      </section>
    </main>
  );
}
function Brand() {
  return (
    <div className="brand">
      <span className="brand-mark">
        <Router size={20} />
      </span>
      <b>
        U60<span> Dash</span>
      </b>
    </div>
  );
}
function Workspace() {
  const [route, setRoute] = useState<Route>(readRoute);
  useEffect(() => {
    const change = () => setRoute(readRoute());
    window.addEventListener("hashchange", change);
    return () => window.removeEventListener("hashchange", change);
  }, []);
  const nav = navigation.find((n) => n.id === route.page)!;
  const actual: Route = { ...route, tab: route.tab || nav.tabs[0]?.[0] };
  return (
    <LiveProvider topics={topics(actual)}>
      <Shell route={actual} />
    </LiveProvider>
  );
}
function Shell({ route }: { route: Route }) {
  const [mobile, setMobile] = useState(false),
    [search, setSearch] = useState(false),
    [query, setQuery] = useState("");
  const live = useLive(),
    nav = navigation.find((n) => n.id === route.page)!;
  useEffect(() => {
    setMobile(false);
  }, [route.page]);
  return (
    <div className="product-shell professional complete-product">
      {mobile && (
        <button
          className="nav-scrim"
          aria-label={t("Close navigation")}
          onClick={() => setMobile(false)}
        />
      )}
      <aside className={"product-sidebar " + (mobile ? "mobile-open" : "")}>
        <Brand />
        <div className="device-switch">
          <span className="device-mini">
            <Router size={18} />
          </span>
          <span>
            <strong>U60 Pro</strong>
            <small>{t("Mobile router")}</small>
          </span>
        </div>
        <nav aria-label={t("Main navigation")}>
          {navigation.map((item, i) => (
            <div key={item.id}>
              {(i === 0 || navigation[i - 1].section !== item.section) && (
                <span className="nav-section">{t(item.section)}</span>
              )}
              <button
                className={
                  "nav-item " + (item.id === route.page ? "selected" : "")
                }
                aria-current={item.id === route.page ? "page" : undefined}
                onClick={() => go(item.id)}
              >
                <item.icon size={18} />
                <span>{t(item.label)}</span>
              </button>
            </div>
          ))}
        </nav>
        <div className="sidebar-foot">
          <span className="account-avatar">
            <LockKeyhole size={17} />
          </span>
          <span>
            <b>{t("Local administrator")}</b>
            <small>{t("Authenticated session")}</small>
          </span>
          <button
            className="icon-btn"
            aria-label={t("Log out")}
            onClick={() => void logout().catch(() => {})}
          >
            <LogOut size={17} />
          </button>
        </div>
      </aside>
      <div className="product-main">
        <header className="product-header">
          <button
            className="icon-btn mobile-menu"
            aria-label={t("Open navigation")}
            onClick={() => setMobile(true)}
          >
            <Menu size={21} />
          </button>
          <div className="breadcrumb">
            <span>U60 Pro</span>
            <ChevronRight size={12} />
            <b>{t(nav.label)}</b>
          </div>
          <div className="header-actions">
            <Status on={live.state === "live"}>
              {t(
                {
                  live: "Live",
                  offline: "Reconnecting",
                  connecting: "Connecting",
                  paused: "Paused",
                }[live.state],
              )}
            </Status>
            <button
              className="icon-btn"
              aria-label={t("Search features")}
              onClick={() => setSearch(true)}
            >
              <Search size={18} />
            </button>
          </div>
        </header>
        <main>
          <div className="page-title product-page-heading">
            <div>
              <span className="eyebrow">{t(nav.section)}</span>
              <h1>
                {t(nav.title)}
                <span className="title-dot" />
              </h1>
              {route.page === "overview" && (
                <p>{t("A clear view of your connection.")}</p>
              )}
            </div>
          </div>
          {nav.tabs.length > 0 && (
            <Tabs
              items={nav.tabs.map(([id, name]) => [id, name])}
              value={route.tab!}
              onChange={(tab) => go(route.page, tab)}
            />
          )}
          <div
            className="page-content"
            key={[route.page, route.tab, route.detail].join("/")}
          >
            {route.page === "overview" ? (
              <Overview />
            ) : route.page === "wifi" ? (
              <WifiPage tab={route.tab!} />
            ) : route.page === "devices" ? (
              <DevicesPage tab={route.tab!} />
            ) : route.page === "services" ? (
              <ServicePages route={route} />
            ) : route.page === "internet" ? (
              <InternetPage route={route} />
            ) : route.page === "messages" ? (
              <MessagesPage tab={route.tab!} />
            ) : route.page === "lan" ? (
              <LanPage route={route} />
            ) : (
              <SystemPage route={route} />
            )}
          </div>
        </main>
      </div>
      {search && (
        <Modal title="Search features" close={() => setSearch(false)}>
          <input
            autoFocus
            aria-label={t("Search features")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <div className="search-results">
            {navigation
              .flatMap((n) => [
                [n.id, undefined, n.label],
                ...n.tabs.map(([id, label]) => [n.id, id, label]),
              ])
              .filter(([, , label]) =>
                t(label!).toLowerCase().includes(query.toLowerCase()),
              )
              .map(([page, tab, label]) => (
                <button
                  key={page + "/" + tab}
                  onClick={() => {
                    go(page as Route["page"], tab);
                    setSearch(false);
                  }}
                >
                  {t(label!)}
                  <ChevronRight size={16} />
                </button>
              ))}
          </div>
        </Modal>
      )}
    </div>
  );
}
