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
- `content/`: corrected block declarations for the content `.hmod`
  (`data/botania/horizon/block/*.json`, 55 files): `horizon-port-pack` put every flower,
  apothecary and pool on a stone template. Copy them over the carried `.hmod`'s files of the same
  names. Their templates, `fluid` property and shapes are Botania's own (`PetalApothecaryBlock`,
  `ManaPoolBlock`, `ManaSpreaderBlock`, `FabricSpecialFlowerBlock`).
- `STATUS.md`: what is done, verified and next.

Design: `.local/modports/botania/design-mechanics.md` (sections 2, 3.1–3.8, 6, appendices A, B).
