//! CLI for the Argus parallel EVM conflict analyzer.
//!
//! Pipeline: fetch txs -> prefetch state -> parallel simulate -> conflict graph -> report.

use clap::{Parser, Subcommand};
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "argus", version, about = "Parallel EVM conflict analyzer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Analyze a block for transaction conflicts.
    Analyze {
        #[arg(short, long, env = "ARGUS_RPC_URL")]
        rpc_url: String,

        #[arg(short, long)]
        block: u64,

        #[arg(long, default_value_t = false)]
        json: bool,

        /// Skip RPC state prefetch; simulate against EmptyDB.
        #[arg(long, default_value_t = false)]
        dry_run: bool,

        /// Sink output: "ndjson" writes NDJSON to stdout,
        /// "ndjson:/path/to/file" writes to file.
        #[arg(long)]
        sink: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze {
            rpc_url,
            block,
            json,
            dry_run,
            sink,
        } => {
            let t0 = Instant::now();

            tracing::info!(rpc_url = %rpc_url, block, dry_run, "starting analysis");

            // 1. Fetch transactions from RPC.
            let provider = argus_provider::rpc::RpcProvider::connect(&rpc_url).await?;
            use argus_provider::DataProvider;
            let transactions = provider.get_block_transactions(block).await?;
            let t_fetch = t0.elapsed();
            tracing::info!(
                txs = transactions.len(),
                elapsed_ms = t_fetch.as_millis(),
                "fetched block"
            );

            // 2. Simulate.
            let access_lists = if dry_run {
                tracing::info!("dry_run mode: simulating against EmptyDB");
                argus_analyzer::simulator::simulate_batch(transactions.clone()).await?
            } else {
                let prefetcher = argus_provider::Prefetcher::new(provider.into_provider());
                let warm_db = prefetcher.prefetch(block, &transactions).await?;
                argus_analyzer::simulator::simulate_batch_with_state(&warm_db, &transactions)?
            };

            let t_sim = t0.elapsed();
            tracing::info!(
                lists = access_lists.len(),
                elapsed_ms = t_sim.as_millis(),
                "simulation done"
            );

            // Stats.
            let txs_with_accesses = access_lists
                .iter()
                .filter(|al| !al.entries.is_empty())
                .count();
            let total_entries: usize = access_lists.iter().map(|al| al.entries.len()).sum();
            tracing::info!(txs_with_accesses, total_entries, "access list stats");

            // 3. Build conflict graph.
            let graph = argus_analyzer::graph::build_conflict_graph(&access_lists);
            let t_total = t0.elapsed();

            tracing::info!(
                conflicts = graph.len(),
                elapsed_ms = t_total.as_millis(),
                "analysis complete"
            );

            // 4. Build report.
            let report = argus_analyzer::reporter::Report::build(
                block,
                &access_lists,
                &graph,
                t_fetch,
                t_total,
            );

            // 5. Sink output.
            if let Some(ref sink_spec) = sink {
                let (summary, conflicts) = report.to_rows_from_graph(&graph);
                let contention = report.to_contention_events(&graph);

                if sink_spec == "ndjson" {
                    let mut s = argus_analyzer::sink::json_stream::JsonStreamSink::stdout();
                    s.write_summary(&summary)?;
                    s.write_conflicts(&conflicts)?;
                    s.write_contention_events(&contention)?;
                    let n = s.finish()?;
                    tracing::info!(rows = n, "ndjson sink: wrote to stdout");
                } else if let Some(path) = sink_spec.strip_prefix("ndjson:") {
                    let file = std::fs::File::create(path)?;
                    let mut s = argus_analyzer::sink::json_stream::JsonStreamSink::new(file);
                    s.write_summary(&summary)?;
                    s.write_conflicts(&conflicts)?;
                    s.write_contention_events(&contention)?;
                    let n = s.finish()?;
                    tracing::info!(rows = n, path, "ndjson sink: wrote to file");
                } else {
                    eprintln!(
                        "Unknown sink: {}. Use 'ndjson' or 'ndjson:/path'",
                        sink_spec
                    );
                }

                // Still print report to stderr so it's visible.
                eprint!("{}", report.render(&graph));
            } else if json {
                println!("{}", serde_json::to_string_pretty(&graph)?);
            } else {
                print!("{}", report.render(&graph));
            }
        }
    }

    Ok(())
}
