#!/usr/bin/env python3
"""gdcheck: static API check of the GDScript sources against Godot's extension_api.json.

Catches the class of error a dry run surfaces first: a method, property, constant or
signal that does not exist on the engine class it is used on. Typed variables, class
constants, singletons, global enums and utility functions are checked; untyped dynamic
access (our own script objects) is left alone.

    tools/gdcheck.py [--api PATH] [files...]

The API JSON comes from `godot --dump-extension-api` or, without a Godot binary, from the
gdext dependency cache (found automatically).
"""
import glob
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

GD_BUILTIN_FUNCS = {
    "str", "float", "int", "bool", "print", "printerr", "push_error", "push_warning", "load", "preload",
    "range", "typeof", "len", "assert", "is_instance_valid", "is_instance_of", "Callable", "Signal",
    "Color", "Vector2", "Vector3", "Vector2i", "Vector3i", "Rect2", "Transform3D", "Basis", "Quaternion",
    "Plane", "AABB", "NodePath", "StringName", "PackedByteArray", "PackedFloat32Array", "PackedStringArray",
    "Array", "Dictionary", "super", "convert", "char", "ord", "dict_to_inst", "inst_to_dict", "get_stack",
    "print_debug", "print_rich", "printt", "prints", "printraw", "type_convert", "type_exists",
}
GD_KEYWORDS = {"if", "elif", "else", "for", "while", "match", "return", "and", "or", "not", "in", "is",
               "as", "func", "var", "const", "signal", "static", "extends", "class_name", "pass", "break",
               "continue", "await", "yield", "self", "true", "false", "null", "void", "enum", "class"}
BUILTIN_TYPE_NAMES = {"int", "float", "bool", "String", "StringName", "NodePath", "Variant"}


def find_api(explicit):
    if explicit:
        return explicit
    cands = glob.glob(os.path.expanduser("~/.cargo/registry/src/*/gdextension-api-*/src/4.7/extension_api.json"))
    cands += glob.glob(os.path.expanduser("~/.cargo/registry/src/*/gdextension-api-*/src/*/extension_api.json"))
    cands += glob.glob(os.path.join(ROOT, "extension_api.json"))
    if not cands:
        sys.exit("gdcheck: no extension_api.json found; pass --api PATH (godot --dump-extension-api)")
    return sorted(cands)[-1]


class Api:
    def __init__(self, path):
        with open(path, "rb") as f:
            raw = json.load(f)
        self.version = raw["header"]["version_full_name"]
        self.classes = {}
        self.sigs = {}  # (class, method) -> (min_args, max_args or None for vararg)
        for c in raw["classes"]:
            members = set()
            for m in c.get("methods", []):
                members.add(m["name"])
                args = m.get("arguments", [])
                required = sum(1 for a in args if "default_value" not in a)
                self.sigs[(c["name"], m["name"])] = (required, None if m.get("is_vararg") else len(args))
            for p in c.get("properties", []):
                members.add(p["name"])
            for s in c.get("signals", []):
                members.add(s["name"])
            for k in c.get("constants", []):
                members.add(k["name"])
            for e in c.get("enums", []):
                members.add(e["name"])
                for v in e.get("values", []):
                    members.add(v["name"])
            self.classes[c["name"]] = {"inherits": c.get("inherits"), "members": members,
                                       "singleton": False}
        for s in raw.get("singletons", []):
            if s["type"] in self.classes:
                self.classes[s["type"]]["singleton"] = True
            self.classes.setdefault(s["name"], self.classes.get(s["type"], {"inherits": None, "members": set()}))
        self.builtins = {}
        for b in raw["builtin_classes"]:
            members = set()
            for m in b.get("methods", []):
                members.add(m["name"])
                args = m.get("arguments", [])
                required = sum(1 for a in args if "default_value" not in a)
                self.sigs[(b["name"], m["name"])] = (required, None if m.get("is_vararg") else len(args))
            for m in b.get("members", []):
                members.add(m["name"])
            for k in b.get("constants", []):
                members.add(k["name"])
            for e in b.get("enums", []):
                members.add(e["name"])
                for v in e.get("values", []):
                    members.add(v["name"])
            self.builtins[b["name"]] = members
        self.globals = set()
        for e in raw.get("global_enums", []):
            self.globals.add(e["name"])
            for v in e.get("values", []):
                self.globals.add(v["name"])
        for k in raw.get("global_constants", []):
            self.globals.add(k["name"])
        self.utility = {u["name"] for u in raw.get("utility_functions", [])}

    def class_has(self, cls, member):
        seen = set()
        while cls and cls not in seen:
            seen.add(cls)
            info = self.classes.get(cls)
            if info is None:
                return True  # unknown class (script class): do not judge
            if member in info["members"]:
                return True
            cls = info["inherits"]
        return False

    def has(self, typ, member):
        if typ in self.builtins:
            return member in self.builtins[typ]
        return self.class_has(typ, member)

    def signature(self, typ, method):
        seen = set()
        while typ and typ not in seen:
            seen.add(typ)
            if (typ, method) in self.sigs:
                return self.sigs[(typ, method)]
            info = self.classes.get(typ)
            if info is None:
                return None
            typ = info["inherits"]
        return None


def check_file(path, api, script_classes):
    src = open(path, encoding="utf-8").read()
    lines = src.splitlines()
    problems = []
    # Script-level symbols that bare calls may resolve to.
    own = set(re.findall(r"^\s*(?:static\s+)?func\s+(\w+)", src, re.M))
    own |= set(re.findall(r"^\s*(?:const|var|signal)\s+(\w+)", src, re.M))
    own |= set(re.findall(r"@onready\s+var\s+(\w+)", src))
    own |= set(re.findall(r"@export\s+var\s+(\w+)", src))
    ext = re.search(r"^extends\s+(\w+)", src, re.M)
    base = ext.group(1) if ext else None

    # Typed locals / members: name -> type. A name declared more than once, or
    # declared without a concrete engine type anywhere, is left unchecked.
    types = {}
    generic = {"Node", "Object", "Dictionary", "Array", "Variant", "RefCounted", "Resource"}
    declared = {}
    for m in re.finditer(r"\bvar\s+(\w+)\s*(:\s*(\w+))?", src):
        declared.setdefault(m.group(1), set()).add(m.group(3) or "")
    ambiguous = {n for n, ts in declared.items() if len(ts) > 1 or "" in ts}
    for m in re.finditer(r"\bvar\s+(\w+)\s*:\s*(\w+)", src):
        types[m.group(1)] = m.group(2)
    for m in re.finditer(r"\bvar\s+(\w+)\s*:?=\s*(\w+)\.new\(\)", src):
        types[m.group(1)] = m.group(2)
    for m in re.finditer(r"\bvar\s+(\w+)\s*:?=\s*(\w+)\(\)", src):
        if m.group(2) in api.builtins:
            types[m.group(1)] = m.group(2)
    for m in re.finditer(r"\bvar\s+(\w+)\s*:?=\s*\$[\w/]+\s*(?:as\s+(\w+))?", src):
        if m.group(2):
            types[m.group(1)] = m.group(2)
    for m in re.finditer(r"\bvar\s+(\w+)\s*:?=\s*[\w.]+\s+as\s+(\w+)", src):
        types[m.group(1)] = m.group(2)
    for m in re.finditer(r"func\s+\w+\(([^)]*)\)", src):
        for arg in m.group(1).split(","):
            am = re.match(r"\s*(\w+)\s*:\s*(\w+)", arg)
            if am:
                if am.group(1) in types and types[am.group(1)] != am.group(2):
                    ambiguous.add(am.group(1))
                types[am.group(1)] = am.group(2)
    for name in list(types):
        if name in ambiguous or types[name] in generic or re.search(r"\b%s\.set_script\(" % re.escape(name), src):
            del types[name]

    code = re.sub(r'"(?:[^"\\]|\\.)*"', '""', src)  # strip string contents
    code = re.sub(r"#.*", "", code)
    for lineno, line in enumerate(code.splitlines(), 1):
        # Class.member / singleton.member / typedvar.member
        for m in re.finditer(r"\b([A-Za-z_]\w*)\.([A-Za-z_]\w*)", line):
            obj, member = m.group(1), m.group(2)
            if obj in script_classes or obj in ("self", "super"):
                continue
            if obj in api.classes or obj in api.builtins:
                typ = obj
            elif obj in types and (types[obj] in api.classes or types[obj] in api.builtins):
                typ = types[obj]
            else:
                continue
            if member == "new":
                continue
            if not api.has(typ, member):
                problems.append((lineno, f"{typ}.{member} does not exist"))
                continue
            # arity of a direct call
            rest = line[m.end():]
            if rest.startswith("("):
                sig = api.signature(typ, member)
                if sig is not None:
                    depth, count, empty, closed = 0, 0, True, False
                    for ch in rest:
                        if ch in "([{":
                            depth += 1
                        elif ch in ")]}":
                            depth -= 1
                            if depth == 0:
                                closed = True
                                break
                        elif ch == "," and depth == 1:
                            count += 1
                        elif depth >= 1 and not ch.isspace():
                            empty = False
                    n = 0 if (empty and count == 0) else count + 1
                    lo, hi = sig
                    if closed and (n < lo or (hi is not None and n > hi)):
                        problems.append((lineno, f"{typ}.{member}() takes {lo}..{hi if hi is not None else 'n'} args, called with {n}"))
        # bare ALL_CAPS identifiers: global constants/enums
        for m in re.finditer(r"(?<![\w.])([A-Z][A-Z0-9_]{2,})\b", line):
            name = m.group(1)
            if name in own or name in api.globals or name in ("PI", "TAU", "INF", "NAN"):
                continue
            if name in script_classes or name in api.classes or name in api.builtins:
                continue
            problems.append((lineno, f"global constant {name} unknown"))
        # bare function calls
        for m in re.finditer(r"(?<![\w.])([a-z_]\w*)\s*\(", line):
            name = m.group(1)
            if name in GD_KEYWORDS or name in own or name in GD_BUILTIN_FUNCS or name in api.utility:
                continue
            if base and api.class_has(base, name):
                continue
            if name in api.classes or name in api.builtins:
                continue
            problems.append((lineno, f"function {name}() unknown here"))
    return problems, lines


def main():
    args = sys.argv[1:]
    api_path = None
    files = []
    i = 0
    while i < len(args):
        if args[i] == "--api":
            api_path = args[i + 1]
            i += 2
        else:
            files.append(args[i])
            i += 1
    if not files:
        files = sorted(glob.glob(os.path.join(ROOT, "godot", "src", "**", "*.gd"), recursive=True))
    api = Api(find_api(api_path))
    script_classes = set()
    for f in files:
        src = open(f, encoding="utf-8").read()
        m = re.search(r"^class_name\s+(\w+)", src, re.M)
        if m:
            script_classes.add(m.group(1))
    # Autoload names from project.godot
    proj = os.path.join(ROOT, "godot", "project.godot")
    if os.path.exists(proj):
        for m in re.finditer(r'^(\w+)="\*?res://', open(proj).read(), re.M):
            script_classes.add(m.group(1))
    script_classes.add("DbGame")
    total = 0
    print(f"gdcheck  api {api.version}  {len(files)} files")
    for f in files:
        problems, lines = check_file(f, api, script_classes)
        rel = os.path.relpath(f, ROOT)
        if not problems:
            print(f"  {rel:<40} ok")
            continue
        total += len(problems)
        print(f"  {rel:<40} {len(problems)} problem(s)")
        for lineno, msg in problems:
            print(f"      {lineno:>4}  {msg}")
            print(f"            {lines[lineno - 1].strip()[:100]}")
    print("gdcheck  " + ("PASS" if total == 0 else f"FAIL  {total} problem(s)"))
    sys.exit(0 if total == 0 else 1)


if __name__ == "__main__":
    main()
