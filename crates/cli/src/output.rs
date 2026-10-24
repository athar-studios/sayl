//! Output formatters for tabular stdout and structured JSON representations.

use serde::Serialize;
use sayl_storage::TaskRecord;
use crate::progress::format_bytes;

/// Detailed task information serialized when `--json` is supplied to `sayl info`.
#[derive(Debug, Serialize)]
pub struct TaskInfoOutput {
    #[serde(flatten)]
    pub record: TaskRecord,
    pub checkpoint_ranges: Vec<(u64, u64)>,
}

/// Prints a formatted table of download tasks to stdout.
pub fn print_tasks_table(tasks: &[TaskRecord]) {
    if tasks.is_empty() {
        return;
    }
    println!("{:<36}  {:<12}  {:>10}  {:>8}  {}", "ID", "State", "Size", "Progress", "Filename");
    println!("{:-<36}  {:-<12}  {:->10}  {:->8}  {:-<20}", "", "", "", "", "");
    for t in tasks {
        let size_str = match t.total_size {
            Some(s) => format_bytes(s),
            None => "-".to_string(),
        };
        let pct_str = match t.total_size {
            Some(total) if total > 0 => {
                format!("{:.1}%", (t.written as f64 / total as f64 * 100.0).min(100.0))
            }
            _ if t.written > 0 => "running".to_string(),
            _ => "0.0%".to_string(),
        };
        println!(
            "{:<36}  {:<12}  {:>10}  {:>8}  {}",
            t.id,
            t.state.as_str(),
            size_str,
            pct_str,
            t.filename
        );
    }
}

/// Prints detailed human-readable task information to stdout.
pub fn print_task_info(record: &TaskRecord, ranges: &[(u64, u64)]) {
    let size_str = match record.total_size {
        Some(s) => format_bytes(s),
        None => "Unknown".to_string(),
    };
    let written_str = format_bytes(record.written);
    let full_path = record.save_path.join(&record.filename);

    let ranges_str = if ranges.is_empty() {
        "None".to_string()
    } else {
        ranges
            .iter()
            .map(|(s, e)| format!("{}-{}", s, e))
            .collect::<Vec<_>>()
            .join(", ")
    };

    println!("ID:          {}", record.id);
    println!("State:       {}", record.state.as_str());
    println!("URL:         {}", record.url);
    if let Some(final_url) = &record.final_url {
        println!("Final URL:   {}", final_url);
    }
    println!("File Path:   {}", full_path.display());
    println!("Filename:    {}", record.filename);
    println!("Bytes:       {} / {}", written_str, size_str);
    println!("Ranges:      {}", ranges_str);
    if let Some(err) = &record.error {
        println!("Error:       {}", err);
    }
    if let Some(etag) = &record.etag {
        println!("ETag:        {}", etag);
    }
    if let Some(lm) = &record.last_modified {
        println!("Modified:    {}", lm);
    }
}
