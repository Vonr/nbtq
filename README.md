# nbtq

[jq](https://github.com/jqlang/jq) for NBT and SNBT, powered by [jaq](https://github.com/01mf02/jaq).

## Installation

```
cargo install --git http://github.com/Vonr/nbtq --locked
```

## Usage

```
# Subject to change
nbtq '.' /path/to/nbt.dat
nbtq '.' - < /path/to/nbt.dat
echo '{"compound": ["string", 42b, 42s, 42, 42L, [I; 1, 2, 3, 4]]}' | nbtq '.'
```
