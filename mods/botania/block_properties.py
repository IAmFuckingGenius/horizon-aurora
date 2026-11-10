#!/usr/bin/env python3
"""Give Botania's blocks Botania's own block properties in `content/`.

    python3 mods/botania/block_properties.py [--source DIR]

`horizon-port-pack` had no oracle for Botania's blocks, so most of them came out as
`{"template": "minecraft:stone"}`: stone's hardness, sounds and tool, and stone's kind of block for a
flower, a pot or a glass. This reads Botania's block table (`BotaniaBlocks.java`,
`BotaniaFlowerBlocks.java`, `LibBlockNames.java`, `BotaniaBlockFlammability.java` of Botania 1.20.x,
commit 0c06dcf7, downloaded unless `--source` names a folder holding them), follows each block's
`BlockBehaviour.Properties` through its `copy(...)` chain, and writes
`content/data/botania/horizon/block/<name>.json` for every block the carried `dist/botania.hmod`
declares:

- `properties`: hardness and explosion resistance (`strength`, `instabreak`), light (`lightLevel` when it
  is a constant), `requires_tool` (`requiresCorrectToolForDrops`), `occludes: false` (`noOcclusion`),
  `random_ticks` (`randomTicks`), the map colour's id (`mapColor`; a log's colour is the one of its
  `axis=y` states) and `sounds`, a vanilla block with the `SoundType` (`SOUNDS`);
- `template`, by the block's class where the port-pack's was the wrong kind of block (`template_for`):
  stone-like slabs, stairs and pillars on stone ones instead of oak ones (which burn), flammable planks on
  oak planks, flowers on the poppy, glass on glass, the spectral rail on a straight rail, the detectors
  with their `powered` state; otherwise the template stays;
- the outline of the blocks whose class gives one (`SHAPES`, Botania's `getShape`), with `occludes: false`
  so the faces beside them are drawn, and no collision for `noCollission` blocks;
- no `minecraft:equippable` on the items of the blocks on the carved pumpkin's template, as only the
  tiny potato is worn (`TinyPotatoBlockItem.getEquipmentSlot`).

The hand-written declarations (`HAND`: pools, spreaders, apothecaries, flowers Botania's code drives, the
blaze block) keep every key they have; the generator only adds the `properties` keys they lack. Run
`python3 mods/botania/build_content.py --overlay` afterwards.
"""
from pathlib import Path
import argparse
import json
import re
import sys
import tempfile
import urllib.request
import zipfile

HERE = Path(__file__).resolve().parent
OUT = HERE / 'content/data/botania/horizon/block'
COMMIT = '0c06dcf7ae9555d52885f913ecd769ceb94fa823'
SOURCES = {
    'BotaniaBlocks.java': 'Xplat/src/main/java/vazkii/botania/common/block/BotaniaBlocks.java',
    'BotaniaFlowerBlocks.java': 'Xplat/src/main/java/vazkii/botania/common/block/BotaniaFlowerBlocks.java',
    'BotaniaBlockFlammability.java': 'Xplat/src/main/java/vazkii/botania/common/block/BotaniaBlockFlammability.java',
    'LibBlockNames.java': 'Xplat/src/main/java/vazkii/botania/common/lib/LibBlockNames.java',
}

COLORS = ['white', 'orange', 'magenta', 'light_blue', 'yellow', 'lime', 'pink', 'gray', 'light_gray', 'cyan',
          'purple', 'blue', 'brown', 'green', 'red', 'black']
# Written by hand: their templates, states and shapes are Botania's, and the bundle drives them.
HAND = ({f'{c}_mystical_flower' for c in COLORS} | {f'{c}_double_flower' for c in COLORS}
        | {f'apothecary_{v}' for v in ['default', 'deepslate', 'desert', 'forest', 'fungal', 'livingrock', 'mesa',
                                       'mossy', 'mountain', 'plains', 'swamp', 'taiga']}
        | {'mana_pool', 'creative_pool', 'diluted_pool', 'fabulous_pool', 'mana_spreader', 'redstone_spreader',
           'elven_spreader', 'gaia_spreader', 'endoflame', 'hydroangeas', 'pure_daisy', 'blaze_block'})

# `SoundType` → a vanilla block that has it.
SOUNDS = {
    'STONE': 'minecraft:stone', 'WOOD': 'minecraft:oak_planks', 'GRASS': 'minecraft:grass_block',
    'MOSS': 'minecraft:moss_block', 'GRAVEL': 'minecraft:gravel', 'DEEPSLATE': 'minecraft:deepslate',
    'DEEPSLATE_TILES': 'minecraft:deepslate_tiles', 'DEEPSLATE_BRICKS': 'minecraft:deepslate_bricks',
    'TUFF': 'minecraft:tuff', 'CALCITE': 'minecraft:calcite', 'METAL': 'minecraft:iron_block',
    'GLASS': 'minecraft:glass', 'WOOL': 'minecraft:white_wool', 'VINE': 'minecraft:vine',
}
# `MapColor` constants by id.
MAP_COLORS = ['NONE', 'GRASS', 'SAND', 'WOOL', 'FIRE', 'ICE', 'METAL', 'PLANT', 'SNOW', 'CLAY', 'DIRT', 'STONE',
              'WATER', 'WOOD', 'QUARTZ', 'COLOR_ORANGE', 'COLOR_MAGENTA', 'COLOR_LIGHT_BLUE', 'COLOR_YELLOW',
              'COLOR_LIGHT_GREEN', 'COLOR_PINK', 'COLOR_GRAY', 'COLOR_LIGHT_GRAY', 'COLOR_CYAN', 'COLOR_PURPLE',
              'COLOR_BLUE', 'COLOR_BROWN', 'COLOR_GREEN', 'COLOR_RED', 'COLOR_BLACK', 'GOLD', 'DIAMOND', 'LAPIS',
              'EMERALD', 'PODZOL', 'NETHER', 'TERRACOTTA_WHITE', 'TERRACOTTA_ORANGE', 'TERRACOTTA_MAGENTA',
              'TERRACOTTA_LIGHT_BLUE', 'TERRACOTTA_YELLOW', 'TERRACOTTA_LIGHT_GREEN', 'TERRACOTTA_PINK',
              'TERRACOTTA_GRAY', 'TERRACOTTA_LIGHT_GRAY', 'TERRACOTTA_CYAN', 'TERRACOTTA_PURPLE', 'TERRACOTTA_BLUE',
              'TERRACOTTA_BROWN', 'TERRACOTTA_GREEN', 'TERRACOTTA_RED', 'TERRACOTTA_BLACK', 'CRIMSON_NYLIUM',
              'CRIMSON_STEM', 'CRIMSON_HYPHAE', 'WARPED_NYLIUM', 'WARPED_STEM', 'WARPED_HYPHAE', 'WARPED_WART_BLOCK',
              'DEEPSLATE', 'RAW_IRON', 'GLOW_LICHEN']
# `DyeColor.getMapColor()`.
DYE_MAP_COLORS = {'WHITE': 'SNOW', 'ORANGE': 'COLOR_ORANGE', 'MAGENTA': 'COLOR_MAGENTA',
                  'LIGHT_BLUE': 'COLOR_LIGHT_BLUE', 'YELLOW': 'COLOR_YELLOW', 'LIME': 'COLOR_LIGHT_GREEN',
                  'PINK': 'COLOR_PINK', 'GRAY': 'COLOR_GRAY', 'LIGHT_GRAY': 'COLOR_LIGHT_GRAY', 'CYAN': 'COLOR_CYAN',
                  'PURPLE': 'COLOR_PURPLE', 'BLUE': 'COLOR_BLUE', 'BROWN': 'COLOR_BROWN', 'GREEN': 'COLOR_GREEN',
                  'RED': 'COLOR_RED', 'BLACK': 'COLOR_BLACK'}


def of():
    """`BlockBehaviour.Properties.of()`."""
    return {'hardness': 0.0, 'resistance': 0.0, 'sound': 'STONE', 'light': 0, 'map_color': 'NONE',
            'requires_tool': False, 'occludes': True, 'collision': True, 'random_ticks': False}


def vanilla(name, **changes):
    p = of()
    p.update(changes)
    if not p['collision']:
        p['occludes'] = False  # noCollission() clears canOcclude too
    return name, p


# The vanilla blocks Botania copies, as 1.20.1's `Blocks` builds them.
VANILLA = dict([
    vanilla('GLASS', hardness=0.3, resistance=0.3, sound='GLASS', occludes=False),
    vanilla('VINE', hardness=0.2, resistance=0.2, sound='VINE', map_color='PLANT', collision=False,
            random_ticks=True),
    vanilla('RAIL', hardness=0.7, resistance=0.7, sound='METAL', collision=False),
    vanilla('NOTE_BLOCK', hardness=0.8, resistance=0.8, sound='WOOD', map_color='WOOD'),
    vanilla('CARVED_PUMPKIN', hardness=1.0, resistance=1.0, sound='WOOD', map_color='COLOR_ORANGE'),
    vanilla('QUARTZ_BLOCK', hardness=0.8, resistance=0.8, map_color='QUARTZ', requires_tool=True),
    vanilla('POPPY', sound='GRASS', map_color='PLANT', collision=False),
])

# Outlines in pixels (`Block.box`) of the classes whose `getShape` isn't a full cube.
SPECIAL_FLOWER_SHAPE = [[4.8, 0, 4.8, 12.8, 16, 12.8]]
SHAPES = {
    'special_flower': SPECIAL_FLOWER_SHAPE,
    'BotaniaMushroomBlock': SPECIAL_FLOWER_SHAPE,
    'BuriedPetalBlock': [[0, 0, 0, 16, 1.6, 16]],
    'FloatingFlowerBlock': [[1.6, 1.6, 1.6, 14.4, 14.4, 14.4]],
    'FloatingSpecialFlowerBlock': [[1.6, 1.6, 1.6, 14.4, 14.4, 14.4]],
    'flower_pot': [[5, 0, 5, 11, 6, 11]],  # FlowerPotBlock.AABB
    'AnimatedTorchBlock': [[0, 0, 0, 16, 4, 16]],
    'HoveringHourglassBlock': [[4, 0, 4, 12, 18.4, 12]],
    'CorporeaCrystalCubeBlock': [[3, 0, 3, 13, 16, 13]],
    'BotanicalBreweryBlock': [[6, 0.8, 6, 10, 15.2, 10]],
    'CocoonBlock': [[3, 0, 3, 13, 14, 13]],
    'CorporeaIndexBlock': [[2, 2, 2, 14, 14, 14]],
    'EyeOfTheAncientsBlock': [[4, 4, 4, 12, 12, 12]],
    'DrumBlock': [[3, 1, 3, 13, 15, 13]],
    'LuminizerBlock': [[5, 5, 5, 11, 11, 11]],
    'StarfieldCreatorBlock': [[0, 0, 0, 16, 4, 16]],
    'ManaFlameBlock': [[4, 4, 4, 12, 12, 12]],
    'LifeImbuerBlock': [[2, 0, 2, 14, 2, 14]],
    'RunicAltarBlock': [[0, 6, 0, 16, 12, 16], [2, 0, 2, 14, 6, 14]],
    'ManaSplitterBlock': [[4, 0, 4, 12, 16, 12]],
    'LuminizerLauncherBlock': [[0, 0, 0, 16, 4, 16]],
    'TinyPotatoBlock': [[6, 0, 6, 10, 6, 10]],
    'TinyPlanetBlock': [[3, 3, 3, 13, 13, 13]],
    'ManaPrismBlock': [[4, 0, 4, 12, 16, 12]],
    'PylonBlock': [[2, 0, 2, 14, 21, 14]],
    'TeruTeruBozuBlock': [[4, 0.16, 4, 12, 15.84, 12]],
    'SparkTinkererBlock': [[0, 0, 0, 16, 3, 16]],
    'TerrestrialAgglomerationPlateBlock': [[0, 0, 0, 16, 3, 16]],
    'BellowsBlock': [[3, 0, 3, 13, 10, 13]],
    'GaiaHeadBlock': [[4, 0, 4, 12, 8, 12]],  # SkullBlock.SHAPE
}
# Outlines by the `facing` of a carved-pumpkin template: (x axis, z axis).
FACING_SHAPES = {
    'ManaPumpBlock': ([[0, 0, 4, 16, 8, 12]], [[4, 0, 0, 12, 8, 16]]),
    'IncensePlateBlock': ([[6, 0, 2, 10, 1, 14]], [[2, 0, 6, 14, 1, 10]]),
}
# Not full cubes, with outlines by a `facing` the port-pack's declaration doesn't have.
NOT_FULL = {'AvatarBlock', 'WallGaiaHeadBlock'}
FLOWER_CLASSES = {'BotaniaFlowerBlock': '_mystical_flower', 'GlimmeringFlowerBlock': '_shiny_flower',
                  'BuriedPetalBlock': '_buried_petals', 'FloatingFlowerBlock': '_floating_flower',
                  'PetalBlock': '_petal_block', 'BotaniaMushroomBlock': '_mushroom',
                  'BotaniaDoubleFlowerBlock': '_double_flower'}


def split_top(text, sep):
    """`text` split at `sep` outside brackets and strings."""
    parts, depth, start, quoted = [], 0, 0, False
    for i, ch in enumerate(text):
        if ch == '"':
            quoted = not quoted
        elif quoted:
            continue
        elif ch in '([{':
            depth += 1
        elif ch in ')]}':
            depth -= 1
        elif ch == sep and depth == 0:
            parts.append(text[start:i])
            start = i + 1
    parts.append(text[start:])
    return [p.strip() for p in parts]


def call_args(text, open_at):
    """The argument text of the call whose `(` is at `open_at`, and the index after its `)`."""
    depth = 0
    for i in range(open_at, len(text)):
        if text[i] == '(':
            depth += 1
        elif text[i] == ')':
            depth -= 1
            if depth == 0:
                return text[open_at + 1:i], i + 1
    raise ValueError(text[open_at:open_at + 80])


def declarations(text):
    """`public static final <Type> <name> = <expression>;` of a class body."""
    found = {}
    for m in re.finditer(r'(?:public|private) static final [\w.<>]+ (\w+) = ', text):
        depth, i = 0, m.end()
        while True:
            ch = text[i]
            if ch in '([{':
                depth += 1
            elif ch in ')]}':
                depth -= 1
            elif ch == ';' and depth == 0:
                break
            i += 1
        found[m.group(1)] = re.sub(r'\s+\.', '.', ' '.join(text[m.end():i].split()))
    return found


def number(text):
    text = text.strip()
    if text == 'Float.MAX_VALUE':
        return 3.4028234663852886e38
    return float(text.rstrip('Ff'))


def chain(text):
    """`.method(args)...` → [(method, args)]."""
    calls, i = [], 0
    while i < len(text) and text[i] == '.':
        m = re.match(r'\.(\w+)\(', text[i:])
        args, i = call_args(text, i + m.end() - 1)
        calls.append((m.group(1), args))
    return calls


def map_color(arg):
    arg = arg.strip()
    if '->' in arg:  # `state -> state.getValue(AXIS) == Y ? MapColor.A : MapColor.B`: the axis=y colour
        arg = arg.split('?', 1)[1].split(':', 1)[0]
    m = re.fullmatch(r'\s*(MapColor|DyeColor)\.(\w+)\s*', arg)
    if not m:
        raise ValueError(f'map colour {arg}')
    return m.group(2) if m.group(1) == 'MapColor' else DYE_MAP_COLORS[m.group(2)]


def apply(props, calls, where):
    p = dict(props)
    for method, args in calls:
        if method == 'strength':
            values = [number(a) for a in split_top(args, ',')]
            p['hardness'], p['resistance'] = values[0], values[-1]
        elif method == 'instabreak':
            p['hardness'] = p['resistance'] = 0.0
        elif method == 'sound':
            p['sound'] = args.removeprefix('SoundType.')
        elif method == 'lightLevel':
            m = re.fullmatch(r'\s*\w+\s*->\s*(\d+)\s*', args)
            p['light'] = int(m.group(1)) if m else 'state'
        elif method == 'mapColor':
            p['map_color'] = map_color(args)
        elif method == 'requiresCorrectToolForDrops':
            p['requires_tool'] = True
        elif method == 'noOcclusion':
            p['occludes'] = False
        elif method == 'noCollission':
            p['collision'] = False
            p['occludes'] = False  # vanilla's noCollission() also clears canOcclude
        elif method == 'randomTicks':
            p['random_ticks'] = True
        elif method in ('instrument', 'isValidSpawn', 'isViewBlocking', 'isSuffocating', 'offsetType',
                        'pushReaction', 'replaceable', 'noLootTable', 'air'):
            pass
        else:
            raise ValueError(f'{where}: {method}({args}) not known')
    return p


class Table:
    def __init__(self, source):
        read = lambda name: (source / name).read_text()
        names = read('LibBlockNames.java')
        self.strings = dict(re.findall(r'String (\w+) = "([^"]*)";', names))
        for key, first, rest in re.findall(r'String (\w+) = (\w+|"[^"]*") \+ (\w+|"[^"]*");', names):
            self.strings[key] = self.word(first) + self.word(rest)
        self.locations = dict(re.findall(r'ResourceLocation (\w+) = prefix\("([^"]*)"\);', names))
        self.flammable = set(re.findall(r'botania_register\((\w+),', read('BotaniaBlockFlammability.java')))
        self.exprs = {}
        self.exprs.update(declarations(read('BotaniaBlocks.java')))
        for var, expr in declarations(read('BotaniaFlowerBlocks.java')).items():
            self.exprs.setdefault(var, expr)  # its FLOATING_PROPS is BotaniaBlocks'
        self.memo = {}
        self.names = {}  # registry path → variable
        self.registrations(read('BotaniaBlocks.java'), read('BotaniaFlowerBlocks.java'))

    def word(self, token):
        token = token.strip()
        if token.startswith('"'):
            return token.strip('"')
        if token.isdigit():
            return token
        token = token.removeprefix('LibBlockNames.')
        if token in self.strings:
            return self.strings[token]
        m = re.fullmatch(r'\w+\.Variant\.(\w+)\.name\(\)\.toLowerCase\(Locale\.ROOT\)', token)
        if m:
            return m.group(1).lower()
        raise ValueError(f'name part {token}')

    def registrations(self, blocks, flowers):
        body = blocks[blocks.index('public static void registerBlocks'):blocks.index('public static void registerItemBlocks')]
        for var, expr in re.findall(r'r\.accept\((\w+), prefix\((.*?)\)\);', body):
            self.names[''.join(self.word(t) for t in split_top(expr, '+'))] = var
        start = flowers.index('public static void registerBlocks')
        body = flowers[start:flowers.index('public static void', start + 1)]
        for var, expr in re.findall(r'r\.accept\((\w+), (.*?)\);', body):
            self.names[self.flower_name(expr)] = var
        # The colour families, registered in loops over `ColorHelper.supportedColors()`.
        for var, expr in self.exprs.items():
            m = re.match(r'new (\w+)\(DyeColor\.(\w+),', expr)
            if m and m.group(1) in FLOWER_CLASSES:
                self.names[m.group(2).lower() + FLOWER_CLASSES[m.group(1)]] = var
        for var, expr in self.exprs.items():
            m = re.fullmatch(r'flowerPot\((\w+), \d+\)', expr)
            plant = m and next((n for n, v in self.names.items() if v == m.group(1)), None)
            if plant and var not in self.names.values():
                self.names['potted_' + plant] = var

    def flower_name(self, expr):
        """`floating(chibi(LibBlockNames.SUBTILE_X))` and the like."""
        m = re.fullmatch(r'(floating|potted|chibi)\((.*)\)', expr)
        if not m:
            return self.locations[expr.removeprefix('LibBlockNames.')]
        inner = self.flower_name(m.group(2))
        return inner + '_chibi' if m.group(1) == 'chibi' else f'{m.group(1)}_{inner}'

    def kind(self, var):
        """The block's class, or `special_flower` / `flower_pot` for the factories."""
        expr = self.exprs[var]
        if expr.startswith('createSpecialFlowerBlock('):
            return 'special_flower'
        if expr.startswith(('flowerPot(', 'BotaniaBlocks.flowerPot(')):
            return 'flower_pot'
        return re.match(r'new (\w+)', expr).group(1)

    def props(self, var):
        if var in self.memo:
            return self.memo[var]
        expr = self.exprs[var]
        m = re.search(r'(?:BotaniaBlocks\.)?flowerPot\((\w+), (\d+)\)', expr)
        if m:  # BotaniaBlocks.flowerPot
            p = apply(of(), [('instabreak', ''), ('noOcclusion', '')], var)
            p['light'] = int(m.group(2))
        else:
            p = self.properties_in(expr, var)
        self.memo[var] = p
        return p

    def properties_in(self, expr, where):
        for name in ('FLOATING_PROPS', 'FLOWER_PROPS'):
            if re.search(rf'\b{name}\b', expr):
                return self.properties_in(self.exprs[name], name)
        at = expr.index('BlockBehaviour.Properties.')
        m = re.match(r'BlockBehaviour\.Properties\.(of|copy)\(', expr[at:])
        args, end = call_args(expr, at + m.end() - 1)
        if m.group(1) == 'of':
            base = of()
        elif args.startswith('Blocks.'):
            base = VANILLA[args.removeprefix('Blocks.')]
        else:
            base = self.props(args)
        return apply(base, chain(expr[end:]), where)


def template_for(kind, var, table, current):
    """A template of the block's kind, or `current` when the port-pack's is of the right kind."""
    flammable = var in table.flammable
    stone_like = table.props(var)['sound'] != 'WOOD'
    by_kind = {
        'BotaniaFlowerBlock': 'minecraft:poppy', 'GlimmeringFlowerBlock': 'minecraft:poppy',
        'FlowerMotifBlock': 'minecraft:poppy', 'special_flower': 'minecraft:poppy',
        'BuriedPetalBlock': 'minecraft:poppy',
        'BotaniaGlassBlock': 'minecraft:glass', 'PermanentBifrostBlock': 'minecraft:glass',
        'BifrostBlock': 'minecraft:glass', 'BotaniaPaneBlock': 'minecraft:glass_pane',
        'SlabBlock': 'minecraft:oak_slab' if flammable or not stone_like else 'minecraft:stone_slab',
        'BotaniaStairBlock': 'minecraft:oak_stairs' if flammable or not stone_like else 'minecraft:stone_stairs',
        'WallBlock': 'minecraft:cobblestone_wall', 'FenceBlock': 'minecraft:oak_fence',
        'FenceGateBlock': 'minecraft:oak_fence_gate',
        'RotatedPillarBlock': 'minecraft:oak_log' if flammable or not stone_like else 'minecraft:quartz_pillar',
        # The spectral rail is straight only (`RAIL_SHAPE_STRAIGHT`); an unpowered activator rail is a
        # plain straight rail.
        'SpectralRailBlock': 'minecraft:activator_rail',
        'ManaDetectorBlock': 'minecraft:stone', 'EnderOverseerBlock': 'minecraft:stone',
    }
    if kind == 'BotaniaBlock':
        return 'minecraft:oak_planks' if flammable else 'minecraft:stone'
    return by_kind.get(kind, current)


def declaration(name, table, current):
    """The generated declaration of a block the hand-written ones don't cover."""
    var = table.names[name]
    kind = table.kind(var)
    p = table.props(var)
    decl = {'template': template_for(kind, var, table, current['template'])}
    properties = botania_properties(p)
    shape = SHAPES.get(kind)
    facing = FACING_SHAPES.get(kind) if decl['template'] == 'minecraft:carved_pumpkin' else None
    if shape or facing or kind in NOT_FULL or not p['collision']:
        properties['occludes'] = False
    if kind == 'BuriedPetalBlock':
        properties['render_shape'] = 'INVISIBLE'  # BuriedPetalBlock.getRenderShape: only its sparkles
    decl['properties'] = properties
    if kind in ('ManaDetectorBlock', 'EnderOverseerBlock'):
        decl['state_properties'] = {'powered': {'type': 'boolean'}}
    for key in ('state_properties', 'default_state', 'state_light'):
        if key in current and key not in decl:
            decl[key] = current[key]
    pixels = lambda boxes: [[round(v / 16, 4) for v in box] for box in boxes]
    if shape:
        decl['shapes'] = {'default': pixels(shape)}
    if facing:
        x, z = pixels(facing[0]), pixels(facing[1])
        decl['shapes'] = {'property': 'facing', 'values': {'east': x, 'west': x, 'north': z, 'south': z}}
    if not p['collision'] and decl['template'] not in ('minecraft:poppy', 'minecraft:vine',
                                                         'minecraft:activator_rail'):
        decl['collision_shapes'] = {'default': []}
    elif shape and decl['template'] == 'minecraft:poppy':
        decl['collision_shapes'] = {'default': []}  # a declared outline is collision too
    for key in ('components', 'item'):
        if key in current:
            decl[key] = current[key]
    if decl['template'] == 'minecraft:carved_pumpkin' and kind != 'TinyPotatoBlock':
        # Only the tiny potato is worn (TinyPotatoBlockItem.getEquipmentSlot).
        decl.setdefault('components', {})['!minecraft:equippable'] = {}
    return decl


def botania_properties(p):
    out = {'hardness': p['hardness'], 'explosion_resistance': p['resistance']}
    if p['light'] != 'state':
        out['light'] = p['light']
    out['requires_tool'] = p['requires_tool']
    out['map_color'] = MAP_COLORS.index(p['map_color'])
    out['sounds'] = SOUNDS[p['sound']]
    if not p['occludes']:
        out['occludes'] = False
    if p['random_ticks']:
        out['random_ticks'] = True
    return out


def dump(value, indent=0):
    """JSON with two-space indents and each box on one line."""
    pad = '  ' * indent
    if isinstance(value, dict):
        if not value:
            return '{}'
        inner = ',\n'.join(f'{pad}  {json.dumps(k)}: {dump(v, indent + 1)}' for k, v in value.items())
        return '{\n' + inner + '\n' + pad + '}'
    if isinstance(value, list) and value and all(isinstance(v, list) for v in value):
        inner = ',\n'.join(f'{pad}  {dump(v, indent + 1)}' for v in value)
        return '[\n' + inner + '\n' + pad + ']'
    return json.dumps(value)


def fetch(folder):
    for name, path in SOURCES.items():
        url = f'https://raw.githubusercontent.com/VazkiiMods/Botania/{COMMIT}/{path}'
        with urllib.request.urlopen(url, timeout=60) as response:
            (folder / name).write_bytes(response.read())


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    parser.add_argument('--source', type=Path, help='a folder with the four Botania sources')
    args = parser.parse_args()
    if args.source:
        table = Table(args.source)
    else:
        with tempfile.TemporaryDirectory(prefix='botania-blocks-') as folder:
            fetch(Path(folder))
            table = Table(Path(folder))
    with zipfile.ZipFile(HERE / 'dist/botania.hmod') as hmod:
        carried = {Path(n).stem: json.loads(hmod.read(n)) for n in hmod.namelist()
                   if n.startswith('data/botania/horizon/block/') and n.endswith('.json')}
    missing = sorted(set(carried) - set(table.names))
    if missing:
        sys.exit(f'not in Botania\'s block table: {", ".join(missing)}')
    changed = []
    for name in sorted(carried):
        path = OUT / f'{name}.json'
        current = json.loads(path.read_text()) if path.exists() else carried[name]
        if name in HAND:
            properties = dict(current.get('properties', {}))
            for key, value in botania_properties(table.props(table.names[name])).items():
                properties.setdefault(key, value)
            decl = {'template': current['template'], 'properties': properties,
                    **{k: v for k, v in current.items() if k not in ('template', 'properties')}}
        else:
            decl = declaration(name, table, current)
        text = dump(decl) + '\n'
        if not path.exists() or path.read_text() != text:
            path.write_text(text)
            changed.append(name)
        if decl['template'] != carried[name]['template']:
            print(f'{name}: template {carried[name]["template"]} -> {decl["template"]}')
    print(f'{len(changed)} declarations written, {len(carried)} blocks')


if __name__ == '__main__':
    main()
