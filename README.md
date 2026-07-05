# nbtq

[jq](https://github.com/jqlang/jq) for NBT and SNBT, powered by [jaq](https://github.com/01mf02/jaq).

## Installation

```
cargo install --git http://github.com/Vonr/nbtq --locked
```

## Usage

```
# Subject to change
nbtq '.' < /path/to/nbt.dat
echo '{cant:have,spaces:"in between tokens"}' | nbtq '.'
```
