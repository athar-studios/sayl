<script>
  import { t } from "./i18n.js";
  import { createEventDispatcher } from "svelte";
  import logoFull from "../assets/logo-full.svg";
  import logoWhite from "../assets/logo-white.svg";

  export let collapsed = false;
  export let selectedCategory = "all";
  export let selectedStateFilter = "all";
  export let categoryCounts = {};
  export let statusCounts = {};

  const dispatch = createEventDispatcher();

  // Requirement 2: exact order: الكل، مضغوطة، مستندات، صوتيات، فيديو، برامج، أخرى
  const SIDEBAR_CATEGORIES = [
    { id: "all", colorClass: "cat-all" },
    { id: "compressed", colorClass: "cat-compressed" },
    { id: "documents", colorClass: "cat-documents" },
    { id: "audio", colorClass: "cat-audio" },
    { id: "video", colorClass: "cat-video" },
    { id: "programs", colorClass: "cat-programs" },
    { id: "other", colorClass: "cat-other" },
  ];

  // Requirement 2: Status section: جارية (active), مكتملة (completed), متوقفة (paused), فشلت (failed)
  const STATUS_ITEMS = [
    { id: "active", colorClass: "stat-active" },
    { id: "completed", colorClass: "stat-completed" },
    { id: "paused", colorClass: "stat-paused" },
    { id: "failed", colorClass: "stat-failed" },
  ];

  function handleSelectCategory(id) {
    dispatch("select", id);
  }

  function handleSelectState(id) {
    dispatch("selectState", id);
  }

  function toggleCollapse() {
    collapsed = !collapsed;
    dispatch("toggleCollapse", collapsed);
  }
</script>

<aside class="sidebar" class:collapsed aria-label={$t("sidebar.title")}>
  <div class="sidebar-header">
    {#if !collapsed}
      <div class="brand">
        <div class="brand-logo" aria-hidden="true">
          <img src={logoFull} alt="SAYL" class="logo-img logo-light" />
          <img src={logoWhite} alt="SAYL" class="logo-img logo-dark" />
        </div>
        <span class="brand-name">{$t("app.name")}</span>
      </div>
    {/if}

    <button
      class="collapse-btn"
      title={collapsed ? $t("sidebar.expand") : $t("sidebar.collapse")}
      aria-label={collapsed ? $t("sidebar.expand") : $t("sidebar.collapse")}
      on:click={toggleCollapse}
    >
      <svg
        viewBox="0 0 20 20"
        fill="currentColor"
        width="18"
        height="18"
        class="collapse-icon"
        class:flipped={collapsed}
        aria-hidden="true"
      >
        <path
          fill-rule="evenodd"
          d="M12.707 5.293a1 1 0 010 1.414L9.414 10l3.293 3.293a1 1 0 01-1.414 1.414l-4-4a1 1 0 010-1.414l4-4a1 1 0 011.414 0z"
          clip-rule="evenodd"
        />
      </svg>
    </button>
  </div>

  <nav class="sidebar-nav" aria-label={$t("sidebar.title")}>
    <!-- Category list -->
    <ul class="nav-list">
      {#each SIDEBAR_CATEGORIES as cat (cat.id)}
        {@const count = categoryCounts[cat.id] || 0}
        {@const isSelected = selectedCategory === cat.id}
        <li class="nav-item">
          <button
            class="nav-btn category-btn"
            class:active={isSelected}
            title="{$t('categories.' + cat.id)} ({count})"
            aria-label="{$t('categories.' + cat.id)} ({count})"
            aria-current={isSelected ? "true" : undefined}
            on:click={() => handleSelectCategory(cat.id)}
          >
            <span class="icon-wrap {cat.colorClass}" aria-hidden="true">
              {#if cat.id === 'all'}
                <!-- All: Layer Stack -->
                <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
                  <path d="M7 3a1 1 0 000 2h6a1 1 0 100-2H7zM4 7a1 1 0 011-1h10a1 1 0 110 2H5a1 1 0 01-1-1zM2 11a2 2 0 012-2h12a2 2 0 012 2v4a2 2 0 01-2 2H4a2 2 0 01-2-2v-4z" />
                </svg>
              {:else if cat.id === 'compressed'}
                <!-- Compressed: Archive / Zip -->
                <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
                  <path d="M4 3a2 2 0 100 4h12a2 2 0 100-4H4z" />
                  <path fill-rule="evenodd" d="M3 8h14v7a2 2 0 01-2 2H5a2 2 0 01-2-2V8zm5 3a1 1 0 011-1h2a1 1 0 110 2H9a1 1 0 01-1-1z" clip-rule="evenodd" />
                </svg>
              {:else if cat.id === 'documents'}
                <!-- Documents: Document -->
                <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
                  <path fill-rule="evenodd" d="M4 4a2 2 0 012-2h4.586A2 2 0 0112 2.586L15.414 6A2 2 0 0116 7.414V16a2 2 0 01-2 2H6a2 2 0 01-2-2V4zm2 6a1 1 0 011-1h6a1 1 0 110 2H7a1 1 0 01-1-1zm1 3a1 1 0 100 2h6a1 1 0 100-2H7z" clip-rule="evenodd" />
                </svg>
              {:else if cat.id === 'audio'}
                <!-- Audio: Musical Note -->
                <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
                  <path d="M18 3a1 1 0 00-1.196-.98l-10 2A1 1 0 006 5v9.114A4.369 4.369 0 005 14c-1.657 0-3 .895-3 2s1.343 2 3 2 3-.895 3-2V7.82l8-1.6v5.894A4.37 4.37 0 0015 12c-1.657 0-3 .895-3 2s1.343 2 3 2 3-.895 3-2V3z" />
                </svg>
              {:else if cat.id === 'video'}
                <!-- Video: Camera / Film -->
                <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
                  <path d="M2 6a2 2 0 012-2h6a2 2 0 012 2v8a2 2 0 01-2 2H4a2 2 0 01-2-2V6zM14.553 7.106A1 1 0 0013 8v4a1 1 0 001.553.894l3 2A1 1 0 0019 14V6a1 1 0 00-1.447-.894l-3 2z" />
                </svg>
              {:else if cat.id === 'programs'}
                <!-- Programs: Application Window -->
                <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
                  <path fill-rule="evenodd" d="M3 4a1 1 0 011-1h12a1 1 0 011 1v2a1 1 0 01-1 1H4a1 1 0 01-1-1V4zm1 4a1 1 0 00-1 1v7a1 1 0 001 1h12a1 1 0 001-1V9a1 1 0 00-1-1H4zm3 3a1 1 0 011-1h4a1 1 0 110 2H8a1 1 0 01-1-1z" clip-rule="evenodd" />
                </svg>
              {:else}
                <!-- Other: Tag -->
                <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
                  <path fill-rule="evenodd" d="M17.707 9.293a1 1 0 010 1.414l-7 7a1 1 0 01-1.414 0l-7-7A.997.997 0 012 10V5a3 3 0 013-3h5c.256 0 .512.098.707.293l7 7zM5 6a1 1 0 100-2 1 1 0 000 2z" clip-rule="evenodd" />
                </svg>
              {/if}
            </span>

            {#if !collapsed}
              <span class="nav-label">{$t("categories." + cat.id)}</span>
              <span class="nav-badge" class:has-items={count > 0}><bdi>{count}</bdi></span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>

    <!-- Status section (Req 2) -->
    <div class="sidebar-section-divider" aria-hidden="true"></div>

    {#if !collapsed}
      <div class="sidebar-section-header">
        <span class="section-title">{$t("sidebar.statusTitle")}</span>
      </div>
    {/if}

    <ul class="nav-list status-list">
      {#each STATUS_ITEMS as stat (stat.id)}
        {@const count = statusCounts[stat.id] || 0}
        {@const isSelected = selectedStateFilter === stat.id}
        <li class="nav-item">
          <button
            class="nav-btn status-btn"
            class:active={isSelected}
            title="{$t('sidebar.' + stat.id)} ({count})"
            aria-label="{$t('sidebar.' + stat.id)} ({count})"
            aria-current={isSelected ? "true" : undefined}
            on:click={() => handleSelectState(stat.id)}
          >
            <span class="icon-wrap {stat.colorClass}" aria-hidden="true">
              <span class="status-indicator-dot"></span>
            </span>

            {#if !collapsed}
              <span class="nav-label">{$t("sidebar." + stat.id)}</span>
              <span class="nav-badge" class:has-items={count > 0}><bdi>{count}</bdi></span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  </nav>
</aside>

<style>
  .sidebar {
    width: 220px;
    background-color: var(--color-bg-card);
    border-inline-end: 1px solid var(--color-border);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    transition: width var(--transition-normal);
    overflow: hidden;
    user-select: none;
  }

  .sidebar.collapsed {
    width: 60px;
  }

  .sidebar-header {
    height: 4.5rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 0.85rem;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    gap: 0.5rem;
  }

  .collapsed .sidebar-header {
    justify-content: center;
    padding: 0;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    overflow: hidden;
  }

  .brand-logo {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.85rem;
    height: 1.85rem;
    flex-shrink: 0;
  }

  .logo-img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .logo-dark {
    display: none;
  }
  .logo-light {
    display: block;
  }

  :global([data-theme="dark"]) .logo-dark {
    display: block !important;
  }
  :global([data-theme="dark"]) .logo-light {
    display: none !important;
  }

  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme="light"])) .logo-dark {
      display: block;
    }
    :global(:root:not([data-theme="light"])) .logo-light {
      display: none;
    }
  }

  .brand-name {
    font-size: 1.15rem;
    font-weight: 800;
    color: var(--color-text);
    letter-spacing: -0.02em;
    white-space: nowrap;
  }

  .collapse-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 2rem;
    height: 2rem;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    background: transparent;
    transition: background-color var(--transition-normal), color var(--transition-normal);
    flex-shrink: 0;
  }

  .collapse-btn:hover {
    background-color: var(--color-bg-alt);
    color: var(--color-text);
  }

  .collapse-icon {
    transition: transform var(--transition-normal);
  }

  :global(body[dir="rtl"]) .collapse-icon.flipped {
    transform: rotate(180deg);
  }

  :global(body:not([dir="rtl"])) .collapse-icon {
    transform: rotate(180deg);
  }

  :global(body:not([dir="rtl"])) .collapse-icon.flipped {
    transform: rotate(0deg);
  }

  .sidebar-nav {
    flex: 1;
    overflow-y: auto;
    padding: 0.75rem 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .nav-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    margin: 0;
    padding: 0;
  }

  .nav-item {
    margin: 0;
    padding: 0;
  }

  .nav-btn {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.65rem;
    padding: 0.55rem 0.75rem;
    border-radius: var(--radius-sm);
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--color-text-muted);
    background: transparent;
    transition: background-color var(--transition-normal), color var(--transition-normal);
    text-align: start;
    white-space: nowrap;
    cursor: pointer;
  }

  .collapsed .nav-btn {
    justify-content: center;
    padding: 0.55rem 0;
  }

  .nav-btn:hover {
    background-color: var(--color-bg-alt);
    color: var(--color-text);
  }

  .nav-btn.active {
    background-color: var(--color-accent-light);
    color: var(--color-accent);
  }

  .icon-wrap {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 1.25rem;
    height: 1.25rem;
  }

  /* Vibrant distinct category colours (Req 2) */
  .cat-all {
    color: #00b6bd;
  }
  .cat-compressed {
    color: #d97706;
  }
  .cat-documents {
    color: #2563eb;
  }
  .cat-audio {
    color: #e11d48;
  }
  .cat-video {
    color: #7c3aed;
  }
  .cat-programs {
    color: #059669;
  }
  .cat-other {
    color: #64748b;
  }

  :global([data-theme="dark"]) .cat-all {
    color: #22d3ee;
  }
  :global([data-theme="dark"]) .cat-compressed {
    color: #fbbf24;
  }
  :global([data-theme="dark"]) .cat-documents {
    color: #60a5fa;
  }
  :global([data-theme="dark"]) .cat-audio {
    color: #fb7185;
  }
  :global([data-theme="dark"]) .cat-video {
    color: #a78bfa;
  }
  :global([data-theme="dark"]) .cat-programs {
    color: #34d399;
  }
  :global([data-theme="dark"]) .cat-other {
    color: #94a3b8;
  }

  .nav-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .nav-badge {
    font-size: 0.725rem;
    font-weight: 700;
    padding: 0.1rem 0.45rem;
    border-radius: var(--radius-full);
    background-color: var(--color-bg-alt);
    color: var(--color-text-subtle);
    flex-shrink: 0;
  }

  .nav-btn.active .nav-badge {
    background-color: var(--color-accent);
    color: #ffffff;
  }

  /* Section divider */
  .sidebar-section-divider {
    height: 1px;
    background-color: var(--color-border);
    margin: 0.5rem 0.25rem;
  }

  .sidebar-section-header {
    padding: 0.25rem 0.75rem;
  }

  .section-title {
    font-size: 0.725rem;
    font-weight: 700;
    color: var(--color-text-subtle);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  /* Status indicator dots */
  .status-indicator-dot {
    width: 0.6rem;
    height: 0.6rem;
    border-radius: 50%;
    background-color: currentColor;
  }

  .stat-active {
    color: var(--state-running, #00b6bd);
  }
  .stat-completed {
    color: var(--state-completed, #16a34a);
  }
  .stat-paused {
    color: var(--state-paused, #d97706);
  }
  .stat-failed {
    color: var(--state-failed, #dc2626);
  }

  @media (prefers-reduced-motion: reduce) {
    .sidebar {
      transition: none !important;
    }
    .collapse-icon {
      transition: none !important;
    }
  }
</style>
