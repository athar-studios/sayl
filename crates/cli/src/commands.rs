//! Handlers for each CLI subcommand.

use sayl_app::Engine;
use sayl_core::id::TaskId;
use sayl_core::state::DownloadState;

use crate::args::{AddArgs, Cli, EventsArgs, IdArg, RemoveArgs, StartArgs};
use crate::exit::CliExit;
use crate::messages::{self, Language};
use crate::output::{self, TaskInfoOutput};
use crate::progress::ProgressTracker;

/// Resolves a full or prefix TaskId against current store tasks.
pub fn resolve_task_id(engine: &Engine, prefix: &str, lang: Language) -> Result<TaskId, CliExit> {
    let tasks = engine.list_tasks().map_err(|e| {
        eprintln!("{e}");
        CliExit::StoreIoError
    })?;

    let prefix_clean = prefix.trim();
    let prefix_lower = prefix_clean.to_lowercase();

    // Check exact match first
    if let Some(exact) = tasks
        .iter()
        .find(|t| t.id.to_string().eq_ignore_ascii_case(prefix_clean))
    {
        return Ok(exact.id);
    }

    let matching: Vec<_> = tasks
        .into_iter()
        .filter(|t| t.id.to_string().to_lowercase().starts_with(&prefix_lower))
        .collect();

    match matching.len() {
        0 => {
            eprintln!("{}", messages::task_not_found(prefix, lang));
            Err(CliExit::UsageError)
        }
        1 => Ok(matching[0].id),
        _ => {
            let ids: Vec<String> = matching.iter().map(|t| t.id.to_string()).collect();
            eprintln!("{}", messages::ambiguous_prefix(prefix, &ids, lang));
            Err(CliExit::UsageError)
        }
    }
}

/// Handles `sayl add <URL>...`
pub async fn handle_add(engine: &Engine, args: &AddArgs, cli: &Cli) -> Result<CliExit, CliExit> {
    let mut added_ids = Vec::with_capacity(args.urls.len());
    for url in &args.urls {
        match engine.add(url).await {
            Ok(id) => {
                added_ids.push(id);
                if !cli.json {
                    println!("{id}");
                    if !cli.quiet {
                        eprintln!("{}", messages::task_added(id, cli.lang));
                    }
                }
            }
            Err(err) => {
                eprintln!("Error adding URL '{url}': {err}");
                return Err(CliExit::from(err));
            }
        }
    }

    if cli.json {
        let json = serde_json::to_string_pretty(&added_ids).map_err(|e| {
            eprintln!("{e}");
            CliExit::StoreIoError
        })?;
        println!("{json}");
    }

    Ok(CliExit::Success)
}

/// Handles `sayl start <ID>` or `sayl start --all`
pub async fn handle_start(
    engine: &Engine,
    args: &StartArgs,
    cli: &Cli,
) -> Result<CliExit, CliExit> {
    if args.all {
        return handle_start_all(engine, cli).await;
    }

    let id_str = match &args.id {
        Some(s) => s.as_str(),
        None => {
            eprintln!("Error: Task ID is required unless --all is specified");
            return Err(CliExit::UsageError);
        }
    };

    let task_id = resolve_task_id(engine, id_str, cli.lang)?;

    let record = engine
        .get_task(task_id)
        .map_err(|e| {
            eprintln!("{e}");
            CliExit::StoreIoError
        })?
        .ok_or_else(|| {
            eprintln!("{}", messages::task_not_found(id_str, cli.lang));
            CliExit::UsageError
        })?;

    if record.state.is_terminal() {
        return match record.state {
            DownloadState::Completed => Ok(CliExit::Success),
            DownloadState::Cancelled => Ok(CliExit::Cancelled),
            DownloadState::Failed => Ok(CliExit::TaskFailed),
            _ => unreachable!(),
        };
    }

    if !cli.quiet && !cli.json {
        eprintln!("{}", messages::task_started(task_id, cli.lang));
    }

    let mut event_rx = engine.take_event_receiver();
    let mut tracker = ProgressTracker::new(task_id, cli.quiet, cli.json);

    let start_handle = engine.spawn_start(task_id);
    tokio::pin!(start_handle);

    let start_result: Option<Result<(), sayl_core::error::CoreError>>;

    if let Some(rx) = &mut event_rx {
        loop {
            tokio::select! {
                join_res = &mut start_handle => {
                    match join_res {
                        Ok(res) => start_result = Some(res),
                        Err(join_err) => {
                            eprintln!("Task execution error: {join_err}");
                            start_result = Some(Err(sayl_core::error::CoreError::internal(join_err.to_string())));
                        }
                    }
                    break;
                }
                event_res = rx.recv() => {
                    match event_res {
                        Some(event) => {
                            tracker.handle_event(&event);
                        }
                        None => {
                            let res = start_handle.await;
                            match res {
                                Ok(r) => start_result = Some(r),
                                Err(e) => start_result = Some(Err(sayl_core::error::CoreError::internal(e.to_string()))),
                            }
                            break;
                        }
                    }
                }
            }
        }
    } else {
        let res = start_handle.await;
        match res {
            Ok(r) => start_result = Some(r),
            Err(e) => {
                start_result = Some(Err(sayl_core::error::CoreError::internal(e.to_string())))
            }
        }
    }

    tracker.finish();

    // Query terminal state directly from store
    let final_record = engine.get_task(task_id).map_err(|e| {
        eprintln!("{e}");
        CliExit::StoreIoError
    })?;

    if let Some(rec) = final_record {
        match rec.state {
            DownloadState::Completed => {
                if !cli.quiet && !cli.json {
                    eprintln!("{}", messages::task_completed(task_id, cli.lang));
                }
                Ok(CliExit::Success)
            }
            DownloadState::Cancelled => {
                if !cli.quiet && !cli.json {
                    eprintln!("{}", messages::task_cancelled(task_id, cli.lang));
                }
                Ok(CliExit::Cancelled)
            }
            DownloadState::Failed => {
                if let Some(err) = &rec.error {
                    if !cli.quiet && !cli.json {
                        eprintln!("{}", messages::task_failed(task_id, err, cli.lang));
                    }
                }
                Ok(CliExit::TaskFailed)
            }
            _ => {
                if let Some(Err(e)) = start_result {
                    if !cli.quiet && !cli.json {
                        eprintln!(
                            "{}",
                            messages::task_failed(task_id, &e.to_string(), cli.lang)
                        );
                    }
                    Ok(CliExit::TaskFailed)
                } else {
                    Ok(CliExit::TaskFailed)
                }
            }
        }
    } else {
        Ok(CliExit::StoreIoError)
    }
}

/// Handles `sayl start --all`
async fn handle_start_all(engine: &Engine, cli: &Cli) -> Result<CliExit, CliExit> {
    let tasks = engine.list_tasks().map_err(|e| {
        eprintln!("{e}");
        CliExit::StoreIoError
    })?;

    let queued: Vec<TaskId> = tasks
        .into_iter()
        .filter(|t| t.state == DownloadState::Queued)
        .map(|t| t.id)
        .collect();

    if queued.is_empty() {
        if !cli.quiet && !cli.json {
            eprintln!("{}", messages::empty_task_list(cli.lang));
        }
        return Ok(CliExit::Success);
    }

    let mut handles = Vec::new();
    for id in &queued {
        handles.push((*id, engine.spawn_start(*id)));
    }

    let mut any_failed = false;
    let mut any_cancelled = false;

    for (id, handle) in handles {
        match handle.await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                if !cli.quiet && !cli.json {
                    eprintln!("{}", messages::task_failed(id, &err.to_string(), cli.lang));
                }
                any_failed = true;
            }
            Err(e) => {
                eprintln!("Task {id} failed to join: {e}");
                any_failed = true;
            }
        }
    }

    for id in queued {
        if let Ok(Some(rec)) = engine.get_task(id) {
            if rec.state == DownloadState::Failed {
                any_failed = true;
            } else if rec.state == DownloadState::Cancelled {
                any_cancelled = true;
            }
        }
    }

    if any_failed {
        Ok(CliExit::TaskFailed)
    } else if any_cancelled {
        Ok(CliExit::Cancelled)
    } else {
        Ok(CliExit::Success)
    }
}

/// Handles `sayl list`
pub fn handle_list(engine: &Engine, cli: &Cli) -> Result<CliExit, CliExit> {
    let tasks = engine.list_tasks().map_err(|e| {
        eprintln!("{e}");
        CliExit::StoreIoError
    })?;

    if cli.json {
        let json = serde_json::to_string_pretty(&tasks).map_err(|e| {
            eprintln!("{e}");
            CliExit::StoreIoError
        })?;
        println!("{json}");
    } else {
        if tasks.is_empty() && !cli.quiet {
            eprintln!("{}", messages::empty_task_list(cli.lang));
        } else {
            output::print_tasks_table(&tasks);
        }
    }

    Ok(CliExit::Success)
}

/// Handles `sayl info <ID>`
pub fn handle_info(engine: &Engine, args: &IdArg, cli: &Cli) -> Result<CliExit, CliExit> {
    let task_id = resolve_task_id(engine, &args.id, cli.lang)?;

    let record = engine
        .get_task(task_id)
        .map_err(|e| {
            eprintln!("{e}");
            CliExit::StoreIoError
        })?
        .ok_or_else(|| {
            eprintln!("{}", messages::task_not_found(&args.id, cli.lang));
            CliExit::UsageError
        })?;

    let checkpoint = engine.load_checkpoint(task_id).map_err(|e| {
        eprintln!("{e}");
        CliExit::StoreIoError
    })?;

    let ranges = checkpoint.map(|c| c.range_map).unwrap_or_default();

    if cli.json {
        let out = TaskInfoOutput {
            record,
            checkpoint_ranges: ranges,
        };
        let json = serde_json::to_string_pretty(&out).map_err(|e| {
            eprintln!("{e}");
            CliExit::StoreIoError
        })?;
        println!("{json}");
    } else {
        output::print_task_info(&record, &ranges);
    }

    Ok(CliExit::Success)
}

/// Handles `sayl pause <ID>`
pub async fn handle_pause(engine: &Engine, args: &IdArg, cli: &Cli) -> Result<CliExit, CliExit> {
    let task_id = resolve_task_id(engine, &args.id, cli.lang)?;

    engine.pause(task_id).await.map_err(|e| {
        eprintln!("{e}");
        CliExit::from(e)
    })?;

    if !cli.quiet && !cli.json {
        eprintln!("{}", messages::task_paused(task_id, cli.lang));
    }

    Ok(CliExit::Success)
}

/// Handles `sayl cancel <ID>`
pub async fn handle_cancel(engine: &Engine, args: &IdArg, cli: &Cli) -> Result<CliExit, CliExit> {
    let task_id = resolve_task_id(engine, &args.id, cli.lang)?;

    let record = engine
        .get_task(task_id)
        .map_err(|e| {
            eprintln!("{e}");
            CliExit::StoreIoError
        })?
        .ok_or_else(|| {
            eprintln!("{}", messages::task_not_found(&args.id, cli.lang));
            CliExit::UsageError
        })?;

    if record.state == DownloadState::Cancelled {
        if !cli.quiet && !cli.json {
            eprintln!("{}", messages::task_already_cancelled(task_id, cli.lang));
        }
        return Ok(CliExit::Success);
    }

    engine.cancel(task_id).await.map_err(|e| {
        eprintln!("{e}");
        CliExit::from(e)
    })?;

    if !cli.quiet && !cli.json {
        eprintln!("{}", messages::task_cancelled(task_id, cli.lang));
    }

    Ok(CliExit::Cancelled)
}

/// Handles `sayl remove <ID> [--delete-file]`
pub fn handle_remove(engine: &Engine, args: &RemoveArgs, cli: &Cli) -> Result<CliExit, CliExit> {
    let task_id = resolve_task_id(engine, &args.id, cli.lang)?;

    let record = engine
        .get_task(task_id)
        .map_err(|e| {
            eprintln!("{e}");
            CliExit::StoreIoError
        })?
        .ok_or_else(|| {
            eprintln!("{}", messages::task_not_found(&args.id, cli.lang));
            CliExit::UsageError
        })?;

    if args.delete_file {
        let final_path = record.save_path.join(&record.filename);
        let part_path = record.save_path.join(format!("{}.part", record.filename));
        if final_path.exists() {
            let _ = std::fs::remove_file(&final_path);
        }
        if part_path.exists() {
            let _ = std::fs::remove_file(&part_path);
        }
    }

    engine.delete_task(task_id).map_err(|e| {
        eprintln!("{e}");
        CliExit::from(e)
    })?;

    if !cli.quiet && !cli.json {
        eprintln!("{}", messages::task_removed(task_id, cli.lang));
    }

    Ok(CliExit::Success)
}

/// Handles `sayl recover`
pub async fn handle_recover(engine: &Engine, cli: &Cli) -> Result<CliExit, CliExit> {
    let updated = engine.recover().await.map_err(|e| {
        eprintln!("{e}");
        CliExit::StoreIoError
    })?;

    if updated.is_empty() {
        if !cli.quiet && !cli.json {
            eprintln!("{}", messages::no_tasks_to_recover(cli.lang));
        }
    } else {
        if !cli.quiet && !cli.json {
            eprintln!("{}", messages::recover_summary(updated.len(), cli.lang));
            for id in &updated {
                if let Ok(Some(rec)) = engine.get_task(*id) {
                    eprintln!("  {} -> {}", rec.id, rec.state.as_str());
                }
            }
        }
    }

    Ok(CliExit::Success)
}

/// Handles `sayl events [--idle-exit <ms>]`
pub async fn handle_events(
    engine: &Engine,
    args: &EventsArgs,
    _cli: &Cli,
) -> Result<CliExit, CliExit> {
    let mut rx = engine.take_event_receiver().ok_or_else(|| {
        eprintln!("Error: Engine event channel has already been taken");
        CliExit::StoreIoError
    })?;

    let idle_duration = args.idle_exit.map(std::time::Duration::from_millis);

    loop {
        if let Some(dur) = idle_duration {
            tokio::select! {
                event_res = rx.recv() => {
                    match event_res {
                        Some(event) => {
                            let json = serde_json::to_string(&event).map_err(|e| {
                                eprintln!("{e}");
                                CliExit::StoreIoError
                            })?;
                            println!("{json}");
                        }
                        None => break,
                    }
                }
                _ = tokio::time::sleep(dur) => {
                    break;
                }
                _ = tokio::signal::ctrl_c() => {
                    break;
                }
            }
        } else {
            tokio::select! {
                event_res = rx.recv() => {
                    match event_res {
                        Some(event) => {
                            let json = serde_json::to_string(&event).map_err(|e| {
                                eprintln!("{e}");
                                CliExit::StoreIoError
                            })?;
                            println!("{json}");
                        }
                        None => break,
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    break;
                }
            }
        }
    }

    Ok(CliExit::Success)
}
