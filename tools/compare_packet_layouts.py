#!/usr/bin/env python3
"""Compare the wire layout of every packet two server jars share, from their bytecode.

For each packet class (`net.minecraft.network.protocol.**Packet`, by its Mojang name) present in both
versions, this lists the FriendlyByteBuf calls of its buffer constructor (what it reads) and of its
`write` method, in order, with the names mapped through each version's `server.txt` (the official
ProGuard mappings). Calls into other methods that take the buffer (a nested record's `write`, a
lambda passed to `writeCollection`/`readOptional`...) are followed and their calls inlined, so a
field that moved into a shared record still compares equal. Packets whose sequences differ are
printed with both sequences; a codec-based packet (1.20.5+ StreamCodec) has no such methods and is
reported as such rather than compared.

This is a dev tool for the older-servers adapter (docs/native-client.md): it reads the player's own
prepared jars (`.local/versions/<release>/jars/server-game.jar`, the unbundled server, and
`server.txt`) and never writes anything. It needs `javap` (any JDK).

  python3 tools/compare_packet_layouts.py --old .local/versions/1.20.1 --new .local/versions/1.20.2
  python3 tools/compare_packet_layouts.py --old ... --new ... --packet ClientboundRespawnPacket
"""

import argparse
import re
import subprocess
import sys
from pathlib import Path

PRIMITIVES = {
    "void": "V", "boolean": "Z", "byte": "B", "char": "C", "short": "S",
    "int": "I", "long": "J", "float": "F", "double": "D",
}
FRIENDLY = "net.minecraft.network.FriendlyByteBuf"
PACKET = "net.minecraft.network.protocol.Packet"
METHOD_LINE = re.compile(r"^\s+(?:\d+:\d+:)?(\S+) ([^\s(]+)\((.*)\)(?::\d+(?::\d+)?)? -> (\S+)$")
CALL = re.compile(r"invoke(virtual|static|interface|special)\s+#\d+\s+// (?:Interface)?Method (\S+?)\.(\"?[^.:\"]+\"?):(\S+)")
DYNAMIC = re.compile(r"invokedynamic\s+#\d+,\s+0\s+// InvokeDynamic #(\d+):")
HANDLE = re.compile(r"REF_invoke\w+ (\S+?)\.(\"?[^.:\"]+\"?):(\S+)")


class Mappings:
    """One version's server.txt: classes and methods both ways, methods keyed by descriptor."""

    def __init__(self, path):
        self.obf = {}  # mojang class name -> obfuscated
        self.mojang = {}  # obfuscated -> mojang
        methods = []
        current = None
        for line in path.read_text(encoding="utf-8").splitlines():
            if not line or line.startswith("#"):
                continue
            if not line.startswith(" "):
                name, _, obf = line.rstrip(":").partition(" -> ")
                self.obf[name] = obf
                self.mojang[obf] = name
                current = name
                continue
            m = METHOD_LINE.match(line)
            if m and current:
                ret, name, args, obf = m.groups()
                methods.append((current, ret, name, [a for a in args.split(",") if a], obf))
        self.methods = {}  # (obf owner, obf name, obf descriptor) -> mojang name
        self.by_owner = {}  # mojang owner -> [(mojang name, mojang args, obf name, obf descriptor)]
        for owner, ret, name, args, obf in methods:
            desc = "(" + "".join(self.descriptor(a) for a in args) + ")" + self.descriptor(ret)
            self.methods[(self.obf[owner], obf, desc)] = name
            self.by_owner.setdefault(owner, []).append((name, args, obf, desc))

    def descriptor(self, java):
        dims = java.count("[]")
        base = java.replace("[]", "")
        if base in PRIMITIVES:
            d = PRIMITIVES[base]
        else:
            d = "L" + self.obf.get(base, base).replace(".", "/") + ";"
        return "[" * dims + d

    def simple(self, obf_internal):
        name = self.mojang.get(obf_internal.replace("/", "."), obf_internal.replace("/", "."))
        return name.rsplit(".", 1)[-1]


class Jar:
    """Disassembled classes of one jar, cached: {class: {(name, descriptor): (instructions, bootstraps)}}."""

    def __init__(self, jar):
        self.jar = jar
        self.classes = {}

    def load(self, names):
        missing = [n for n in dict.fromkeys(names) if n not in self.classes]
        for i in range(0, len(missing), 200):
            batch = missing[i:i + 200]
            out = subprocess.run(["javap", "-v", "-p", "-classpath", str(self.jar), *batch],
                                 capture_output=True, text=True, check=False).stdout
            for block in out.split("Classfile ")[1:]:
                header = block.splitlines()[0]
                name = header.rsplit("!/", 1)[-1].removesuffix(".class").replace("/", ".")
                self.classes[name] = parse_class(block)
            for n in batch:
                self.classes.setdefault(n, {})

    def method(self, cls, name, desc):
        self.load([cls])
        return self.classes[cls].get((name, desc))


def parse_class(block):
    """Methods of one `javap -v` class: their instructions and the class's bootstrap method handles."""
    lines = block.splitlines()
    bootstraps = {}
    if "BootstrapMethods:" in lines:
        start = lines.index("BootstrapMethods:")
        index = None
        for line in lines[start + 1:]:
            m = re.match(r"^\s+(\d+): #\d+ REF_invokeStatic", line)
            if m:
                index = int(m.group(1))
                continue
            h = HANDLE.search(line)
            if h and index is not None and index not in bootstraps and "LambdaMetafactory" not in line:
                bootstraps[index] = h.groups()
    methods = {}
    simple = None
    i = 0
    while i < len(lines):
        line = lines[i]
        if simple is None:
            m = re.match(r"^(?:public |final |abstract |)*(?:class|interface|enum) (\S+)", line.strip())
            if m and not line.startswith(" "):
                simple = m.group(1).split("<")[0].rsplit(".", 1)[-1]
        head = re.match(r"^  (?:[\w<>,.?\[\] $]+ )?([\w$<>]+)\((.*)\);$", line)
        if head and i + 1 < len(lines) and lines[i + 1].strip().startswith("descriptor:"):
            name = head.group(1)
            if simple and name.rsplit(".", 1)[-1] == simple:
                name = "<init>"
            desc = lines[i + 1].split("descriptor:", 1)[1].strip()
            body = []
            j = i + 2
            while j < len(lines) and lines[j].strip() and not re.match(r"^  \S", lines[j]):
                body.append(lines[j])
                j += 1
            methods[(name, desc)] = (body, bootstraps)
            i = j
            continue
        i += 1
    return methods


def calls(jar, maps, cls, name, desc, depth=0, seen=None):
    """The FriendlyByteBuf calls of one method, with buffer-taking callees inlined."""
    seen = seen or set()
    key = (cls, name, desc)
    if key in seen or depth > 5:
        return []
    seen = seen | {key}
    found = jar.method(cls, name, desc)
    if not found:
        return []
    body, bootstraps = found
    friendly = maps.obf[FRIENDLY]
    out = []
    for line in body:
        m = CALL.search(line)
        if m:
            _, owner, callee, cdesc = m.groups()
            out.extend(call(jar, maps, owner, callee.strip('"'), cdesc, friendly, depth, seen))
            continue
        d = DYNAMIC.search(line)
        if d and int(d.group(1)) in bootstraps:
            owner, callee, cdesc = bootstraps[int(d.group(1))]
            out.extend(call(jar, maps, owner, callee.strip('"'), cdesc, friendly, depth, seen))
    return out


def call(jar, maps, owner, name, desc, friendly, depth, seen):
    owner_dot = owner.replace("/", ".")
    if owner_dot == friendly:
        mojang = maps.methods.get((friendly, name, desc), name)
        return [f"{mojang}{pretty(maps, desc)}"]
    if "L" + friendly + ";" in desc and not owner_dot.startswith(("java.", "io.netty.", "com.")):
        inner = calls(jar, maps, owner_dot, name, desc, depth + 1, seen)
        label = f"{maps.simple(owner_dot)}.{maps.methods.get((owner_dot, name, desc), name)}"
        return [f"[{label}]"] + ["  " + c for c in inner] if inner else [f"[{label}]"]
    return []


def pretty(maps, desc):
    args = re.findall(r"\[*(?:[ZBCSIJFDV]|L[^;]+;)", desc.split(")")[0][1:])
    names = []
    for a in args:
        dims = a.count("[")
        a = a.lstrip("[")
        base = maps.simple(a[1:-1]) if a.startswith("L") else {v: k for k, v in PRIMITIVES.items()}[a]
        names.append(base + "[]" * dims)
    return "(" + ",".join(names) + ")"


def layouts(version_dir, filter_name, helpers=()):
    maps = Mappings(version_dir / "server.txt")
    jar = Jar(version_dir / "jars" / "server-game.jar")
    friendly = maps.obf[FRIENDLY]
    packet = maps.obf[PACKET]
    write = next((obf for (owner, obf, desc), name in maps.methods.items()
                  if owner == packet and name == "write" and desc == f"(L{friendly};)V"), "a")
    packets = {name: obf for name, obf in maps.obf.items()
               if name.startswith("net.minecraft.network.protocol.") and re.search(r"(Clientbound|Serverbound)\w*Packet(\$\w+)?$", name)
               and (not filter_name or filter_name in name)}
    jar.load(packets.values())
    out = {}
    for name, obf in packets.items():
        reads = calls(jar, maps, obf, "<init>", f"(L{friendly};)V")
        writes = calls(jar, maps, obf, write, f"(L{friendly};)V")
        has_read = jar.method(obf, "<init>", f"(L{friendly};)V") is not None
        has_write = jar.method(obf, write, f"(L{friendly};)V") is not None
        out[name] = (reads if has_read else None, writes if has_write else None)
    # Classes outside the packets that read or write the buffer (recipe serializers, particle options,
    # entity data serializers...): each named method taking the buffer, compared on its own.
    owners = [n for n in maps.by_owner if any(n.startswith(p) for p in helpers)]
    jar.load(maps.obf[n] for n in owners)
    for owner in owners:
        for name, args, obf, desc in maps.by_owner[owner]:
            if FRIENDLY in args and not name.startswith("lambda$"):
                key = f"{owner}.{name}({','.join(a.rsplit('.', 1)[-1] for a in args)})"
                if not filter_name or filter_name in key:
                    out[key] = (calls(jar, maps, maps.obf[owner], obf, desc), None)
    return out


def wire(sequence):
    """The buffer calls alone: which helper they were inlined from does not change the bytes."""
    if sequence is None:
        return None
    return [c.strip() for c in sequence if not c.strip().startswith("[")]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--old", type=Path, required=True, help="older prepared version directory")
    parser.add_argument("--new", type=Path, required=True, help="newer prepared version directory")
    parser.add_argument("--packet", help="only packets whose Mojang name contains this")
    parser.add_argument("--all", action="store_true", help="print unchanged packets' sequences too")
    parser.add_argument("--helpers", nargs="*", default=[], metavar="PREFIX",
                        help="also compare every buffer method of classes whose Mojang name starts so "
                             "(e.g. net.minecraft.world.item.crafting. net.minecraft.core.particles.)")
    args = parser.parse_args(argv)
    old = layouts(args.old, args.packet, args.helpers)
    new = layouts(args.new, args.packet, args.helpers)
    short = lambda n: n.removeprefix("net.minecraft.network.protocol.")
    # A packet that moved package (1.20.2 moved keep-alives and the like to `common`) is still the same
    # packet: pair it by its class name when that name is unique on both sides.
    simple = lambda n: n.rsplit(".", 1)[-1]
    for name in sorted(set(old) - set(new)):
        moved = [n for n in set(new) - set(old) if simple(n) == simple(name)]
        if len(moved) == 1 and sum(simple(n) == simple(name) for n in old) == 1:
            new[name] = new.pop(moved[0])
    for name in sorted(set(old) - set(new)):
        print(f"only in {args.old.name}: {short(name)}")
    for name in sorted(set(new) - set(old)):
        print(f"only in {args.new.name}: {short(name)}")
    changed = 0
    for name in sorted(set(old) & set(new)):
        (old_r, old_w), (new_r, new_w) = old[name], new[name]
        same = wire(old_r) == wire(new_r) and wire(old_w) == wire(new_w)
        if same and not args.all:
            continue
        changed += not same
        print(f"\n== {short(name)}{'' if same else '  (changed)'}")
        for label, a, b in (("read", old_r, new_r), ("write", old_w, new_w)):
            if wire(a) == wire(b) and not args.all:
                continue
            print(f"  {label} {args.old.name}: " + ("(no buffer method)" if a is None else ""))
            for c in a or []:
                print(f"      {c}")
            print(f"  {label} {args.new.name}: " + ("(no buffer method)" if b is None else ""))
            for c in b or []:
                print(f"      {c}")
    print(f"\n{changed} shared packets changed layout", file=sys.stderr)


if __name__ == "__main__":
    main()
