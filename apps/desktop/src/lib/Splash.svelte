<script>
  import { onMount, onDestroy } from "svelte";
  import { t } from "./i18n.js";
  import logoWhite from "../assets/logo-white.svg";

  export let ready = false;

  let fading = false;
  let hidden = false;
  let timer = null;
  let fadeTimer = null;

  function dismiss() {
    if (fading || hidden) return;
    fading = true;
    if (timer) clearTimeout(timer);
    // ≤200ms fade
    fadeTimer = setTimeout(() => {
      hidden = true;
    }, 200);
  }

  $: if (ready && !fading && !hidden) {
    dismiss();
  }

  onMount(() => {
    // Hard maximum of 1.5s
    timer = setTimeout(() => {
      dismiss();
    }, 1500);
  });

  onDestroy(() => {
    if (timer) clearTimeout(timer);
    if (fadeTimer) clearTimeout(fadeTimer);
  });
</script>

{#if !hidden}
  <div
    class="splash-screen"
    class:fading
    role="status"
    aria-live="polite"
    aria-label={$t("splash.tagline")}
  >
    <div class="splash-content">
      <div class="logo-shimmer-wrap" aria-hidden="true">
        <img src={logoWhite} alt="SAYL" class="splash-logo-base" />
        <div
          class="logo-shimmer-overlay"
          style="-webkit-mask-image: url('{logoWhite}'); mask-image: url('{logoWhite}');"
        ></div>
      </div>
      <p class="splash-tagline">{$t("splash.tagline")}</p>
    </div>
  </div>
{/if}

<style>
  .splash-screen {
    position: fixed;
    inset: 0;
    z-index: 99999;
    background-color: #0b1a4e;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 1;
    pointer-events: auto;
    transition: opacity 200ms cubic-bezier(0.16, 1, 0.3, 1);
    user-select: none;
  }

  .splash-screen.fading {
    opacity: 0;
    pointer-events: none;
  }

  .splash-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1.25rem;
  }

  .logo-shimmer-wrap {
    position: relative;
    width: 180px;
    height: 225px;
    display: flex;
    align-items: center;
    justify-content: center;
    animation: splash-wave 3s ease-in-out infinite alternate;
  }

  .splash-logo-base {
    width: 100%;
    height: 100%;
    object-fit: contain;
    filter: drop-shadow(0 8px 24px rgba(1, 178, 187, 0.35));
  }

  .logo-shimmer-overlay {
    position: absolute;
    inset: 0;
    -webkit-mask-size: contain;
    -webkit-mask-repeat: no-repeat;
    -webkit-mask-position: center;
    mask-size: contain;
    mask-repeat: no-repeat;
    mask-position: center;
    background: linear-gradient(
      180deg,
      rgba(6, 170, 185, 0.1) 0%,
      rgba(1, 178, 187, 0.7) 60%,
      rgba(1, 210, 220, 0.95) 100%
    );
    background-size: 100% 200%;
    animation: teal-fill 1.4s ease-out forwards;
    mix-blend-mode: screen;
    pointer-events: none;
  }

  .splash-tagline {
    color: #9cb2db;
    font-size: 0.95rem;
    font-weight: 500;
    margin: 0;
    letter-spacing: 0.02em;
    text-align: center;
  }

  @keyframes teal-fill {
    0% {
      background-position: 0 160%;
      opacity: 0.1;
    }
    60% {
      opacity: 0.8;
    }
    100% {
      background-position: 0 0%;
      opacity: 1;
    }
  }

  @keyframes splash-wave {
    0% {
      transform: translateY(0px) scale(1);
    }
    50% {
      transform: translateY(-5px) scale(1.015);
    }
    100% {
      transform: translateY(0px) scale(1);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .splash-screen {
      transition: opacity 100ms linear;
    }
    .logo-shimmer-wrap {
      animation: none !important;
      transform: none !important;
    }
    .logo-shimmer-overlay {
      animation: none !important;
      background-position: 0 0% !important;
      opacity: 0.7 !important;
    }
  }
</style>
