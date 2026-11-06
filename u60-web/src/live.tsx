import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import {
  api,
  bearer,
  EventParser,
  sessionStore,
  setSession,
  type ObjectData,
} from "./api";
export type Reading = {
  data: ObjectData | null;
  error: string | null;
  age_ms: number;
  seq: number;
  received: number;
};
type Live = {
  state: "connecting" | "live" | "offline" | "paused";
  readings: Record<string, Reading>;
  history: ObjectData[];
};
const Context = createContext<Live>({
  state: "connecting",
  readings: {},
  history: [],
});
export function useSession() {
  return useSyncExternalStore(sessionStore.subscribe, sessionStore.get);
}
export function LiveProvider({
  topics,
  children,
}: {
  topics: string[];
  children: ReactNode;
}) {
  const { token, epoch } = useSession();
  const [live, setLive] = useState<Live>({
    state: "connecting",
    readings: {},
    history: [],
  });
  const topicKey = [...new Set(topics)].sort().join(",");
  const instance = useRef<unknown>(undefined);
  useEffect(() => {
    setLive((s) => ({
      ...s,
      readings: Object.fromEntries(
        Object.entries(s.readings).filter(([k]) =>
          topicKey.split(",").includes(k),
        ),
      ),
      history: topicKey.split(",").includes("traffic") ? s.history : [],
    }));
    let disposed = false,
      controller: AbortController | undefined,
      retry: ReturnType<typeof setTimeout> | undefined,
      delay = 1000;
    const disconnect = () => {
      controller?.abort();
      clearTimeout(retry);
    };
    const connect = async () => {
      disconnect();
      if (disposed || !token || document.hidden || !topicKey) {
        setLive((s) => ({ ...s, state: "paused" }));
        return;
      }
      controller = new AbortController();
      const signal = controller.signal;
      setLive((s) => ({ ...s, state: "connecting" }));
      const ownController = controller;
      let watchdog: ReturnType<typeof setTimeout> | undefined;
      const touch = () => {
        clearTimeout(watchdog);
        watchdog = setTimeout(() => ownController.abort(), 45000);
      };
      try {
        touch();
        const response = await fetch(
          "/api/events?topics=" + encodeURIComponent(topicKey),
          { headers: bearer(), signal, cache: "no-store", credentials: "omit" },
        );
        if (disposed || signal.aborted || sessionStore.get().epoch !== epoch)
          return;
        if (response.status === 401) {
          setSession("");
          return;
        }
        if (
          !response.ok ||
          !response.body ||
          !response.headers.get("Content-Type")?.includes("text/event-stream")
        )
          throw new Error("Live data unavailable");
        const reader = response.body.getReader(),
          decoder = new TextDecoder(),
          parser = new EventParser();
        for (;;) {
          const part = await reader.read();
          if (part.done) break;
          touch();
          for (const frame of parser.push(
            decoder.decode(part.value, { stream: true }),
          )) {
            if (
              disposed ||
              signal.aborted ||
              sessionStore.get().epoch !== epoch
            )
              return;
            if (frame.event === "bye") {
              if (
                (frame.data as ObjectData).reason === "token_expired" ||
                (frame.data as ObjectData).reason === "logout"
              )
                setSession("");
              await reader.cancel();
              throw new Error("Stream ended");
            }
            if (frame.event === "hello") {
              delay = 1000;
              const id = (frame.data as ObjectData).agent_instance;
              const changed =
                instance.current !== undefined && instance.current !== id;
              instance.current = id;
              setLive((s) => ({
                ...s,
                state: "live",
                ...(changed ? { readings: {}, history: [] } : {}),
              }));
              continue;
            }
            if (!topicKey.split(",").includes(frame.event)) continue;
            const value = frame.data as Reading;
            setLive((s) =>
              s.readings[frame.event]?.seq === value.seq &&
              s.readings[frame.event]?.error === value.error
                ? s
                : {
                    ...s,
                    state: "live",
                    readings: {
                      ...s.readings,
                      [frame.event]: { ...value, received: Date.now() },
                    },
                    history:
                      frame.event === "traffic"
                        ? [
                            ...s.history.slice(-59),
                            {
                              ...(value.error ? {} : value.data),
                              time: Date.now(),
                            },
                          ]
                        : s.history,
                  },
            );
          }
        }
      } catch {
        /* Reconnect only reads; never repeat a mutation. */
      } finally {
        clearTimeout(watchdog);
      }
      if (
        !disposed &&
        !document.hidden &&
        token &&
        controller === ownController
      ) {
        setLive((s) => ({ ...s, state: "offline" }));
        retry = setTimeout(connect, delay);
        delay = Math.min(delay * 2, 15000);
      }
    };
    const visibility = () => {
      if (document.hidden) {
        disconnect();
        setLive((s) => ({ ...s, state: "paused" }));
      } else void connect();
    };
    void connect();
    document.addEventListener("visibilitychange", visibility);
    return () => {
      disposed = true;
      disconnect();
      document.removeEventListener("visibilitychange", visibility);
    };
  }, [token, epoch, topicKey]);
  return <Context.Provider value={live}>{children}</Context.Provider>;
}
export function useLive() {
  return useContext(Context);
}
export function useReading(topic: string) {
  return useLive().readings[topic];
}
export function useResource<T = ObjectData>(
  path: string | null,
  options: { method?: string; body?: unknown; timeout?: number } = {},
) {
  const { epoch, token } = useSession();
  const [revision, reload] = useState(0);
  const [state, setState] = useState<{
    data?: T;
    error?: Error;
    loading: boolean;
  }>({ loading: true });
  const signature = JSON.stringify(options);
  const stable = useMemo(() => JSON.parse(signature), [signature]);
  useEffect(() => {
    const controller = new AbortController();
    if (!path || !token) {
      setState({ loading: false });
      return;
    }
    setState((s) => ({ ...s, error: undefined, loading: true }));
    api<T>(path, { ...stable, signal: controller.signal }).then(
      (data) => {
        if (!controller.signal.aborted) setState({ data, loading: false });
      },
      (error) => {
        if (!controller.signal.aborted)
          setState((s) => ({ ...s, error, loading: false }));
      },
    );
    return () => controller.abort();
  }, [path, stable, revision, epoch, token]);
  return { ...state, reload: () => reload((n) => n + 1) };
}
export function useJobPolling(
  resource: { loading: boolean; reload: () => void },
  busy: boolean,
  delay = 2000,
) {
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const schedule = () => {
      clearTimeout(timer);
      if (busy && !resource.loading && !document.hidden)
        timer = setTimeout(resource.reload, delay);
    };
    schedule();
    document.addEventListener("visibilitychange", schedule);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("visibilitychange", schedule);
    };
  }, [busy, resource.loading, delay]);
}
