# nbtq

[jq](https://github.com/jqlang/jq) for NBT and SNBT, powered by [jaq](https://github.com/01mf02/jaq).

## Installation

```
cargo install --git http://github.com/Vonr/nbtq --locked

# Using https://github.com/cargo-bins/cargo-binstall
cargo binstall --git https://github.com/Vonr/nbtq nbtq-cli --locked
```

## Usage

```
Usage: nbtq [OPTIONS] [CODE] [PATH]

Arguments:
  [CODE]  [default: .]
  [PATH]

Options:
      --color <COLOR>           [possible values: auto, always, never]
  -p, --pretty <PRETTY>         Whether to format output in a human-readable fashion [default: true] [possible values: true, false]
  -r, --raw                     Output top-level strings without escapes or quotes
  -Q, --quote <QUOTE_STRINGS>   Whether to quote strings [default: auto] [possible values: auto, always, never]
      --no-suffix               Don't suffix numbers with their types
      --no-prefix               Don't prefix primitive arrays with their types
  -o, --output <OUTPUT>         Output file
  -f, --format <OUTPUT_FORMAT>  Output file format [possible values: input, nbt, snbt, gz1, gz2, gz3, gz4, gz5, gz, gz7, gz8, gz9]
  -h, --help                    Print help
  -V, --version                 Print version
```
