import { useState, useEffect, useCallback, useRef } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdateState =
  | { status: "idle" }
  | { status: "available"; version: string; body?: string }
  | { status: "downloading"; progress: number; total: number }
  | { status: "ready" };

const CHECK_INTERVAL = 4 * 60 * 60 * 1000; // 4 hours
const STARTUP_DELAY = 5000; // 5 seconds

interface UseUpdateCheckerOptions {
  autoUpdate?: boolean;
}

export function useUpdateChecker(options: UseUpdateCheckerOptions = {}) {
  const { autoUpdate = false } = options;
  const [state, setState] = useState<UpdateState>({ status: "idle" });
  const [dismissed, setDismissed] = useState(false);
  const updateRef = useRef<Update | null>(null);
  const autoUpdateRef = useRef(autoUpdate);

  // Keep ref in sync so the callback always sees the latest value
  useEffect(() => {
    autoUpdateRef.current = autoUpdate;
  }, [autoUpdate]);

  const doInstallAndRestart = useCallback(async (update: Update) => {
    let downloaded = 0;
    let contentLength = 0;

    await update.downloadAndInstall((event) => {
      if (event.event === "Started") {
        contentLength = event.data.contentLength ?? 0;
        setState({ status: "downloading", progress: 0, total: contentLength });
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        setState({
          status: "downloading",
          progress: downloaded,
          total: contentLength,
        });
      } else if (event.event === "Finished") {
        setState({ status: "ready" });
      }
    });

    setState({ status: "ready" });
    await relaunch();
  }, []);

  const checkForUpdate = useCallback(async () => {
    try {
      const update = await check();
      if (update) {
        updateRef.current = update;

        if (autoUpdateRef.current) {
          // Auto-update: download, install, and restart without user interaction
          await doInstallAndRestart(update);
          return;
        }

        setState({
          status: "available",
          version: update.version,
          body: update.body ?? undefined,
        });
      }
    } catch {
      // Silently ignore update check failures
    }
  }, [doInstallAndRestart]);

  const installUpdate = useCallback(async () => {
    const update = updateRef.current;
    if (!update) return;

    let downloaded = 0;
    let contentLength = 0;

    await update.downloadAndInstall((event) => {
      if (event.event === "Started") {
        contentLength = event.data.contentLength ?? 0;
        setState({ status: "downloading", progress: 0, total: contentLength });
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        setState({
          status: "downloading",
          progress: downloaded,
          total: contentLength,
        });
      } else if (event.event === "Finished") {
        setState({ status: "ready" });
      }
    });

    setState({ status: "ready" });
  }, []);

  const restartApp = useCallback(async () => {
    await relaunch();
  }, []);

  const dismiss = useCallback(() => {
    setDismissed(true);
  }, []);

  // Startup check + interval
  useEffect(() => {
    const startupTimer = setTimeout(checkForUpdate, STARTUP_DELAY);
    const interval = setInterval(checkForUpdate, CHECK_INTERVAL);
    return () => {
      clearTimeout(startupTimer);
      clearInterval(interval);
    };
  }, [checkForUpdate]);

  return {
    state,
    visible: state.status !== "idle" && !dismissed,
    installUpdate,
    restartApp,
    dismiss,
  };
}
