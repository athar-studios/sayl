<script>
  import { t, currentLang } from "./i18n.js";
  import { createEventDispatcher } from "svelte";

  export let collapsed = false;
  export let selectedCategory = "all";
  export let selectedStateFilter = "all";
  export let categoryCounts = {};
  export let statusCounts = {};

  const dispatch = createEventDispatcher();

  let statusBranchExpanded = true;
  let categoriesBranchExpanded = true;

  function handleSelectAll() {
    selectedCategory = "all";
    selectedStateFilter = "all";
    dispatch("select", "all");
    dispatch("selectState", "all");
  }

  function handleSelectStatus(statusId) {
    selectedCategory = "all";
    selectedStateFilter = statusId;
    dispatch("select", "all");
    dispatch("selectState", statusId);
  }

  function handleSelectCategory(catId) {
    selectedCategory = catId;
    selectedStateFilter = "all";
    dispatch("select", catId);
    dispatch("selectState", "all");
  }

  function toggleCollapse() {
    collapsed = !collapsed;
    dispatch("toggleCollapse", collapsed);
  }
</script>

<aside class="sidebar-tree" class:collapsed id="tour-category-tree" aria-label={$t("tree.all")}>
  {#if !collapsed}
    <!-- Root node: الكل (All Downloads) -->
    <div
      class="tree-node tree-root"
      class:active={selectedCategory === "all" && selectedStateFilter === "all"}
      role="button"
      tabindex="0"
      on:click={handleSelectAll}
      on:keydown={(e) => e.key === "Enter" && handleSelectAll()}
    >
      <div class="node-icon root-icon" aria-hidden="true">
        <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
          <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
        </svg>
      </div>
      <span class="node-title">{$t("tree.all")}</span>
      <span class="node-badge"><bdi dir="ltr">{statusCounts.all || categoryCounts.all || 0}</bdi></span>
    </div>

    <!-- 1. فرع الحالة (Status Branch) -->
    <div class="tree-branch">
      <div
        class="tree-section-header"
        role="button"
        tabindex="0"
        on:click={() => (statusBranchExpanded = !statusBranchExpanded)}
        on:keydown={(e) => e.key === "Enter" && (statusBranchExpanded = !statusBranchExpanded)}
      >
        <span class="branch-arrow" class:expanded={statusBranchExpanded} aria-hidden="true">
          {statusBranchExpanded ? "▼" : "▶"}
        </span>
        <span class="branch-header-title">{$t("tree.status")}</span>
      </div>

      {#if statusBranchExpanded}
        <div class="branch-items">
          <!-- غير مكتمل (Incomplete) -->
          <div
            class="tree-node tree-subnode"
            class:active={selectedStateFilter === "incomplete"}
            role="button"
            tabindex="0"
            on:click={() => handleSelectStatus("incomplete")}
            on:keydown={(e) => e.key === "Enter" && handleSelectStatus("incomplete")}
          >
            <div class="node-icon status-icon-incomplete" aria-hidden="true">
              <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15">
                <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z" clip-rule="evenodd" />
              </svg>
            </div>
            <span class="node-title">{$t("tree.incomplete")}</span>
            <span class="node-badge"><bdi dir="ltr">{statusCounts.incomplete || 0}</bdi></span>
          </div>

          <!-- مكتمل (Completed) -->
          <div
            class="tree-node tree-subnode"
            class:active={selectedStateFilter === "completed"}
            role="button"
            tabindex="0"
            on:click={() => handleSelectStatus("completed")}
            on:keydown={(e) => e.key === "Enter" && handleSelectStatus("completed")}
          >
            <div class="node-icon status-icon-completed" aria-hidden="true">
              <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15">
                <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
              </svg>
            </div>
            <span class="node-title">{$t("tree.completed")}</span>
            <span class="node-badge"><bdi dir="ltr">{statusCounts.completed || 0}</bdi></span>
          </div>

          <!-- قائمة الانتظار (Queue) -->
          <div
            class="tree-node tree-subnode"
            class:active={selectedStateFilter === "queue"}
            role="button"
            tabindex="0"
            on:click={() => handleSelectStatus("queue")}
            on:keydown={(e) => e.key === "Enter" && handleSelectStatus("queue")}
          >
            <div class="node-icon status-icon-queue" aria-hidden="true">
              <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15">
                <path fill-rule="evenodd" d="M3 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1z" clip-rule="evenodd" />
              </svg>
            </div>
            <span class="node-title">{$t("tree.queue")}</span>
            <span class="node-badge"><bdi dir="ltr">{statusCounts.queue || 0}</bdi></span>
          </div>
        </div>
      {/if}
    </div>

    <!-- 2. فرع التصنيفات (Categories Branch) -->
    <div class="tree-branch">
      <div
        class="tree-section-header"
        role="button"
        tabindex="0"
        on:click={() => (categoriesBranchExpanded = !categoriesBranchExpanded)}
        on:keydown={(e) => e.key === "Enter" && (categoriesBranchExpanded = !categoriesBranchExpanded)}
      >
        <span class="branch-arrow" class:expanded={categoriesBranchExpanded} aria-hidden="true">
          {categoriesBranchExpanded ? "▼" : "▶"}
        </span>
        <span class="branch-header-title">{$t("tree.categories")}</span>
      </div>

      {#if categoriesBranchExpanded}
        <div class="branch-items">
          <!-- فيديو (Video) -->
          <div
            class="tree-node tree-subnode"
            class:active={selectedCategory === "video" && selectedStateFilter === "all"}
            role="button"
            tabindex="0"
            on:click={() => handleSelectCategory("video")}
            on:keydown={(e) => e.key === "Enter" && handleSelectCategory("video")}
          >
            <div class="node-icon cat-icon-video" aria-hidden="true">
              <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15">
                <path d="M2 6a2 2 0 012-2h6a2 2 0 012 2v8a2 2 0 01-2 2H4a2 2 0 01-2-2V6zM14.553 7.106A1 1 0 0014 8v4a1 1 0 00.553.894l2 1A1 1 0 0018 13V7a1 1 0 00-1.447-.894l-2 1z" />
              </svg>
            </div>
            <span class="node-title">{$t("tree.video")}</span>
            <span class="node-badge"><bdi dir="ltr">{categoryCounts.video || 0}</bdi></span>
          </div>

          <!-- صوت (Audio) -->
          <div
            class="tree-node tree-subnode"
            class:active={selectedCategory === "audio" && selectedStateFilter === "all"}
            role="button"
            tabindex="0"
            on:click={() => handleSelectCategory("audio")}
            on:keydown={(e) => e.key === "Enter" && handleSelectCategory("audio")}
          >
            <div class="node-icon cat-icon-audio" aria-hidden="true">
              <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15">
                <path d="M18 3a1 1 0 00-1.196-.98l-10 2A1 1 0 006 5v9.114A4.369 4.369 0 005 14c-1.657 0-3 .895-3 2s1.343 2 3 2 3-.895 3-2V7.82l8-1.6v5.894A4.37 4.37 0 0015 12c-1.657 0-3 .895-3 2s1.343 2 3 2 3-.895 3-2V3z" />
              </svg>
            </div>
            <span class="node-title">{$t("tree.audio")}</span>
            <span class="node-badge"><bdi dir="ltr">{categoryCounts.audio || 0}</bdi></span>
          </div>

          <!-- مستندات (Documents) -->
          <div
            class="tree-node tree-subnode"
            class:active={selectedCategory === "documents" && selectedStateFilter === "all"}
            role="button"
            tabindex="0"
            on:click={() => handleSelectCategory("documents")}
            on:keydown={(e) => e.key === "Enter" && handleSelectCategory("documents")}
          >
            <div class="node-icon cat-icon-docs" aria-hidden="true">
              <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15">
                <path fill-rule="evenodd" d="M4 4a2 2 0 012-2h4.586A2 2 0 0112 2.586L15.414 6A2 2 0 0116 7.414V16a2 2 0 01-2 2H6a2 2 0 01-2-2V4z" clip-rule="evenodd" />
              </svg>
            </div>
            <span class="node-title">{$t("tree.documents")}</span>
            <span class="node-badge"><bdi dir="ltr">{categoryCounts.documents || 0}</bdi></span>
          </div>

          <!-- برامج (Programs) -->
          <div
            class="tree-node tree-subnode"
            class:active={selectedCategory === "programs" && selectedStateFilter === "all"}
            role="button"
            tabindex="0"
            on:click={() => handleSelectCategory("programs")}
            on:keydown={(e) => e.key === "Enter" && handleSelectCategory("programs")}
          >
            <div class="node-icon cat-icon-programs" aria-hidden="true">
              <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15">
                <path fill-rule="evenodd" d="M2 5a2 2 0 012-2h12a2 2 0 012 2v10a2 2 0 01-2 2H4a2 2 0 01-2-2V5zm3.293 1.293a1 1 0 011.414 0l3 3a1 1 0 010 1.414l-3 3a1 1 0 01-1.414-1.414L7.586 10 5.293 7.707a1 1 0 010-1.414zM11 12a1 1 0 100 2h3a1 1 0 100-2h-3z" clip-rule="evenodd" />
              </svg>
            </div>
            <span class="node-title">{$t("tree.programs")}</span>
            <span class="node-badge"><bdi dir="ltr">{categoryCounts.programs || 0}</bdi></span>
          </div>

          <!-- مضغوطة (Compressed) -->
          <div
            class="tree-node tree-subnode"
            class:active={selectedCategory === "compressed" && selectedStateFilter === "all"}
            role="button"
            tabindex="0"
            on:click={() => handleSelectCategory("compressed")}
            on:keydown={(e) => e.key === "Enter" && handleSelectCategory("compressed")}
          >
            <div class="node-icon cat-icon-compressed" aria-hidden="true">
              <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15">
                <path fill-rule="evenodd" d="M5 2a2 2 0 00-2 2v14l3.5-2 3.5 2 3.5-2 3.5 2V4a2 2 0 00-2-2H5zm2 5a1 1 0 011-1h4a1 1 0 110 2H8a1 1 0 01-1-1zm0 3a1 1 0 011-1h4a1 1 0 110 2H8a1 1 0 01-1-1z" clip-rule="evenodd" />
              </svg>
            </div>
            <span class="node-title">{$t("tree.compressed")}</span>
            <span class="node-badge"><bdi dir="ltr">{categoryCounts.compressed || 0}</bdi></span>
          </div>

          <!-- أخرى (Other) -->
          <div
            class="tree-node tree-subnode"
            class:active={selectedCategory === "other" && selectedStateFilter === "all"}
            role="button"
            tabindex="0"
            on:click={() => handleSelectCategory("other")}
            on:keydown={(e) => e.key === "Enter" && handleSelectCategory("other")}
          >
            <div class="node-icon cat-icon-other" aria-hidden="true">
              <svg viewBox="0 0 20 20" fill="currentColor" width="15" height="15">
                <path fill-rule="evenodd" d="M10 2a8 8 0 100 16 8 8 0 000-16zM9 9a1 1 0 012 0v5a1 1 0 11-2 0V9zm1-4a1 1 0 100 2 1 1 0 000-2z" clip-rule="evenodd" />
              </svg>
            </div>
            <span class="node-title">{$t("tree.other")}</span>
            <span class="node-badge"><bdi dir="ltr">{categoryCounts.other || 0}</bdi></span>
          </div>
        </div>
      {/if}
    </div>
  {/if}
</aside>

<style>
  .sidebar-tree {
    width: 230px;
    background: var(--color-bg-subtle);
    border-inline-end: 1px solid var(--color-border);
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    font-size: 12.5px;
    user-select: none;
    -webkit-user-select: none;
    flex-shrink: 0;
    padding: 6px 0;
  }

  .sidebar-tree.collapsed {
    display: none;
  }

  .tree-node {
    display: flex;
    align-items: center;
    padding: 5px 12px;
    gap: 7px;
    cursor: default;
    color: var(--text);
    border-inline-start: 3px solid transparent;
    transition: background-color 0.1s;
    height: 28px;
  }

  .tree-node:hover {
    background: var(--color-bg-card-hover);
    color: var(--text);
  }

  .tree-node.active {
    background: var(--color-accent-light);
    color: var(--accent);
    font-weight: 600;
    border-inline-start-color: var(--accent);
  }

  .tree-root {
    font-weight: 600;
    margin-bottom: 4px;
    border-bottom: 1px solid var(--color-border-subtle);
    padding-bottom: 8px;
    height: 32px;
  }

  .tree-branch {
    margin-top: 4px;
  }

  .tree-section-header {
    display: flex;
    align-items: center;
    padding: 6px 10px 4px 10px;
    font-size: 11px;
    font-weight: 700;
    color: var(--text-subtle);
    letter-spacing: 0.02em;
    cursor: default;
    gap: 6px;
  }

  .branch-arrow {
    font-size: 8px;
    width: 10px;
    color: var(--text-subtle);
    display: inline-block;
  }

  .branch-header-title {
    flex: 1;
  }

  .tree-subnode {
    padding-inline-start: 26px;
  }

  .node-icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-subtle);
  }

  .root-icon {
    color: var(--accent);
  }

  .status-icon-incomplete {
    color: #d97706;
  }

  .status-icon-completed {
    color: #16a34a;
  }

  .status-icon-queue {
    color: #6366f1;
  }

  .cat-icon-video {
    color: #8b5cf6;
  }

  .cat-icon-audio {
    color: #ec4899;
  }

  .cat-icon-docs {
    color: #3b82f6;
  }

  .cat-icon-programs {
    color: #10b981;
  }

  .cat-icon-compressed {
    color: #f59e0b;
  }

  .cat-icon-other {
    color: var(--text-subtle);
  }

  .node-title {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .node-badge {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-subtle);
    background: var(--color-bg-card);
    padding: 1px 6px;
    border-radius: 10px;
    min-width: 18px;
    text-align: center;
  }

  .tree-node.active .node-badge {
    background: var(--color-accent-light);
    color: var(--accent);
  }
</style>
