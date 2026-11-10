// Botania for Horizon Aurora: Botania's block entities drawn on the client, added to
// `crates/horizon-client/src/game.rs` as `mod botania_render` by horizon-mixins, beside the
// `botania` module. A modified port of Vazkii's Botania renderers (`ManaPoolBlockEntityRenderer`,
// `PetalApothecaryBlockEntityRenderer`, `ManaSpreaderBlockEntityRenderer`; Botania License, see
// LICENSE and NOTICE.md in this bundle). Their arithmetic is in `botania_core::render`; this
// module draws with Horizon's block entity helpers (`native_block_entity_submit`).
//
// Inner attributes and `//!` comments are not allowed in a horizon-mixins module payload.

use super::Game;
use super::native_block_entity_draw::BlockEntityDraw;
use botania_core::apothecary::Fluid;
use botania_core::consts::{PoolVariant, SpreaderVariant};
use botania_core::pool::Pool;
use botania_core::render;
use horizon_assets::item_model::DisplayContext;
use horizon_client::containers::ModBlockEntity;
use horizon_client::items::ItemStack;
use horizon_world::nbt::{Compound, JavaString, Tag};
use horizon_world::registry::BlockStateId;
use std::cell::RefCell;
use std::collections::HashMap;

// The apothecary's block entity id, as Botania names its type.
const ALTAR: &str = "botania:altar";

// Light at its brightest, as `BlockEntityDraw::light` packs it (`0xF000F0`).
const FULL_BRIGHT: u32 = 0xF0 | (0xF0 << 8);

// The fabulous pool's own model, drawn here: its block is left out of the chunk mesh
// (`ENTITYBLOCK_ANIMATED` in the content's declaration, as `ManaPoolBlock.getRenderShape`).
const FABULOUS_POOL: &str = "botania:block/fabulous_pool";

// A spreader variant's body (its blockstate's model) and core (`MiscellaneousModels`). The body's
// block is left out of the chunk mesh (`ManaSpreaderBlock.getRenderShape`: `ENTITYBLOCK_ANIMATED`).
fn spreader_models(variant: SpreaderVariant) -> (&'static str, &'static str) {
    match variant {
        SpreaderVariant::Mana => (
            "botania:block/mana_spreader",
            "botania:block/mana_spreader_core",
        ),
        SpreaderVariant::Redstone => (
            "botania:block/redstone_spreader",
            "botania:block/redstone_spreader_core",
        ),
        SpreaderVariant::Elven => (
            "botania:block/elven_spreader",
            "botania:block/elven_spreader_core",
        ),
        SpreaderVariant::Gaia => (
            "botania:block/gaia_spreader",
            "botania:block/gaia_spreader_core",
        ),
    }
}

fn key(k: &str) -> JavaString {
    JavaString::from(k)
}

fn get_int(nbt: &Compound, k: &str) -> Option<i32> {
    match nbt.get(&key(k))? {
        Tag::Int(v) => Some(*v),
        Tag::Short(v) => Some(i32::from(*v)),
        Tag::Byte(v) => Some(i32::from(*v)),
        Tag::Long(v) => i32::try_from(*v).ok(),
        _ => None,
    }
}

fn get_float(nbt: &Compound, k: &str) -> Option<f32> {
    match nbt.get(&key(k))? {
        Tag::Float(bits) => Some(f32::from_bits(*bits)),
        Tag::Double(bits) => Some(f64::from_bits(*bits) as f32),
        Tag::Int(v) => Some(*v as f32),
        _ => None,
    }
}

// `native_standalone_models`: the models drawn here that no blockstate turns or tints.
pub(super) fn standalone_models(models: &mut Vec<String>) {
    let spreaders = SpreaderVariant::ALL.into_iter().map(spreader_models);
    let names = spreaders
        .flat_map(|(body, core)| [body, core])
        .chain([FABULOUS_POOL]);
    for name in names {
        if !models.iter().any(|m| m == name) {
            models.push(name.to_owned());
        }
    }
}

// `native_block_entity_update_tag`: an apothecary's items for the client to draw, as Botania's
// `writePacketNBT` sends them (`Items`): the slots up to the first empty one, which are all its
// renderer reads. Always sent, empty too, as the client's copy merges the tag's keys.
pub(super) fn update_tag(entity: &ModBlockEntity, tag: &mut Option<Compound>) {
    if entity.id != ALTAR {
        return;
    }
    let items: Vec<Tag> = entity
        .items
        .iter()
        .map_while(|s| s.as_ref().filter(|s| s.count > 0))
        .enumerate()
        .map(|(slot, s)| {
            let mut c = Compound::new();
            c.insert(key("Slot"), Tag::Byte(slot as i8));
            c.insert(key("id"), Tag::String(JavaString::from(&*s.item)));
            c.insert(key("count"), Tag::Int(s.count as i32));
            Tag::Compound(c)
        })
        .collect();
    tag.get_or_insert_with(Compound::new)
        .insert(key("Items"), Tag::List(items));
}

// The stacks the server sent in `update_tag`.
fn sent_items(nbt: &Compound) -> Vec<ItemStack> {
    let Some(Tag::List(list)) = nbt.get(&key("Items")) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|t| {
            let Tag::Compound(c) = t else { return None };
            let Some(Tag::String(id)) = c.get(&key("id")) else {
                return None;
            };
            let count = get_int(c, "count").unwrap_or(1).max(1) as u32;
            Some(ItemStack::new(&id.to_string_lossy(), count))
        })
        .collect()
}

// `native_block_entity_submit`: a pool's mana, an apothecary's fluid and petals, a spreader
// turned to its aim.
pub(super) fn submit(
    game: &Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    if let Some(variant) = PoolVariant::of_block(block) {
        pool(game, pos, variant, entity, draw);
    } else if let Some(variant) = SpreaderVariant::of_block(block) {
        spreader(game, pos, variant, entity, draw);
    } else if block.starts_with("botania:apothecary_") {
        apothecary(game, pos, state, entity, draw);
    }
}

fn time_of(draw: &BlockEntityDraw) -> f64 {
    draw.game_time as f64 + f64::from(draw.partial)
}

// A flat square facing up at `y` blocks across `from..to` texels of the block, textured with the
// same texels of `sprite` (`RenderHelper.renderIconCropped` turned flat by `rotateX(90)`).
fn flat(
    game: &Game<'_>,
    draw: &mut BlockEntityDraw,
    sprite: &str,
    [x0, z0]: [f32; 2],
    [x1, z1]: [f32; 2],
    y: f32,
    tint: u32,
) {
    let corners = [
        [x0 / 16.0, y, z0 / 16.0],
        [x0 / 16.0, y, z1 / 16.0],
        [x1 / 16.0, y, z1 / 16.0],
        [x1 / 16.0, y, z0 / 16.0],
    ];
    game.native_draw_quad(draw, sprite, corners, [x0, z0, x1, z1], tint);
}

// `ManaPoolBlockEntityRenderer.render`: the fabulous pool's model going round the rainbow, the
// overlay of a catalyst or mana void under it, then the mana's surface at its level.
fn pool(
    game: &Game<'_>,
    pos: [i32; 3],
    variant: PoolVariant,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    let time = time_of(draw);
    let inside = render::pool_inside(variant);
    if variant == PoolVariant::Fabulous {
        let color = render::fabulous_pool_color(time, pos);
        game.native_draw_model(draw, FABULOUS_POOL, color);
    }
    let (start, end) = ([inside.start; 2], [inside.end; 2]);
    let below = game.native_block([pos[0], pos[1] - 1, pos[2]]);
    if let Some(sprite) = render::pool_overlay(&below) {
        // Botania fades it in and out (alpha 0.2 to 0.8); the helpers draw opaque vertices.
        flat(game, draw, sprite, start, end, inside.bottom, 0xFF_FFFF);
    }
    let mut p = Pool::new(variant);
    p.mana = get_int(&entity.nbt, "mana").unwrap_or(0);
    if let Some(cap) = get_int(&entity.nbt, "manaCap") {
        p.cap = cap;
    }
    let Some(y) = render::pool_surface_height(variant, p.current(), p.cap) else {
        return;
    };
    // `MANA_POOL_WATER`: Botania's shader colours the mana in moving bands of cyan and ignores the
    // light; here the texture is tinted to the shader's colour two texels at a time, full bright.
    let t = render::shader_time(draw.game_time, draw.partial);
    let light = draw.light;
    draw.light = FULL_BRIGHT;
    let mut x = inside.start;
    while x < inside.end {
        let x1 = (x + 2.0).min(inside.end);
        let mut z = inside.start;
        while z < inside.end {
            let z1 = (z + 2.0).min(inside.end);
            let tint = render::mana_water_tint([(x + x1) / 2.0, (z + z1) / 2.0], t);
            flat(
                game,
                draw,
                "botania:block/mana_water",
                [x, z],
                [x1, z1],
                y,
                tint,
            );
            z = z1;
        }
        x = x1;
    }
    draw.light = light;
}

// `ManaSpreaderBlockEntityRenderer.render`: the body turned by `rotationY` about x, then
// `rotationX + 90` about y, through the block's centre (the Gaia spreader's tinted round the
// rainbow), and its core turning about the body's own vertical axis and bobbing along it.
fn spreader(
    game: &Game<'_>,
    pos: [i32; 3],
    variant: SpreaderVariant,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    let (body, core) = spreader_models(variant);
    let rotation_x = get_float(&entity.nbt, "rotationX").unwrap_or(0.0);
    let rotation_y = get_float(&entity.nbt, "rotationY").unwrap_or(0.0);
    let time = time_of(draw);
    let tint = if variant == SpreaderVariant::Gaia {
        render::gaia_spreader_color(time, pos)
    } else {
        0xFF_FFFF
    };
    let centre = [0.5; 3];
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], rotation_x + 90.0, centre);
    draw.rotate_about([1.0, 0.0, 0.0], rotation_y, centre);
    game.native_draw_model(draw, body, tint);
    let (degrees, bob) = render::spreader_core(time);
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], degrees, centre);
    draw.translate([0.0, bob, 0.0]);
    game.native_draw_model(draw, core, 0xFF_FFFF);
    draw.pop();
    draw.pop();
}

thread_local! {
    // Biome ids by name, for the water's colour; checked against the name before use.
    static BIOME_IDS: RefCell<HashMap<String, u16>> = RefCell::new(HashMap::new());
}

// `BiomeColors.getAverageWaterColor` at `pos`, as its own biome's water colour (not blended over
// the biome blend radius); vanilla's default where the biome isn't known.
fn water_color(game: &Game<'_>, pos: [i32; 3]) -> u32 {
    const DEFAULT: u32 = 0x3F_76E4;
    let Some(name) = game.native_biome(pos) else {
        return DEFAULT;
    };
    let cached = BIOME_IDS.with(|ids| ids.borrow().get(&name).copied());
    let id = match cached {
        Some(id) if game.native_biome_name(id) == Some(name.as_str()) => Some(id),
        _ => {
            let found = (0..=u16::MAX)
                .map_while(|id| game.native_biome_name(id).map(|n| (id, n == name)))
                .find_map(|(id, same)| same.then_some(id));
            if let Some(id) = found {
                BIOME_IDS.with(|ids| ids.borrow_mut().insert(name.clone(), id));
            }
            found
        }
    };
    id.and_then(|id| game.native_biome_tint(id, pos[0], pos[2]))
        .map_or(DEFAULT, |colors| colors.water & 0xFF_FFFF)
}

// `PetalApothecaryBlockEntityRenderer.render`: in water, the petals going round under its surface;
// the water (biome coloured, alpha 0.7) or the lava (full bright) at 15/16 of the block, cropped to
// texels 3 to 13. Kept while nothing moves: no fluid, lava, or water without petals.
fn apothecary(
    game: &Game<'_>,
    pos: [i32; 3],
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    let fluid = game
        .native_block_state_property(state, "fluid")
        .and_then(Fluid::of_name)
        .unwrap_or_default();
    if fluid == Fluid::Empty {
        draw.keep();
        return;
    }
    let petals = if fluid == Fluid::Water {
        sent_items(&entity.nbt)
    } else {
        Vec::new()
    };
    draw.push();
    draw.translate([0.5, 1.25, 0.5]);
    if !petals.is_empty() {
        draw.push();
        draw.translate([-0.05, -0.38, 0.0]);
        draw.scale([1.0 / 8.0; 3]);
        let poses = render::apothecary_petals(time_of(draw), petals.len());
        for (stack, petal) in petals.iter().zip(poses) {
            draw.push();
            draw.translate(petal.at);
            draw.rotate_about(petal.axis, petal.degrees, [1.0 / 16.0; 3]);
            game.native_draw_item(draw, stack, DisplayContext::Ground, None, 0);
            draw.pop();
        }
        draw.pop();
    }
    draw.translate([-0.5, -0.3125, -0.5]);
    let (id, argb) = if fluid == Fluid::Lava {
        ("minecraft:lava", 0xFFFF_FFFF)
    } else {
        // `(int) (0.7F * 255F)` = 178.
        ("minecraft:water", (0xB2 << 24) | water_color(game, pos))
    };
    let stack = crate::mod_api::NativeFluidStack {
        fluid: id.to_owned(),
        amount: 1000,
        components: Compound::new(),
    };
    let options = crate::mod_api::NativeFluidRenderOptions {
        argb: Some(argb),
        render_bottom: false,
        uv_shrink_ratio: 0.0,
    };
    // A box of no height: only its top, facing up, culled from below as Botania's
    // `translucentCullBlockSheet` is.
    let (a, b) = (3.0 / 16.0, 13.0 / 16.0);
    game.native_draw_fluid_box(draw, &stack, [a, 0.0, a, b, 0.0, b], options);
    draw.pop();
    if petals.is_empty() {
        draw.keep();
    }
}
