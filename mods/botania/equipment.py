#!/usr/bin/env python3
"""Write `content/data/botania/horizon/item/*.json` for Botania's weapons, tools and armour.

    python3 mods/botania/equipment.py

horizon-port-pack declares every item it can't read from data as a stick. Botania's tools and armour
take their numbers in code (BotaniaAPIImpl.ItemTier and ArmorMaterial, the item classes'
constructors, BotaniaItems' durabilities: Botania 1.20.1-456), so they are written here as 26.3
components over the vanilla item of the same kind and mining level. What Botania adds on top in code
(mana repair, the Terra Blade's beam, the Thundercaller's chain lightning, the Starcaller's stars,
the Terra Shatterer's area mining, set bonuses, Botania's armour model) is not here.
"""
import json
from pathlib import Path

OUT = Path(__file__).resolve().parent / 'content/data/botania/horizon/item'

# ItemTier: uses, mining speed, attack damage bonus, mining level, enchantability, repair item.
TIERS = {
    'manasteel': (300, 6.2, 2.0, 3, 20, 'botania:manasteel_ingot'),
    'elementium': (720, 6.2, 2.0, 3, 20, 'botania:elementium_ingot'),
    'terrasteel': (2300, 9.0, 4.0, 4, 26, 'botania:terrasteel_ingot'),
    'vitreous': (125, 4.8, 0.0, 0, 10, 'minecraft:glass'),
}
INCORRECT = {0: 'wooden', 3: 'diamond', 4: 'netherite'}
TEMPLATE_MATERIAL = {0: 'wooden', 3: 'diamond', 4: 'netherite'}

# Item: (tier, kind, attack damage modifier of the class, attack speed, durability if its own).
TOOLS = {
    'manasteel_sword': ('manasteel', 'sword', 3, -2.4, None),
    'manasteel_pick': ('manasteel', 'pickaxe', 1, -2.8, None),
    'manasteel_shovel': ('manasteel', 'shovel', 1.5, -3.0, None),
    'manasteel_axe': ('manasteel', 'axe', 6.0, -3.1, None),
    'manasteel_hoe': ('manasteel', 'hoe', None, -1.0, None),
    'elementium_sword': ('elementium', 'sword', 3, -2.4, None),
    'elementium_pickaxe': ('elementium', 'pickaxe', 1, -2.8, None),
    'elementium_shovel': ('elementium', 'shovel', 1.5, -3.0, None),
    'elementium_axe': ('elementium', 'axe', 6.0, -3.1, None),
    'elementium_hoe': ('elementium', 'hoe', None, -1.0, None),
    'terra_sword': ('terrasteel', 'sword', 3, -2.4, None),
    'terra_pick': ('terrasteel', 'pickaxe', 1, -2.8, None),
    'terra_axe': ('terrasteel', 'axe', 5.0, -3.0, None),
    'star_sword': ('terrasteel', 'sword', 3, -2.4, None),
    'thunder_sword': ('terrasteel', 'sword', 3, -1.5, None),
    'ender_dagger': ('manasteel', 'sword', 3, -1.25, 69),
    'glass_pickaxe': ('vitreous', 'pickaxe', 1, -1.0, None),
}
MINEABLE = {'pickaxe': 'pickaxe', 'shovel': 'shovel', 'axe': 'axe', 'hoe': 'hoe'}

# ArmorMaterial: durability multiplier, defence (helmet, chestplate, leggings, boots), enchantability,
# toughness, repair item; and the vanilla look worn (Botania draws its own model). A content item can
# only set component types some vanilla item has by default, so no dyed_color: leather stays brown.
ARMOUR = {
    'manasteel': (16, (2, 6, 5, 2), 18, 0.0, 'botania:manasteel_ingot', 'diamond'),
    'manaweave': (5, (1, 3, 2, 1), 18, 0.0, 'botania:manaweave_cloth', 'leather'),
    'elementium': (18, (2, 6, 5, 2), 18, 0.0, 'botania:elementium_ingot', 'iron'),
    'terrasteel': (34, (3, 8, 6, 3), 26, 3.0, 'botania:terrasteel_ingot', 'netherite'),
}
PIECES = (('helmet', 'head', 11), ('chestplate', 'chest', 16), ('leggings', 'legs', 15), ('boots', 'feet', 13))


def f32(value):
    """Java float, as vanilla's components print it (-2.4 is -2.4000000953674316)."""
    import struct
    return struct.unpack('f', struct.pack('f', value))[0]


def write(name, data):
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / f'{name}.json').write_text(json.dumps(data, indent=2) + '\n')


def tool(name, tier, kind, damage, speed, durability):
    uses, mining, bonus, level, enchant, repair = TIERS[tier]
    # SwordItem and DiggerItem: the class's damage plus the tier's bonus; a hoe's damage is
    # minus the bonus (ManasteelHoeItem), so its modifier is 0.
    amount = 0.0 if kind == 'hoe' else float(damage) + bonus
    components = {
        'minecraft:max_damage': durability or uses,
        'minecraft:enchantable': {'value': enchant},
        'minecraft:repairable': {'items': repair},
        'minecraft:attribute_modifiers': [
            {'type': 'minecraft:attack_damage', 'id': 'minecraft:base_attack_damage', 'amount': amount,
             'operation': 'add_value', 'slot': 'mainhand'},
            {'type': 'minecraft:attack_speed', 'id': 'minecraft:base_attack_speed', 'amount': f32(speed),
             'operation': 'add_value', 'slot': 'mainhand'},
        ],
    }
    material = TEMPLATE_MATERIAL[level]
    if kind != 'sword':
        components['minecraft:tool'] = {'rules': [
            {'blocks': f'#minecraft:incorrect_for_{INCORRECT[level]}_tool', 'correct_for_drops': False},
            {'blocks': f'#minecraft:mineable/{MINEABLE[kind]}', 'correct_for_drops': True, 'speed': f32(mining)},
        ]}
    if material == 'netherite':
        components['!minecraft:damage_resistant'] = {}
    write(name, {'template': f'minecraft:{material}_{kind}', 'components': components})


def armour(material, multiplier, defence, enchant, toughness, repair, look):
    for (piece, slot, health), armor in zip(PIECES, defence):
        components = {
            'minecraft:max_damage': health * multiplier,
            'minecraft:enchantable': {'value': enchant},
            'minecraft:repairable': {'items': repair},
            'minecraft:attribute_modifiers': [
                {'type': 'minecraft:armor', 'id': f'minecraft:armor.{piece}', 'amount': float(armor),
                 'operation': 'add_value', 'slot': slot},
                {'type': 'minecraft:armor_toughness', 'id': f'minecraft:armor.{piece}', 'amount': toughness,
                 'operation': 'add_value', 'slot': slot},
            ],
        }
        if look == 'netherite':
            components['!minecraft:damage_resistant'] = {}
        write(f'{material}_{piece}', {'template': f'minecraft:{look}_{piece}', 'components': components})


def main():
    for name, spec in TOOLS.items():
        tool(name, *spec)
    for name in ('manasteel_shears', 'elementium_shears'):
        # ManasteelShearsItem and ElementiumShearsItem: vanilla shears with 238 uses, as vanilla's.
        write(name, {'template': 'minecraft:shears'})
    for name in ('livingwood_bow', 'crystal_bow'):
        write(name, {'template': 'minecraft:bow', 'components': {'minecraft:max_damage': 500}})
    for material, spec in ARMOUR.items():
        armour(material, *spec)
    print(f'{len(list(OUT.glob("*.json")))} files in {OUT}')


if __name__ == '__main__':
    main()
