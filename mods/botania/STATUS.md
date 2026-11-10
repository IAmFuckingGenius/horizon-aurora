# Status

## Done

- `mods/botania/` layout, `LICENSE` (Botania's `LICENSE.txt`, verbatim, sha256 `e71cfc14…`),
  `bundle/NOTICE.md` (credit to Vazkii, modified port), README.
- Root `Cargo.toml` excludes `mods/botania`: Horizon's workspace never builds it.
- `botania-core` (`bundle/core`, std only):
  - `consts`: Appendix B numbers (spreader variants, pools, link ranges, item delays, Endoflame,
    Pure Daisy, apothecary, petal colours);
  - `mth`: 1.20.1 `Mth.sin/cos` (float table), `floor/frac/sign/lerp`, `Direction`
    (`getNearest`, `orderedByNearest[0]` for spreader placement);
  - `json`: dependency-free JSON reader/writer (key order kept) for 1.20 recipe files.

## Verified

- `cargo test` of `botania-core`: 6 passed, 0 failed (2026-11-10). Note: in a git worktree nested
  inside the main checkout, cargo finds the outer `Cargo.toml`; test a copy outside the repo then.
- No engine build yet: the workflow `android-native-engine.yml` was not on Aurora `main`
  (still on branch `mixins-attr`, with the `lib.rs` `cfg_attr` fix 992b722384).

## Next

1. Core: 1.20 recipe parser (`petal_apothecary`, `pure_daisy`, `state_copying_pure_daisy`,
   `mana_infusion`, unwrap `gog_alternation` → `base`, `nbt_output_wrapper`, `twig_wand` → 26.3
   `crafting_shaped`); petal greedy-multiset matching; infusion catalyst priority.
2. Core: burst (velocity `calculateBurstVelocity`, spawn `y+0.5-1/1024`, vanilla `clip`
   traverse + AABB clip, mana loss, death on tick 99 / 19.8 blocks), receiver scan (400 steps),
   gametest `spreader_pool_default_aim` (pool gets mana by tick 3), aiming (`bindTo`, wand
   function aim, placement), pool/spreader transfer, comparator, Endoflame (coal 1600 → 1197/1200
   mana by tick parity), Pure Daisy (1193 ticks).
3. Bundle v1 (`bundle/horizon-mixin.toml`, named hooks only, hooks API ≤ 415): HEAD on `mods`
   (row "Botania"), `block_entity` / `block_entity_ticks` / `block_entity_tick` for
   `botania:mana_pool`, `[[crate]] botania-core`, `[[module]]` in `crates/horizon-client/src/game.rs`
   (not `lib.rs`: its cfg'd glob import blocks modules). Then dispatch the engine workflow
   (`abis=arm64-v8a`, `publish=false`) once it is on `main`.

## Notes for the bundle

- Hook signatures (Aurora `mixins-attr`): `native_mods(&mut self, mods: &mut Vec<NativeMod>)`,
  `native_block_entity(&mut self, block: &str, entity: &mut Option<(String, Compound)>)`,
  `native_block_entity_ticks(&self, block: &str, ticks: &mut bool)`,
  `native_block_entity_tick(&mut self, pos: [i32; 3], entity: &mut ModBlockEntity)`,
  `native_level_tick_post(&mut self, dimension: &str)`. `Compound` = `BTreeMap<JavaString, Tag>`.
- Unwrapped recipes must use their own paths (`recipe/botania:<name>_unwrapped`): the content
  `.hmod`'s file at the same path would override a mod's `native_data_pack_files` entry.
