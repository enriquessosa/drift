use clap::Parser;
use drift::{app, cli::Cli, collectors};
fn main() {
    match app::run(Cli::parse(), collectors::collect) {
        Ok(output) => print!("{output}"),
        Err(e) => {
            eprintln!("drift: {e:#}");
            std::process::exit(1);
        }
    }
}
