#!/usr/bin/env python3
"""Generate crates/horizon-protocol/src/adapter/v<OLD>.rs for a release before 1.20.2, whose data
generator has no packets report: the packet order comes from the server jar itself.

    generate_legacy_packet_id_adapter.py --old .local/versions/1.20.1

`--old` holds `jars/server-game.jar` (the server bundle's inner jar), `server.txt` (Mojang's
official mappings for it) and `version.json`. `javap` (any JDK) lists `ConnectionProtocol`'s static
initializer: each state's flows add their packet classes in id order. The classes are matched to
the pinned version's packets report (`.local/datagen/out/reports/packets.json`) by the name Mojang
gives each packet type since 1.20.5, which follows the class name (`ClientboundAddEntityPacket` is
`add_entity`), with `RENAMES` for the few that differ. Nothing from the jar is written but numbers.
"""

import argparse
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PER_LINE = 12
STATES = {"HANDSHAKING": "handshake", "PLAY": "play", "STATUS": "status", "LOGIN": "login"}
ORDER = ["handshake", "status", "login", "play"]

# Old class (without package) -> the pinned report's name, where the name doesn't follow the class.
RENAMES = {
    "ClientboundBundlePacket": "minecraft:bundle_delimiter",
    "ClientboundLoginPacket": "minecraft:login",
    "ClientboundCustomPayloadPacket": "minecraft:custom_payload",
    "ServerboundCustomPayloadPacket": "minecraft:custom_payload",
    "ClientboundKeepAlivePacket": "minecraft:keep_alive",
    "ServerboundKeepAlivePacket": "minecraft:keep_alive",
    "ClientboundDisconnectPacket": "minecraft:disconnect",
    "ClientboundPingPacket": "minecraft:ping",
    "ServerboundPongPacket": "minecraft:pong",
    "ClientboundResourcePackPacket": "minecraft:resource_pack_push",
    "ServerboundResourcePackPacket": "minecraft:resource_pack",
    "ClientboundUpdateEnabledFeaturesPacket": None,  # configuration-only since 1.20.2
    "ClientboundHorseScreenOpenPacket": "minecraft:mount_screen_open",
    "ClientboundSetCarriedItemPacket": "minecraft:set_held_slot",
    "ClientboundStatusResponsePacket": "minecraft:status_response",
    "ClientboundPongResponsePacket": "minecraft:pong_response",
    "ServerboundPingRequestPacket": "minecraft:ping_request",
    "ServerboundStatusRequestPacket": "minecraft:status_request",
    "ClientIntentionPacket": "minecraft:intention",
    "ClientboundHelloPacket": "minecraft:hello",
    "ServerboundHelloPacket": "minecraft:hello",
    "ClientboundGameProfilePacket": "minecraft:login_finished",
    "ClientboundLoginCompressionPacket": "minecraft:login_compression",
    "ClientboundCustomQueryPacket": "minecraft:custom_query",
    "ServerboundCustomQueryPacket": "minecraft:custom_query_answer",
    "ServerboundKeyPacket": "minecraft:key",
    "ClientboundLoginDisconnectPacket": "minecraft:login_disconnect",
    "ClientboundUpdateTagsPacket": "minecraft:update_tags",
    "ServerboundClientInformationPacket": "minecraft:client_information",
}


def snake(name):
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", name).lower()


def packet_name(cls):
    """The pinned report's name for an old packet class (`a.b.ClientboundMoveEntityPacket$Pos`)."""
    simple = cls.rsplit(".", 1)[-1]
    outer, _, inner = simple.partition("$")
    if outer in RENAMES and not inner:
        return RENAMES[outer]
    base = re.sub(r"^(Clientbound|Serverbound)", "", outer)
    base = re.sub(r"Packet$", "", base)
    return "minecraft:" + snake(base + inner)


def mappings(path):
    """Obfuscated class name -> Mojang's, and PacketFlow's obfuscated fields -> their names."""
    classes, flow_fields, current = {}, {}, None
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith(" ") and line.endswith(":") and " -> " in line:
            named, obf = line[:-1].split(" -> ")
            classes[obf] = named
            current = named
        elif current == "net.minecraft.network.protocol.PacketFlow" and " -> " in line:
            left, obf = line.strip().split(" -> ")
            if left.endswith("SERVERBOUND") or left.endswith("CLIENTBOUND"):
                flow_fields[obf] = left.split()[-1]
    return classes, flow_fields


def packet_order(old):
    """{state: {direction: [Mojang class, ...]}} in id order, from javap's listing."""
    classes, flow_fields = mappings(old / "server.txt")
    obf = {v: k for k, v in classes.items()}
    protocol = obf["net.minecraft.network.ConnectionProtocol"]
    flow = obf["net.minecraft.network.protocol.PacketFlow"]
    listing = subprocess.run(
        ["javap", "-c", "-p", "-classpath", str(old / "jars/server-game.jar"), protocol],
        capture_output=True, text=True, check=True,
    ).stdout
    body = listing[listing.index("static {};"):]
    order, state, direction = {}, None, None
    for line in body.splitlines():
        if m := re.search(r"// String (HANDSHAKING|PLAY|STATUS|LOGIN)$", line):
            state = STATES[m.group(1)]
            order[state] = {}
        elif m := re.search(rf"getstatic .*// Field {re.escape(flow)}\.(\w+):", line):
            direction = flow_fields[m.group(1)].lower()
            order[state][direction] = []
        elif state and direction and (m := re.search(r"ldc(?:_w)? .*// class (\S+)$", line)):
            order[state][direction].append(classes.get(m.group(1), m.group(1)))
    return order


def cells(values):
    return ["None" if v is None else f"Some({v})" for v in values]


def rust(old_name, protocol, order, report, pinned_protocol, pinned_name):
    """The module, as tools/generate_packet_id_adapter.py writes one: clientbound tables give
    the pinned id of each old id, serverbound tables the old id of each pinned id."""
    out = [
        f"//! Packet ids of protocol {protocol} ({old_name}) against the pinned {pinned_protocol} "
        f"({pinned_name}).",
        "//! Generated by tools/generate_legacy_packet_id_adapter.py from the old server jar's",
        "//! `ConnectionProtocol` and the pinned packets report; do not edit by hand.",
        "use crate::State;",
        "",
        f"pub const PROTOCOL: i32 = {protocol};",
        f'pub const NAME: &str = "{old_name}";',
        "",
    ]
    unmatched = []
    for state in ORDER + ["configuration"]:
        for direction in ["clientbound", "serverbound"]:
            classes = order.get(state, {}).get(direction, [])
            pinned = {
                name: entry["protocol_id"]
                for name, entry in report.get(state, {}).get(direction, {}).items()
            }
            old_to_pinned = [pinned.get(packet_name(cls)) for cls in classes]
            for cls, value in zip(classes, old_to_pinned):
                if value is None:
                    unmatched.append((state, direction, cls, packet_name(cls)))
            if direction == "clientbound":
                values = old_to_pinned
            else:
                values = [None] * len(pinned)
                for old_id, value in enumerate(old_to_pinned):
                    if value is not None:
                        values[value] = old_id
                # Nothing past the last packet this server knows.
                while values and values[-1] is None:
                    values.pop()
            out.append("#[rustfmt::skip]")
            out.append(f"const {state.upper()}_{direction.upper()}: &[Option<u8>] = &[")
            row = cells(values)
            for k in range(0, len(row), PER_LINE):
                out.append("    " + ", ".join(row[k:k + PER_LINE]) + ",")
            out.append("];")
    out.append("")
    for direction, doc in [
        ("clientbound", f"For each id a {old_name} server sends in `state`, the pinned version's id "
                        "(`None`: no such packet)."),
        ("serverbound", f"For each pinned-version id Horizon sends in `state`, the id a {old_name} "
                        "server knows it by."),
    ]:
        out.append(f"/// {doc}")
        out.append(f"pub fn {direction}(state: State) -> &'static [Option<u8>] {{")
        out.append("    match state {")
        for state, name in [("Handshake", "HANDSHAKE"), ("Status", "STATUS"), ("Login", "LOGIN"),
                            ("Configuration", "CONFIGURATION"), ("Play", "PLAY")]:
            out.append(f"        State::{state} => {name}_{direction.upper()},")
        out.append("    }")
        out.append("}")
    return "\n".join(out) + "\n", unmatched


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--old", type=Path, required=True)
    parser.add_argument("--protocol", type=int, required=True)
    parser.add_argument("--report", type=Path, default=ROOT / ".local/datagen/out/reports/packets.json")
    parser.add_argument("--out", type=Path)
    parser.add_argument("--list", action="store_true", help="print the old packets and their pinned names")
    args = parser.parse_args()
    old_name = json.loads((args.old / "version.json").read_text())["id"]
    order = packet_order(args.old)
    report = json.loads(args.report.read_text())
    if args.list:
        for state in ORDER:
            for direction, classes in order.get(state, {}).items():
                for i, cls in enumerate(classes):
                    print(f"{state} {direction} {i:#04x} {cls.rsplit('.', 1)[-1]} {packet_name(cls)}")
        return
    version = (ROOT / "crates/horizon-core/src/version.rs").read_text()
    pinned_protocol = int(re.search(r"PROTOCOL_VERSION: i32 = (\d+);", version).group(1))
    pinned_name = re.search(r'VANILLA_VERSION: &str = "([^"]+)";', version).group(1)
    text, unmatched = rust(old_name, args.protocol, order, report, pinned_protocol, pinned_name)
    for state, direction, cls, name in unmatched:
        print(f"no pinned packet: {state} {direction} {cls} ({name})")
    out = args.out or ROOT / f"crates/horizon-protocol/src/adapter/v{old_name.replace('.', '_')}.rs"
    out.write_text(text, encoding="utf-8")
    print(f"wrote {out.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
