//! `lfq` — run an `lf` command on the wave server. `lfq run <lf command…>` is
//! `lf <command…>` executed remotely and unsandboxed; see
//! [`loopflow::lf::commands::lfq`]. The top-level namespace is otherwise
//! reserved for future native verbs (`attach`, `ls`, `logs`).

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.split_first() {
        Some((verb, rest)) if verb == "run" => match loopflow::lf::commands::lfq::run(rest) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("lfq: {err:#}");
                1
            }
        },
        Some((verb, _)) => {
            eprintln!("lfq: unknown command '{verb}' — only `lfq run <lf command…>` is supported");
            2
        }
        None => {
            eprintln!("usage: lfq run <lf command…>");
            2
        }
    };
    std::process::exit(code);
}
