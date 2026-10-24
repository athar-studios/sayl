use std::env;
use std::process;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    let mut size: usize = 32 * 1024 * 1024; // Default: 32 MiB
    let mut scenario: Option<String> = None;
    let mut json_output = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--size" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --size requires a byte count argument");
                    process::exit(1);
                }
                match args[i].parse::<usize>() {
                    Ok(s) => size = s,
                    Err(e) => {
                        eprintln!("Error: invalid size '{}': {}", args[i], e);
                        process::exit(1);
                    }
                }
            }
            "--scenario" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --scenario requires a scenario name");
                    process::exit(1);
                }
                scenario = Some(args[i].clone());
            }
            "--json" => {
                json_output = true;
            }
            "-h" | "--help" => {
                println!("SAYL Performance Benchmark (PERF-001)\n");
                println!("Usage: sayl-bench [OPTIONS]\n");
                println!("Options:");
                println!("  --size <BYTES>       Payload size in bytes (default: 33554432 / 32 MiB)");
                println!("  --scenario <NAME>    Specific scenario: all, sequential, parallel, parallel_4, parallel_8, range, resume, memory");
                println!("  --json               Output machine-readable JSON");
                println!("  -h, --help           Print help");
                process::exit(0);
            }
            other => {
                eprintln!("Error: unknown argument '{}'", other);
                eprintln!("Run with --help for usage information");
                process::exit(1);
            }
        }
        i += 1;
    }

    match sayl_bench::run_all_benchmarks(size, scenario.as_deref()).await {
        Ok(report) => {
            if json_output {
                println!("{}", report.to_json());
            } else {
                print!("{}", report.to_table());
            }
        }
        Err(e) => {
            eprintln!("Benchmark failure: {}", e);
            process::exit(1);
        }
    }
}
