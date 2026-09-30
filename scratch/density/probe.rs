use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::Mutex;
use std::time::Instant;

use clap::Parser;
use loopflow::engine::invocation::QueuedInvocation;
use loopflow::engine::{ConcreteSkill, ConcreteStep, Skill};
use loopflow::lf::{commands, Cli, Commands, FlowCommand};
use rusqlite::ffi;
use serde_json::{json, Value};

static QUERIES: Mutex<Vec<String>> = Mutex::new(Vec::new());

unsafe extern "C" fn trace(
    _: u32,
    _: *mut c_void,
    statement: *mut c_void,
    _: *mut c_void,
) -> c_int {
    // SAFETY: SQLite supplies the live statement for SQLITE_TRACE_PROFILE;
    // expanded_sql allocates a nul-terminated string freed by sqlite3_free.
    unsafe {
        let connection = ffi::sqlite3_db_handle(statement.cast());
        let filename = ffi::sqlite3_db_filename(connection, c"main".as_ptr());
        if filename.is_null()
            || CStr::from_ptr(filename).to_string_lossy()
                != std::env::var("LF_DB_PATH").expect("isolated database")
        {
            return 0;
        }
        let sql = ffi::sqlite3_expanded_sql(statement.cast());
        if !sql.is_null() {
            QUERIES
                .lock()
                .expect("query collector")
                .push(CStr::from_ptr(sql).to_string_lossy().into_owned());
            ffi::sqlite3_free(sql.cast());
        }
    }
    0
}

unsafe extern "C" fn extension(
    db: *mut ffi::sqlite3,
    _: *mut *mut c_char,
    _: *const ffi::sqlite3_api_routines,
) -> c_int {
    // SAFETY: SQLite supplies its live connection; callback retains no pointers.
    unsafe {
        ffi::sqlite3_trace_v2(
            db,
            ffi::SQLITE_TRACE_PROFILE,
            Some(trace),
            std::ptr::null_mut(),
        )
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("template") => {
            let invocation = QueuedInvocation::new(
                "density",
                vec![ConcreteStep::Skill(ConcreteSkill {
                    skill: Skill::named("inspect"),
                    sources: vec![],
                    id: None,
                    human: false,
                    repeat: None,
                })],
            )?;
            println!("{}", serde_json::to_string(&invocation)?);
            return Ok(());
        }
        Some("payload") => {
            let payload: Value = serde_json::from_slice(&std::fs::read(&args[2])?)?;
            let mut samples = Vec::new();
            for _ in 0..101 {
                let start = Instant::now();
                std::hint::black_box(serde_json::to_vec_pretty(&payload)?);
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            println!("{}", json!(samples));
            return Ok(());
        }
        Some("sql") => {
            let queries: Vec<String> = serde_json::from_slice(&std::fs::read(&args[3])?)?;
            let db = rusqlite::Connection::open_with_flags(
                &args[2],
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )?;
            let mut samples = Vec::new();
            let mut counts = Vec::new();
            for pass in 0..21 {
                let start = Instant::now();
                for sql in &queries {
                    let mut query = db.prepare(sql)?;
                    let columns = query.column_count();
                    let mut rows = query.query([])?;
                    let mut count = 0;
                    while let Some(row) = rows.next()? {
                        for column in 0..columns {
                            std::hint::black_box(row.get_ref(column)?);
                        }
                        count += 1;
                    }
                    if pass == 0 {
                        counts.push(count);
                    }
                }
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            println!(
                "{}",
                json!({"samples_ms": samples, "rows": counts, "sqlite": rusqlite::version()})
            );
            return Ok(());
        }
        _ => {}
    }
    // SAFETY: the callback has SQLite's documented extension entry signature
    // and remains loaded for this process's lifetime.
    unsafe {
        ffi::sqlite3_auto_extension(Some(extension));
    }
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Session { cmd }) => commands::session::run(&cmd)?,
        Some(Commands::Exec { cmd }) => commands::exec::run(&cmd)?,
        Some(Commands::Flow {
            cmd: FlowCommand::List { inventory, json },
        }) => commands::flow_inventory::list(&inventory, json)?,
        Some(Commands::Flow {
            cmd:
                FlowCommand::Show {
                    name,
                    sessions: true,
                    json,
                },
        }) => commands::flow_inventory::inspect(&name, json)?,
        _ => anyhow::bail!("only read-only inventory commands are supported"),
    }
    eprintln!(
        "{}",
        serde_json::to_string(&*QUERIES.lock().expect("query collector"))?
    );
    Ok(())
}
