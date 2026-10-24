<script>
  import { t } from "./i18n.js";
  import TaskCard from "./TaskCard.svelte";
  import TaskTable from "./TaskTable.svelte";
  import EmptyState from "./EmptyState.svelte";
  import { createEventDispatcher } from "svelte";

  export let tasks = [];
  export let selectedId = null;
  export let totalTasksCount = 0;
  export let viewMode = "cards"; // "cards" | "table"

  const dispatch = createEventDispatcher();
</script>

<div class="task-list-container">
  {#if tasks.length === 0 && totalTasksCount === 0}
    <EmptyState on:add={() => dispatch("add")} />
  {:else if tasks.length === 0 && totalTasksCount > 0}
    <div class="empty-tab-notice">
      <p>{$t("tabs.empty")}</p>
    </div>
  {:else if viewMode === "table"}
    <div class="table-view-wrapper">
      <TaskTable
        {tasks}
        {selectedId}
        on:select={(e) => dispatch("select", e.detail)}
        on:pause={(e) => dispatch("pause", e.detail)}
        on:resume={(e) => dispatch("resume", e.detail)}
        on:cancel={(e) => dispatch("cancel", e.detail)}
        on:reveal={(e) => dispatch("reveal", e.detail)}
      />
    </div>
  {:else}
    <div class="task-grid" role="feed" aria-busy="false">
      {#each tasks as task (task.id)}
        <TaskCard
          {task}
          selected={selectedId === task.id}
          on:select={(e) => dispatch("select", e.detail)}
          on:pause={(e) => dispatch("pause", e.detail)}
          on:resume={(e) => dispatch("resume", e.detail)}
          on:cancel={(e) => dispatch("cancel", e.detail)}
          on:reveal={(e) => dispatch("reveal", e.detail)}
        />
      {/each}
    </div>
  {/if}
</div>

<style>
  .task-list-container {
    flex: 1;
    overflow-y: auto;
    padding: 1.25rem 1.5rem;
    display: flex;
    flex-direction: column;
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
    max-width: 1100px;
    margin: 0 auto;
    width: 100%;
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
