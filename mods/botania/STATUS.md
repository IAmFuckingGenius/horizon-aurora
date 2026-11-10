# Status

Nothing here has run on a device or in a game yet: "builds" below means the Android engine
compiled with the bundle in CI, not that it works in play.

## Done

- `mods/botania/` layout, `LICENSE` (Botania's `LICENSE.txt`, verbatim, sha256 `e71cfc14…`),
  `bundle/NOTICE.md` (credit to Vazkii, modified port), README.
- Root `Cargo.toml` excludes `mods/botania`: Horizon's workspace never builds it.
- `botania-core` (`bundle/core`, std only, no dependencies):
  - `consts`: Appendix B numbers; `mth`: 1.20.1 `Mth.sin/cos` (float table), `floor/frac/sign`,
    `Direction` (`getNearest`, `orderedByNearest[0]`); `json`: dependency-free JSON reader/writer;
  - `recipe`: the 1.20 recipe files as Botania's serializers read them (`petal_apothecary`,
    `pure_daisy`, `state_copying_pure_daisy`, `mana_infusion` with `catalyst`), vanilla
    `Ingredient` and Botania's `StateIngredient` (block, state, tag, blocks, tag_excluding,
    compound), `PetalsRecipe.matches` (greedy multiset), `getMatchingRecipe` (catalyst first),
    and `unwrap`: `gog_alternation` (→ `base`), `nbt_output_wrapper` (→ result with
    `minecraft:custom_data`), `twig_wand` and 1.20 vanilla crafting → 26.3 `crafting_shaped` /
    `crafting_shapeless`;
  - `pool`: `receiveMana` clamp, `isFull` (mana void), comparator (`calculateComparatorLevel`),
    the 10-tick sync, a spreader's draw from a pool beside it;
  - `flower`: `distSqr`, closest target, soil delay and `DelayHelper`, generating flower binding
    (first tick only) and transfer (`emptyManaIntoCollector`), the Endoflame tick;
  - `daisy`: the Pure Daisy's eight positions, one visit a tick, `ticksRemaining`;
  - `burst`: `calculateBurstVelocity`, the spawn point (`y + 0.5 - 1/1024`), vanilla's
    `BlockGetter.clip` traversal with `VoxelShape.clip`/`AABB.clip`, `onHitBlock` (same block
    twice, passing leaves/bushes, the source until left, receiver delivery, death), mana loss,
    the 400-step receiver scan with its path, `bindTo` aiming and the placement rotation;
  - `apothecary`: `collideEntityItem` (fill from buckets, lava, craft with the reagent, take one
    item, the remembered reagent keeping seeds out), `tickRecipeKeep`, comparator.
- Bundle (`bundle/horizon-mixin.toml`): named hook points only, no pin, no `[api]`:
  - `[[module]] botania` in `crates/horizon-client/src/game.rs` (`runtime.rs`),
    `[[crate]] core` (`botania-core`), `data/recipe/` (25 files);
  - hooks: `mods` (row "Botania 0.1.0"), `block_entity`, `block_entity_container`,
    `block_entity_ticks`, `block_entity_tick`, `block_entity_update_tag`, `level_tick_post`,
    `has_analog_output`, `analog_output`, `data_pack_files`;
  - mana pools (`mana_pool`, `diluted_pool`, `creative_pool`, `fabulous_pool`): block entity
    `botania:mana_pool`, `mana`/`manaCap` in its NBT, capacity per variant, sync every 10 ticks
    when the mana changed, comparator output (cached while the pool ticks; neighbours told in
    `level_tick_post`, when the block entity is back in the world);
  - Endoflame: block entity `botania:endoflame` (`ticksExisted`, `mana`, `binding`, `burnTime`),
    Botania's tick through `botania-core`: item age from the item entity (`age`), furnace burn
    time from the game's fuel table, crafting remainders and spreaders skipped, sound
    `botania:endoflame`;
  - Petal Apothecary (12 variants): block entity `botania:altar` with 16 slots (a mod container,
    no faces, no menu), `fluid` from the block's property when the block has one, else from the
    block entity's NBT; recipes from `native_recipes_of_type("botania:petal_apothecary")`;
  - Pure Daisy: block entity `botania:pure_daisy` (`position`, `ticksRemaining0..7`), recipes of
    both daisy types, state copying through the block registry;
  - mana spreaders (4 variants, no lenses/padding): block entity `botania:mana_spreader`
    (`mana`, `rotationX`, `rotationY`, `canShootBurst`), rotation facing the placer (`place_block`
    hook, applied on the first tick), draw from adjacent pools except the receiver, redstone
    (`native_neighbor_signal`; the redstone variant on a rising edge), receiver scan with
    `botania-core::burst::scan_receiver` over `native_block_collision` (pools as their burst slab,
    `#minecraft:leaves` passed) on aim change and every 10 ticks, one burst in flight per spreader;
  - virtual bursts (design 3.6 variant A): kept in the module, moved in `level_tick_post` from
    the tick after they are shot, delivering to pools with `native_with_mod_block_entity`, lost
    over an unloaded chunk; no particles yet (clients see nothing of a burst);
  - phase 1 crafting recipes (apothecary, pools, livingrock slab, spreader, wand, twig,
    livingwood, petals) given in 26.3's form under `recipe/botania:horizon/<name>`.
- `content/`: 55 corrected block declarations for the content `.hmod` (see README).

## Verified

- `cargo test` of `botania-core` (scratch copy, `CARGO_BUILD_JOBS=2`, nice/ionice): 33 passed,
  0 failed. Expected values are Botania's, not the port's: Endoflame coal → 1197/1200 mana by tick
  parity, Pure Daisy 1193 ticks (first position visited) to 1200, burst death on tick 99 after
  19.8 blocks (elven 139, Gaia 151), gametest `spreader_pool_default_aim` (pool hit by tick 3 for
  all four variants), gametest `ApothecaryRecipeTest.testItemEnterPrevention`, comparator levels,
  catalyst priority, the jar's recipe files.
- All 25 bundle recipes unwrap into 26.3 form (scratch-only test, output checked by eye).
- Engine runs (`android-native-engine.yml`, arm64-v8a, `publish=false`):
  - 42728593487, bundle 162df0be9c: failed in `plan`, `E_INVALID_HELPER` (helper `ticks`
    shadowed the parameter `ticks`); fixed by prefixing every helper with `botania_`.
  - 42729019307, bundle 71a3d55aa7 (Botania row, pools, Endoflame, comparator, recipes):
    **success**. horizon-mixins applied 9 injections, 1 module, 1 crate, 1 data folder; the
    release library built in 18 min 11 s; `.hengine` packed (artifact
    `native-engine-botania-arm64-v8a`, 14 days).
  - RUN3: apothecary, Pure Daisy, spreaders and bursts: RESULT PENDING.

## Not verified / known gaps

- Nothing ran in a game. In particular unchecked: that `native_item_entities` sees items in the
  Endoflame's box, that `native_summon` makes the apothecary's output, that the game drops a mod
  container's slots when the apothecary breaks, the recipes' load under `botania:horizon/`.
- Port deviation: generating flowers bind to the nearest **pool** within 6 blocks, because
  spreaders and bursts aren't in the game yet (Botania binds them to spreaders, `ManaCollector`).
  Switch to spreaders once they exist.
- The carried content mod is wrong for phase 1: every flower, apothecary and pool is a stone cube
  (`template: minecraft:stone`), the apothecary has no `fluid` property, and its crafting recipes
  are in 1.20's form, which Horizon refuses (`result.item`). `content/` and the bundle's recipes
  cover phase 1; `horizon-port-pack carry` should do both itself.
- `native_blocks` can't fix the declarations from code: a block the content mod declares too is
  "declared twice" and the whole namespace is refused (`horizon_mods::Content::declare`).
- Not done in game yet: lava setting items on fire (`Collide::Ignite` does nothing), fish buckets
  releasing their fish, rain filling the apothecary, the apothecary's right click (`trySetLastRecipe`,
  take back the last item, buckets), particles (G1/G2), levelEvent 2001 when the daisy converts,
  floating flowers, Enchanted Soil's double tick.

## Next

1. Get the bundle with the apothecary, the Pure Daisy and the spreaders through an engine build.
2. Bind generating flowers to spreaders as Botania does, once spreaders are seen working; burst
   particles on clients (spawn/death messages, `ParticleEngine::controlled`); the wand of the
   forest to aim spreaders (`use_item_on`, `aim_at`) and bind flowers.
3. Mana infusion in the pool (`infusion_pick`, `ManaInfusionSpawned` flags), manasteel.
4. A device test: import the `.hengine`, the content `.hmod` with `content/` applied, and check
   the Botania row, a pool's comparator, an Endoflame filling a pool, an apothecary craft.

## Notes for the bundle

- horizon-mixins rules met so far: a helper is one private fn whose name doesn't shadow any of
  the target's parameters (`E_INVALID_HELPER`); a module payload has no inner attributes or `//!`.
- A mod container's items live in `ModBlockEntity.items`, not under `Items` in its NBT.
- Unwrapped recipes use their own paths (`recipe/botania:horizon/<name>`): the content `.hmod`'s
  file at the same path would override a mod's `native_data_pack_files` entry.
