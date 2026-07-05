use std::io::{Read, Write};

use anyhow::Context;
use fastnbt::Value;
use flate2::write::GzDecoder;
use jaq_core::load::{Arena, File, Loader};
use jaq_core::{Ctx, Vars, data, unwrap_valr};
use nbtq::Val;

fn main() {
    let mut args = std::env::args();
    let _name = args.next().unwrap();
    let code = args.next().unwrap();

    let mut input = Vec::new();
    std::io::stdin().lock().read_to_end(&mut input).unwrap();

    let input: Value = fastnbt::from_bytes(&input)
        .or_else(|_| {
            let decoded = Vec::new();
            let mut decoder = GzDecoder::new(decoded);
            decoder.write_all(&input)?;
            input = decoder.finish().context("gzip decode failure")?;
            fastnbt::from_bytes(&input).context("failed post-ungzip parse")
        })
        .or_else(|_| {
            fastsnbt::from_str::<Value>(
                std::str::from_utf8(&input)
                    .context("failed utf8 check after non-stringified failures")?
                    .trim_ascii_end(),
            )
            .context("failed snbt parse")
        })
        .expect("input should be NBT or SNBT");
    let input = Val(input);

    let program = File {
        code: code.as_str(),
        path: (),
    };

    let defs = jaq_core::defs().chain(jaq_std::defs()).chain(nbtq::defs());
    let funs = jaq_core::funs().chain(jaq_std::funs()).chain(nbtq::funs());

    let loader = Loader::new(defs);
    let arena = Arena::default();

    let modules = loader.load(&arena, program).unwrap();

    let filter = jaq_core::Compiler::default()
        .with_funs(funs)
        .compile(modules)
        .expect("should compile");

    let ctx = Ctx::<data::JustLut<Val>>::new(&filter.lut, Vars::new([]));
    let out = filter.id.run((ctx, input)).map(unwrap_valr);

    for value in out {
        match value {
            Ok(v) => println!("{v}"),
            Err(e) => eprintln!("{e}"),
        }
    }
}
