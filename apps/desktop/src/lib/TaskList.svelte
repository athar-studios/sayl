<script>
  import { t } from "./i18n.js";
  import TaskCard from "./TaskCard.svelte";
  import TaskTable from "./TaskTable.svelte";
  import EmptyState from "./EmptyState.svelte";
  import { createEventDispatcher } from "svelte";

  export let tasks = [];
  export let selectedId = null;
  export let selectedIds = [];
  export let totalTasksCount = 0;
  export let viewMode = "table"; // Requirement 4: default view_mode is "table"

  const dispatch = createEventDispatcher();

  $: isSelected = (id) => (selectedIds.length > 0 ? selectedIds.includes(id) : selectedId === id);
</script>

<div class="task-list-container" class:cards-mode={viewMode === "cards"}>
  {#if tasks.length === 0 && totalTasksCount === 0}
    <EmptyState on:add={() => dispatch("add")} />
  {:else if tasks.length === 0 && totalTasksCount > 0}
    <div class="empty-tab-notice">
      <p>{$t("tabs.empty")}</p>
    </div>
  {:else if viewMode === "cards"}
    <div class="task-grid" role="feed" aria-busy="false">
      {#each tasks as task (task.id)}
        <TaskCard
          {task}
          selected={isSelected(task.id)}
          on:select={(e) => dispatch("select", e.detail)}
          on:pause={(e) => dispatch("pause", e.detail)}
          on:resume={(e) => dispatch("resume", e.detail)}
          on:retry={(e) => dispatch("retry", e.detail)}
          on:cancel={(e) => dispatch("cancel", e.detail)}
          on:delete={(e) => dispatch("delete", e.detail)}
          on:reveal={(e) => dispatch("reveal", e.detail)}
          on:openFile={(e) => dispatch("openFile", e.detail)}
          on:properties={(e) => dispatch("properties", e.detail)}
        />
      {/each}
    </div>
  {:else}
    <div class="table-view-wrapper">
      <TaskTable
        {tasks}
        {selectedId}
        {selectedIds}
        on:select={(e) => dispatch("select", e.detail)}
        on:pause={(e) => dispatch("pause", e.detail)}
        on:resume={(e) => dispatch("resume", e.detail)}
        on:retry={(e) => dispatch("retry", e.detail)}
        on:cancel={(e) => dispatch("cancel", e.detail)}
        on:delete={(e) => dispatch("delete", e.detail)}
        on:reveal={(e) => dispatch("reveal", e.detail)}
        on:openFile={(e) => dispatch("openFile", e.detail)}
        on:properties={(e) => dispatch("properties", e.detail)}
      />
    </div>
  {/if}
</div>

<style>
  .task-list-container {
    flex: 1;
    overflow-y: auto;
    overflow-x: auto;
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    min-width: 0;
  }

  .task-list-container.cards-mode {
    padding: 1.25rem 1.5rem;
  }

  .task-grid {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    max-width: 1000px;
    margin: 0 auto;
    width: 100%;
  }

  .table-view-wrapper {
    width: 100%;
    height: 100%;
    flex: 1;
    display: flex;
    min-width: 0;
    overflow-x: auto;
  }

  .empty-tab-notice {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 1;
    padding: 3rem 1rem;
    color: var(--color-text-subtle);
    font-size: 0.95rem;
  }
</style>
