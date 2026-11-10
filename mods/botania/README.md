# Botania for Horizon Aurora (native mod, work in progress)

A modified port of Vazkii's [Botania](https://github.com/VazkiiMods/Botania) mechanics to Horizon,
under the Botania License (`LICENSE`, verbatim; see `bundle/NOTICE.md`). Blocks, items, textures
and recipes come from a separate content `.hmod`; this folder is the code.

- `bundle/`: the horizon-mixins bundle (the folder the Android engine workflow is given as
  `github:IAmFuckingGenius/horizon-aurora@<commit>:mods/botania/bundle`). It carries its own
  `LICENSE` and `NOTICE.md` because only the bundle folder is distributed.
- `bundle/core/`: `botania-core`, the logic without the game (a `[[crate]]` of the bundle). No
  dependencies. horizon-mixins refuses a bundle crate with `[workspace]` or `Cargo.lock`, so instead
  the root `Cargo.toml` excludes `mods/botania`, and the crate is never part of Horizon's build.
- `bundle/data/recipe/`: Botania's phase 1 crafting recipes as its jar has them (1.20 form). The
  module gives them to the server in 26.3's form (`native_data_pack_files`), because the content
  mod carries them in a form 26.3 can't read.
- `content/`: files to copy over the carried content `.hmod`'s files of the same paths:
  - every block declaration (`data/botania/horizon/block/*.json`, 676 files). 56 are written by
    hand: `horizon-port-pack` put every flower, apothecary and pool on a stone template. Their
    templates, `fluid` property and shapes are Botania's own (`PetalApothecaryBlock`,
    `ManaPoolBlock`, `ManaSpreaderBlock`, `FabricSpecialFlowerBlock`); the blaze block's item
    is furnace fuel, its burn time `data/botania/context_int_provider/cooking/time_blaze_block.json`
    (Botania gives it in a Forge event). The rest, and the hand-written ones' missing
    `properties`, come from `block_properties.py` (below);
  - the 16 mystical flower patches (`data/botania/worldgen/configured_feature/`), placing the
    flower with `minecraft:simple_block` instead of Botania's own `botania:mystical_flower`
    feature type, which Horizon skips. Botania's 5% tall flowers are left out.
- `block_properties.py`: run by hand (`python3 mods/botania/block_properties.py`, then
  `build_content.py --overlay`). It downloads Botania's block table (`BotaniaBlocks.java`,
  `BotaniaFlowerBlocks.java`, `LibBlockNames.java`, `BotaniaBlockFlammability.java` at the pinned
  commit; `--source DIR` reads local copies), follows each block's `BlockBehaviour.Properties`
  through its `copy(...)` chain and writes every declaration's `properties` (hardness, explosion
  resistance, light, tool, map colour, a vanilla block with the `SoundType`, occlusion, random
  ticks), a template of the right kind where the port-pack's wasn't one (stone-like slabs, stairs
  and pillars off the oak ones that burn, flowers on the poppy, glass on glass, the spectral rail on
  a straight rail) and the outlines Botania's classes give. It keeps every key of the hand-written
  declarations (`HAND`) and only adds the `properties` they lack; a second run changes nothing.
- `STATUS.md`: what is done, verified and next.

Design: `.local/modports/botania/design-mechanics.md` (sections 2, 3.1–3.8, 6, appendices A, B).
