<script>
  import { t, currentLang, setLanguage } from "./i18n.js";
  import { safeInvoke } from "./tauri.js";
  import { DEFAULT_SETTINGS, normalizeSettings, SETTINGS_SECTIONS } from "./settings.js";
  import iconSvg from "../assets/icon.svg";
  import {
    SUPPORTED_BROWSERS,
    BROWSER_ICONS,
    getBrowserAction,
    getBrowserStatusInfo,
    getHostStatusSummary,
    hasInstalledBrowsers,
  } from "./browsers.js";
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
  let nativeHostStatus = null;
  let browserList = [];
  let testResult = null;
  let testingHost = false;
  let extensionGuideOpen = false;
  let guideBrowser = null;
  let stagedExtensionDir = "";

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
    }
  }

  $: if (open && initialSection !== prevInitialSection) {
    prevInitialSection = initialSection;
    if (initialSection) {
      activeSection = initialSection;
    }
  }

  $: hostSummary = getHostStatusSummary(nativeHostStatus, browserList);
  $: hasBrowsers = hasInstalledBrowsers(browserList);

  async function loadExtensionStatus() {
    if (loadingExtensionState) return;
    loadingExtensionState = true;
    try {
      const [nhRes, bRes] = await Promise.allSettled([
        safeInvoke("native_host_status"),
        safeInvoke("browser_status"),
      ]);

      if (nhRes.status === "fulfilled" && nhRes.value) {
        nativeHostStatus = nhRes.value;
        if (nhRes.value.extension_dir) {
          stagedExtensionDir = nhRes.value.extension_dir;
        }
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

  async function handleTestHost() {
    testingHost = true;
    testResult = null;
    try {
      const res = await safeInvoke("test_native_host");
      testResult = res || { ok: true, roundtrip_ms: 0, message: "OK" };
    } catch (err) {
      testResult = {
        ok: false,
        roundtrip_ms: 0,
        message: typeof err === "string" ? err : err?.message || String(err),
      };
    } finally {
      testingHost = false;
    }
  }

  function handleActivateBrowser(browser) {
    const action = getBrowserAction(browser.id);
    if (action.type === "store") {
      if (action.available && action.url) {
        if (typeof window !== "undefined") {
          window.open(action.url, "_blank");
        }
      }
    } else {
      guideBrowser = browser;
      extensionGuideOpen = true;
    }
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
      const updated = await safeInvoke("set_settings", { settings: payload });
      if (updated) {
        settings = { ...updated };
        localSettings = normalizeSettings(updated);
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

              <div class="categories-grid">
                <div class="cat-card">
                  <span class="cat-icon">📄</span>
                  <div class="cat-info">
                    <strong>{$t("settings.catFolderDocuments")}</strong>
                    <small>pdf, doc, docx, txt, epub, xlsx, pptx</small>
                  </div>
                </div>

                <div class="cat-card">
                  <span class="cat-icon">⚙️</span>
                  <div class="cat-info">
                    <strong>{$t("settings.catFolderPrograms")}</strong>
                    <small>exe, msi, apk, appimage, deb, rpm</small>
                  </div>
                </div>

                <div class="cat-card">
                  <span class="cat-icon">🎬</span>
                  <div class="cat-info">
                    <strong>{$t("settings.catFolderVideo")}</strong>
                    <small>mp4, mkv, webm, avi, mov, flv</small>
                  </div>
                </div>

                <div class="cat-card">
                  <span class="cat-icon">🎵</span>
                  <div class="cat-info">
                    <strong>{$t("settings.catFolderAudio")}</strong>
                    <small>mp3, wav, flac, aac, m4a, ogg</small>
                  </div>
                </div>

                <div class="cat-card">
                  <span class="cat-icon">📦</span>
                  <div class="cat-info">
                    <strong>{$t("settings.catFolderCompressed")}</strong>
                    <small>zip, rar, 7z, tar, gz, bz2, iso</small>
                  </div>
                </div>

                <div class="cat-card">
                  <span class="cat-icon">📁</span>
                  <div class="cat-info">
                    <strong>{$t("settings.catFolderOther")}</strong>
                    <small>other file extensions</small>
                  </div>
                </div>
              </div>
            </div>

          <!-- 4. NOTIFICATIONS & WINDOWS -->
          {:else if activeSection === "notifications"}
            <div class="section-pane">
              <h4 id="section-heading" class="section-heading">{$t("settings.navNotifications")}</h4>

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

              <!-- Native Host Status Line & Test Action -->
              <div class="host-status-box">
                <div class="host-status-left">
                  <span class="status-dot {hostSummary.level}"></span>
                  <div class="host-status-text">
                    <strong class="host-title">{$t("extension.hostStatus")}</strong>
                    <span class="host-badge {hostSummary.level}">{$t(hostSummary.textKey)}</span>
                  </div>
                </div>

                <div class="host-actions">
                  <button
                    type="button"
                    class="btn btn-secondary btn-sm"
                    disabled={testingHost}
                    on:click={handleTestHost}
                  >
                    {#if testingHost}
                      <span>{$t("extension.testingConnection")}</span>
                    {:else}
                      <span>{$t("extension.testConnection")}</span>
                    {/if}
                  </button>

                  <button
                    type="button"
                    class="btn btn-secondary btn-sm"
                    disabled={loadingExtensionState}
                    on:click={loadExtensionStatus}
                    title={$t("extension.refresh")}
                  >
                    <svg viewBox="0 0 20 20" fill="currentColor" width="14" height="14" aria-hidden="true">
                      <path
                        fill-rule="evenodd"
                        d="M4 2a1 1 0 011 1v2.101a7.002 7.002 0 0111.601 2.566 1 1 0 11-1.885.666A5.002 5.002 0 005.999 7H9a1 1 0 010 2H4a1 1 0 01-1-1V3a1 1 0 011-1zm.008 9.057a1 1 0 011.276.61A5.002 5.002 0 0014.001 13H11a1 1 0 110-2h5a1 1 0 011 1v5a1 1 0 11-2 0v-2.101a7.002 7.002 0 01-11.601-2.566 1 1 0 01.61-1.276z"
                        clip-rule="evenodd"
                      />
                    </svg>
                  </button>
                </div>
              </div>

              <!-- Test Native Host Result Feedback -->
              {#if testResult}
                <div class="test-feedback-bar" class:success={testResult.ok} class:error={!testResult.ok}>
                  {#if testResult.ok}
                    <span>{$t("extension.testOk").replace("{ms}", testResult.roundtrip_ms ?? 0)}</span>
                  {:else}
                    <span>{$t("extension.testFailed").replace("{msg}", testResult.message || "")}</span>
                  {/if}
                </div>
              {/if}

              <!-- Browsers List -->
              <div class="browsers-list">
                {#if hasBrowsers}
                  {#each browserList.filter((b) => b && b.installed) as browser (browser.id)}
                    {@const statusInfo = getBrowserStatusInfo(browser)}
                    {@const action = getBrowserAction(browser.id)}
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
                          <span class="browser-status-badge {statusInfo.level}">
                            {$t(statusInfo.textKey)}
                          </span>
                        </div>
                      </div>

                      <div class="browser-action-col">
                        {#if action.type === "store"}
                          {#if action.available}
                            <button
                              type="button"
                              class="btn btn-primary btn-sm"
                              on:click={() => handleActivateBrowser(browser)}
                            >
                              {$t("extension.activate")}
                            </button>
                          {:else}
                            <button
                              type="button"
                              class="btn btn-secondary btn-sm"
                              disabled
                              title={$t("extension.storeUnavailable")}
                            >
                              {$t("extension.storeUnavailable")}
                            </button>
                          {/if}
                        {:else}
                          <button
                            type="button"
                            class="btn btn-primary btn-sm"
                            on:click={() => handleActivateBrowser(browser)}
                          >
                            {$t("extension.activate")}
                          </button>
                        {/if}
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
                    <button type="button" class="btn btn-secondary btn-sm" on:click={loadExtensionStatus}>
                      {$t("extension.refresh")}
                    </button>
                  </div>
                {/if}
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
  on:pollStatus={loadExtensionStatus}
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
    color: var(--color-text);
    margin: 0;
  }

  .close-btn {
    width: 2.25rem;
    height: 2.25rem;
    min-width: 2.25rem;
    min-height: 2.25rem;
    color: var(--color-text-muted);
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
    background: var(--color-bg-hover, rgba(0, 0, 0, 0.05));
    color: var(--color-text);
  }

  .settings-layout {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  .settings-nav {
    width: 200px;
    border-inline-end: 1px solid var(--color-border);
    background: var(--color-bg-subtle, rgba(0, 0, 0, 0.08));
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
    color: var(--color-text-muted);
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
    background: var(--color-bg-hover);
    color: var(--color-text);
  }

  .nav-tab-btn.active {
    background: #01b2bb;
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
    color: var(--color-text);
    margin: 0;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid var(--color-border);
  }

  .section-intro {
    font-size: 0.88rem;
    color: var(--color-text-muted);
    margin: 0;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1.5rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--color-border-subtle, rgba(255, 255, 255, 0.05));
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
    color: var(--color-text);
    margin-bottom: 0.2rem;
  }

  .setting-desc {
    font-size: 0.82rem;
    line-height: 1.45;
    color: var(--color-text-muted);
    margin: 0;
  }

  .setting-control {
    flex-shrink: 0;
  }

  .setting-control.full-width {
    width: 100%;
  }

  .form-control {
    background-color: var(--color-bg-input, rgba(0, 0, 0, 0.2));
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text);
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
    background-color: var(--color-border, #334155);
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
    background-color: #01b2bb;
  }

  .switch-toggle input:checked + .switch-slider:before {
    transform: translateX(20px);
  }

  .switch-toggle.disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .hint-disabled {
    color: var(--color-text-muted);
    font-style: italic;
  }

  .setting-info-badge {
    background: var(--color-bg-hover, rgba(255, 255, 255, 0.07));
    border: 1px solid var(--color-border);
    padding: 0.35rem 0.65rem;
    border-radius: var(--radius-sm);
    font-size: 0.8rem;
    color: var(--color-text-muted);
  }

  /* Categories Grid */
  .categories-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 0.75rem;
  }

  .cat-card {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem;
    background: var(--color-bg-card-subtle, rgba(255, 255, 255, 0.03));
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
  }

  .cat-icon {
    font-size: 1.5rem;
  }

  .cat-info strong {
    display: block;
    font-size: 0.85rem;
    color: var(--color-text);
  }

  .cat-info small {
    color: var(--color-text-muted);
    font-size: 0.75rem;
  }

  /* Extension Section Styles */
  .host-status-box {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: var(--color-bg-card-subtle, rgba(255, 255, 255, 0.03));
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

  .status-dot.warning {
    background: #f59e0b;
  }

  .status-dot.muted {
    background: #64748b;
  }

  .host-status-text {
    display: flex;
    flex-direction: column;
  }

  .host-title {
    font-size: 0.88rem;
    color: var(--color-text);
  }

  .host-badge {
    font-size: 0.78rem;
    font-weight: 600;
  }

  .host-badge.success {
    color: #10b981;
  }

  .host-badge.warning {
    color: #f59e0b;
  }

  .host-badge.muted {
    color: var(--color-text-muted);
  }

  .host-actions {
    display: flex;
    gap: 0.5rem;
  }

  .test-feedback-bar {
    padding: 0.5rem 0.75rem;
    border-radius: var(--radius-sm);
    font-size: 0.82rem;
    font-weight: 600;
  }

  .test-feedback-bar.success {
    background: rgba(16, 185, 129, 0.15);
    color: #10b981;
    border: 1px solid rgba(16, 185, 129, 0.3);
  }

  .test-feedback-bar.error {
    background: rgba(239, 68, 68, 0.15);
    color: #ef4444;
    border: 1px solid rgba(239, 68, 68, 0.3);
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
    padding: 0.65rem 0.85rem;
    background: var(--color-bg-card-subtle, rgba(255, 255, 255, 0.03));
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
    color: #00e5ff;
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
    color: var(--color-text);
  }

  .browser-status-badge {
    font-size: 0.76rem;
  }

  .browser-status-badge.success {
    color: #10b981;
  }

  .browser-status-badge.warning {
    color: #f59e0b;
  }

  .browser-status-badge.muted {
    color: var(--color-text-muted);
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
    color: var(--color-text-muted);
    opacity: 0.5;
  }

  .empty-title {
    font-size: 0.95rem;
    color: var(--color-text);
  }

  .empty-desc {
    font-size: 0.82rem;
    color: var(--color-text-muted);
    max-width: 380px;
    margin: 0 0 0.5rem;
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
    color: var(--color-text);
    margin: 0;
  }

  .about-version {
    font-size: 0.82rem;
    font-weight: 600;
    color: #00e5ff;
  }

  .about-desc {
    font-size: 0.85rem;
    color: var(--color-text-muted);
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
    color: #10b981;
  }

  .error-banner {
    background: rgba(239, 68, 68, 0.15);
    color: #ef4444;
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
    background: var(--color-bg-hover, rgba(255, 255, 255, 0.08));
    color: var(--color-text);
    border: 1px solid var(--color-border);
    white-space: nowrap;
    cursor: pointer;
    pointer-events: auto;
  }

  .btn-browse:hover {
    background: var(--color-bg-card-subtle, rgba(255, 255, 255, 0.14));
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
    background: #01b2bb;
    color: #ffffff;
  }

  .btn-primary:hover:not(:disabled) {
    background: #0299a1;
  }

  .btn-secondary {
    background: var(--color-bg-hover, rgba(255, 255, 255, 0.08));
    color: var(--color-text);
    border: 1px solid var(--color-border);
  }

  .btn-secondary:hover:not(:disabled) {
    background: var(--color-bg-card-subtle, rgba(255, 255, 255, 0.14));
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
