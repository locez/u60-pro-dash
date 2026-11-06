import { useEffect, useRef, useState } from "react";
import { api, bearer, sessionStore } from "./api";
import { useMutation } from "./mutations";
import { ErrorState, Note, Panel, PanelTitle, Stats } from "./ui";
import { t } from "./i18n";

export function LanTest() {
  const mutation = useMutation();
  const [phase, setPhase] = useState("Idle"),
    [download, setDownload] = useState<string>(),
    [upload, setUpload] = useState<string>(),
    [error, setError] = useState<Error>();
  const controller = useRef<AbortController | undefined>(undefined),
    mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    const pause = () => {
      if (document.hidden) controller.current?.abort();
    };
    const unsubscribe = sessionStore.subscribe(() =>
      controller.current?.abort(),
    );
    document.addEventListener("visibilitychange", pause);
    return () => {
      mounted.current = false;
      controller.current?.abort();
      unsubscribe();
      document.removeEventListener("visibilitychange", pause);
    };
  }, []);
  const start = async () => {
    const own = new AbortController(),
      epoch = sessionStore.get().epoch;
    controller.current = own;
    const deadline = setTimeout(() => own.abort(), 60000);
    const present = () =>
      mounted.current &&
      !own.signal.aborted &&
      sessionStore.get().epoch === epoch;
    const size = 10 * 1024 * 1024;
    try {
      await mutation.run(async () => {
        setError(undefined);
        setDownload(undefined);
        setUpload(undefined);
        setPhase("Download");
        let started = performance.now(),
          received = 0;
        const response = await fetch(`/api/lan/download?size=${size}`, {
          headers: bearer(),
          credentials: "omit",
          cache: "no-store",
          signal: own.signal,
        });
        if (!response.ok || !response.body)
          throw new Error("Local throughput test failed");
        const reader = response.body.getReader();
        for (;;) {
          const chunk = await reader.read();
          if (chunk.done) break;
          received += chunk.value.byteLength;
          if (!present()) {
            await reader.cancel();
            return;
          }
        }
        if (received !== size) throw new Error("The transfer was incomplete");
        if (!present()) return;
        setDownload(
          ((received * 8) / (performance.now() - started) / 1000).toFixed(1) +
            " Mbps",
        );
        setPhase("Upload");
        const payload = new Uint8Array(size);
        started = performance.now();
        const result = await api("/api/lan/upload", {
          method: "POST",
          raw: payload.buffer,
          contentType: "application/octet-stream",
          signal: own.signal,
          timeout: 60000,
        });
        if (!present()) return;
        if (Number(result.bytes) !== size)
          throw new Error("The transfer was incomplete");
        setUpload(
          ((size * 8) / (performance.now() - started) / 1000).toFixed(1) +
            " Mbps",
        );
        setPhase("Completed");
      });
    } catch (e) {
      if (mounted.current) {
        setPhase(own.signal.aborted ? "Cancelled" : "Failed");
        if (!own.signal.aborted) setError(e as Error);
      }
    } finally {
      clearTimeout(deadline);
      if (controller.current === own) controller.current = undefined;
      if (own.signal.aborted && mounted.current) setPhase("Cancelled");
    }
  };
  return (
    <Panel>
      <PanelTitle title="Local throughput test" />
      <Note>
        {t(
          "Measures this browser’s connection to the router using 10 MiB in each direction. Connect locally to avoid testing or consuming a remote connection.",
        )}
      </Note>
      <ErrorState error={error} />
      <Stats
        items={[
          ["Download", download],
          ["Upload", upload],
          ["Phase", t(phase)],
        ]}
      />
      <div className="action-row">
        <button
          className="btn primary"
          disabled={mutation.busy}
          onClick={() => void start()}
        >
          {t("Start test")}
        </button>
        <button
          className="btn"
          disabled={!controller.current}
          onClick={() => controller.current?.abort()}
        >
          {t("Stop")}
        </button>
      </div>
    </Panel>
  );
}
