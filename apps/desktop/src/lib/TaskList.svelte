<script>
  import TaskCard from "./TaskCard.svelte";
  import EmptyState from "./EmptyState.svelte";
  import { createEventDispatcher } from "svelte";

  export let tasks = [];
  export let selectedId = null;

  const dispatch = createEventDispatcher();
</script>

<div class="task-list-container">
  {#if tasks.length === 0}
    <EmptyState on:add={() => dispatch("add")} />
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
</style>
