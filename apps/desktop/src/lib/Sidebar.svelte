<script>
  import { t } from "./i18n.js";
  import { CATEGORIES } from "./classify.js";
  import { createEventDispatcher } from "svelte";
  import logoFull from "../assets/logo-full.svg";
  import logoWhite from "../assets/logo-white.svg";

  export let collapsed = false;
  export let selectedCategory = "all";
  export let categoryCounts = {};

  const dispatch = createEventDispatcher();

  function handleSelect(id) {
    dispatch("select", id);
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
        <!-- In RTL: collapse points forward (inline-start / right) to collapse, expand points inward -->
        <path
          fill-rule="evenodd"
          d="M12.707 5.293a1 1 0 010 1.414L9.414 10l3.293 3.293a1 1 0 01-1.414 1.414l-4-4a1 1 0 010-1.414l4-4a1 1 0 011.414 0z"
          clip-rule="evenodd"
        />
      </svg>
    </button>
  </div>

  <nav class="sidebar-nav" role="navigation" aria-label={$t("sidebar.title")}>
    <ul class="category-list">
      {#each CATEGORIES as cat (cat.id)}
        {@const count = categoryCounts[cat.id] || 0}
        {@const isSelected = selectedCategory === cat.id}
        <li class="category-item">
          <button
            class="category-btn"
            class:active={isSelected}
            title="{$t('categories.' + cat.id)} ({count})"
            aria-label="{$t('categories.' + cat.id)} ({count})"
            aria-current={isSelected ? "true" : undefined}
            on:click={() => handleSelect(cat.id)}
          >
            <span class="cat-icon" aria-hidden="true">
              {#if cat.id === 'all'}
                <!-- All / Layer Icon -->
                <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
                  <path d="M7 3a1 1 0 000 2h6a1 1 0 100-2H7zM4 7a1 1 0 011-1h10a1 1 0 110 2H5a1 1 0 01-1-1zM2 11a2 2 0 012-2h12a2 2 0 012 2v4a2 2 0 01-2 2H4a2 2 0 01-2-2v-4z" />
                </svg>
              {:else if cat.id === 'video'}
                <!-- Video Icon -->
                <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
                  <path d="M2 6a2 2 0 012-2h6a2 2 0 012 2v8a2 2 0 01-2 2H4a2 2 0 01-2-2V6zM14.553 7.106A1 1 0 0013 8v4a1 1 0 001.553.894l3 2A1 1 0 0019 14V6a1 1 0 00-1.447-.894l-3 2z" />
                </svg>
              {:else if cat.id === 'audio'}
                <!-- Audio Icon -->
                <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
                  <path d="M18 3a1 1 0 00-1.196-.98l-10 2A1 1 0 006 5v9.114A4.369 4.369 0 005 14c-1.657 0-3 .895-3 2s1.343 2 3 2 3-.895 3-2V7.82l8-1.6v5.894A4.37 4.37 0 0015 12c-1.657 0-3 .895-3 2s1.343 2 3 2 3-.895 3-2V3z" />
                </svg>
              {:else if cat.id === 'documents'}
                <!-- Documents Icon -->
                <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
                  <path fill-rule="evenodd" d="M4 4a2 2 0 012-2h4.586A2 2 0 0112 2.586L15.414 6A2 2 0 0116 7.414V16a2 2 0 01-2 2H6a2 2 0 01-2-2V4zm2 6a1 1 0 011-1h6a1 1 0 110 2H7a1 1 0 01-1-1zm1 3a1 1 0 100 2h6a1 1 0 100-2H7z" clip-rule="evenodd" />
                </svg>
              {:else if cat.id === 'programs'}
                <!-- Programs Icon -->
                <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
                  <path fill-rule="evenodd" d="M3 4a1 1 0 011-1h12a1 1 0 011 1v2a1 1 0 01-1 1H4a1 1 0 01-1-1V4zm1 4a1 1 0 00-1 1v7a1 1 0 001 1h12a1 1 0 001-1V9a1 1 0 00-1-1H4zm3 3a1 1 0 011-1h4a1 1 0 110 2H8a1 1 0 01-1-1z" clip-rule="evenodd" />
                </svg>
              {:else if cat.id === 'compressed'}
                <!-- Compressed / Archive Icon -->
                <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
                  <path d="M4 3a2 2 0 100 4h12a2 2 0 100-4H4z" />
                  <path fill-rule="evenodd" d="M3 8h14v7a2 2 0 01-2 2H5a2 2 0 01-2-2V8zm5 3a1 1 0 011-1h2a1 1 0 110 2H9a1 1 0 01-1-1z" clip-rule="evenodd" />
                </svg>
              {:else}
                <!-- Other Icon -->
                <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
                  <path fill-rule="evenodd" d="M17.707 9.293a1 1 0 010 1.414l-7 7a1 1 0 01-1.414 0l-7-7A.997.997 0 012 10V5a3 3 0 013-3h5c.256 0 .512.098.707.293l7 7zM5 6a1 1 0 100-2 1 1 0 000 2z" clip-rule="evenodd" />
                </svg>
              {/if}
            </span>

            {#if !collapsed}
              <span class="cat-label">{$t("categories." + cat.id)}</span>
              <span class="cat-badge" class:has-items={count > 0}><bdi>{count}</bdi></span>
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
    height: 3.75rem;
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
  }

  .category-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin: 0;
    padding: 0;
  }

  .category-item {
    margin: 0;
    padding: 0;
  }

  .category-btn {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.65rem;
    padding: 0.6rem 0.75rem;
    border-radius: var(--radius-sm);
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--color-text-muted);
    background: transparent;
    transition: background-color var(--transition-normal), color var(--transition-normal);
    text-align: start;
    white-space: nowrap;
  }

  .collapsed .category-btn {
    justify-content: center;
    padding: 0.65rem 0;
  }

  .category-btn:hover {
    background-color: var(--color-bg-alt);
    color: var(--color-text);
  }

  .category-btn.active {
    background-color: var(--color-accent-light);
    color: var(--color-accent);
  }

  .cat-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .cat-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .cat-badge {
    font-size: 0.725rem;
    font-weight: 700;
    padding: 0.1rem 0.45rem;
    border-radius: var(--radius-full);
    background-color: var(--color-bg-alt);
    color: var(--color-text-subtle);
    flex-shrink: 0;
  }

  .category-btn.active .cat-badge {
    background-color: var(--color-accent);
    color: #ffffff;
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
