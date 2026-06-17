//! Local performance probes for OxideDB hot paths.

use oxide_core::event::bus::EventBusConfig;
use oxide_core::{
    BeforeEventContext, BeforeEventHandler, BeforeEventType, EventBus, HandlerMetadata,
    InMemoryEventBus,
};
use rusqlite::{params, Connection};
use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

const DEFAULT_ROWS: usize = 200_000;
const DEFAULT_ITERATIONS: usize = 50_000;
const DEFAULT_LIMIT: usize = 50;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "all".to_string());

    match command.as_str() {
        "all" => {
            run_log_index_benchmark(parse_arg(args.next(), DEFAULT_ROWS)?).await?;
            run_list_pagination_benchmark(DEFAULT_ROWS, DEFAULT_ROWS / 2, DEFAULT_LIMIT)?;
            run_event_dispatch_benchmark(DEFAULT_ITERATIONS).await?;
        }
        "log-index" => {
            run_log_index_benchmark(parse_arg(args.next(), DEFAULT_ROWS)?).await?;
        }
        "list-pagination" => {
            let rows = parse_arg(args.next(), DEFAULT_ROWS)?;
            let offset = parse_arg(args.next(), rows / 2)?;
            let limit = parse_arg(args.next(), DEFAULT_LIMIT)?;
            run_list_pagination_benchmark(rows, offset, limit)?;
        }
        "event-dispatch" => {
            run_event_dispatch_benchmark(parse_arg(args.next(), DEFAULT_ITERATIONS)?).await?;
        }
        _ => print_usage(),
    }

    Ok(())
}

fn parse_arg(raw: Option<String>, default: usize) -> Result<usize, Box<dyn Error>> {
    match raw {
        Some(value) => Ok(value.parse::<usize>()?),
        None => Ok(default),
    }
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  cargo run -p oxide-benchmarks -- all");
    eprintln!("  cargo run -p oxide-benchmarks -- log-index [rows]");
    eprintln!("  cargo run -p oxide-benchmarks -- list-pagination [rows] [offset] [limit]");
    eprintln!("  cargo run -p oxide-benchmarks -- event-dispatch [iterations]");
}

async fn run_log_index_benchmark(rows: usize) -> Result<(), Box<dyn Error>> {
    let conn = Connection::open_in_memory()?;
    create_log_schema(&conn)?;
    seed_log_entries(&conn, rows)?;

    let cutoff = rows.saturating_sub(rows / 10) as i64;
    let query = "SELECT COUNT(*) FROM log_entries WHERE level = 0 AND timestamp >= ?1";

    let before_plan = explain_query_plan(&conn, query, cutoff)?;
    let before = time_count_query(&conn, query, cutoff)?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_log_entries_level_timestamp ON log_entries(level, timestamp)",
        [],
    )?;

    let after_plan = explain_query_plan(&conn, query, cutoff)?;
    let after = time_count_query(&conn, query, cutoff)?;

    println!("log-index rows={rows} cutoff={cutoff}");
    println!("  before: {:>8.3} ms | {before_plan}", before);
    println!("  after:  {:>8.3} ms | {after_plan}", after);
    Ok(())
}

fn create_log_schema(conn: &Connection) -> Result<(), Box<dyn Error>> {
    conn.execute_batch(
        r#"
        CREATE TABLE log_entries (
            id TEXT PRIMARY KEY,
            correlation_id TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            level INTEGER NOT NULL,
            message TEXT NOT NULL,
            module TEXT NOT NULL,
            location TEXT,
            context_json TEXT NOT NULL,
            error_info TEXT,
            stack_trace TEXT,
            metrics_json TEXT,
            created_at INTEGER NOT NULL DEFAULT (unixepoch())
        );
        CREATE INDEX idx_log_entries_timestamp ON log_entries(timestamp DESC);
        CREATE INDEX idx_log_entries_level ON log_entries(level);
        "#,
    )?;
    Ok(())
}

fn seed_log_entries(conn: &Connection, rows: usize) -> Result<(), Box<dyn Error>> {
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO log_entries (
                id, correlation_id, timestamp, level, message, module, context_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for row in 0..rows {
            let level = if row % 20 == 0 { 0 } else { 2 };
            stmt.execute(params![
                format!("id-{row:08}"),
                format!("corr-{row:08}"),
                row as i64,
                level,
                "synthetic log entry",
                "bench",
                "{}"
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

fn explain_query_plan(
    conn: &Connection,
    query: &str,
    cutoff: i64,
) -> Result<String, Box<dyn Error>> {
    let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {query}"))?;
    let rows = stmt.query_map([cutoff], |row| row.get::<_, String>(3))?;
    let mut details = Vec::new();
    for row in rows {
        details.push(row?);
    }
    Ok(details.join(" | "))
}

fn time_count_query(conn: &Connection, query: &str, cutoff: i64) -> Result<f64, Box<dyn Error>> {
    let started = Instant::now();
    let count: i64 = conn.query_row(query, [cutoff], |row| row.get(0))?;
    std::hint::black_box(count);
    Ok(started.elapsed().as_secs_f64() * 1_000.0)
}

fn run_list_pagination_benchmark(
    rows: usize,
    offset: usize,
    limit: usize,
) -> Result<(), Box<dyn Error>> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(
        r#"
        CREATE TABLE records (
            id TEXT PRIMARY KEY,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            title TEXT NOT NULL
        );
        CREATE INDEX idx_records_created_at ON records(created_at);
        "#,
    )?;
    seed_records(&conn, rows)?;

    let exact_query =
        "SELECT COUNT(*) OVER() AS total_count, * FROM records ORDER BY created_at ASC LIMIT ?1 OFFSET ?2";
    let fast_query = "SELECT * FROM records ORDER BY created_at ASC LIMIT ?1 OFFSET ?2";

    let exact_plan = explain_limit_query_plan(&conn, exact_query, limit, offset)?;
    let exact = time_page_query(&conn, exact_query, limit, offset)?;
    let fast_plan = explain_limit_query_plan(&conn, fast_query, limit + 1, offset)?;
    let fast = time_page_query(&conn, fast_query, limit + 1, offset)?;

    println!("list-pagination rows={rows} offset={offset} limit={limit}");
    println!("  exact-total: {:>8.3} ms | {exact_plan}", exact);
    println!("  limit+1:     {:>8.3} ms | {fast_plan}", fast);
    Ok(())
}

fn seed_records(conn: &Connection, rows: usize) -> Result<(), Box<dyn Error>> {
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO records (id, created_at, updated_at, title)
             VALUES (?1, ?2, ?3, ?4)",
        )?;
        for row in 0..rows {
            stmt.execute(params![
                format!("id-{row:08}"),
                row as i64,
                row as i64,
                format!("title {row}")
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

fn explain_limit_query_plan(
    conn: &Connection,
    query: &str,
    limit: usize,
    offset: usize,
) -> Result<String, Box<dyn Error>> {
    let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {query}"))?;
    let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
        row.get::<_, String>(3)
    })?;
    let mut details = Vec::new();
    for row in rows {
        details.push(row?);
    }
    Ok(details.join(" | "))
}

fn time_page_query(
    conn: &Connection,
    query: &str,
    limit: usize,
    offset: usize,
) -> Result<f64, Box<dyn Error>> {
    let started = Instant::now();
    let mut stmt = conn.prepare(query)?;
    let rows = stmt.query_map(params![limit as i64, offset as i64], |_| Ok(()))?;
    let mut count = 0usize;
    for row in rows {
        row?;
        count += 1;
    }
    std::hint::black_box(count);
    Ok(started.elapsed().as_secs_f64() * 1_000.0)
}

async fn run_event_dispatch_benchmark(iterations: usize) -> Result<(), Box<dyn Error>> {
    let default = time_event_dispatch("default", InMemoryEventBus::new(), iterations).await?;
    let testing_config = time_event_dispatch(
        "testing-config",
        InMemoryEventBus::with_config(EventBusConfig::testing()),
        iterations,
    )
    .await?;

    println!("event-dispatch iterations={iterations}");
    println!("  default:        {:>8.3} us/dispatch", default);
    println!("  testing-config: {:>8.3} us/dispatch", testing_config);
    Ok(())
}

async fn time_event_dispatch(
    label: &str,
    bus: InMemoryEventBus,
    iterations: usize,
) -> Result<f64, Box<dyn Error>> {
    let bus = Arc::new(bus);
    let handler: BeforeEventHandler =
        Arc::new(|_context: &mut BeforeEventContext| Box::pin(async { Ok(()) }));

    bus.subscribe_before(
        BeforeEventType::RecordCreate.name(),
        handler,
        HandlerMetadata::new(format!("{label}-noop"), "No-op handler".to_string()),
    )
    .await?;

    let started = Instant::now();
    for index in 0..iterations {
        let mut context = BeforeEventContext::new_create(
            "bench".to_string(),
            serde_json::json!({ "index": index }),
        );
        bus.dispatch_before(BeforeEventType::RecordCreate, &mut context)
            .await?;
    }

    Ok(started.elapsed().as_secs_f64() * 1_000_000.0 / iterations as f64)
}
