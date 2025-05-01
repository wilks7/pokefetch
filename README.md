# Pokefetch  
**by discomanfulanito — for everyone, as code should be**

This script fetches a random Pokémon sprite and displays it alongside system info using a fetcher (currently only works with `fastfetch`). Sprites are dynamically centered for clean terminal aesthetics.
Easily configurable padding. Even special forms of Pokémon are available

---

## Requirements

- [`pokeget-rs`](https://github.com/talwat/pokeget-rs) (for fetching Pokémon sprites)  
- [`fastfetch`](https://github.com/fastfetch-cli/fastfetch) (only supported fetcher so far)  
- Bash (or any POSIX-compatible shell)

---

## Configuration

You can customize the following inside the script:

```bash
POKEMON_LIST=(victini "mimikyu -s" celebi furret "mewtwo --mega-y")
FETCHER="fastfetch" # only neofetch is currently supported

EXTRA_PADDING_H=2
EXTRA_PADDING_W=0
WIDTH=38
```

- `POKEMON_LIST`: List of Pokémon to choose from (check `pokeget` for more info)
- `EXTRA_PADDING_H`: Additional vertical padding  
- `EXTRA_PADDING_W`: Additional horizontal padding  
- `WIDTH`: Maximum sprite display width

---

## Installation

Copy the code and paste it in your .bashrc, that simple.

But remember to make sure `pokeget` and `fastfetch` are installed and available in your `PATH`.

---

## License
Pokeget is licensed under the **GNU General Public License v3.0**.

It uses external tools (`pokeget`, `neofetch`) which are licensed under MIT,
but this project does not include or modify their source code.
