use std::io::{IsTerminal, Read, Write};

use anyhow::{Context, bail};
pub use anyhow::{Error, Result};
use clap::builder::PossibleValue;
use clap::{ArgAction, CommandFactory, Parser};
use flate2::Compression;
use flate2::write::{GzDecoder, GzEncoder};
use jaq_core::load::{Arena, Loader};
use jaq_core::{Ctx, Vars, data, unwrap_valr};
use nbtq_core::Val;
use nbtq_core::nbt::{Nbt, NbtTag};
use nbtq_core::print::{QuoteMode, WriterStyles};

#[derive(Parser, Debug)]
#[command(name = "nbtq", version, about, long_about = None)]
struct Args {
    /// [default: .]
    filter: Option<String>,

    /// path to the input or - for stdin
    path: Option<String>,

    // Whether to color the output, overrides $NO_COLOR and $FORCE_COLOR
    #[arg(long = "color", value_enum)]
    color: Option<Ternary>,

    /// Whether to format output in a human-readable fashion
    #[arg(long = "pretty", short = 'p', action = ArgAction::Set, default_value_t = true)]
    pretty: bool,

    /// Output top-level strings without escapes or quotes
    #[arg(long = "raw", short = 'r', default_value_t = false)]
    raw: bool,

    /// Whether to quote strings
    #[arg(long = "quote", short = 'Q', value_enum, default_value_t = Ternary::Auto)]
    quote_strings: Ternary,

    /// Don't suffix numbers with their types
    #[arg(long = "no-suffix", action = ArgAction::SetFalse, default_value_t = true)]
    suffix_numbers: bool,

    /// Don't prefix primitive arrays with their types
    #[arg(long = "no-prefix", action = ArgAction::SetFalse, default_value_t = true)]
    prefix_arrays: bool,

    /// Output file path
    #[arg(long = "output", short = 'o')]
    output: Option<String>,

    /// Output file format
    #[arg(long = "format", short = 'f', value_enum)]
    output_format: Option<OutputFormat>,

    /// Treat [PATH] as SNBT input
    #[arg(long = "args")]
    input_from_args: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ternary {
    Auto,
    Always,
    Never,
}

impl clap::ValueEnum for Ternary {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Auto, Self::Always, Self::Never]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self {
            Self::Auto => Some(PossibleValue::new("auto")),
            Self::Always => Some(PossibleValue::new("always").aliases(["yes", "y", "true"])),
            Self::Never => Some(PossibleValue::new("never").aliases(["no", "n", "false"])),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputFormat {
    Input,
    Snbt,
    Nbt,
    Gzip,
    Gzip1,
    Gzip2,
    Gzip3,
    Gzip4,
    Gzip5,
    Gzip6,
    Gzip7,
    Gzip8,
    Gzip9,
}

impl OutputFormat {
    fn write(self, writer: &mut impl std::io::Write, value: Nbt) -> Result<()> {
        match self {
            OutputFormat::Nbt => Self::write_nbt(writer, value),
            OutputFormat::Snbt => Self::write_snbt(writer, value),
            OutputFormat::Gzip | OutputFormat::Gzip6 => Self::write_gzip(6, writer, value),
            OutputFormat::Gzip1 => Self::write_gzip(1, writer, value),
            OutputFormat::Gzip2 => Self::write_gzip(2, writer, value),
            OutputFormat::Gzip3 => Self::write_gzip(3, writer, value),
            OutputFormat::Gzip4 => Self::write_gzip(4, writer, value),
            OutputFormat::Gzip5 => Self::write_gzip(5, writer, value),
            OutputFormat::Gzip7 => Self::write_gzip(7, writer, value),
            OutputFormat::Gzip8 => Self::write_gzip(8, writer, value),
            OutputFormat::Gzip9 => Self::write_gzip(9, writer, value),
            OutputFormat::Input => unreachable!(),
        }
    }

    fn write_nbt(writer: &mut impl std::io::Write, value: Nbt) -> Result<()> {
        value.write_to_writer(writer)?;
        Ok(())
    }

    fn write_gzip(level: u8, writer: &mut impl std::io::Write, value: Nbt) -> Result<()> {
        let mut encoder = GzEncoder::new(writer, Compression::new(level as u32));
        Self::write_nbt(&mut encoder, value)
    }

    fn write_snbt(writer: &mut impl std::io::Write, value: Nbt) -> Result<()> {
        writer.write_all(value.to_string().as_bytes())?;
        Ok(())
    }
}

impl clap::ValueEnum for OutputFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::Input,
            Self::Nbt,
            Self::Snbt,
            Self::Gzip,
            Self::Gzip1,
            Self::Gzip2,
            Self::Gzip3,
            Self::Gzip4,
            Self::Gzip5,
            Self::Gzip6,
            Self::Gzip7,
            Self::Gzip8,
            Self::Gzip9,
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self {
            Self::Input => Some(PossibleValue::new("input").alias("in")),
            Self::Gzip => Some(PossibleValue::new("gz").alias("gzip")),
            Self::Gzip1 => Some(PossibleValue::new("gz1").alias("gzip1")),
            Self::Gzip2 => Some(PossibleValue::new("gz2").alias("gzip2")),
            Self::Gzip3 => Some(PossibleValue::new("gz3").alias("gzip3")),
            Self::Gzip4 => Some(PossibleValue::new("gz4").alias("gzip4")),
            Self::Gzip5 => Some(PossibleValue::new("gz5").alias("gzip5")),
            Self::Gzip6 => Some(PossibleValue::new("gz6").alias("gzip6")),
            Self::Gzip7 => Some(PossibleValue::new("gz7").alias("gzip7")),
            Self::Gzip8 => Some(PossibleValue::new("gz8").alias("gzip8")),
            Self::Gzip9 => Some(PossibleValue::new("gz9").alias("gzip9")),
            Self::Nbt => Some(PossibleValue::new("nbt").alias("dat")),
            Self::Snbt => Some(PossibleValue::new("snbt")),
        }
    }
}

fn main() -> Result<()> {
    let Args {
        filter,
        path,
        color,
        pretty,
        raw,
        quote_strings,
        suffix_numbers,
        prefix_arrays,
        output,
        mut output_format,
        input_from_args,
    } = Args::parse();

    let mut stdin = std::io::stdin().lock();
    if stdin.is_terminal() && filter.is_none() {
        Args::command().print_help()?;
        return Ok(());
    }

    let filter = filter.unwrap_or_else(|| String::from("."));

    if output.is_none() {
        if output_format.is_some() {
            bail!("`--format`/`-f` can only be used with `--output`/`-o`");
        }
    } else if output_format.is_none() {
        bail!("`--format`/`-f` must be specified for `--output`/`-o`");
    }

    let program = jaq_core::load::File {
        code: filter.as_str(),
        path: (),
    };

    let defs = jaq_core::defs()
        .chain(jaq_std::defs())
        .chain(nbtq_core::defs());
    let funs = jaq_core::funs()
        .chain(jaq_std::funs())
        .chain(nbtq_core::funs());

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

    let mut input_format = OutputFormat::Nbt;
    let (name, input) = if input_from_args {
        let Some(input) = path else {
            bail!("No input given in args.\nUsage: nbtq --args <filter> <input>");
        };

        input_format = OutputFormat::Snbt;
        (
            String::new(),
            input.parse().context("input should be SNBT")?,
        )
    } else {
        let mut input = Vec::new();
        if let Some(path) = path
            && path != "-"
        {
            let mut file = std::fs::OpenOptions::new().read(true).open(path)?;
            file.read_to_end(&mut input)?;
        } else {
            stdin.read_to_end(&mut input)?;
        }

        let nbt = Nbt::read(&mut input.as_slice())
            .or_else(|_| {
                let decoded = Vec::new();
                let mut decoder = GzDecoder::new(decoded);
                decoder.write_all(&input)?;
                input = decoder.finish().context("gzip decode failure")?;
                input_format = OutputFormat::Gzip6;
                Nbt::read(&mut input.as_slice()).context("failed post-ungzip parse")
            })
            .or_else(|_| {
                input_format = OutputFormat::Snbt;
                std::str::from_utf8(&input)
                    .context("failed utf8 check after non-stringified failures")?
                    .trim_ascii_end()
                    .parse()
                    .context("failed snbt parse")
            })
            .context("input should be NBT or SNBT")?;

        (nbt.name, nbt.root_tag)
    };

    if output_format == Some(OutputFormat::Input) {
        output_format = Some(input_format);
    }

    let input = Val(input.into());
    let mut out = match filter
        .id
        .run((ctx, input))
        .map(unwrap_valr)
        .map(|v| v.map(|v| v.0))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(out) => out,
        Err(e) => {
            bail!("Error running filter: {e}");
        }
    };

    let is_terminal = std::io::stdout().is_terminal();
    let mut env_color = is_terminal;
    for var in std::env::vars() {
        match var.0.as_str() {
            "NO_COLOR" => env_color = var.1 == "0",
            "FORCE_COLOR" => env_color = var.1 != "0",
            _ => (),
        }
    }

    let color = match color {
        Some(Ternary::Auto) => is_terminal,
        Some(Ternary::Always) => true,
        Some(Ternary::Never) => false,
        None => env_color,
    };

    let quote_mode = match quote_strings {
        Ternary::Auto => QuoteMode::IfNeeded,
        Ternary::Always => QuoteMode::Always,
        Ternary::Never => QuoteMode::Never,
    };

    let styles = if color {
        Some(WriterStyles::default())
    } else {
        None
    };

    if let Some(path) = output {
        let NbtTag::Compound(tag) = (match out.len() {
            0 => bail!("No values to write"),
            1 => out.remove(0),
            _ => bail!("Too many values to write"),
        }) else {
            bail!("Cannot write a non-Compound value");
        };

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        output_format
            .unwrap()
            .write(&mut file, Nbt::new(name, tag))?;

        return Ok(());
    }

    for v in out {
        let quote_mode = match v {
            NbtTag::String(_) if raw => QuoteMode::Never,
            _ => quote_mode,
        };

        println!(
            "{}",
            nbtq_core::print::to_snbt_string(
                &v,
                nbtq_core::print::WriterOptions {
                    pretty,
                    quote_mode,
                    styles,
                    suffix_numbers,
                    prefix_arrays,
                    ..Default::default()
                }
            )?
        )
    }

    Ok(())
}
