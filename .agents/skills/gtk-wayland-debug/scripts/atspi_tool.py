#!/usr/bin/env python3
"""Inspect and operate the private Toniator AT-SPI tree."""

from __future__ import annotations

import argparse
from dataclasses import asdict, dataclass
import json
import sys
from typing import Iterator


try:
    import pyatspi
except ImportError as error:
    raise SystemExit("Python AT-SPI bindings are missing; install python3-pyatspi") from error


@dataclass
class NodeRecord:
    """Hold a stable textual snapshot of one accessible node."""

    path: str
    name: str
    role: str
    description: str
    text: str | None
    value: float | None
    minimum: float | None
    maximum: float | None
    actions: list[str]


def safe_text(getter) -> str:
    """Return an accessible string while tolerating disappearing nodes."""
    try:
        return str(getter() or "")
    except Exception:
        return "<unavailable>"


def safe_value(node) -> tuple[float | None, float | None, float | None]:
    """Read the optional scalar value interface without mutating the node."""
    try:
        value = node.queryValue()
        return float(value.currentValue), float(value.minimumValue), float(value.maximumValue)
    except Exception:
        return None, None, None


def safe_node_text(node) -> str | None:
    """Read the optional editable or static text content of a node."""
    try:
        text = node.queryText()
        return str(text.getText(0, text.characterCount))
    except Exception:
        return None


def safe_actions(node) -> list[str]:
    """Read available AT-SPI action names without invoking them."""
    try:
        action = node.queryAction()
        return [str(action.getName(index)) for index in range(action.nActions)]
    except Exception:
        return []


def record_node(node, path: str) -> NodeRecord:
    """Snapshot semantic properties used by queries and evidence output."""
    current, minimum, maximum = safe_value(node)
    return NodeRecord(
        path=path,
        name=safe_text(lambda: node.name),
        role=safe_text(node.getRoleName),
        description=safe_text(lambda: node.description),
        text=safe_node_text(node),
        value=current,
        minimum=minimum,
        maximum=maximum,
        actions=safe_actions(node),
    )


def walk(node, path: str = "desktop", depth: int = 0, max_depth: int = 30) -> Iterator[tuple[object, NodeRecord, int]]:
    """Traverse a bounded AT-SPI subtree while tolerating stale children."""
    record = record_node(node, path)
    yield node, record, depth
    if depth >= max_depth:
        return
    try:
        child_count = node.childCount
    except Exception:
        return
    for index in range(child_count):
        try:
            child = node.getChildAtIndex(index)
            if child is None:
                continue
            child_record = record_node(child, "")
            segment = f"{index}:{child_record.role}:{child_record.name}"
            yield from walk(child, f"{path}/{segment}", depth + 1, max_depth)
        except Exception:
            continue


def desktop():
    """Return the private accessibility desktop root."""
    return pyatspi.Registry.getDesktop(0)


def application_roots(application_query: str | None) -> list[object]:
    """Select application roots by a case-insensitive accessible-name match."""
    root = desktop()
    if not application_query:
        return [root]
    query = application_query.casefold()
    roots = []
    for index in range(root.childCount):
        try:
            application = root.getChildAtIndex(index)
            name = safe_text(lambda: application.name)
            if query in name.casefold():
                roots.append(application)
        except Exception:
            continue
    return roots


def find_matches(arguments) -> list[tuple[object, NodeRecord]]:
    """Find bounded semantic matches across selected application roots."""
    query = arguments.query.casefold()
    role_query = arguments.role.casefold() if arguments.role else None
    matches = []
    roots = application_roots(arguments.application)
    for root_index, root in enumerate(roots):
        root_path = f"application[{root_index}]"
        for node, record, _depth in walk(root, root_path, max_depth=arguments.depth):
            name_matches = record.name.casefold() == query if arguments.exact else query in record.name.casefold()
            role_matches = role_query is None or role_query == record.role.casefold()
            if name_matches and role_matches:
                matches.append((node, record))
                if arguments.limit and len(matches) >= arguments.limit:
                    return matches
    return matches


def print_records(records: list[NodeRecord], as_json: bool) -> None:
    """Render accessible records as JSON or compact semantic lines."""
    if as_json:
        print(json.dumps([asdict(record) for record in records], indent=2, sort_keys=True))
        return
    for index, record in enumerate(records):
        details = [f"[{index}]", record.role or "unknown", repr(record.name), f"path={record.path}"]
        if record.value is not None:
            details.append(f"value={record.value:g}")
            details.append(f"range={record.minimum:g}..{record.maximum:g}")
        if record.text is not None and record.text != record.name:
            details.append(f"text={record.text!r}")
        if record.actions:
            details.append(f"actions={','.join(record.actions)}")
        print(" ".join(details))


def run_tree(arguments) -> int:
    """Print a bounded semantic tree for the selected application."""
    roots = application_roots(arguments.application)
    if not roots:
        print(f"no application matched: {arguments.application}", file=sys.stderr)
        return 3
    if arguments.json:
        records = []
        for root_index, root in enumerate(roots):
            records.extend(record for _node, record, _depth in walk(root, f"application[{root_index}]", max_depth=arguments.depth))
        print_records(records, True)
        return 0
    for root_index, root in enumerate(roots):
        for _node, record, depth in walk(root, f"application[{root_index}]", max_depth=arguments.depth):
            details = f"{record.role or 'unknown'} {record.name!r}"
            if record.value is not None:
                details += f" value={record.value:g} range={record.minimum:g}..{record.maximum:g}"
            if record.text is not None and record.text != record.name:
                details += f" text={record.text!r}"
            if record.actions:
                details += f" actions={','.join(record.actions)}"
            print(f"{'  ' * depth}{details}")
    return 0


def run_find(arguments) -> int:
    """Print all accessible nodes matching a name and optional role."""
    matches = find_matches(arguments)
    print_records([record for _node, record in matches], arguments.json)
    return 0 if matches else 3


def choose_match(arguments) -> tuple[object, NodeRecord]:
    """Select one node, rejecting unresolved ambiguity with exit status 4."""
    matches = find_matches(arguments)
    if not matches:
        print("no accessible node matched", file=sys.stderr)
        raise SystemExit(3)
    if arguments.index is None and len(matches) != 1:
        print_records([record for _node, record in matches], False)
        print("multiple nodes matched; repeat with --index N", file=sys.stderr)
        raise SystemExit(4)
    index = arguments.index or 0
    if index < 0 or index >= len(matches):
        print_records([record for _node, record in matches], False)
        raise SystemExit(f"match index {index} is out of range")
    return matches[index]


def run_action(arguments) -> int:
    """Invoke one explicit AT-SPI action, focus request, or bounded value edit."""
    if arguments.commit and arguments.set_text is None:
        raise SystemExit("--commit is valid only with --set-text")
    node, before = choose_match(arguments)
    if arguments.actions:
        print_records([before], arguments.json)
        return 0
    if arguments.set_value is not None:
        if before.minimum is None or before.maximum is None:
            raise SystemExit("matched node does not expose the AT-SPI Value interface")
        if not before.minimum <= arguments.set_value <= before.maximum:
            raise SystemExit(f"value {arguments.set_value:g} is outside {before.minimum:g}..{before.maximum:g}")
        value = node.queryValue()
        value.currentValue = arguments.set_value
    elif arguments.set_text is not None:
        editable = node.queryEditableText()
        if not editable.setTextContents(arguments.set_text):
            raise SystemExit("AT-SPI editable-text request was rejected")
        if arguments.commit:
            action = node.queryAction()
            available = [str(action.getName(index)) for index in range(action.nActions)]
            if "activate" not in available:
                raise SystemExit(f"cannot commit text through AT-SPI; actions={available}")
            if not action.doAction(available.index("activate")):
                raise SystemExit("AT-SPI activate action was rejected after setting text")
    elif arguments.focus:
        try:
            focused = node.queryComponent().grabFocus()
        except Exception as error:
            detail = str(error).strip() or type(error).__name__
            raise SystemExit(f"AT-SPI focus request failed: {detail}") from None
        if not focused:
            raise SystemExit("AT-SPI focus request was rejected")
    else:
        action = node.queryAction()
        available = [str(action.getName(index)) for index in range(action.nActions)]
        requested = arguments.activate
        if requested is True:
            preferred = ("activate", "click", "press", "open")
            selected_name = next((name for name in preferred if name in available), None)
        else:
            selected_name = requested
        if not selected_name or selected_name not in available:
            raise SystemExit(f"requested action is unavailable; actions={available}")
        action_index = available.index(selected_name)
        if not action.doAction(action_index):
            raise SystemExit(f"AT-SPI action was rejected: {selected_name}")

    after = record_node(node, before.path)
    print(json.dumps({"before": asdict(before), "after": asdict(after)}, indent=2, sort_keys=True))
    return 0


def add_query_arguments(command_parser: argparse.ArgumentParser) -> None:
    """Add common name, role, application, and traversal constraints."""
    command_parser.add_argument("query")
    command_parser.add_argument("--application", default="Toniator")
    command_parser.add_argument("--role")
    command_parser.add_argument("--exact", action="store_true")
    command_parser.add_argument("--depth", type=int, default=30)
    command_parser.add_argument("--limit", type=int, default=0)
    command_parser.add_argument("--json", action="store_true")


def build_parser() -> argparse.ArgumentParser:
    """Build the AT-SPI inspection and action command parser."""
    command_parser = argparse.ArgumentParser()
    subparsers = command_parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("check")

    tree_parser = subparsers.add_parser("tree")
    tree_parser.add_argument("--application")
    tree_parser.add_argument("--depth", type=int, default=12)
    tree_parser.add_argument("--json", action="store_true")

    find_parser = subparsers.add_parser("find")
    add_query_arguments(find_parser)

    action_parser = subparsers.add_parser("action")
    add_query_arguments(action_parser)
    action_parser.add_argument("--index", type=int)
    action_group = action_parser.add_mutually_exclusive_group(required=True)
    action_group.add_argument("--activate", nargs="?", const=True, metavar="ACTION")
    action_group.add_argument("--set-value", type=float)
    action_group.add_argument("--set-text")
    action_group.add_argument("--focus", action="store_true")
    action_group.add_argument("--actions", action="store_true")
    action_parser.add_argument("--commit", action="store_true", help="activate after --set-text")
    return command_parser


def main() -> int:
    """Dispatch one bounded AT-SPI inspection or action."""
    arguments = build_parser().parse_args()
    if arguments.command == "check":
        return 0
    if arguments.command == "tree":
        return run_tree(arguments)
    if arguments.command == "find":
        return run_find(arguments)
    if arguments.command == "action":
        return run_action(arguments)
    raise AssertionError(f"unhandled command: {arguments.command}")


if __name__ == "__main__":
    sys.exit(main())
