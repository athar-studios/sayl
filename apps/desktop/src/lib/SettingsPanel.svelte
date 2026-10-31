<script>
  import { t, currentLang, setLanguage } from "./i18n.js";
  import { safeInvoke } from "./tauri.js";
  import {
    DEFAULT_SETTINGS,
    normalizeSettings,
    SETTINGS_SECTIONS,
    categoryFolderRows,
    normalizeCategoryOverrides,
    pruneCategoryOverrides,
  } from "./settings.js";
  import { getCategoryLabel } from "./classify.js";
  import iconSvg from "../assets/icon.svg";
  import { BROWSER_ICONS, hasInstalledBrowsers } from "./browsers.js";
  import ExtensionGuideDialog from "./ExtensionGuideDialog.svelte";
  import { createEventDispatcher } from "svelte";

  export let open = false;
  export let settings = { ...DEFAULT_SETTINGS };
  export let initialSection = "general";

  const dispatch = createEventDispatcher();

  let activeSection = initialSection || "general";
  let localSettings = normalizeSettings(settings);
  let saving = false;
  let saveSuccess = false;
  let saveError = null;

  // Browser extension section state
  let loadingExtensionState = false;
  let browserList = [];
  let extensionGuideOpen = false;
  let guideBrowser = null;
  let stagedExtensionDir = "";
  let copiedDiagnostic = false;

  /** Per-category folder overrides being edited (empty keeps the default folder). */
  let categoryOverrides = normalizeCategoryOverrides(settings.category_folders);
  $: categoryRows = categoryFolderRows(localSettings.download_dir, categoryOverrides);

  const sections = SETTINGS_SECTIONS;

  let wasOpen = false;
  let prevInitialSection = initialSection;

  $: if (open !== wasOpen) {
    wasOpen = open;
    if (open) {
      localSettings = normalizeSettings({
        ...settings,
        language: $currentLang,
        theme: settings.theme || "system",
      });
      saveSuccess = false;
      saveError = null;
      activeSection = initialSection || "general";
      categoryOverrides = normalizeCategoryOverrides(settings.category_folders);
    }
  }

  $: if (open && initialSection !== prevInitialSection) {
    prevInitialSection = initialSection;
    if (initialSection) {
      activeSection = initialSection;
    }
  }

  $: hasBrowsers = hasInstalledBrowsers(browserList);
  $: isConnected =
    Array.isArray(browserList) && browserList.some((b) => b && b.installed && b.connected);

  async function loadExtensionStatus() {
    if (loadingExtensionState) return;
    loadingExtensionState = true;
    try {
      const [nhRes, bRes] = await Promise.allSettled([
        safeInvoke("native_host_status"),
        safeInvoke("browser_status"),
      ]);

      if (nhRes.status === "fulfilled" && nhRes.value && nhRes.value.extension_dir) {
        stagedExtensionDir = nhRes.value.extension_dir;
      }

      if (bRes.status === "fulfilled" && Array.isArray(bRes.value)) {
        browserList = bRes.value;
      } else {
        browserList = [];
      }
    } catch {
      browserList = [];
    } finally {
      loadingExtensionState = false;
    }
  }

  async function handleCopyDiagnosticReport() {
    try {
      const browsers = Array.isArray(browserList) ? browserList : [];
      const lines = [
        $t("extension.reportHeader"),
        `${$t("extension.reportDate")}: ${new Date().toLocaleString()}`,
        `${$t("extension.reportAppVersion")}: 0.1.0`,
        `${$t("extension.reportExtensionFolder")}: ${stagedExtensionDir || "—"}`,
        `${$t("extension.reportStatus")}: ${
          isConnected ? $t("extension.connectedStatus") : $t("extension.notInstalledStatus")
        }`,
        `${$t("extension.reportBrowsers")}: ${
          browsers.length
            ? browsers
                .map((b) => `${b.name || b.id} (${b.installed ? $t("extension.reportInstalled") : $t("extension.reportNotInstalled")})`)
                .join("، ")
            : $t("extension.reportNoBrowsers")
        }`,
      ];
      const text = lines.join("\n");
      if (typeof navigator !== "undefined" && navigator.clipboard) {
        await navigator.clipboard.writeText(text);
      }
      copiedDiagnostic = true;
      setTimeout(() => {
        copiedDiagnostic = false;
      }, 3000);
    } catch (e) {
      console.warn("Failed to copy support report:", e);
    }
  }

  function handleActivateBrowser(browser) {
    guideBrowser = browser;
    extensionGuideOpen = true;
  }

  async function handleBrowseFolder() {
    saveError = null;
    try {
      if (typeof window !== "undefined" && (window.__TAURI_INTERNALS__ || window.__TAURI__)) {
        const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
        const selected = await openDialog({
          directory: true,
          multiple: false,
          defaultPath: localSettings.download_dir || undefined,
        });
        if (selected && typeof selected === "string") {
          localSettings.download_dir = selected;
        }
      } else {
        await safeInvoke("open_download_folder");
      }
    } catch (err) {
      saveError = typeof err === "string" ? err : err?.message || String(err);
    }
  }

  /**
   * Picks the save folder for one category with the OS dialog.
   * @param {string} categoryId
   */
  async function handleBrowseCategoryFolder(categoryId) {
    saveError = null;
    try {
      if (typeof window !== "undefined" && (window.__TAURI_INTERNALS__ || window.__TAURI__)) {
        const row = categoryRows.find((entry) => entry.id === categoryId);
        const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
        const selected = await openDialog({
          directory: true,
          multiple: false,
          defaultPath: row?.folder || localSettings.download_dir || undefined,
        });
        if (selected && typeof selected === "string") {
          setCategoryFolder(categoryId, selected);
        }
      }
    } catch (err) {
      saveError = typeof err === "string" ? err : err?.message || String(err);
    }
  }

  /**
   * Stores the edited folder for one category.
   * @param {string} categoryId
   * @param {string} value
   */
  function setCategoryFolder(categoryId, value) {
    categoryOverrides = { ...categoryOverrides, [categoryId]: value || "" };
  }

  /** Drops every override so each category follows the download directory again. */
  function handleResetCategoryFolders() {
    categoryOverrides = normalizeCategoryOverrides({});
  }

  async function handleSave() {
    saving = true;
    saveError = null;
    saveSuccess = false;
    try {
      const payload = normalizeSettings({
        ...settings,
        ...localSettings,
      });
      if (!payload.download_dir || !payload.download_dir.trim()) {
        payload.download_dir = settings.download_dir || "C:\\Users\\User\\Downloads";
      }
      // Only folders the user actually changed are stored; everything else keeps
      // following the download directory.
      payload.category_folders = pruneCategoryOverrides(categoryOverrides, payload.download_dir);
      const updated = await safeInvoke("set_settings", { settings: payload });
      if (updated) {
        settings = { ...updated };
        localSettings = normalizeSettings(updated);
        categoryOverrides = normalizeCategoryOverrides(updated.category_folders);
        if (updated.language) {
          setLanguage(updated.language);
        }
        dispatch("changeTheme", updated.theme);
        saveSuccess = true;
        setTimeout(() => {
          saveSuccess = false;
        }, 3000);
        dispatch("update", updated);
      }
    } catch (err) {
      saveError = typeof err === "string" ? err : err?.message || String(err);
    } finally {
      saving = false;
    }
  }

  function handleReplayTour() {
    handleClose();
    dispatch("replayTour");
  }

  function handleClose() {
    dispatch("close");
  }

  function handleKeydown(e) {
    if (!open) return;
    if (e.key === "Escape" && !extensionGuideOpen) {
      e.preventDefault();
      e.stopPropagation();
      handleClose();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if open}
  <div class="modal-backdrop" on:click={handleClose} on:keydown={handleKeydown} role="presentation">
    <div
      class="modal-card settings-dialog"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="settings-title"
    >
      <!-- Header -->
      <div class="modal-header">
        <h3 id="settings-title" class="modal-title">{$t("settings.title")}</h3>
        <button
          type="button"
          class="close-btn"
          on:click|stopPropagation|preventDefault={handleClose}
          aria-label={$t("settings.close")}
        >
          <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18" aria-hidden="true">
            <path
              fill-rule="evenodd"
              d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
              clip-rule="evenodd"
            />
          </svg>
        </button>
      </div>

      <!-- Settings Layout: Side Navigation & Section Panel -->
      <div class="settings-layout">
        <!-- Sidebar Navigation -->
        <nav class="settings-nav" aria-label="Settings Sections">
          {#each sections as sec}
            <button
              type="button"
              id={"tour-nav-" + sec.id}
              class="nav-tab-btn"
              class:active={activeSection === sec.id}
              on:click={(e) => {
                e.preventDefault();
                e.stopPropagation();
                activeSection = sec.id;
              }}
            >
              <span>{$t(sec.labelKey)}</span>
            </button>
          {/each}
        </nav>

        <!-- Active Section Content Area -->
        <div class="settings-content-panel" role="region" aria-labelledby="section-heading">
          <!-- 1. GENERAL -->
          {#if activeSection === "general"}
            <div class="section-pane">
              <h4 id="section-heading" class="section-heading">{$t("settings.navGeneral")}</h4>

              <!-- Close Action -->
              <div class="setting-row">
                <div class="setting-meta">
                  <label for="close-action-select" class="setting-title">{$t("settings.closeAction")}</label>
                  <p class="setting-desc">{$t("settings.closeActionDesc")}</p>
                </div>
                <div class="setting-control">
                  <select
                    id="close-action-select"
                    class="form-control"
                    bind:value={localSettings.close_action}
                  >
                    <option value="background">{$t("settings.closeActionBackground")}</option>
                    <option value="quit">{$t("settings.closeActionQuit")}</option>
                  </select>
                </div>
              </div>

              <!-- Run at Startup -->
              <div class="setting-row">
                <div class="setting-meta">
                  <span class="setting-title">{$t("settings.runAtStartup")}</span>
                  <p class="setting-desc">{$t("settings.runAtStartupDesc")}</p>
                </div>
                <div class="setting-control">
                  <label class="switch-toggle">
                    <input
                      type="checkbox"
                      bind:checked={localSettings.run_at_startup}
                    />
                    <span class="switch-slider"></span>
                  </label>
                </div>
              </div>

              <!-- Clipboard Monitor -->
              <div class="setting-row">
                <div class="setting-meta">
                  <span class="setting-title">{$t("settings.clipboardMonitor")}</span>
                  <p class="setting-desc">{$t("settings.clipboardMonitorDesc")}</p>
                </div>
                <div class="setting-control">
                  <label class="switch-toggle">
                    <input
                      type="checkbox"
                      bind:checked={localSettings.clipboard_monitor}
                    />
                    <span class="switch-slider"></span>
                  </label>
                </div>
              </div>
            </div>

          <!-- 2. DOWNLOADS -->
          {:else if activeSection === "downloads"}
            <div class="section-pane">
              <h4 id="section-heading" class="section-heading">{$t("settings.navDownloads")}</h4>

              <!-- Download Directory -->
              <div class="setting-row stacked">
                <div class="setting-meta">
                  <label for="download-dir" class="setting-title">{$t("settings.downloadDir")}</label>
                  <p class="setting-desc">{$t("settings.downloadDirDesc")}</p>
                </div>
                <div class="setting-control full-width">
                  <div class="dir-input-row">
                    <input
                      id="download-dir"
                      type="text"
                      dir="ltr"
                      class="form-control"
                      bind:value={localSettings.download_dir}
                    />
                    <button class="btn btn-browse" type="button" on:click={handleBrowseFolder}>
                      {$t("settings.browse")}
                    </button>
                  </div>
                </div>
              </div>

              <!-- Max Concurrent Tasks -->
              <div class="setting-row">
                <div class="setting-meta">
                  <label for="max-tasks" class="setting-title">{$t("settings.maxTasks")}</label>
                  <p class="setting-desc">{$t("settings.maxTasksDesc")}</p>
                </div>
                <div class="setting-control">
                  <input
                    id="max-tasks"
                    type="number"
                    min="1"
                    max="32"
                    class="form-control number-input"
                    bind:value={localSettings.max_concurrent_tasks}
                  />
                </div>
              </div>

              <!-- Max Connections Per Task -->
              <div class="setting-row">
                <div class="setting-meta">
                  <label for="max-conns" class="setting-title">{$t("settings.maxConnections")}</label>
                  <p class="setting-desc">{$t("settings.maxConnectionsDesc")}</p>
                </div>
                <div class="setting-control">
                  <input
                    id="max-conns"
                    type="number"
                    min="1"
                    max="64"
                    class="form-control number-input"
                    bind:value={localSettings.max_connections_per_task}
                  />
                </div>
              </div>

              <!-- Duplicate Behavior -->
              <div class="setting-row">
                <div class="setting-meta">
                  <span class="setting-title">{$t("settings.duplicateBehavior")}</span>
                  <p class="setting-desc">{$t("settings.duplicateBehaviorDesc")}</p>
                </div>
                <div class="setting-control">
                  <span class="setting-info-badge">{$t("duplicate.openExisting")} / {$t("duplicate.downloadAgain")}</span>
                </div>
              </div>

              <!-- Auto Close Download Window on Finish -->
              <div class="setting-row">
                <div class="setting-meta">
                  <span class="setting-title">{$t("settings.closeDownloadWindowOnFinish")}</span>
                  <p class="setting-desc">{$t("settings.closeDownloadWindowOnFinishDesc")}</p>
                </div>
                <div class="setting-control">
                  <label class="switch-toggle">
                    <input
                      type="checkbox"
                      bind:checked={localSettings.close_download_window_on_finish}
                    />
                    <span class="switch-slider"></span>
                  </label>
                </div>
              </div>
            </div>

          <!-- 3. CATEGORIES -->
          {:else if activeSection === "categories"}
            <div class="section-pane">
              <h4 id="section-heading" class="section-heading">{$t("settings.navCategories")}</h4>
              <p class="section-intro">{$t("settings.categoriesDesc")}</p>

              <div class="category-folders">
                <div class="category-folder-head">
                  <p class="section-intro">{$t("settings.categoryFoldersDesc")}</p>
                  <button
                    type="button"
                    class="btn btn-secondary btn-sm"
                    on:click={handleResetCategoryFolders}
                  >
                    {$t("settings.categoryResetDefaults")}
                  </button>
                </div>

                {#each categoryRows as row (row.id)}
                  <div class="category-folder-row">
                    <div class="category-folder-meta">
                      <strong class="category-folder-title">
                        {getCategoryLabel(row.id, $currentLang)}
                      </strong>
                      {#if !row.isCustom}
                        <span class="category-folder-badge">
                          {$t("settings.categoryFolderDefault")}
                        </span>
                      {/if}
                    </div>
                    <div class="dir-input-row">
                      <input
                        id={`category-folder-${row.id}`}
                        class="form-control"
                        type="text"
                        dir="ltr"
                        value={row.folder}
                        aria-label={getCategoryLabel(row.id, $currentLang)}
                        on:input={(event) => setCategoryFolder(row.id, event.currentTarget.value)}
                      />
                      <button
                        type="button"
                        class="btn btn-browse"
                        on:click={() => handleBrowseCategoryFolder(row.id)}
                      >
                        {$t("settings.browse")}
                      </button>
                    </div>
                  </div>
                {/each}
              </div>
            </div>

          <!-- 4. NOTIFICATIONS & WINDOWS -->
          {:else if activeSection === "notifications"}
            <div class="section-pane">
              <h4 id="section-heading" class="section-heading">{$t("settings.navNotifications")}</h4>

              <!-- Download complete window -->
              <div class="setting-row">
                <div class="setting-meta">
                  <span class="setting-title">{$t("settings.showDownloadCompleteWindow")}</span>
                  <p class="setting-desc">{$t("settings.showDownloadCompleteWindowDesc")}</p>
                </div>
                <div class="setting-control">
                  <label class="switch-toggle">
                    <input
                      type="checkbox"
                      bind:checked={localSettings.show_download_complete_window}
                    />
                    <span class="switch-slider"></span>
                  </label>
                </div>
              </div>

              <!-- Floating Progress Window -->
              <div class="setting-row">
                <div class="setting-meta">
                  <span class="setting-title">{$t("settings.showDownloadWindow")}</span>
                  <p class="setting-desc">{$t("settings.showDownloadWindowDesc")}</p>
                </div>
                <div class="setting-control">
                  <label class="switch-toggle">
                    <input
                      type="checkbox"
                      bind:checked={localSettings.show_download_window}
                    />
                    <span class="switch-slider"></span>
                  </label>
                </div>
              </div>

              <!-- First Close Notice Explanation -->
              <div class="setting-row">
                <div class="setting-meta">
                  <span class="setting-title">{$t("settings.firstCloseNotice")}</span>
                  <p class="setting-desc">{$t("settings.firstCloseNoticeDesc")}</p>
                </div>
                <div class="setting-control">
                  <span class="setting-info-badge">
                    {localSettings.close_notice_shown ? "✓ " + $t("summary.shutdownArmed") : "—"}
                  </span>
                </div>
              </div>

              <!-- Desktop Notifications -->
              <div class="setting-row">
                <div class="setting-meta">
                  <span class="setting-title">{$t("settings.notificationsTitle")}</span>
                  <p class="setting-desc">
                    {$t("settings.notificationsDesc")}<br />
                    <span class="hint-disabled">{$t("settings.notificationsDisabled")}</span>
                  </p>
                </div>
                <div class="setting-control">
                  <label class="switch-toggle disabled">
                    <input type="checkbox" disabled />
                    <span class="switch-slider"></span>
                  </label>
                </div>
              </div>
            </div>

          <!-- 5. POWER -->
          {:else if activeSection === "power"}
            <div class="section-pane">
              <h4 id="section-heading" class="section-heading">{$t("settings.navPower")}</h4>

              <!-- Sleep Prevention -->
              <div class="setting-row">
                <div class="setting-meta">
                  <span class="setting-title">{$t("settings.preventSleep")}</span>
                  <p class="setting-desc">{$t("settings.preventSleepDesc")}</p>
                </div>
                <div class="setting-control">
                  <label class="switch-toggle">
                    <input
                      type="checkbox"
                      bind:checked={localSettings.prevent_sleep}
                    />
                    <span class="switch-slider"></span>
                  </label>
                </div>
              </div>

              <!-- Auto Shutdown info -->
              <div class="setting-row">
                <div class="setting-meta">
                  <span class="setting-title">{$t("shutdown.menuTitle")}</span>
                  <p class="setting-desc">{$t("settings.autoShutdownDesc")}</p>
                </div>
                <div class="setting-control">
                  <span class="setting-info-badge">{$t("summary.autoShutdown")}</span>
                </div>
              </div>
            </div>

          <!-- 6. APPEARANCE & LANGUAGE -->
          {:else if activeSection === "appearance"}
            <div class="section-pane">
              <h4 id="section-heading" class="section-heading">{$t("settings.navAppearance")}</h4>

              <!-- Theme -->
              <div class="setting-row">
                <div class="setting-meta">
                  <label for="theme-select" class="setting-title">{$t("settings.theme")}</label>
                  <p class="setting-desc">{$t("settings.themeDesc")}</p>
                </div>
                <div class="setting-control">
                  <select id="theme-select" class="form-control" bind:value={localSettings.theme}>
                    <option value="system">{$t("toolbar.themeSystem")}</option>
                    <option value="dark">{$t("toolbar.themeDark")}</option>
                    <option value="light">{$t("toolbar.themeLight")}</option>
                  </select>
                </div>
              </div>

              <!-- Language -->
              <div class="setting-row">
                <div class="setting-meta">
                  <label for="lang-select" class="setting-title">{$t("settings.language")}</label>
                  <p class="setting-desc">{$t("settings.languageDesc")}</p>
                </div>
                <div class="setting-control">
                  <select id="lang-select" class="form-control" bind:value={localSettings.language}>
                    <option value="ar">العربية (Arabic)</option>
                    <option value="en">English (الإنجليزية)</option>
                  </select>
                </div>
              </div>

              <!-- View Mode -->
              <div class="setting-row">
                <div class="setting-meta">
                  <label for="view-mode-select" class="setting-title">{$t("settings.viewMode")}</label>
                  <p class="setting-desc">{$t("settings.viewModeDesc")}</p>
                </div>
                <div class="setting-control">
                  <select id="view-mode-select" class="form-control" bind:value={localSettings.view_mode}>
                    <option value="table">{$t("settings.viewTable")}</option>
                    <option value="cards">{$t("settings.viewCards")}</option>
                  </select>
                </div>
              </div>
            </div>

          <!-- 7. BROWSER EXTENSION -->
          {:else if activeSection === "extension"}
            <div class="section-pane extension-section-pane" id="tour-extension-section">
              <h4 id="section-heading" class="section-heading">{$t("extension.title")}</h4>
              <p class="section-intro">{$t("extension.desc")}</p>

              <!-- Single Status Line -->
              <div class="host-status-box">
                <div class="host-status-left">
                  <span class="status-dot {isConnected ? 'success' : 'muted'}"></span>
                  <div class="host-status-text">
                    {#if isConnected}
                      <span class="status-text-bold">{$t("extension.connectedStatus")}</span>
                    {:else}
                      <span class="status-text-bold">{$t("extension.notInstalledStatus")}</span>
                    {/if}
                  </div>
                </div>

                {#if !isConnected}
                  <div class="host-actions">
                    <button
                      type="button"
                      class="btn btn-primary btn-sm"
                      on:click={() => {
                        guideBrowser = browserList.find((b) => b && b.installed) || null;
                        extensionGuideOpen = true;
                      }}
                    >
                      {$t("extension.installButton")}
                    </button>
                  </div>
                {/if}
              </div>

              <!-- Browsers List -->
              <div class="browsers-list">
                {#if hasBrowsers}
                  {#each browserList.filter((b) => b && b.installed) as browser (browser.id)}
                    <div class="browser-row">
                      <div class="browser-meta">
                        <div class="browser-icon" aria-hidden="true">
                          {#if BROWSER_ICONS[browser.id]}
                            {@html BROWSER_ICONS[browser.id]}
                          {:else}
                            <svg viewBox="0 0 20 20" fill="currentColor" width="20" height="20">
                              <path fill-rule="evenodd" d="M10 2a8 8 0 100 16 8 8 0 000-16zm-7 8a7 7 0 1114 0 7 7 0 01-14 0z" clip-rule="evenodd" />
                            </svg>
                          {/if}
                        </div>
                        <div class="browser-names">
                          <strong class="browser-name">{browser.name || browser.id}</strong>
                        </div>
                      </div>

                      <div class="browser-action-col">
                        <button
                          type="button"
                          class="btn btn-primary btn-sm"
                          on:click={() => handleActivateBrowser(browser)}
                        >
                          {$t("extension.activate")}
                        </button>
                      </div>
                    </div>
                  {/each}
                {:else}
                  <!-- Clear Empty State when no supported browser is detected -->
                  <div class="empty-browsers-state">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="40" height="40" class="empty-browser-icon" aria-hidden="true">
                      <circle cx="12" cy="12" r="10" />
                      <line x1="12" y1="8" x2="12" y2="12" />
                      <line x1="12" y1="16" x2="12.01" y2="16" />
                    </svg>
                    <strong class="empty-title">{$t("extension.noBrowsersFound")}</strong>
                    <p class="empty-desc">{$t("extension.noBrowsersDesc")}</p>
                  </div>
                {/if}
              </div>

              <!-- Hidden diagnostics: small link at the bottom -->
              <div class="diagnostics-copy-row">
                <button
                  type="button"
                  class="link-btn-copy-diag"
                  on:click={handleCopyDiagnosticReport}
                >
                  {#if copiedDiagnostic}
                    <span>{$t("extension.reportCopied")}</span>
                  {:else}
                    <span>{$t("extension.copyReport")}</span>
                  {/if}
                </button>
              </div>
            </div>

          <!-- 8. ABOUT -->
          {:else if activeSection === "about"}
            <div class="section-pane">
              <h4 id="section-heading" class="section-heading">{$t("settings.aboutTitle")}</h4>

              <div class="about-hero">
                <img src={iconSvg} alt="سيل" class="about-logo-img" />
                <div class="about-meta">
                  <h3 class="about-app-name">{$t("app.fullName")}</h3>
                  <span class="about-version">{$t("settings.version")} 0.1.0</span>
                  <p class="about-desc">{$t("settings.aboutDesc")}</p>
                </div>
              </div>

              <!-- Replay Tour Button -->
              <div class="setting-row">
                <div class="setting-meta">
                  <span class="setting-title">{$t("settings.replayTour")}</span>
                  <p class="setting-desc">{$t("settings.replayTourDesc")}</p>
                </div>
                <div class="setting-control">
                  <button type="button" class="btn btn-secondary" on:click={handleReplayTour}>
                    {$t("settings.replayTour")}
                  </button>
                </div>
              </div>

              <!-- Links -->
              <div class="setting-row">
                <div class="setting-meta">
                  <span class="setting-title">{$t("settings.links")}</span>
                  <p class="setting-desc">github.com/sayl-app • sayl.download</p>
                </div>
                <div class="setting-control">
                  <div class="links-row">
                    <a
                      href="https://github.com"
                      target="_blank"
                      rel="noreferrer"
                      class="btn btn-secondary btn-sm"
                    >
                      {$t("settings.github")}
                    </a>
                  </div>
                </div>
              </div>
            </div>
          {/if}
        </div>
      </div>

      <!-- Feedback Banner -->
      {#if saveSuccess}
        <div class="feedback-banner success-banner" role="status" aria-live="polite">
          <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
            <path
              fill-rule="evenodd"
              d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
              clip-rule="evenodd"
            />
          </svg>
          <span>{$t("settings.savedNotice")}</span>
        </div>
      {/if}

      {#if saveError}
        <div class="feedback-banner error-banner" role="alert" aria-live="assertive">
          <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16" aria-hidden="true">
            <path
              fill-rule="evenodd"
              d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z"
              clip-rule="evenodd"
            />
          </svg>
          <span>{saveError}</span>
        </div>
      {/if}

      <!-- Modal Footer -->
      <div class="modal-footer">
        <button class="btn btn-secondary" type="button" on:click={handleClose}>
          {$t("settings.close")}
        </button>
        <button class="btn btn-primary" type="button" disabled={saving} on:click={handleSave}>
          {#if saving}
            <span>...</span>
          {:else}
            <span>{$t("settings.save")}</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- 2-Step Guided Extension Dialog -->
<ExtensionGuideDialog
  open={extensionGuideOpen}
  browser={guideBrowser}
  extensionPath={stagedExtensionDir}
  on:close={() => {
    extensionGuideOpen = false;
    guideBrowser = null;
    loadExtensionStatus();
  }}
/>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background-color: rgba(7, 15, 46, 0.65);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    padding: 1rem;
  }

  .settings-dialog {
    background-color: var(--color-bg-card);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    width: 100%;
    max-width: 780px;
    height: 560px;
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    pointer-events: auto;
    position: relative;
    z-index: 101;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.5rem;
    border-bottom: 1px solid var(--color-border);
    position: relative;
    z-index: 20;
  }

  .modal-title {
    font-size: 1.15rem;
    font-weight: 700;
    color: var(--text);
    margin: 0;
  }

  .close-btn {
    width: 2.25rem;
    height: 2.25rem;
    min-width: 2.25rem;
    min-height: 2.25rem;
    color: var(--text-muted);
    border-radius: var(--radius-sm);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    cursor: pointer;
    position: relative;
    z-index: 30;
    pointer-events: auto;
    padding: 0;
  }

  .close-btn:hover {
    background: var(--color-bg-card-hover);
    color: var(--text);
  }

  .settings-layout {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  .settings-nav {
    width: 200px;
    border-inline-end: 1px solid var(--color-border);
    background: var(--color-bg-subtle);
    display: flex;
    flex-direction: column;
    padding: 0.5rem;
    gap: 0.25rem;
    overflow-y: auto;
    pointer-events: auto;
    position: relative;
    z-index: 10;
    flex-shrink: 0;
  }

  .nav-tab-btn {
    text-align: start;
    padding: 0.6rem 0.85rem;
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    color: var(--text-muted);
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 120ms ease;
    user-select: none;
    width: 100%;
    display: block;
    pointer-events: auto;
  }

  .nav-tab-btn span {
    pointer-events: none;
  }

  .nav-tab-btn:hover {
    background: var(--color-bg-card-hover);
    color: var(--text);
  }

  .nav-tab-btn.active {
    background: var(--accent);
    color: #ffffff;
  }

  .settings-content-panel {
    flex: 1;
    overflow-y: auto;
    padding: 1.25rem 1.5rem;
  }

  .section-pane {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .section-heading {
    font-size: 1.1rem;
    font-weight: 700;
    color: var(--text);
    margin: 0;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid var(--color-border);
  }

  .section-intro {
    font-size: 0.88rem;
    color: var(--text-muted);
    margin: 0;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1.5rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--color-border-subtle);
  }

  .setting-row.stacked {
    flex-direction: column;
    align-items: stretch;
    gap: 0.5rem;
  }

  .setting-meta {
    flex: 1;
    min-width: 0;
  }

  .setting-title {
    display: block;
    font-size: 0.92rem;
    font-weight: 600;
    color: var(--text);
    margin-bottom: 0.2rem;
  }

  .setting-desc {
    font-size: 0.82rem;
    line-height: 1.45;
    color: var(--text-muted);
    margin: 0;
  }

  .setting-control {
    flex-shrink: 0;
  }

  .setting-control.full-width {
    width: 100%;
  }

  .form-control {
    background-color: var(--color-bg-input);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--text);
    padding: 0.45rem 0.75rem;
    font-size: 0.88rem;
    min-width: 160px;
  }

  .number-input {
    width: 90px;
    min-width: unset;
  }

  .dir-input-row {
    display: flex;
    gap: 0.5rem;
    width: 100%;
  }

  .dir-input-row .form-control {
    flex: 1;
  }

  /* Switch Toggle Styling */
  .switch-toggle {
    position: relative;
    display: inline-block;
    width: 44px;
    height: 24px;
    cursor: pointer;
  }

  .switch-toggle input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .switch-slider {
    position: absolute;
    inset: 0;
    background-color: var(--color-border);
    border-radius: 24px;
    transition: 0.2s;
  }

  .switch-slider:before {
    position: absolute;
    content: "";
    height: 18px;
    width: 18px;
    left: 3px;
    bottom: 3px;
    background-color: white;
    border-radius: 50%;
    transition: 0.2s;
  }

  .switch-toggle input:checked + .switch-slider {
    background-color: var(--accent);
  }

  .switch-toggle input:checked + .switch-slider:before {
    transform: translateX(20px);
  }

  .switch-toggle.disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .hint-disabled {
    color: var(--text-muted);
    font-style: italic;
  }

  .setting-info-badge {
    background: var(--color-bg-subtle);
    border: 1px solid var(--color-border);
    padding: 0.35rem 0.65rem;
    border-radius: var(--radius-sm);
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  /* Category folder overrides */
  .category-folders {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .category-folder-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }

  .category-folder-row {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.75rem;
    background: var(--color-bg-card);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
  }

  .category-folder-meta {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .category-folder-title {
    font-size: 0.85rem;
    color: var(--text);
  }

  .category-folder-badge {
    font-size: 0.72rem;
    color: var(--text-muted);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: 0.1rem 0.45rem;
  }
  /* Extension Section Styles */
  .host-status-box {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: var(--color-bg-subtle);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: 0.75rem 1rem;
    gap: 1rem;
  }

  .host-status-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .status-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }

  .status-dot.success {
    background: #10b981;
    box-shadow: 0 0 6px #10b981;
  }

  .status-dot.muted {
    background: var(--color-text-subtle);
  }

  .host-status-text {
    display: flex;
    align-items: center;
  }

  .status-text-bold {
    font-size: 0.92rem;
    font-weight: 700;
    color: var(--text);
  }

  .host-actions {
    display: flex;
    gap: 0.5rem;
  }

  .browsers-list {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .browser-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1rem;
    background: var(--color-bg-card);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
  }

  .browser-meta {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .browser-icon {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--accent);
  }

  .browser-icon :global(svg) {
    width: 22px;
    height: 22px;
  }

  .browser-names {
    display: flex;
    flex-direction: column;
  }

  .browser-name {
    font-size: 0.9rem;
    color: var(--text);
  }

  .diagnostics-copy-row {
    margin-top: 1rem;
    padding-top: 0.75rem;
    border-top: 1px solid var(--color-border-subtle);
    display: flex;
    justify-content: center;
  }

  .link-btn-copy-diag {
    background: transparent;
    border: none;
    color: var(--text-subtle);
    font-size: 0.8125rem;
    text-decoration: underline;
    cursor: pointer;
    padding: 0.35rem 0.65rem;
    border-radius: var(--radius-sm);
    transition: color 120ms ease;
  }

  .link-btn-copy-diag:hover {
    color: var(--accent);
  }

  .empty-browsers-state {
    text-align: center;
    padding: 2rem 1rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
  }

  .empty-browser-icon {
    color: var(--text-subtle);
    opacity: 0.7;
  }

  .empty-title {
    font-size: 0.95rem;
    color: var(--text);
  }

  .empty-desc {
    font-size: 0.82rem;
    color: var(--text-muted);
    max-width: 380px;
    margin: 0;
  }

  /* About section styles */
  .about-hero {
    display: flex;
    align-items: center;
    gap: 1.25rem;
    padding: 1rem 0;
  }

  .about-logo-img {
    width: 52px;
    height: 52px;
    object-fit: contain;
    opacity: 1;
    filter: none;
    flex-shrink: 0;
  }

  .about-app-name {
    font-size: 1.15rem;
    font-weight: 700;
    color: var(--text);
    margin: 0;
  }

  .about-version {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--accent);
  }

  .about-desc {
    font-size: 0.85rem;
    color: var(--text-muted);
    margin: 0.25rem 0 0;
  }

  /* Feedback banner */
  .feedback-banner {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 1.5rem;
    font-size: 0.85rem;
    font-weight: 600;
  }

  .success-banner {
    background: rgba(16, 185, 129, 0.15);
    color: var(--color-success);
  }

  .error-banner {
    background: rgba(239, 68, 68, 0.15);
    color: var(--color-danger);
  }

  /* Modal Footer */
  .modal-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.75rem;
    padding: 0.85rem 1.5rem;
    border-top: 1px solid var(--color-border);
    position: relative;
    z-index: 10;
    pointer-events: auto;
  }

  .btn-browse {
    background: var(--color-bg-card);
    color: var(--text);
    border: 1px solid var(--color-border);
    white-space: nowrap;
    cursor: pointer;
    pointer-events: auto;
  }

  .btn-browse:hover {
    background: var(--color-bg-card-hover);
  }

  .btn {
    padding: 0.45rem 1rem;
    border-radius: var(--radius-sm);
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
    border: none;
    transition: background 150ms ease;
  }

  .btn-sm {
    padding: 0.35rem 0.75rem;
    font-size: 0.82rem;
  }

  .btn-primary {
    background: var(--accent);
    color: #ffffff;
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--color-accent-hover);
  }

  .btn-secondary {
    background: var(--color-bg-card);
    color: var(--text);
    border: 1px solid var(--color-border);
  }

  .btn-secondary:hover:not(:disabled) {
    background: var(--color-bg-card-hover);
  }

  .diagnostics-copy-row {
    margin-top: 1rem;
    text-align: center;
  }

  .link-btn-copy-diag {
    background: none;
    border: none;
    color: var(--text-subtle);
    font-size: 0.78rem;
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .link-btn-copy-diag:hover {
    color: var(--accent);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
