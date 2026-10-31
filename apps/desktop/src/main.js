import { mount } from "svelte";
import "./styles.css";
import App from "./App.svelte";
import StandaloneDownloadWindow from "./lib/StandaloneDownloadWindow.svelte";
import StandaloneCompleteWindow from "./lib/StandaloneCompleteWindow.svelte";
import { recordFirstPaint } from "./lib/perf.js";

const urlParams = new URLSearchParams(window.location.search);
const taskId = urlParams.get("task");
const completeId = urlParams.get("complete");
const target = document.getElementById("app");

const app = completeId
  ? mount(StandaloneCompleteWindow, { target, props: { taskId: completeId } })
  : taskId
    ? mount(StandaloneDownloadWindow, { target, props: { taskId } })
    : mount(App, { target });

if (typeof requestAnimationFrame === "function") {
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      const now = typeof performance !== "undefined" ? performance.now() : Date.now();
      recordFirstPaint(now);
    });
  });
}

export default app;
