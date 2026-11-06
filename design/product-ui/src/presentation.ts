// Publication captures use neutral sample names. Interactive previews show
// the actual installed service names.
export const publicPresentation =
  new URLSearchParams(location.search).get("presentation") === "public";
export const presentationQuery: Record<string, string> = publicPresentation
  ? { presentation: "public" }
  : {};
export const serviceNames = [
  "Tailscale",
  "WireGuard",
  publicPresentation ? "规则代理" : "Mihomo",
];
