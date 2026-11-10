//! `ManaItemHandlerImpl`: a player's mana items pay for tools, armour and other items
//! (`requestMana`, `requestManaExact`, the `ForTool` variants with the armour's discount) and take
//! mana some items make (`dispatchMana`, `dispatchManaExact`), and `ToolCommons.damageItemIfPossible`,
//! which pays mana instead of durability.
//!
//! The mana items are those of the player's main inventory and off hand, in that order
//! (`getManaItems`: `Inventory.items` then `offhand`; armour slots don't count). No accessories
//! inventory exists in Horizon (`getManaAccesories` is empty).

use crate::mana_item::ManaStack;

/// The stack asking for or sending mana (`ManaItemHandler`'s `stack` argument).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Asker {
    /// Its own `ManaItem`, if it is one (`findManaItem(stack)`).
    pub mana: Option<ManaStack>,
    /// It is in `#botania:terra_pick_blacklist` (what a Terra Shatterer asks of a sender).
    pub terra_blacklisted: bool,
}

/// `requestMana`: takes up to `to_get` from the mana items, in order, and says how much it got
/// (removing it when `remove`). `items` are the player's mana items without the asking stack.
pub fn request_mana(items: &mut [ManaStack], asker: &Asker, to_get: i32, remove: bool) -> i32 {
    let mut received: i32 = 0;
    for item in items.iter_mut() {
        if item.can_export_to_item() && item.mana() > 0 {
            if let Some(r) = &asker.mana
                && !r.can_receive_from_item(item.terra_blacklisted)
            {
                continue;
            }
            let mana = to_get.wrapping_sub(received).min(item.mana());
            if remove {
                item.add_mana(mana.wrapping_neg());
            }
            received = received.wrapping_add(mana);
            if received >= to_get {
                break;
            }
        }
    }
    received
}

/// `requestManaExact`: takes exactly `to_get` or nothing.
pub fn request_mana_exact(items: &mut [ManaStack], asker: &Asker, to_get: i32, remove: bool) -> bool {
    let mut received: i32 = 0;
    let mut taken: Vec<(usize, i32)> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        if item.can_export_to_item() {
            if let Some(r) = &asker.mana
                && !r.can_receive_from_item(item.terra_blacklisted)
            {
                continue;
            }
            let mana = to_get.wrapping_sub(received).min(item.mana());
            if remove {
                taken.push((i, mana));
            }
            received = received.wrapping_add(mana);
            if received >= to_get {
                break;
            }
        }
    }
    if received == to_get {
        for (i, mana) in taken {
            items[i].add_mana(mana.wrapping_neg());
        }
        true
    } else {
        false
    }
}

/// `dispatchMana`: gives `to_send` to the first mana item that takes mana from the sender, and
/// says how much fitted (it is given all of it: `addMana` clamps).
pub fn dispatch_mana(items: &mut [ManaStack], sender: &Asker, to_send: i32, add: bool) -> i32 {
    for item in items.iter_mut() {
        if item.can_receive_from_item(sender.terra_blacklisted) {
            if let Some(s) = &sender.mana
                && !s.can_export_to_item()
            {
                continue;
            }
            let sum = item.mana().wrapping_add(to_send);
            let received = if sum <= item.max_mana() {
                to_send
            } else {
                to_send.wrapping_sub(sum.wrapping_sub(item.max_mana()))
            };
            if add {
                item.add_mana(to_send);
            }
            return received;
        }
    }
    0
}

/// `dispatchManaExact`: gives `to_send` to the first mana item it fits in whole.
pub fn dispatch_mana_exact(items: &mut [ManaStack], sender: &Asker, to_send: i32, add: bool) -> bool {
    for item in items.iter_mut() {
        if item.mana().wrapping_add(to_send) <= item.max_mana()
            && item.can_receive_from_item(sender.terra_blacklisted)
        {
            if let Some(s) = &sender.mana
                && !s.can_export_to_item()
            {
                continue;
            }
            if add {
                item.add_mana(to_send);
            }
            return true;
        }
    }
    false
}

/// `getFullDiscountForTools`: the worn armour's `ManaDiscountArmor.getDiscount`s, added in
/// `Inventory.armor`'s order (feet first; only helmets give one), then 5% per Unbreaking level of
/// the tool, in `float`.
pub fn full_discount(armor_discounts: &[f32], unbreaking: i32) -> f32 {
    let mut discount = 0.0_f32;
    for d in armor_discounts {
        discount += *d;
    }
    discount += unbreaking as f32 * 0.05;
    discount
}

/// `discountManaForTool`: `(int) (cost * max(0, 1 - discount))` in `float`.
pub fn discount_cost(cost: i32, discount: f32) -> i32 {
    let multiplier = (1.0_f32 - discount).max(0.0);
    (cost as f32 * multiplier) as i32
}

/// What `damageItemIfPossible` asks of the game: whether the player's mana items can pay
/// `requestManaExactForTool(stack, player, manaPerDamage, remove)`, and the level's random
/// (`level.getRandom().nextInt(bound)`).
pub trait DamagePayer {
    fn pay(&mut self, remove: bool) -> bool;
    fn next_int(&mut self, bound: i32) -> i32;
}

/// `ToolCommons.damageItemIfPossible`: each point of damage paid with mana instead, while the
/// mana lasts (Unbreaking makes a paid point free with a chance of `level / (level + 1)`). The
/// damage left to the item.
pub fn damage_item_if_possible(amount: i32, unbreaking: i32, payer: &mut impl DamagePayer) -> i32 {
    if amount == 0 {
        return amount;
    }
    let mut amount = amount;
    while amount > 0 {
        if payer.pay(false) {
            if payer.next_int(unbreaking + 1) == 0 {
                payer.pay(true);
            }
            amount -= 1;
        } else {
            break;
        }
    }
    amount
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mana_item::{Kind, ManaStack, Nbt};

    // Expected values: Botania's `ManaItemHandlerImpl` and `ToolCommons.damageItemIfPossible`
    // run in Java over the same items (see the bundle's notes on the oracle), not this port.

    fn item(kind: Kind, mana: i32) -> ManaStack {
        ManaStack::new(
            kind,
            1,
            Nbt {
                mana,
                ..Nbt::default()
            },
        )
    }

    fn manas(items: &[ManaStack]) -> Vec<i32> {
        items.iter().map(ManaStack::mana).collect()
    }

    #[test]
    fn request_takes_in_order() {
        let tool = Asker::default();
        let mut items = vec![
            item(Kind::Tablet, 100),
            item(Kind::Band, 0),
            item(Kind::GreaterBand, 1000),
        ];
        assert_eq!(request_mana(&mut items, &tool, 500, false), 500);
        assert_eq!(manas(&items), vec![100, 0, 1000], "a simulation removes nothing");
        assert_eq!(request_mana(&mut items, &tool, 500, true), 500);
        assert_eq!(manas(&items), vec![0, 0, 600]);
        assert_eq!(request_mana(&mut items, &tool, 1000, true), 600);
        assert_eq!(manas(&items), vec![0, 0, 0]);
        // A Terra Shatterer gives nothing; a creative tablet gives without end.
        let mut items = vec![
            item(Kind::TerraPick, 50_000),
            ManaStack::new(
                Kind::Tablet,
                1,
                Nbt {
                    creative: true,
                    ..Nbt::default()
                },
            ),
        ];
        assert_eq!(request_mana(&mut items, &tool, 2000, true), 2000);
        assert_eq!(manas(&items), vec![50_000, 501_000]);
    }

    #[test]
    fn exact_takes_all_or_nothing() {
        let tool = Asker::default();
        let mut items = vec![item(Kind::Tablet, 100), item(Kind::Band, 30)];
        assert!(!request_mana_exact(&mut items, &tool, 200, true));
        assert_eq!(manas(&items), vec![100, 30]);
        assert!(request_mana_exact(&mut items, &tool, 120, true));
        assert_eq!(manas(&items), vec![0, 10]);
        assert!(request_mana_exact(&mut items, &tool, 0, true), "nothing asked: paid");
        // A tablet asking takes from a band; a creative one takes from nothing.
        let tablet = Asker {
            mana: Some(item(Kind::Tablet, 0)),
            terra_blacklisted: false,
        };
        let mut items = vec![item(Kind::Band, 500)];
        assert!(request_mana_exact(&mut items, &tablet, 400, true));
        assert_eq!(manas(&items), vec![100]);
        let creative = Asker {
            mana: Some(ManaStack::new(
                Kind::Tablet,
                1,
                Nbt {
                    creative: true,
                    ..Nbt::default()
                },
            )),
            terra_blacklisted: false,
        };
        assert_eq!(request_mana(&mut items, &creative, 50, true), 0);
        // A Terra Shatterer refuses blacklisted senders.
        let pick = Asker {
            mana: Some(item(Kind::TerraPick, 0)),
            terra_blacklisted: false,
        };
        let mut black = item(Kind::Band, 500);
        black.terra_blacklisted = true;
        let mut items = vec![black, item(Kind::Tablet, 300)];
        assert_eq!(request_mana(&mut items, &pick, 400, true), 300);
        assert_eq!(manas(&items), vec![500, 0]);
    }

    #[test]
    fn dispatch_fills_the_first_taker() {
        // The Terrasteel Helmet's 10 mana: blacklisted for a Terra Shatterer.
        let helm = Asker {
            mana: None,
            terra_blacklisted: true,
        };
        let mut items = vec![
            item(Kind::TerraPick, 0),
            item(Kind::Mirror, 0),
            item(Kind::Tablet, 499_995),
            item(Kind::Band, 0),
        ];
        assert!(dispatch_mana_exact(&mut items, &helm, 10, true));
        assert_eq!(manas(&items), vec![0, 0, 499_995, 10]);
        // dispatchMana stops at the first taker, full or not, and reports what fitted.
        let mut items = vec![item(Kind::Tablet, 499_995), item(Kind::Band, 0)];
        assert_eq!(dispatch_mana(&mut items, &helm, 10, true), 5);
        assert_eq!(manas(&items), vec![500_000, 0]);
        assert_eq!(dispatch_mana(&mut items, &helm, 10, false), 0);
        let none = Asker::default();
        let mut picks = vec![item(Kind::TerraPick, 0)];
        assert!(dispatch_mana_exact(&mut picks, &none, 10, true));
        assert_eq!(manas(&picks), vec![10]);
    }

    #[test]
    fn discount_follows_botania_floats() {
        // (cost, helmet discount, Unbreaking) -> cost, as Java's float arithmetic gives them.
        let cases = [
            (60, 0.0, 0, 60),
            (60, 0.0, 3, 51),
            (60, 0.1, 0, 54),
            (60, 0.4, 0, 36),
            (60, 0.4, 1, 32),
            (60, 0.2, 3, 39),
            (120, 0.1, 0, 108),
            (120, 0.4, 1, 65),
            (120, 0.2, 2, 84),
            (30, 0.4, 3, 13),
            (100, 0.4, 1, 54),
            (200, 0.4, 1, 109),
            (240, 0.4, 1, 131),
            (1000, 0.4, 1, 549),
            (7, 0.2, 1, 5),
            (7, 0.4, 0, 4),
            (60, 0.4, 12, 0),
        ];
        for (cost, helm, unbreaking, want) in cases {
            let d = full_discount(&[helm], unbreaking);
            assert_eq!(discount_cost(cost, d), want, "{cost} {helm} {unbreaking}");
        }
    }

    struct Script {
        mana: i32,
        cost: i32,
        rolls: Vec<i32>,
        asked: Vec<i32>,
    }

    impl DamagePayer for Script {
        fn pay(&mut self, remove: bool) -> bool {
            if self.mana < self.cost {
                return false;
            }
            if remove {
                self.mana -= self.cost;
            }
            true
        }
        fn next_int(&mut self, bound: i32) -> i32 {
            self.asked.push(bound);
            self.rolls.remove(0)
        }
    }

    #[test]
    fn damage_paid_with_mana() {
        // 5 points, 250 mana at 60 a point, no Unbreaking: 4 paid, 1 left, 10 mana left.
        let mut s = Script {
            mana: 250,
            cost: 60,
            rolls: vec![0; 8],
            asked: Vec::new(),
        };
        assert_eq!(damage_item_if_possible(5, 0, &mut s), 1);
        assert_eq!(s.mana, 10);
        assert_eq!(s.asked, vec![1, 1, 1, 1]);
        // Unbreaking II: a roll of 0 pays, others are free.
        let mut s = Script {
            mana: 120,
            cost: 60,
            rolls: vec![1, 0, 2, 0],
            asked: Vec::new(),
        };
        assert_eq!(damage_item_if_possible(4, 2, &mut s), 0);
        assert_eq!(s.mana, 0);
        assert_eq!(s.asked, vec![3, 3, 3, 3]);
        // No mana: all of it is left, and nothing is drawn.
        let mut s = Script {
            mana: 0,
            cost: 60,
            rolls: Vec::new(),
            asked: Vec::new(),
        };
        assert_eq!(damage_item_if_possible(3, 0, &mut s), 3);
        assert_eq!(damage_item_if_possible(0, 0, &mut s), 0);
    }
}
