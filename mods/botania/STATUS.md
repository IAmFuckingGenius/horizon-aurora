# Status

The port has been played in Waydroid (x86_64, Dev 25 APK, engine with the content packed in):
see "In game" under Verified, the merged branches' weapons, mana and renderers too. It has not
been played on a phone yet.

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
    the 400-step receiver scan with its path, `bindTo` aiming and the placement rotation, the
    port's trail size;
  - `apothecary`: `collideEntityItem` (fill from buckets, lava, craft with the reagent, take one
    item, the remembered reagent keeping seeds out), `tickRecipeKeep`, comparator; the right click
    (`PetalApothecaryBlock.use`: `trySetLastRecipe`, `withdrawFromInventory`, bucket withdraw and
    deposit), the fish a fish bucket releases, items burning in lava (5 health, hurt from the
    second tick of contact);
  - `wand`: `WandOfTheForestItem.useOn` for the ported blocks (complete a binding, select or
    deselect, a spreader's or pool's `onUsedByWand`), `calculateViewVector`, the point clicked on
    a face, a spreader's function-mode aim (`onUsedByWand` while sneaking), `Mth.hsvToRgb` and
    the binding beam.
- Bundle (`bundle/horizon-mixin.toml`): named hook points only, no pin, no `[api]`:
  - `[[module]] botania` in `crates/horizon-client/src/game.rs` (`runtime.rs`),
    `[[crate]] core` (`botania-core`), `data/recipe/` (25 files);
  - hooks: `mods` (row "Botania 0.1.0"), `block_entity`, `block_entity_container`,
    `block_entity_ticks`, `block_entity_tick`, `block_entity_update_tag`, `level_tick_post`,
    `has_analog_output`, `analog_output`, `data_pack_files`, `place_block`, `use_item_on`,
    `use_item`, `right_click_block` (all version 1, NATIVE_HOOKS_API 415 has them);
  - mana pools (`mana_pool`, `diluted_pool`, `creative_pool`, `fabulous_pool`): block entity
    `botania:mana_pool`, `mana`/`manaCap` in its NBT, capacity per variant, sync every 10 ticks
    when the mana changed, comparator output (cached while the pool ticks; neighbours told in
    `level_tick_post`, when the block entity is back in the world); mana infusion of the items
    in its cell (`native_recipes_of_type("botania:mana_infusion")`, catalyst = the block under
    it, output at `y + 1.5` flagged so it isn't infused again, craft sound at most every 6 ticks);
    `outputting` toggled by the wand: an outputting pool charges the mana items lying in it,
    any other drains them (see mana items below);
  - Endoflame: block entity `botania:endoflame` (`ticksExisted`, `mana`, `binding`, `burnTime`),
    Botania's tick through `botania-core`: item age from the item entity (`age`), furnace burn
    time from the game's fuel table, crafting remainders and spreaders skipped, sound
    `botania:endoflame`; it binds on its first tick to the nearest mana spreader within 6 blocks
    (`distSqr` of the corners, Botania's `ManaCollector` lookup done over the block entities in
    range) and empties its mana into it every tick up to the spreader's room; a flower an earlier
    bundle bound to a pool binds again once; smoke and a flame at the item it lights, and a flame
    one tick in ten while it burns (server particles);
  - Petal Apothecary (12 variants): block entity `botania:altar` with 16 slots (a mod container,
    no faces, no menu), `fluid` from the block's property when the block has one, else from the
    block entity's NBT; recipes from `native_recipes_of_type("botania:petal_apothecary")`; a fish
    bucket thrown in releases its fish above (with the bucket's entity data, `FromBucket`); an
    item thrown into its lava burns up on its sixth tick of contact unless fire resistant; right
    click in the main hand's turn: an empty hand puts the last recipe back (paid from the
    inventory, free in creative) or takes the last item out, an empty bucket takes the water or
    lava, a water or lava bucket fills an empty one (bucket sounds; creative keeps the bucket);
  - Pure Daisy: block entity `botania:pure_daisy` (`position`, `ticksRemaining0..7`), recipes of
    both daisy types, state copying through the block registry;
  - mana spreaders (4 variants, no lenses/padding): block entity `botania:mana_spreader`
    (`mana`, `rotationX`, `rotationY`, `canShootBurst`), rotation facing the placer (`place_block`
    hook, applied on the first tick), draw from adjacent pools except the receiver, redstone
    (`native_neighbor_signal`; the redstone variant on a rising edge), receiver scan with
    `botania-core::burst::scan_receiver` over `native_block_collision` (pools as their burst slab,
    `#minecraft:leaves` passed) on aim change and every 10 ticks, one burst in flight per spreader,
    fire sound `botania:spreader_fire` at volume 0.05; spreaders receive bursts too (a chain of
    spreaders), clamped to their capacity;
  - virtual bursts (design 3.6 variant A): kept in the module, moved in `level_tick_post` from
    the tick after they are shot, delivering to pools and spreaders with
    `native_with_mod_block_entity`, lost over an unloaded chunk; the trail is one
    `minecraft:dust` particle a tick per burst in the spreader's colour (`0x20FF20`, elven
    `0xFF45C4`, redstone `0xFF2020`), scale 0.35–1 by the mana left, and four where it dies,
    sent by the server in one `with_level` pass per level tick (design §7's subtle mode);
  - Wand of the Forest (`botania:twig_wand`, `botania:dreamwood_wand`): state in the stack's
    `minecraft:custom_data` under Botania's names (`bindMode`, `boundTileX/Y/Z`); in bind mode a
    sneak-click on an Endoflame or a spreader selects it (`botania:ding` to the player, a line
    on the action bar), a sneak-click on another block completes the binding: a flower binds to
    a spreader within 6 blocks (else a line says so), a spreader aims at the centre of the
    clicked block's shape at any range; a rainbow beam of dust and "Bound" on success. In function
    mode a sneak-click on a spreader turns it away from the point clicked (the look ray met with
    the face's plane). A sneak-click on a pool toggles `outputting`. A sneak-click in the air
    switches mode (`botaniamisc.wandMode.*` on the action bar). Every wand click in the air is
    used (the hand swings), as Botania's `use` succeeds;
  - the item use a client sends after a block click it saw no answer to (`startUseItem` after a
    PASS) does nothing for 250 ms after a click the module answered, so a selection doesn't also
    switch the wand's mode and an apothecary's bucket doesn't act twice;
  - block entity renderers (`[[module]] botania_render`, `renderers.rs`; hooks
    `block_entity_submit`, `standalone_models` and a second `block_entity_update_tag` injection,
    `update_tag_items`), Botania's three renderers with their arithmetic in `botania-core::render`:
    - pool (`ManaPoolBlockEntityRenderer`): the mana's surface, only with mana, from 1/16 + 0.001
      up to 7/16 (diluted 5/16, creative 9/16) by `mana / manaCap`, cropped to texels 2–14
      (diluted 1–15), `botania:block/mana_water` tinted two texels at a time to the colour
      Botania's `mana_pool` shader gives it (red 0, moving bands of green and blue) and full
      bright; under it the overlay of an alchemy or conjuration catalyst or a mana void below;
      the fabulous pool's model tinted round the rainbow (`hsvToRgb(frac((t + new
      Random(x ^ y ^ z).nextInt(100000)) * 0.005), 0.6, 1)`);
    - apothecary (`PetalApothecaryBlockEntityRenderer`): water (`block/water_still`, its biome's
      water colour, alpha 0.7) or lava (full bright) at 15/16, texels 3–13, through
      `native_draw_fluid_box` (a box of no height: its top only, culled from below); in water
      its items (sent to clients as `Items`, the slots up to the first empty one) going round
      under the surface as dropped items an eighth of their size, with Botania's radius, bob and
      tumble; kept (`draw.keep`) while nothing moves (no fluid, lava, water without items);
    - spreader (`ManaSpreaderBlockEntityRenderer`): its model turned `rotationX + 90` about y and
      `rotationY` about x through the centre (the Gaia spreader's tinted round the rainbow), its
      core (`botania:block/*_spreader_core`) turning a degree a tick about the body's axis and
      bobbing `sin(t / 20) * 0.05` along it;
    - the four spreaders and the fabulous pool are `ENTITYBLOCK_ANIMATED` in their content
      declarations (`render_shape`), as Botania's `getRenderShape`: the chunk mesh leaves them
      out and the renderer draws them, within 64 blocks as every block entity renderer;
      `dist/botania.hmod` carries the five changed declarations (`build_content.py --overlay`,
      sha256 `6197a5db…`);
  - phase 1 crafting recipes (apothecary, pools, livingrock slab, spreader, wand, twig,
    livingwood, petals) given in 26.3's form under `recipe/botania:horizon/<name>`.
- Mana items (`[[module]] botania_mana`, `bundle/mana.rs`; numbers in `botania-core`'s
  `mana_item`, `mana_handler`, `mana_gear`; hooks `inventory_tick`, `item_damaged`, `item_bar`,
  `tooltip`, `tooltip_component`, `tooltip_component_draw`, a second `use_item_on`,
  `modify_default_components`):
  - mana in `minecraft:custom_data` under Botania's keys (`mana`, `creative`, `oneUse`, the
    mirror's `maxMana`, `manaBacklog`, `pos`); Mana Tablet 500 000 (×count; creative 501 000,
    never drained, `oneUse` refuses pools), Band of Mana 500 000, Greater Band 2 000 000, Mana
    Mirror (shows its bound pool: the pool takes what is spent from it, sneak-use on a pool binds
    it, `botania:ding`), Terra Shatterer (`Integer.MAX_VALUE`, ranks at 10k/1M/10M/100M/1B, only
    from pools, refuses `#botania:terra_pick_blacklist` senders); stack size 1; bar
    `round(13·f)` wide in `hsvToRgb(f/3, 1, 1)` (none for the creative tablet), "Creative" and
    the rank / "Get a Life" lines, a pulsing mana bar in the tooltip;
  - pools: outputting charges, otherwise drains, 1000 a tick per item (`canSpare`/`canAccept`,
    a mana void under it), the item entity updated; every 10 pool ticks a moving item gets a
    dust arc (`0x00C6FF`) along Botania's charging-wisp parabola;
  - `ManaItemHandler` over `Inventory.items` + off hand (armour only for the discount):
    `requestMana(Exact)`, `…ForTool` (helmet with its full set: manasteel/elementium 10%,
    terrasteel 20%, manaweave 40%; +5% per Unbreaking level), `dispatchMana(Exact)`;
  - mana instead of durability: each point of wear the game deals (after vanilla's Unbreaking)
    paid with `manaPerDamage` less the discount, while the mana lasts: manasteel and elementium
    tools and swords and the Thundercaller 60, shears 30, Terra Shatterer/Truncator/Blade 100,
    Starcaller 120, Vitreous Pickaxe 160, Livingwood Bow 40, Crystal Bow 200 (100 with Infinity),
    all armour 70 (manaweave helmet 30); the Soulscribe never pays;
  - mending each tick of a damaged stack anywhere in the inventory: tools 2×their cost with the
    discount (pickaxes, the Terra Shatterer and the Vitreous Pickaxe 120, axes/shovels/hoes/
    swords 120, shears 60, Terra Truncator/Blade 200, Starcaller 240, bows 80), armour 140
    (manaweave helmet 60) with no discount;
  - Terrasteel set worn: a heart every 80 ticks while hurt with food 1–17, 10 mana to the mana
    items every 10 ticks; knockback resistance 0.15/0.4/0.3/0.15 per piece (full set 1.0);
  - Band of Mana and Greater Band of Mana crafted as plain recipes (they start empty).
- `content/`: 55 corrected block declarations and the 16 mystical flower patches with
  `minecraft:simple_block` for the content `.hmod` (see README). The local
  `.local/modports/botania/botania-1.20.1-456-content.hmod` has both applied (sha256
  `4add364b…`; the copy before the flower patches is `….hmod.before-flowers`).

## Verified

- `cargo test` of `botania-core` (scratch copy, `CARGO_BUILD_JOBS=2`, nice/ionice): 55 passed,
  0 failed (11 of them `weapons`: bow charge, blade burst flight and strikes, chain order and
  damage, star spawn and fall, Terra Shatterer ranks and area, mana costs). Expected values are Botania's, not the port's: Endoflame coal → 1197/1200
  mana by tick parity, Pure Daisy 1193 ticks (first position visited) to 1200, burst death on
  tick 99 after 19.8 blocks (elven 139, Gaia 151), gametest `spreader_pool_default_aim` (pool hit
  by tick 3 for all four variants), gametest `ApothecaryRecipeTest.testItemEnterPrevention`,
  gametests `BindingTest.generatingFlowerAutoBindTest` and `generatingFlowerManualBindTest`
  (flowers 6 blocks away along each axis bind to a spreader, 7 away don't), comparator levels,
  catalyst priority, the jar's recipe files; the wand's `useOn` order (select, bind, deselect, a
  selection whose block is gone), `onUsedByWand` aim on each face (east → 360, west → 180,
  south → 270, north → 90, top → pitch -90), vanilla's view vector, `hsvToRgb`, the apothecary's
  right click (refill paid per item, creative free, forgotten after 400 ticks, buckets), items
  burning up on the sixth tick.
- With the mana items: 60 passed, 0 failed. Expected values from Botania's code run in Java
  (`hsvToRgb` bars, `encode/decodeRelativeItemPosition`, the charging parabola, the
  `ManaItemImpl`s, `ManaItemHandlerImpl`'s loops, the pool's item transfer,
  `discountManaForTool`, `damageItemIfPossible`).
- All 25 bundle recipes unwrap into 26.3 form (scratch-only test, output checked by eye).
- Engine runs (`android-native-engine.yml`, arm64-v8a, `publish=false`):
  - 42728593487, bundle 162df0be9c: failed in `plan`, `E_INVALID_HELPER` (helper `ticks`
    shadowed the parameter `ticks`); fixed by prefixing every helper with `botania_`.
  - 42729019307, bundle 71a3d55aa7 (Botania row, pools, Endoflame, comparator, recipes):
    **success**. horizon-mixins applied 9 injections, 1 module, 1 crate, 1 data folder; the
    release library built in 18 min 11 s; `.hengine` packed (artifact
    `native-engine-botania-arm64-v8a`, 14 days).
  - 42731705291, bundle d42992fccb (+ apothecary, Pure Daisy, spreaders, virtual bursts,
    `block_entity_container`, `place_block`): **success**; 11 injections, 1 module, 1 crate,
    1 data folder; release library in 15 min 31 s.
  - 42734189256, bundle 5cccde1e31 (+ mana infusion in the pool): **success**; 11 injections,
    1 module, 1 crate, 1 data folder; release library in 13 min 52 s.
  - 42739701095, bundle 9e67fecbb1: cancelled after 6 min, replaced by the next run.
  - 42740458235, bundle 62468c1f06 (+ flowers feeding spreaders, spreaders receiving bursts,
    the Wand of the Forest, dust trails, the apothecary's right click, fish buckets, lava,
    Endoflame particles, `use_item_on`, `use_item`, `right_click_block`): **success**; 14
    injections, 1 module, 1 crate, 1 data folder; release library in 16 min 55 s, no warnings
    from the bundle; `botania-arm64-v8a.hengine` 36.1 MB (artifact
    `native-engine-botania-arm64-v8a`, until 2026-11-24). This is the current bundle.
  - 42873359539 (x86_64), bundle 7e3ca26d84 (branch `botania-render`: + block entity renderers,
    `block_entity_submit`, `standalone_models`, `update_tag_items`, module `botania_render`,
    content with the five `ENTITYBLOCK_ANIMATED` declarations): **success**; 17 injections,
    2 modules; no warnings from the bundle. `cargo test` of `botania-core`: 49 passed.
  - 42878996755 (x86_64), bundle 122763f1c8 (branch `botania-weapons`: bows, Terra Blade,
    Thundercaller, Starcaller, Terra Shatterer, elementium shovel and pickaxe, mana tools):
    **success**; 29 injections, 2 modules (`botania`, `botania_weapons`), 1 crate, 1 data
    folder; release library in 13 min 43 s, no warnings from the bundle. Not played yet.
  - 42879206886 (x86_64, engine id `botania-mana`), bundle 3db3aa1e87 (branch `botania-mana`:
    + mana items, `ManaItemHandler`, mana instead of durability, set bonuses): **success**; 22
    injections, 2 modules, 1 crate, 1 data folder; no warnings from the bundle. An earlier
    dispatch of it under engine id `botania` (42876970105) was cancelled while pending by
    another `botania` dispatch: the workflow keeps one pending run per engine id.

### In game (Waydroid x86_64, 2026-11-10)

Driven through horizon-ctl (`control.enable` in the app's private folder), engine run
42850654648's library with `dist/botania.hmod` 9a1935e1. All of these work:

- The engine puts `botania.hmod` into a new instance by itself; content loads with no warnings.
- Weapons and tools, read with `/attribute` and checked against hits on an iron golem: manasteel
  sword 6 damage / 1.6 speed, Terra Blade 8, Thundercaller speed 2.5, Soulscribe 6 / 2.75; the
  manasteel pickaxe breaks obsidian (242 ticks) and drops it, the vitreous one gets no drop from
  iron ore. Full terrasteel armour: armor 20, toughness 12.
- Icons, held items and worn armour are drawn (vanilla looks: diamond, leather, iron, netherite).
- Mana pool infusion: iron → manasteel, diamond → mana diamond, cookie → mana cookie, short grass
  → grass seeds; an empty pool infuses nothing. Alchemy catalyst: acacia log → dark oak log,
  short grass → fern. Conjuration catalyst: 1 leaf block → 2.
- Petal Apothecary: a water bucket fills it (the empty bucket comes back), a cod bucket fills it
  and lets the cod out; 4 white petals and seeds give a Pure Daisy and use the water; broken, it
  drops the petals it holds. The Pure Daisy turns 4 stone into livingrock and 4 oak logs into livingwood.
- Wand of the Forest: function mode aims a spreader; bind mode selects a spreader and binds it to
  a pool ("Selected", "Bound"); sneak-clicking a pool toggles `outputting`; sneak-clicking air
  switches the mode. A placed spreader faces the player (rotationX 90 with the player to the
  south), as `ManaSpreaderBlock.setPlacedBy`.
- Endoflame binds to the nearest spreader on its first tick, burns thrown coal (burnTime 800),
  the spreader shoots 160-mana bursts with a green dust trail into the pool.
- A comparator on a pool lights a lamp for a filled or creative pool, not for an empty one.
- Mystical flowers generate in new chunks (white and lime ones within 96 blocks of 4000 4000)
  and `/place feature botania:mystical_flowers` places them.

Found and fixed during the test: `dyed_color` made Horizon refuse the whole content
(`equipment.py`), and recipes naming `minecraft:grass` never matched (port-pack now renames it to
`minecraft:short_grass`, and `minecraft:scute` to `minecraft:turtle_scute`).

### In game, merged branches (Waydroid x86_64, 2026-11-11)

Engines of runs 42885776931 (bundle 16917714b1), 42890320899 (bundle 898349366a, aurora main
861c9b5539) and a debug build, with `dist/botania.hmod` d0a8f774. All of these work:

- Bows: the livingwood and crystal bows draw (pull models, first person), shoot and wear; mana pays
  the wear when there is some.
- Terra Blade: a swing at the air (nothing in reach, no block outline under the crosshair) shoots
  the beam; it hit a pig 8 blocks away (10 → 2). Thundercaller: the lightning jumped from the
  zombie hit to two zombies 3 blocks away (5–6 damage each; pigs aren't `Enemy`, as in Botania).
  Starcaller: a star falls at night after a swing. Terra Shatterer: sneak-use with 2 000 000 mana
  turns it on (`enabled`), a mined block takes the 3×3 around it, and the mana runs down while on.
- Mana: a damaged manasteel pickaxe mends a point a tick from a tablet in the inventory (100 points
  for 12 000 mana); an outputting pool fills a tablet at 1000 a tick and a normal one drains it;
  full terrasteel heals a point every 80 ticks at food 5.
- Botania's armour models on the player (third person), the armour bar (20).
- Block entity renderers: a pool's mana (60% after `data merge … {mana:600000}`), the creative pool
  full, the fabulous pool, the spreaders (mana, gaia) and their cores, the apothecary's water and
  a petal in it.
- Advancements: the tab and toast icons, and the tabs' backgrounds (livingwood).

Found and fixed during the test:
- the Android launcher started the game without registering native mods' standalone models and
  model layers, so the spreaders and the fabulous pool drew nothing (aurora main 861c9b5539);
- 1.20 advancement backgrounds named a texture file, which 26.3 doesn't read (port-pack, main
  19a7d5e785);
- 258 plain items stood on `minecraft:stick` with its `cooking_fuel`: they burned in a furnace and
  an Endoflame burnt a tablet lying in a pool beside it (port-pack, main 123db71fc2).

## How to test on a phone

1. Download the engine: GitHub › IAmFuckingGenius/horizon-aurora › Actions › run 42895670428 ›
   artifact `native-engine-botania-arm64-v8a` (a ZIP holding `botania-arm64-v8a.hengine`; kept
   14 days; `…-armeabi-v7a` for 32-bit phones). Or: `gh run download 42895670428 -R
   IAmFuckingGenius/horizon-aurora -n native-engine-botania-arm64-v8a`. Unzip it and copy the
   `.hengine` to the phone.
2. In Horizon Aurora: **Settings › Engines › Import**, pick `botania-arm64-v8a.hengine`, then
   **Start** on "Aurora + Botania". The app restarts on that engine. Options › Mods should list
   "Botania 0.1.0".
3. Nothing else to add: the engine carries `dist/botania.hmod` (sha256 `d0a8f774…`) and puts it
   into the instance it plays.
4. Make a new world (mystical flowers are placed when chunks generate; old chunks have none).
   Survival checklist:
   - find mystical flowers (16 colours, on grass in the Overworld), pick them and craft each into
     2 petals;
   - craft a Petal Apothecary (7 cobblestone and a petal: `CPC`, ` C `, `CCC`), fill it: throw
     in a water bucket or right click it with one; throw in 4 white petals, then wheat seeds: a
     Pure Daisy pops out. A right click with an empty hand within 20 s puts the 4 petals back;
   - plant the Pure Daisy with stone or logs on the 8 blocks around it: after about a minute
     they turn into livingrock / livingwood logs;
   - craft livingwood twigs (2 livingwood logs), the Wand of the Forest (2 petals, 3 twigs), a
     mana pool (5 livingrock) and a mana spreader (6 livingwood logs, a copper ingot, a petal);
     make an Endoflame in the apothecary (2 brown, 1 red, 1 light gray petal, then seeds);
   - place the pool, then the spreader beside it: the spreader faces you. With the wand,
     sneak-click the spreader (selected), then sneak-click the pool (Bound, a rainbow line): the
     spreader aims at the pool;
   - place the Endoflame within 6 blocks of the spreader: it binds by itself (or sneak-click it,
     then the spreader). Throw coal near it (within 3 blocks): flames, mana goes to the spreader,
     a green dust trail flies to the pool every ~107 ticks; a comparator on the pool rises;
   - throw an iron ingot into the pool with 3000+ mana: a manasteel ingot comes out on top.
   - also worth a look: sneak-click in the air switches the wand's mode (action bar); in function
     mode a sneak-click on a spreader's side turns it away from the click; a fish bucket into an
     empty apothecary releases the fish; an item thrown into a lava apothecary burns up.
   Things to report: the Botania row, a wrong first-tick binding, mana not reaching the spreader
   or the pool, no trail, the wand's mode switching on a selection, the game log's
   "placed features cannot run" lines.

## Not verified / known gaps

- Not played on a phone yet; the whole chain hasn't been crafted in survival (its recipes are
  registered: `/recipe give` unlocks each).
- Bows, weapons and tools (`bundle/weapons.rs`, rules in `botania-core` `weapons`):
  - engine-only: the third-person bow arm pose (`mobs/anim.rs` picks `ArmPose::Bow` by the id
    `minecraft:bow`, not by the use animation) and the touch projectile crosshair;
  - the beam, stars and chain lightning hit mobs only (no PvP), drawn as vanilla dust (G2): big
    green squares for the beam; a beam or star is dropped when its owner leaves; the Starcaller's
    timer is per player, not per sword; Unbreaking rolls twice when mana pays for wear;
  - Terra Shatterer: no pool charging, no rank in the tooltip, no enabled model; its NBT changes
    every tick while on;
  - not ported: Terra Truncator, axe beheading, hoes, the shears' area, the sword's pixies, the
    rings (Thor, temperance, Loki).
- The blaze block isn't furnace fuel (Botania gives it 24 000 ticks in a Forge event; the port's
  items carry no fuel).
- Mana items, not ported: elementium armour's pixies (no pixie entity), manaweave's rod
  proficiency (no rods), the manaweave boots' powder snow walking, accessories (Horizon has no
  accessory inventory: a band works from the inventory only), bellows (pools charge at 1000), the
  ring recipes carrying the tablet's mana (`botania:mana_upgrade`: the crafting grid isn't
  reachable from a hook), the creative tablet's epic rarity, Botania's wisp sprites (dust instead),
  the tooltip bar's rank letters for the Terra Shatterer.
- Mana items, approximations: wear is heard after vanilla's Unbreaking, so each remaining point
  is paid in full (Botania rolls its own Unbreaking first: the same mana on average for tools,
  slightly more for armour); `stackInSlot == stack` is matched by value; a Mana Mirror bound to a
  pool in another dimension or an unloaded chunk is left as it is (Botania loads the chunk).
- Ender air: the function `botania:ender_air_release` doesn't load (no `botania:ender_air`
  entity, which the Pure Daisy leaves when it turns end stone and a glass bottle picks up), and
  no ender air bottles outside the End.
- Trail and beam (engine gap G2): server-sent vanilla dust, not Botania's wisps and sparkles:
  no additive blending, no wisp sprite, the dust's own motion; one particle packet per burst per
  tick to nearby players. Botania's full trail (2–5 wisps a tick) isn't offered.
- Client prediction: Botania answers clicks on both sides; the port answers on the server only.
  The 250 ms window after an answered click is a heuristic.
- Wand: no rotation of other blocks on a sneak-click (`manipulateBlockstate`), no HUD, no outline
  of the bound block, no aim preview, no petal colours (G8), not used by dispensers; the item
  name's "(Bind Mode)" and the bind model aren't shown; the selection isn't cleared by
  `inventoryTick` when its block goes (it is ignored at the next click instead).
- Apothecary: buckets act in the main hand only; a fish bucket used by hand isn't handled; a
  burning item shows no fire; rain doesn't fill it.
- Mystical flowers: Botania's 5% tall flowers are left out; mushrooms not touched.
- Block entity renderers, against Botania: the helpers draw opaque vertex
  colours and no shader of the mod's, so the pool's catalyst overlay doesn't fade in and out
  (Botania: alpha 0.2–0.8) and the `mana_pool` shader is imitated by tints per two texels, with
  the sprite placed in a nominal 2048 atlas for its bands; the apothecary's water is its biome's
  colour, not blended over neighbouring biomes; time is the level's game time, not
  `ticksInGame`; a spreader's lens, padding and scaffolding aren't drawn (not ported).
- Not done in game yet: levelEvent 2001 when the daisy converts, floating flowers, Enchanted
  Soil's double tick, pool colour with petals, lenses, other generating flowers.
- Mana items not played yet: the creative tablet, the bands, the Mana Mirror.
- `native_blocks` can't fix the declarations from code: a block the content mod declares too is
  "declared twice" and the whole namespace is refused (`horizon_mods::Content::declare`).

## Next

1. The device test above, on the run 42895670428 engine.
2. Block entity renderers on a phone: a spreader turning with the wand, its core spinning, the
   fabulous pool's rainbow, a catalyst's overlay; the apothecary's lava.
3. Burst particles drawn by the client (mod messages + `ParticleEngine::controlled` with colour
   and size) if the dust trail costs too much on phones or looks wrong.
4. The wand's HUD and the bound-block outline (`native_hud`, `native_world_primitives`); mana
   tablet and the pool's `outputting`.
5. Phase 2 flowers (Hydroangeas, Daybloom) on the same spreader binding.

## Notes for the bundle

- horizon-mixins rules met so far: a helper is one private fn whose name doesn't shadow any of
  the target's parameters (`E_INVALID_HELPER`); a module payload has no inner attributes or `//!`.
- A mod container's items live in `ModBlockEntity.items`, not under `Items` in its NBT.
- Unwrapped recipes use their own paths (`recipe/botania:horizon/<name>`): the content `.hmod`'s
  file at the same path would win over a mod's `native_data_pack_files` entry.
- A block click answered on the server only is followed by the client's `UseItem` for the same
  click; swallow it (see `answered` in `runtime.rs`).
- Server particles go out in one `Game::with_level` pass: `native_send_particles_in_place` builds
  a level view per call.
