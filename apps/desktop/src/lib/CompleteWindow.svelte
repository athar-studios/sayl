<script>
  /**
   * Window "c" entry point, kept at this path so StandaloneCompleteWindow (the
   * native `?complete=<id>` window opened by `open_complete_window`) and any
   * other caller keep working. The IDM-style layout lives in
   * `windows/CompleteWindow.svelte`.
   */
  import CompleteWindowView from "./windows/CompleteWindow.svelte";

  export let task;
</script>

<CompleteWindowView {task} on:close on:reveal on:openFile on:dontShowAgain />
