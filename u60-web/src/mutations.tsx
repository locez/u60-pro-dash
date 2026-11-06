import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useSession } from "./live";
import { Modal } from "./ui";
import { t } from "./i18n";
type Confirmation = {
  message: string;
  title?: string;
  danger?: boolean;
  phrase?: string;
};
const Context = createContext<{
  busy: boolean;
  confirm: (options: Confirmation) => Promise<boolean>;
  run: <T>(work: () => Promise<T>) => Promise<T>;
}>({ busy: false, confirm: async () => false, run: async (work) => work() });
export function MutationProvider({ children }: { children: ReactNode }) {
  const { epoch } = useSession();
  const [busy, setBusy] = useState(false),
    [pending, setPending] = useState<Confirmation | null>(null),
    [phrase, setPhrase] = useState("");
  const resolve = useRef<((value: boolean) => void) | undefined>(undefined),
    inFlight = useRef(false);
  const finish = (yes: boolean) => {
    resolve.current?.(yes);
    resolve.current = undefined;
    setPending(null);
    setPhrase("");
  };
  useEffect(
    () => () => {
      resolve.current?.(false);
    },
    [epoch],
  );
  return (
    <Context.Provider
      value={{
        busy,
        confirm: (options) => {
          finish(false);
          setPending(options);
          return new Promise((yes) => {
            resolve.current = yes;
          });
        },
        run: async (work) => {
          if (inFlight.current)
            throw new Error("Wait for the current operation");
          inFlight.current = true;
          setBusy(true);
          try {
            return await work();
          } finally {
            inFlight.current = false;
            setBusy(false);
          }
        },
      }}
    >
      {children}
      {pending && (
        <Modal
          title={pending.title || "This change interrupts connections"}
          close={() => finish(false)}
        >
          <p>{t(pending.message)}</p>
          {pending.phrase && (
            <label className="field">
              <span>{t("Type {0} to confirm", pending.phrase)}</span>
              <input
                value={phrase}
                onChange={(e) => setPhrase(e.target.value)}
                autoComplete="off"
              />
            </label>
          )}
          <div className="dialog-actions">
            <button className="btn" onClick={() => finish(false)}>
              {t("Cancel change")}
            </button>
            <button
              className={"btn " + (pending.danger ? "danger" : "primary")}
              disabled={!!pending.phrase && phrase !== pending.phrase}
              onClick={() => finish(true)}
            >
              {t("Continue and apply")}
            </button>
          </div>
        </Modal>
      )}
    </Context.Provider>
  );
}
export const useMutation = () => useContext(Context);
