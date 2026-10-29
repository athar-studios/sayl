<script>
  /**
   * Window "b" entry point, kept at this path so App.svelte's floating stack and
   * StandaloneDownloadWindow (native `?task=` window) keep working unchanged.
   *
   * While a task is downloading it renders the IDM-style status window; once the
   * task is explicitly `Completed` it renders the completion layout. The
   * frontend never infers completion from anything except the task state.
   */
  import DownloadingWindow from "./windows/DownloadingWindow.svelte";
  import CompleteWindow from "./windows/CompleteWindow.svelte";

  export let task;
  export let autoShutdown = false;
  export let autoClose = false;
  export let isNativeWindow = false;
  export let pendingStart = null;
  /** Optional inputs used by the read-only «حدود السرعة» tab. */
  export let maxConnections = null;
  export let speedLimitKbps = null;

  $: isCompleted = task?.state === "Completed";
</script>

{#if task}
  {#if isCompleted}
    <CompleteWindow
      {task}
      on:close
      on:reveal
      on:openFile
      on:dontShowAgain
    />
  {:else}
    <DownloadingWindow
      {task}
      {autoShutdown}
      {autoClose}
      {isNativeWindow}
      {pendingStart}
      {maxConnections}
      {speedLimitKbps}
      on:close
      on:pause
      on:resume
      on:retry
      on:cancel
      on:reveal
      on:openFile
      on:toggleAutoShutdown
      on:toggleAutoClose
    />
  {/if}
{/if}
