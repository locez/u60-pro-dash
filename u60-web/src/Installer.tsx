import { useEffect, useRef, useState } from "react";
import { api, sessionStore, type ObjectData } from "./api";
import { useJobPolling, useResource } from "./live";
import { useMutation } from "./mutations";
import { Definition, ErrorState, Field, Note, Panel, PanelTitle } from "./ui";
import { t } from "./i18n";
function base64(bytes: Uint8Array) {
  let result = "";
  for (let i = 0; i < bytes.length; i += 8192)
    result += String.fromCharCode(...bytes.subarray(i, i + 8192));
  return btoa(result);
}
export async function uploadSoftware(
  service: string,
  file: File,
  version: string,
  progress: (received: number) => void,
  live: () => boolean,
) {
  const epoch = sessionStore.get().epoch;
  const endpoint = "/api/software/" + service;
  let id: string | undefined;
  let started = false;
  try {
    const init = await api(endpoint + "/upload", {
      method: "POST",
      body: { name: file.name, size: file.size, version },
    });
    id = init.upload_id;
    const size = Math.min(Number(init.chunk_size) || 262144, 262144);
    for (let offset = 0; offset < file.size; offset += size) {
      if (!live()) throw new Error("Upload cancelled");
      const data = base64(
        new Uint8Array(await file.slice(offset, offset + size).arrayBuffer()),
      );
      if (!live()) throw new Error("Upload cancelled");
      const chunk = await api(endpoint + "/upload/chunk", {
        method: "POST",
        body: { upload_id: id, offset, data },
      });
      progress(chunk.bytes_received);
    }
    if (!live()) throw new Error("Upload cancelled");
    started = true;
    return await api(endpoint + "/upload/install", {
      method: "POST",
      body: { upload_id: id },
    });
  } finally {
    if (id && !started && sessionStore.get().epoch === epoch)
      await api(endpoint + "/upload/cancel", {
        method: "POST",
        body: { upload_id: id },
      }).catch(() => {});
  }
}
export function Installer({
  service,
  installed,
  onInstalled,
}: {
  service: string;
  installed?: ObjectData;
  onInstalled?: () => void;
}) {
  const resource = useResource("/api/software/" + service + "/status"),
    mutation = useMutation();
  const [state, setState] = useState<ObjectData>(),
    [error, setError] = useState<Error>(),
    [file, setFile] = useState<File>(),
    [version, setVersion] = useState(""),
    [received, setReceived] = useState(0);
  const mounted = useRef(true),
    phase = useRef("");
  useEffect(
    () => () => {
      mounted.current = false;
    },
    [],
  );
  useEffect(() => {
    if (resource.data) {
      setState(resource.data);
      if (resource.data.phase === "succeeded" && phase.current !== "succeeded")
        onInstalled?.();
      phase.current = resource.data.phase;
    }
  }, [resource.data]);
  useJobPolling(resource, !!state?.busy);
  const release = state?.available;
  const currentVersion = String(
    installed?.client_version ||
      installed?.version ||
      installed?.pinned_version ||
      "",
  ).match(/\d+\.\d+\.\d+/)?.[0];
  const availableVersion = String(release?.version || "").replace(/^v/, "");
  const upToDate = !!currentVersion && currentVersion === availableVersion;
  const older =
    !!currentVersion &&
    /^\d+\.\d+\.\d+$/.test(availableVersion) &&
    currentVersion.localeCompare(availableVersion, undefined, {
      numeric: true,
    }) > 0;
  const busy = mutation.busy || state?.busy;
  const work = async (fn: () => Promise<ObjectData>) => {
    try {
      await mutation.run(async () => {
        setError(undefined);
        const data = await fn();
        if (mounted.current) setState(data);
      });
    } catch (error) {
      if (mounted.current) {
        setError(error as Error);
        resource.reload();
      }
    }
  };
  return (
    <Panel>
      <PanelTitle
        title="Installation & updates"
        note={
          service === "metacubexd"
            ? "Install the static dashboard independently."
            : "Automatically selects the official stable Linux ARM64 package and verifies its SHA-256."
        }
      />
      <ErrorState error={error || resource.error} retry={resource.reload} />
      <Definition
        items={[
          [
            "Installed version",
            installed?.client_version ||
              installed?.version ||
              installed?.pinned_version ||
              t(installed?.installed ? "Not reported" : "Not installed"),
          ],
          ["Latest stable version", release?.version || t("Not checked")],
          [
            "Platform",
            service === "metacubexd" ? "Static web files" : "Linux ARM64",
          ],
          ["Status", state?.error || t(state?.phase || "idle")],
        ]}
      />
      <div className="action-row">
        <button
          className="btn"
          disabled={busy}
          onClick={() =>
            void work(() =>
              api("/api/software/" + service + "/release", { timeout: 90000 }),
            )
          }
        >
          {t("Check for updates")}
        </button>
        <button
          className="btn primary"
          disabled={busy || !release || upToDate}
          onClick={async () => {
            if (
              await mutation.confirm({
                title: "Install / upgrade",
                message: t(
                  "Install version {0}? The running service may restart briefly. Existing configuration is retained.",
                  release.version,
                ),
              })
            )
              void work(() =>
                api("/api/software/" + service + "/install", {
                  method: "POST",
                  body: { version: release.version, sha256: release.sha256 },
                }),
              );
          }}
        >
          {t(
            upToDate
              ? "Up to date"
              : older
                ? "Install older release"
                : installed?.installed
                  ? "Upgrade"
                  : "Install release",
          )}
        </button>
      </div>
      {state?.busy && (
        <progress
          max={state.bytes_total || file?.size || 1}
          value={state.bytes_received || received || undefined}
        />
      )}
      <details className="advanced-details">
        <summary>{t("Package details")}</summary>
        <Definition
          items={[
            ["Filename", release?.name],
            ["SHA-256", release?.sha256 || state?.sha256],
          ]}
        />
      </details>
      <details className="advanced-details">
        <summary>{t("Manual installation")}</summary>
        <p>
          {t(
            service === "metacubexd"
              ? "Use the official compressed-dist.tgz static archive."
              : service === "tailscale"
                ? "Use the official tailscale_<version>_arm64.tgz archive."
                : "Use the Linux ARM64 .gz package or static executable.",
          )}
        </p>
        <div className="form-grid">
          <Field label="Installation package">
            <input
              type="file"
              disabled={busy}
              accept=".tgz,.gz,.bin,.tar.gz"
              onChange={(e) => setFile(e.target.files?.[0])}
            />
          </Field>
          <Field label="Package version" hint="Optional for official filenames">
            <input
              value={version}
              disabled={busy}
              onChange={(e) => setVersion(e.target.value)}
            />
          </Field>
        </div>
        <button
          className="btn"
          disabled={!file || busy}
          onClick={async () => {
            if (!file) return;
            const max = (service === "mihomo" ? 96 : 120) * 1024 * 1024;
            if (file.size === 0 || file.size > max) {
              setError(
                new Error("This package is empty or exceeds the upload limit."),
              );
              return;
            }
            if (
              !(await mutation.confirm({
                title: "Upload and install",
                message: t(
                  "Upload and install {0}? The router will validate the package before replacing the installed software.",
                  file.name,
                ),
              }))
            )
              return;
            const epoch = sessionStore.get().epoch;
            void work(() =>
              uploadSoftware(
                service,
                file,
                version,
                setReceived,
                () => mounted.current && epoch === sessionStore.get().epoch,
              ),
            );
          }}
        >
          {t("Upload and install")}
        </button>
        {received > 0 && <progress value={received} max={file?.size || 1} />}
      </details>
    </Panel>
  );
}
