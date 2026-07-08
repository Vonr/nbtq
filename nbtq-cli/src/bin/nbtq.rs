use std::io::{IsTerminal, Read, Write};

use anyhow::Context;
pub use anyhow::{Error, Result};
use clap::Parser;
use flate2::write::GzDecoder;
use jaq_core::load::{Arena, File, Loader};
use jaq_core::{Ctx, Vars, data, unwrap_valr};
use nbtq::Val;
use nbtq::nbt::{Nbt, NbtTag};
use nbtq::print::WriterStyles;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(default_value = ".")]
    code: String,
    path: Option<String>,
}

fn main() -> Result<()> {
    let Args { code, path } = Args::parse();

    let mut input = Vec::new();
    if let Some(path) = path
        && path != "-"
    {
        let mut file = std::fs::OpenOptions::new().read(true).open(path)?;
        file.read_to_end(&mut input)?;
    } else {
        std::io::stdin().lock().read_to_end(&mut input)?;
    }

    let input = Nbt::read(&mut input.as_slice())
        .map(|n| NbtTag::Compound(n.root_tag))
        .or_else(|_| {
            let decoded = Vec::new();
            let mut decoder = GzDecoder::new(decoded);
            decoder.write_all(&input)?;
            input = decoder.finish().context("gzip decode failure")?;
            Nbt::read(&mut input.as_slice())
                .map(|n| NbtTag::Compound(n.root_tag))
                .context("failed post-ungzip parse")
        })
        .or_else(|_| {
            std::str::from_utf8(&input)
                .context("failed utf8 check after non-stringified failures")?
                .trim_ascii_end()
                .parse()
                .context("failed snbt parse")
        })
        .context("input should be NBT or SNBT")?;
    let input = Val(input);

    let program = File {
        code: code.as_str(),
        path: (),
    };

    let defs = jaq_core::defs().chain(jaq_std::defs()).chain(nbtq::defs());
    let funs = jaq_core::funs().chain(jaq_std::funs()).chain(nbtq::funs());

    let loader = Loader::new(defs);
    let arena = Arena::default();

    let modules = match loader.load(&arena, program) {
        Ok(modules) => modules,
        Err(errors) => {
            eprintln!("Error loading program:");
            for e in errors {
                eprintln!("- {:?}", e.1);
            }

            std::process::exit(1);
        }
    };

    let filter = match jaq_core::Compiler::default()
        .with_funs(funs)
        .compile(modules)
    {
        Ok(filter) => filter,
        Err(errors) => {
            eprintln!("Error compiling filter:");
            for e in errors {
                eprintln!("- {:?}", e.1);
            }

            std::process::exit(1);
        }
    };

    let ctx = Ctx::<data::JustLut<Val>>::new(&filter.lut, Vars::new([]));
    let out = filter.id.run((ctx, input)).map(unwrap_valr);

    for value in out {
        match value {
            Ok(v) => println!(
                "{}",
                nbtq::print::to_snbt_string(
                    &v.0,
                    nbtq::print::WriterOptions {
                        pretty: true,
                        styles: std::io::stdout().is_terminal().then(WriterStyles::default),
                        ..Default::default()
                    }
                )?
            ),
            Err(e) => eprintln!("{e}"),
        }
    }

    Ok(())
}
