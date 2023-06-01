use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, author, about, long_about = None)]
pub struct Args {
    #[arg(long)]
    pub(crate) config_path: Option<PathBuf>,

    #[arg(long)]
    pub(crate) localhost: bool,
}
