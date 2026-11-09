//! 1.20 and 1.20.1 (protocol 763) to 1.20.2: the packets whose layout changed, read off both
//! releases' server jars (`tools/compare_packet_layouts.py`), and the ids 1.20.2 renumbered.
//!
//! 1.20.1 has no configuration state: its play login carries the registries, which the session
//! makes a configuration of before the login ([`login`]). Its network NBT carries a root name, which
//! the readers take while its packets are translated (`buf::named_nbt`); the packets that would pass
//! on undecoded are written again here without it (tag queries, mob effects).
use super::*;
use crate::adapter::pinned_ids::entity_type as entity;

/// 1.20.1's `add_player` (play), which 1.20.2 replaced with `add_entity` ([`add_player`]).
pub(crate) const ADD_PLAYER: i32 = 0x03;
/// 1.20.1's `recipe` (play), the recipe book's changes: 1.20.2 and 1.20.3 number it 63.
pub(crate) const RECIPE_BOOK: i32 = 0x3d;
/// 1.20.1's serverbound `swing` (play).
pub(crate) const SWING: i32 = 0x2f;

/// 1.20.1's packet in 1.20.2's layout; `None` when it is laid out alike. Ids stay the server's,
/// but for mob effects, which 1.20.1 numbered from 1. The play login is the session's
/// ([`login`]): it arrives here already in 1.20.2's layout.
pub(super) fn clientbound(
    session: &Session,
    state: State,
    id: i32,
    payload: &[u8],
) -> Result<Option<Vec<u8>>> {
    if state != State::Play {
        return Ok(None);
    }
    use packet_ids::play::clientbound as cb;
    let mut r = Reader::new(payload);
    let mut w = Writer::new();
    match id {
        cb::RESPAWN => respawn(&mut r, &mut w)?,
        cb::FORGET_LEVEL_CHUNK => {
            // Two Ints (x, then z) became FriendlyByteBuf.writeChunkPos: ChunkPos.toLong, x in the
            // low half.
            let x = r.i32()?;
            let z = r.i32()?;
            w.i64(i64::from(x as u32) | (i64::from(z) << 32));
        }
        cb::UPDATE_ADVANCEMENTS => advancements(&mut r, &mut w)?,
        cb::UPDATE_MOB_EFFECT => {
            w.varint(r.varint()?);
            w.varint(mob_effect(session, r.varint()?)?);
            w.u8(r.u8()?); // amplifier
            w.varint(r.varint()?); // duration
            w.u8(r.u8()?); // flags
            let factor = r.bool()?;
            w.bool(factor);
            if factor {
                w.nbt(&r.nbt()?);
            }
        }
        cb::REMOVE_MOB_EFFECT => {
            w.varint(r.varint()?);
            w.varint(mob_effect(session, r.varint()?)?);
        }
        cb::TAG_QUERY => {
            w.varint(r.varint()?);
            w.nbt(&r.nbt()?);
        }
        _ => return Ok(None),
    }
    finish(&r)?;
    Ok(Some(w.data))
}

fn finish(r: &Reader) -> Result<()> {
    if r.remaining() == 0 {
        Ok(())
    } else {
        Err(crate::Error::new("trailing 1.20.1 packet bytes"))
    }
}

/// 1.20.1 registers its mob effects from 1 (MobEffects' explicit ids); 1.20.2 from 0, as the pinned
/// version still numbers those 33.
fn mob_effect(session: &Session, id: i32) -> Result<i32> {
    session
        .version
        .ids()
        .and_then(|ids| ids.mob_effect.up(id))
        .ok_or_else(|| crate::Error::new("unknown 1.20.1 mob effect"))
}

/// An optional GlobalPos: the dimension, then the packed block position.
fn last_death(r: &mut Reader, w: &mut Writer) -> Result<()> {
    let present = r.bool()?;
    w.bool(present);
    if present {
        w.string(&r.string(32767)?);
        w.i64(r.i64()?);
    }
    Ok(())
}

/// 1.20.1's respawn keeps its data-kept flags after the flat flag; 1.20.2's CommonPlayerSpawnInfo
/// leaves them for the end, after the last death and the portal cooldown.
fn respawn(r: &mut Reader, w: &mut Writer) -> Result<()> {
    w.string(&r.string(32767)?); // dimension type
    w.string(&r.string(32767)?); // dimension
    w.i64(r.i64()?); // hashed seed
    w.u8(r.u8()?); // game type
    w.i8(r.i8()?); // previous game type, -1 for none
    w.bool(r.bool()?); // debug
    w.bool(r.bool()?); // flat
    let kept = r.u8()?;
    last_death(r, w)?;
    w.varint(r.varint()?); // portal cooldown
    w.u8(kept);
    Ok(())
}

/// 1.20.1's Advancement.Builder sends its criteria by name (no data) before the requirements;
/// 1.20.2's Advancement only the requirements, which name them. Everything else is copied as it
/// came: components are still JSON and items still carry 1.20.1's NBT for the later steps.
fn advancements(r: &mut Reader, w: &mut Writer) -> Result<()> {
    w.bool(r.bool()?); // reset
    let n = r.count(r.remaining())?;
    w.varint(n as i32);
    for _ in 0..n {
        w.string(&r.string(32767)?);
        let parent = r.bool()?;
        w.bool(parent);
        if parent {
            w.string(&r.string(32767)?);
        }
        let display = r.bool()?;
        w.bool(display);
        if display {
            let start = r.position();
            r.string(262144)?; // title
            r.string(262144)?; // description
            if r.bool()? {
                r.varint()?;
                r.i8()?;
                r.nbt()?;
            }
            r.varint()?; // frame
            let flags = r.i32()?;
            if flags & 1 != 0 {
                r.string(32767)?; // background
            }
            r.f32()?;
            r.f32()?;
            w.bytes(r.since(start));
        }
        for _ in 0..r.count(r.remaining())? {
            r.string(32767)?; // a criterion's name; Criterion.serializeToNetwork writes nothing more
        }
        let start = r.position();
        for _ in 0..r.count(r.remaining())? {
            for _ in 0..r.count(r.remaining())? {
                r.string(32767)?;
            }
        }
        r.bool()?; // sends telemetry
        w.bytes(r.since(start));
    }
    // The removed advancements and the progress are laid out alike.
    w.bytes(r.rest());
    Ok(())
}

/// 1.20.1's play login: the registries it carries (RegistrySynchronization.NETWORK_CODEC, one
/// compound of every synchronized registry, as 1.20.2's configuration sends them), and the login
/// in 1.20.2's layout. 1.20.2 moved the game types after the levels into CommonPlayerSpawnInfo, the
/// view distances and death screen ahead of it, and added whether crafting is limited (a game rule
/// 1.20.1 never told its clients: off).
pub(crate) fn login(payload: &[u8]) -> Result<(horizon_world::nbt::Tag, Vec<u8>)> {
    let mut r = Reader::new(payload);
    let mut w = Writer::new();
    w.i32(r.i32()?); // player id
    w.bool(r.bool()?); // hardcore
    let game_type = r.u8()?;
    let previous = r.i8()?;
    let levels = r.count(r.remaining())?;
    w.varint(levels as i32);
    for _ in 0..levels {
        w.string(&r.string(32767)?);
    }
    let registries = r.nbt()?;
    if !matches!(registries, horizon_world::nbt::Tag::Compound(_)) {
        return Err(crate::Error::new("1.20.1 login without registries"));
    }
    let dimension_type = r.string(32767)?;
    let dimension = r.string(32767)?;
    let seed = r.i64()?;
    for _ in 0..3 {
        w.varint(r.varint()?); // max players, view distance, simulation distance
    }
    w.bool(r.bool()?); // reduced debug info
    w.bool(r.bool()?); // show death screen
    w.bool(false); // do limited crafting
    let debug = r.bool()?;
    let flat = r.bool()?;
    w.string(&dimension_type);
    w.string(&dimension);
    w.i64(seed);
    w.u8(game_type);
    w.i8(previous);
    w.bool(debug);
    w.bool(flat);
    last_death(&mut r, &mut w)?;
    w.varint(r.varint()?); // portal cooldown
    finish(&r)?;
    Ok((registries, w.data))
}

/// 1.20.1's add_player (entity, profile id, position, yaw, pitch) as 1.20.2's AddEntity of a player:
/// the type in 1.20.1's numbering (the later steps renumber it), the head turned as the body, no
/// data and no motion, as ClientPacketListener.handleAddPlayer left them.
pub(crate) fn add_player(session: &Session, payload: &[u8]) -> Result<Vec<u8>> {
    let mut r = Reader::new(payload);
    let id = r.varint()?;
    let uuid = r.uuid()?;
    let position = [r.f64()?, r.f64()?, r.f64()?];
    let yaw = r.i8()?;
    let pitch = r.i8()?;
    finish(&r)?;
    let player = session
        .version
        .ids()
        .and_then(|ids| ids.entity_type.down(entity::PLAYER))
        .ok_or_else(|| crate::Error::new("1.20.1 has no player entity type"))?;
    let mut w = Writer::new();
    w.varint(id);
    w.uuid(uuid);
    w.varint(player);
    for v in position {
        w.f64(v);
    }
    w.i8(pitch);
    w.i8(yaw);
    w.i8(yaw); // head
    w.varint(0); // data
    for _ in 0..3 {
        w.i16(0); // motion
    }
    Ok(w.data)
}

/// 1.20.2 gave display entities a position/rotation interpolation duration (data 10, after the
/// transformation's start and duration): 1.20.1's later display entries move up one.
pub(crate) fn entity_data(session: &Session, data: &mut play::SetEntityData) {
    let kind = session.kinds.lock().unwrap().get(&data.id).copied();
    if matches!(
        kind,
        Some(entity::BLOCK_DISPLAY | entity::ITEM_DISPLAY | entity::TEXT_DISPLAY)
    ) {
        for (index, _, _) in &mut data.values {
            if *index >= 10 {
                *index += 1;
            }
        }
    }
}

/// Horizon's login start (name, profile id) as 1.20.1 reads it: the profile id was optional.
pub(crate) fn hello(payload: &[u8]) -> Result<Vec<u8>> {
    let mut r = Reader::new(payload);
    let name = r.string(16)?;
    let uuid = r.uuid()?;
    finish(&r)?;
    let mut w = Writer::new();
    w.string(&name);
    w.bool(true);
    w.uuid(uuid);
    Ok(w.data)
}

/// Horizon's beacon effects in 1.20.1's numbering (from 1); one 1.20.1 lacks is sent as none.
pub(crate) fn set_beacon(ids: &Ids, payload: &[u8]) -> Result<Vec<u8>> {
    let mut r = Reader::new(payload);
    let mut w = Writer::new();
    for _ in 0..2 {
        let effect = if r.bool()? {
            ids.mob_effect.down(r.varint()?)
        } else {
            None
        };
        w.bool(effect.is_some());
        if let Some(effect) = effect {
            w.varint(effect);
        }
    }
    finish(&r)?;
    Ok(w.data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buf::named_nbt;
    use horizon_world::nbt::Tag;

    // Every 1.20.1 payload below is written by hand after the 1.20.1 server jar's packet writers
    // (field order read with javap and server.txt), and every expected 1.20.2 payload after the
    // 1.20.2 jar's; neither comes from the code under test.

    fn hex(s: &str) -> Vec<u8> {
        let s: String = s.split_whitespace().collect();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
    fn cat(parts: &[&str]) -> Vec<u8> {
        hex(&parts.concat())
    }

    /// 1.20.1's play login packet id (ClientboundLoginPacket, 0x28).
    const OLD_LOGIN: i32 = 0x28;

    fn session() -> Session {
        let s = Session::new(ServerVersion::V1_20_1);
        s.set_reading(State::Play);
        s
    }

    /// A string as FriendlyByteBuf.writeUtf writes it (all ASCII here).
    fn utf(s: &str) -> String {
        format!("{:02x}{}", s.len(), hex_of(s.as_bytes()))
    }
    fn hex_of(b: &[u8]) -> String {
        b.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// RegistrySynchronization.NETWORK_CODEC's NBT for one dimension type, as 1.20.1's
    /// FriendlyByteBuf.writeNbt writes it: a named root with an empty name.
    fn named_registries() -> String {
        // {"minecraft:dimension_type":{type:"minecraft:dimension_type",value:[{name:"minecraft:overworld",id:0,element:{height:384}}]}}
        let mut s = String::from("0a0000");
        s += &format!("0a{}", nbt_name("minecraft:dimension_type"));
        s += &format!(
            "08{}{}",
            nbt_name("type"),
            nbt_name("minecraft:dimension_type")
        );
        s += &format!("09{}0a00000001", nbt_name("value"));
        s += &format!("08{}{}", nbt_name("name"), nbt_name("minecraft:overworld"));
        s += &format!("03{}00000000", nbt_name("id"));
        s += &format!(
            "0a{}03{}0000018000",
            nbt_name("element"),
            nbt_name("height")
        );
        s += "00"; // the list entry's end
        s += "00"; // the registry's end
        s += "00"; // the root's end
        s
    }
    fn nbt_name(s: &str) -> String {
        format!("{:04x}{}", s.len(), hex_of(s.as_bytes()))
    }

    fn old_login() -> Vec<u8> {
        cat(&[
            "0000002a", // player id 42
            "00",       // not hardcore
            "01",       // creative
            "ff",       // no previous game type
            "01",       // one level
            &utf("minecraft:overworld"),
            &named_registries(),
            &utf("minecraft:overworld"), // dimension type
            &utf("minecraft:overworld"), // dimension
            "0000000000000007",          // hashed seed
            "14 08 06",                  // max players, view and simulation distance
            "00 01",                     // reduced debug info, show death screen
            "00 01",                     // debug, flat
            "00",                        // no last death
            "00",                        // portal cooldown
        ])
    }

    #[test]
    fn a_1_20_1_login_is_laid_out_as_1_20_2_s() {
        let _named = named_nbt(true, false);
        let (registries, login) = login(&old_login()).unwrap();
        // ClientboundLoginPacket.write in 1.20.2, then CommonPlayerSpawnInfo.write.
        let expected = cat(&[
            "0000002a",
            "00",
            "01",
            &utf("minecraft:overworld"),
            "14 08 06",
            "00 01 00", // reduced debug info, death screen, limited crafting
            &utf("minecraft:overworld"),
            &utf("minecraft:overworld"),
            "0000000000000007",
            "01 ff",
            "00 01", // debug, flat
            "00",    // no last death
            "00",    // portal cooldown
        ]);
        assert_eq!(login, expected);
        let Some(Tag::Compound(dimension_types)) = registries.get("minecraft:dimension_type")
        else {
            panic!("{registries:?}")
        };
        assert_eq!(dimension_types.len(), 2); // its type and its entries
    }

    #[test]
    fn the_play_login_arrives_as_a_configuration_then_the_login() {
        use crate::packets::{configuration as cfg, play};
        let s = session();
        let (id, body) = s.clientbound(OLD_LOGIN, old_login()).unwrap().unwrap();
        let mut packets = vec![(id, body)];
        while let Some(p) = s.next_incoming() {
            packets.push(p);
        }
        // The enabled features first, the registries, the finish, then the play login.
        let cfg::Clientbound::UpdateEnabledFeatures(features) =
            cfg::Clientbound::decode(packets[0].0, &packets[0].1).unwrap()
        else {
            panic!("{:?}", packets[0])
        };
        assert_eq!(features.features, ["minecraft:vanilla"]);
        let (login_id, login) = packets.last().unwrap();
        let play::Clientbound::Login(login) = play::Clientbound::decode(*login_id, login).unwrap()
        else {
            panic!("{login_id}")
        };
        assert_eq!(login.player_id, 42);
        assert_eq!(login.spawn.dimension, "minecraft:overworld");
        assert_eq!(login.spawn.dimension_type, 0);
        assert_eq!(login.spawn.game_type, 1);
        assert_eq!(login.spawn.previous_game_type, None);
        assert!(login.spawn.flat && !login.spawn.debug);
        let configuration = &packets[1..packets.len() - 1];
        let finish = configuration
            .iter()
            .position(|(id, _)| *id == packet_ids::configuration::clientbound::FINISH_CONFIGURATION)
            .unwrap();
        assert_eq!(
            finish,
            configuration.len() - 1,
            "the finish ends the configuration"
        );
        let registries: Vec<String> = configuration[..finish]
            .iter()
            .map(
                |(id, body)| match cfg::Clientbound::decode(*id, body).unwrap() {
                    cfg::Clientbound::RegistryData(r) => r.registry,
                    other => panic!("{other:?}"),
                },
            )
            .collect();
        assert_eq!(registries[0], "minecraft:dimension_type");
        assert!(registries.iter().any(|r| r == "minecraft:world_clock"));
    }

    #[test]
    fn a_respawn_moves_its_data_kept_flags_to_the_end() {
        let s = session();
        let death = format!("01{}0000004000000041", utf("minecraft:overworld"));
        let old = cat(&[
            &utf("minecraft:overworld"),
            &utf("minecraft:the_nether"),
            "0000000000000007",
            "00 02", // survival, previously adventure
            "00 00", // not debug, not flat
            "03",    // keep all data
            &death,
            "05", // portal cooldown
        ]);
        let expected = cat(&[
            &utf("minecraft:overworld"),
            &utf("minecraft:the_nether"),
            "0000000000000007",
            "00 02",
            "00 00",
            &death,
            "05",
            "03", // the data kept, last
        ]);
        assert_eq!(
            clientbound(
                &s,
                State::Play,
                packet_ids::play::clientbound::RESPAWN,
                &old
            )
            .unwrap(),
            Some(expected)
        );
    }

    #[test]
    fn an_added_player_becomes_a_player_entity() {
        use crate::packets::play;
        let s = session();
        let old = cat(&[
            "07",                               // entity 7
            "000000000000000000000000000000ab", // profile id
            "4000000000000000",                 // x 2.0
            "4050000000000000",                 // y 64.0
            "c008000000000000",                 // z -3.0
            "40",                               // yaw
            "f0",                               // pitch
        ]);
        let (id, body) = s.clientbound(ADD_PLAYER, old).unwrap().unwrap();
        let play::Clientbound::AddEntity(e) = play::Clientbound::decode(id, &body).unwrap() else {
            panic!("{id}")
        };
        assert_eq!((e.id, e.uuid, e.kind), (7, 0xab, entity::PLAYER));
        assert_eq!(e.position, [2.0, 64.0, -3.0]);
        assert_eq!((e.yaw, e.pitch, e.head_yaw), (0x40, -0x10, 0x40));
        assert_eq!(e.velocity, [0.0; 3]);
    }

    #[test]
    fn named_nbt_in_a_tag_query_reaches_horizon_without_its_name() {
        use crate::packets::play;
        let s = session();
        // 1.20.1's ClientboundTagQueryPacket: transaction 5, then {a:1b} as a named root.
        let old = hex("05 0a0000 01000161 01 00");
        let (id, body) = s.clientbound(0x66, old).unwrap().unwrap();
        assert_eq!(body, hex("05 0a 01000161 01 00"));
        let play::Clientbound::TagQuery(q) = play::Clientbound::decode(id, &body).unwrap() else {
            panic!("{id}")
        };
        assert_eq!(q.tag, Tag::compound([("a", Tag::Byte(1))]));
    }

    #[test]
    fn mob_effects_count_from_zero_as_1_20_2_s() {
        let s = session();
        use packet_ids::play::clientbound as cb;
        // Entity 3 gets speed (1.20.1's id 1), amplifier 1, 600 ticks, flags 6, no factor data.
        let update = hex("03 01 01 d804 06 00");
        assert_eq!(
            clientbound(&s, State::Play, cb::UPDATE_MOB_EFFECT, &update).unwrap(),
            Some(hex("03 00 01 d804 06 00"))
        );
        // Darkness, 1.20.1's last (33).
        assert_eq!(
            clientbound(&s, State::Play, cb::REMOVE_MOB_EFFECT, &hex("03 21")).unwrap(),
            Some(hex("03 20"))
        );
        // A beacon's speed and haste go back up by one.
        let ids = s.version.ids().unwrap();
        assert_eq!(
            set_beacon(ids, &hex("01 00 01 02")).unwrap(),
            hex("01 01 01 03")
        );
    }

    #[test]
    fn a_forgotten_chunk_is_one_packed_position() {
        let s = session();
        let old = hex("fffffffe 00000005"); // x -2, z 5
        assert_eq!(
            clientbound(
                &s,
                State::Play,
                packet_ids::play::clientbound::FORGET_LEVEL_CHUNK,
                &old
            )
            .unwrap(),
            Some(hex("00000005 fffffffe")) // ChunkPos.asLong: z high, x low
        );
    }

    #[test]
    fn advancements_lose_their_criteria_names() {
        let s = session();
        let tail = "00 00"; // no removed advancements, no progress
        let head = format!("01 01 {}", utf("a:b")); // reset, one advancement, its id
        let criteria = format!("01 {}", utf("c")); // the criteria by name
        let requirements = format!("01 01 {}", utf("c")); // [[c]]
        let old = cat(&[
            &head,
            "00 00", // no parent, no display
            &criteria,
            &requirements,
            "00", // no telemetry
            tail,
        ]);
        let expected = cat(&[&head, "00 00", &requirements, "00", tail]);
        assert_eq!(
            clientbound(
                &s,
                State::Play,
                packet_ids::play::clientbound::UPDATE_ADVANCEMENTS,
                &old
            )
            .unwrap(),
            Some(expected)
        );
    }

    #[test]
    fn display_entries_after_the_new_duration_move_up_one() {
        use crate::entity_data::{Value, serializer};
        let s = session();
        s.kinds.lock().unwrap().insert(4, entity::TEXT_DISPLAY);
        let mut data = play::SetEntityData {
            id: 4,
            values: vec![
                (9, serializer::INT, Value::Int(3)),
                (10, serializer::VECTOR3, Value::Floats(vec![0.0, 1.0, 0.0])),
            ],
        };
        entity_data(&s, &mut data);
        assert_eq!(data.values[0].0, 9);
        assert_eq!(data.values[1].0, 11);
    }

    #[test]
    fn the_login_start_states_an_optional_profile_id() {
        let s = Session::new(ServerVersion::V1_20_1);
        let mut handshake = Writer::new();
        handshake.varint(packet_ids::handshake::serverbound::INTENTION);
        handshake.varint(crate::PROTOCOL_VERSION);
        handshake.string("example.org");
        handshake.u16(25565);
        handshake.varint(crate::packets::handshake::LOGIN);
        s.serverbound(&handshake.data).unwrap();
        let mut hello = Writer::new();
        hello.varint(packet_ids::login::serverbound::HELLO);
        hello.string("Wire");
        hello.uuid(0xab);
        let sent = s.serverbound(&hello.data).unwrap().unwrap();
        assert_eq!(
            sent,
            cat(&["00", &utf("Wire"), "01", "000000000000000000000000000000ab"])
        );
    }

    #[test]
    fn the_configuration_the_client_sends_goes_as_play_or_not_at_all() {
        let s = Session::new(ServerVersion::V1_20_1);
        s.set_reading(State::Login);
        s.set_writing(State::Login);
        // login_finished (1.20.1's game profile): the server is in play from here.
        s.clientbound(
            0x02,
            cat(&["000000000000000000000000000000ab", &utf("Wire"), "00"]),
        )
        .unwrap();
        assert_eq!(s.reading(), State::Play);
        let ack = [packet_ids::login::serverbound::LOGIN_ACKNOWLEDGED as u8];
        assert_eq!(s.serverbound(&ack).unwrap(), None);
        assert_eq!(s.writing(), State::Configuration);
        // The brand: 1.20.1's play custom_payload (0x0d), the payload as it was.
        let mut brand = Writer::new();
        brand.varint(packet_ids::configuration::serverbound::CUSTOM_PAYLOAD);
        brand.string("minecraft:brand");
        brand.string("vanilla");
        let sent = s.serverbound(&brand.data).unwrap().unwrap();
        assert_eq!(sent[0], 0x0d);
        assert_eq!(&sent[1..], &brand.data[1..]);
        // A keep-alive answer has no 1.20.1 meaning before play.
        let keep_alive = [4u8, 0, 0, 0, 0, 0, 0, 0, 1];
        assert_eq!(s.serverbound(&keep_alive).unwrap(), None);
        let finish = [packet_ids::configuration::serverbound::FINISH_CONFIGURATION as u8];
        assert_eq!(s.serverbound(&finish).unwrap(), None);
        assert_eq!(s.writing(), State::Play);
    }
}
