use std::error::Error as StdError;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use clap::Parser;
use graphmat::{belief_prop, heuristics, CodeMetadata, ObjectCode};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The file to load initial mappings from.
    #[arg(short, long)]
    seeds: Option<PathBuf>,
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

    let lhs_file = fs::read(args.first)?;
    let lhs_file = object::read::File::parse(&lhs_file[..])?;
    let lhs_file = ObjectCode::load(&lhs_file)?;
    let rhs_file = fs::read(args.second)?;
    let rhs_file = object::read::File::parse(&rhs_file[..])?;
    let rhs_file = ObjectCode::load(&rhs_file)?;

    let seeds = args
        .seeds
        .map(|path| load_seeds(&path, lhs_file.text_section_base(), rhs_file.text_section_base()))
        .transpose()?
        .unwrap_or_default();

    let lhs = CodeMetadata::load(&lhs_file, seeds.iter().map(|&(s, _)| s))?;
    let rhs = CodeMetadata::load(&rhs_file, seeds.iter().map(|&(_, s)| s))?;

    let res = belief_prop(
        &lhs,
        &rhs,
        [(lhs_file.entrypoint(), rhs_file.entrypoint())]
            .into_iter()
            .chain(seeds),
        &heuristics![heuristics::RelativeCodeSize, heuristics::CallOrder],
    );

    let mut out = BufWriter::new(File::create(args.output)?);

    writeln!(
        out,
        "{}",
        res.format(lhs_file.text_section_base(), rhs_file.text_section_base())
    )?;

    Ok(())
}

fn load_seeds(path: &Path, lhs_base: u64, rhs_base: u64) -> Result<Vec<(u64, u64)>, Box<dyn StdError>> {
    let mut seeds = vec![];
    for line in BufReader::new(File::open(path)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let (lhs, rhs) = line.split_once(',').expect("invalid seed file");
        let lhs = u64::from_str_radix(lhs.trim(), 16)? - lhs_base;
        let rhs = u64::from_str_radix(rhs.trim(), 16)? - rhs_base;
        seeds.push((lhs, rhs));
    }
    Ok(seeds)
}
