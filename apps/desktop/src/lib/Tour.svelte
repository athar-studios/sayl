<script>
  import { onMount, onDestroy, createEventDispatcher, tick } from "svelte";
  import { t, currentLang } from "./i18n.js";
  import {
    TOUR_STEPS,
    getNextStepIndex,
    getPrevStepIndex,
    getRtlKeyAction,
    computeCutoutBounds,
    setTourCompleted,
  } from "./tour.js";

  export let active = false;

  const dispatch = createEventDispatcher();

  let currentStepIndex = 0;
  let cutout = { top: 0, left: 0, width: 0, height: 0, isFallback: true };
  let cardStyle = "";
  let tourCardEl = null;
  let previousActiveEl = null;
  let windowWidth = 1024;
  let windowHeight = 768;

  $: isRtl = $currentLang === "ar";
  $: currentStep = TOUR_STEPS[currentStepIndex] || TOUR_STEPS[0];
  $: isLastStep = currentStepIndex === TOUR_STEPS.length - 1;

  $: if (active) {
    onTourActivated();
  } else {
    onTourDeactivated();
  }

  $: if (active && currentStep) {
    tick().then(() => updateGeometry());
  }

  function onTourActivated() {
    currentStepIndex = 0;
    previousActiveEl = document.activeElement;
    if (typeof window !== "undefined") {
      windowWidth = window.innerWidth;
      windowHeight = window.innerHeight;
      window.addEventListener("resize", handleWindowResize);
      window.addEventListener("scroll", handleScroll, true);
    }
    tick().then(() => {
      updateGeometry();
      focusCard();
    });
  }

  function onTourDeactivated() {
    if (typeof window !== "undefined") {
      window.removeEventListener("resize", handleWindowResize);
      window.removeEventListener("scroll", handleScroll, true);
    }
    if (previousActiveEl && typeof previousActiveEl.focus === "function") {
      try {
        previousActiveEl.focus();
      } catch {}
    }
  }

  function handleWindowResize() {
    if (typeof window !== "undefined") {
      windowWidth = window.innerWidth;
      windowHeight = window.innerHeight;
      updateGeometry();
    }
  }

  function handleScroll() {
    updateGeometry();
  }

  function findStepElement(step) {
    if (typeof document === "undefined") return null;
    let el = null;
    if (step.selector) {
      try {
        el = document.querySelector(step.selector);
      } catch {}
    }
    if (!el && step.fallbackSelector) {
      try {
        el = document.querySelector(step.fallbackSelector);
      } catch {}
    }
    return el;
  }

  function updateGeometry() {
    if (!active || !currentStep) return;

    cutout = computeCutoutBounds(
      currentStep.selector,
      () => findStepElement(currentStep),
      { width: windowWidth, height: windowHeight }
    );

    if (cutout.isFallback) {
      cardStyle = `top: 50%; left: 50%; transform: translate(-50%, -50%);`;
      return;
    }

    const cardWidth = 380;
    const cardHeight = 220;
    let top = 0;
    let left = cutout.left;

    // Prefer placing below the cutout if there is room
    if (cutout.top + cutout.height + cardHeight + 16 <= windowHeight) {
      top = cutout.top + cutout.height + 12;
    } else if (cutout.top - cardHeight - 16 >= 0) {
      // Place above cutout
      top = Math.max(16, cutout.top - cardHeight - 12);
    } else {
      // Centered vertically if crowded
      top = Math.max(16, Math.min(windowHeight - cardHeight - 16, cutout.top));
    }

    // Horizontal clamping within viewport
    const maxLeft = Math.max(16, windowWidth - cardWidth - 16);
    left = Math.max(16, Math.min(maxLeft, left));

    cardStyle = `top: ${Math.round(top)}px; left: ${Math.round(left)}px;`;
  }

  function focusCard() {
    if (tourCardEl) {
      const btn = tourCardEl.querySelector("button.btn-primary, button");
      if (btn) btn.focus();
    }
  }

  function handleNext() {
    if (isLastStep) {
      handleFinish();
      return;
    }
    currentStepIndex = getNextStepIndex(currentStepIndex, TOUR_STEPS.length);
    tick().then(() => {
      updateGeometry();
      focusCard();
    });
  }

  function handlePrev() {
    currentStepIndex = getPrevStepIndex(currentStepIndex);
    tick().then(() => {
      updateGeometry();
      focusCard();
    });
  }

  function handleSkip() {
    setTourCompleted();
    active = false;
    dispatch("skip");
  }

  function handleFinish() {
    setTourCompleted();
    active = false;
    dispatch("finish");
  }

  function handleOpenGuide() {
    dispatch("openExtensionGuide");
  }

  function handleKeydown(e) {
    if (!active) return;

    const action = getRtlKeyAction(e.key, isRtl);
    if (action === "skip") {
      e.preventDefault();
      e.stopPropagation();
      handleSkip();
      return;
    }
    if (action === "next") {
      e.preventDefault();
      e.stopPropagation();
      handleNext();
      return;
    }
    if (action === "prev") {
      e.preventDefault();
      e.stopPropagation();
      handlePrev();
      return;
    }

    // Accessible Focus Trap for Tab key
    if (e.key === "Tab" && tourCardEl) {
      const focusables = tourCardEl.querySelectorAll(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );
      if (focusables.length === 0) return;
      const first = focusables[0];
      const last = focusables[focusables.length - 1];

      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }

  onDestroy(() => {
    onTourDeactivated();
  });
</script>

<svelte:window on:keydown={handleKeydown} />

{#if active}
  <div class="tour-backdrop" role="presentation">
    <!-- SVG Cutout Masking Overlay -->
    <svg class="tour-mask-svg" width="100%" height="100%" aria-hidden="true">
      <defs>
        <mask id="tour-cutout-mask">
          <!-- White fills visible dim backdrop -->
          <rect width="100%" height="100%" fill="white" />
          <!-- Black cuts out highlighted area -->
          {#if !cutout.isFallback}
            <rect
              x={cutout.left}
              y={cutout.top}
              width={cutout.width}
              height={cutout.height}
              rx="8"
              fill="black"
            />
          {/if}
        </mask>
      </defs>
      <!-- Dimmed backdrop with mask hole -->
      <rect
        width="100%"
        height="100%"
        fill="rgba(6, 14, 40, 0.72)"
        mask="url(#tour-cutout-mask)"
      />
      <!-- Glowing highlight frame around target cutout -->
      {#if !cutout.isFallback}
        <rect
          x={cutout.left}
          y={cutout.top}
          width={cutout.width}
          height={cutout.height}
          rx="8"
          fill="none"
          stroke="#00e5ff"
          stroke-width="2"
          stroke-dasharray="6 3"
          class="cutout-ring"
        />
      {/if}
    </svg>

    <!-- Tour Step Card Popover -->
    <div
      bind:this={tourCardEl}
      class="tour-step-card"
      style={cardStyle}
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="tour-step-title"
    >
      <div class="tour-card-header">
        <span class="tour-step-counter">
          {$t("tour.stepCounter")
            .replace("{current}", currentStepIndex + 1)
            .replace("{total}", TOUR_STEPS.length)}
        </span>
        <button class="tour-skip-link" on:click={handleSkip}>
          {$t("tour.skip")}
        </button>
      </div>

      <h4 id="tour-step-title" class="tour-step-title">
        {$t(currentStep.titleKey)}
      </h4>

      <p class="tour-step-desc">
        {$t(currentStep.descKey)}
      </p>

      <!-- Step 6 Special Guide Trigger Button -->
      {#if currentStep.hasGuideAction}
        <div class="guide-action-box">
          <button type="button" class="btn btn-guide" on:click={handleOpenGuide}>
            <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
              <path
                fill-rule="evenodd"
                d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-11a1 1 0 10-2 0v2H7a1 1 0 100 2h2v2a1 1 0 102 0v-2h2a1 1 0 100-2h-2V7z"
                clip-rule="evenodd"
              />
            </svg>
            <span>{$t("tour.openGuide")}</span>
          </button>
        </div>
      {/if}

      <!-- Step Navigation Buttons -->
      <div class="tour-card-footer">
        <div class="step-dots" aria-hidden="true">
          {#each TOUR_STEPS as _, i}
            <span class="dot" class:active={i === currentStepIndex}></span>
          {/each}
        </div>

        <div class="nav-buttons">
          {#if currentStepIndex > 0}
            <button type="button" class="btn btn-secondary" on:click={handlePrev}>
              {$t("tour.prev")}
            </button>
          {/if}

          {#if isLastStep}
            <button type="button" class="btn btn-primary" on:click={handleFinish}>
              {$t("tour.finish")}
            </button>
          {:else}
            <button type="button" class="btn btn-primary" on:click={handleNext}>
              {$t("tour.next")}
            </button>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .tour-backdrop {
    position: fixed;
    inset: 0;
    z-index: 9990;
    pointer-events: auto;
  }

  .tour-mask-svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: auto;
  }

  .cutout-ring {
    animation: dash-glow 2.5s linear infinite;
  }

  @keyframes dash-glow {
    to {
      stroke-dashoffset: -36;
    }
  }

  .tour-step-card {
    position: fixed;
    width: 380px;
    max-width: calc(100vw - 32px);
    background: #0d1a42;
    border: 1px solid #253d7a;
    border-radius: 12px;
    box-shadow: 0 16px 36px rgba(0, 0, 0, 0.65), 0 0 20px rgba(1, 178, 187, 0.15);
    padding: 1.25rem 1.5rem;
    z-index: 9995;
    color: #e2e8f0;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    animation: card-appear 180ms ease-out;
  }

  @keyframes card-appear {
    from {
      opacity: 0;
      transform: scale(0.97);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  .tour-card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .tour-step-counter {
    font-size: 0.75rem;
    font-weight: 700;
    color: #00e5ff;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .tour-skip-link {
    background: transparent;
    border: none;
    color: #8fa0c7;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
    padding: 0.2rem 0.4rem;
    border-radius: 4px;
    transition: color 150ms ease;
  }

  .tour-skip-link:hover {
    color: #cad8f3;
  }

  .tour-step-title {
    font-size: 1.1rem;
    font-weight: 700;
    color: #ffffff;
    margin: 0;
  }

  .tour-step-desc {
    font-size: 0.9rem;
    line-height: 1.5;
    color: #cad8f3;
    margin: 0;
  }

  .guide-action-box {
    margin-top: 0.25rem;
  }

  .btn-guide {
    background: #152755;
    border: 1px solid #01b2bb;
    color: #00e5ff;
    width: 100%;
    padding: 0.55rem;
    font-size: 0.88rem;
    font-weight: 600;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    border-radius: 6px;
    cursor: pointer;
    transition: all 150ms ease;
  }

  .btn-guide:hover {
    background: #01b2bb;
    color: #ffffff;
  }

  .tour-card-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-top: 0.75rem;
    margin-top: 0.25rem;
    border-top: 1px solid #1c2e5c;
  }

  .step-dots {
    display: flex;
    gap: 0.4rem;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #253969;
    transition: all 150ms ease;
  }

  .dot.active {
    width: 16px;
    border-radius: 4px;
    background: #00e5ff;
  }

  .nav-buttons {
    display: flex;
    gap: 0.5rem;
  }

  .btn {
    padding: 0.45rem 0.95rem;
    border-radius: 6px;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    border: none;
    transition: background 150ms ease;
  }

  .btn-primary {
    background: #01b2bb;
    color: #ffffff;
  }

  .btn-primary:hover {
    background: #0299a1;
  }

  .btn-secondary {
    background: #172750;
    color: #cbd5e1;
    border: 1px solid #283c70;
  }

  .btn-secondary:hover {
    background: #21356a;
    color: #ffffff;
  }

  @media (prefers-reduced-motion: reduce) {
    .tour-step-card {
      animation: none;
    }
    .cutout-ring {
      animation: none;
    }
  }
</style>
