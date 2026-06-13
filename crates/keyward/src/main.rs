//! Keyward reference Executor + a mock Orchestrator for the end-to-end demo.
//!
//!   keyward demo          run the self-contained end-to-end demo (default)
//!   keyward executor      dial out to an Orchestrator (env-configured)
//!   keyward orchestrator  serve a single-prompt Orchestrator for manual testing

mod demo;
mod executor;
mod identity;
mod orchestrator;
mod pricing;
mod provider;
mod wire;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cmd = std::env::args().nth(1).unwrap_or_else(|| "demo".into());
    match cmd.as_str() {
        "demo" => demo::run().await,
        "resume-demo" => demo::run_resume().await,
        "executor" => executor::run_cli().await,
        "orchestrator" => orchestrator::run_cli().await,
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        other => {
            eprintln!("unknown subcommand: {other}\n");
            print_usage();
            std::process::exit(2);
        }
    }
}

fn print_usage() {
    eprintln!(
        "keyward — non-custodial BYOK executor (v0 skeleton)\n\n\
         USAGE:\n  \
           keyward demo          self-contained end-to-end demo (no key, no network)\n  \
           keyward resume-demo   drop-the-channel resume + cancel demo (§7)\n  \
           keyward executor      dial out to an Orchestrator\n                        \
             env: KEYWARD_ORCH_URL, KEYWARD_PAIRING_TOKEN, KEYWARD_PROVIDER_KEY|OPENAI_API_KEY\n  \
           keyward orchestrator  serve a single-prompt mock Orchestrator\n                        \
             env: KEYWARD_LISTEN, KEYWARD_PAIRING_TOKEN, KEYWARD_PROVIDER, KEYWARD_MODEL, KEYWARD_PROMPT"
    );
}
