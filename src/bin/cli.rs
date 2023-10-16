use std::error::Error as StdError;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use clap::Parser;
use graphmat::{belief_prop, heuristics, ObjectMetadata};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The first object file to compare.
    #[arg(short, long)]
    first: PathBuf,
    /// The second object file to compare.
    #[arg(short, long)]
    second: PathBuf,
    /// The path to write the mapping to as a CSV file.
    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn StdError>> {
    let args = Args::parse();

    let lhs = ObjectMetadata::load(args.first)?;
    let rhs = ObjectMetadata::load(args.second)?;

    let res = belief_prop(
        &lhs,
        &rhs,
        [(lhs.entry(), rhs.entry())],
        &heuristics![heuristics::CallOrder, heuristics::RelativeCodeSize],
    );

    let mut out = BufWriter::new(File::create(args.output)?);

    for (x, y) in res.mappings() {
        writeln!(out, "{:#X}, {:#X}", x, y)?;
    }

    Ok(())
}
