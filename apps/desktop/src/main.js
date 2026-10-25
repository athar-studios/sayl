import { mount } from "svelte";
import "./styles.css";
import App from "./App.svelte";
import { recordFirstPaint } from "./lib/perf.js";

const app = mount(App, {
  target: document.getElementById("app"),
});

if (typeof requestAnimationFrame === "function") {
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      const now = typeof performance !== "undefined" ? performance.now() : Date.now();
      recordFirstPaint(now);
    });
  });
}

export default app;
