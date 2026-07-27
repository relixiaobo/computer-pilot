#!/usr/bin/env python3
"""Black-box conformance runner for the Computer Pilot stdio protocol."""

from __future__ import annotations

import argparse
import json
import selectors
import subprocess
import sys
import time
from dataclasses import dataclass
from typing import Any


@dataclass
class Check:
    name: str
    passed: bool
    duration_ms: int
    error: str | None = None


class BridgeClient:
    def __init__(
        self,
        command: list[str],
        timeout_ms: int,
        bridge_args: list[str] | None = None,
    ) -> None:
        self.timeout_ms = timeout_ms
        self.process = subprocess.Popen(
            [*command, "bridge", "--stdio", *(bridge_args or [])],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            bufsize=1,
        )
        assert self.process.stdin is not None
        assert self.process.stdout is not None
        self.selector = selectors.DefaultSelector()
        self.selector.register(self.process.stdout, selectors.EVENT_READ)

    def call(self, request: dict[str, Any]) -> dict[str, Any]:
        assert self.process.stdin is not None
        assert self.process.stdout is not None
        self.process.stdin.write(json.dumps(request, separators=(",", ":")) + "\n")
        self.process.stdin.flush()
        events = self.selector.select(self.timeout_ms / 1000)
        if not events:
            raise TimeoutError(f"bridge response exceeded {self.timeout_ms}ms")
        line = self.process.stdout.readline()
        if not line:
            raise RuntimeError("bridge closed stdout before responding")
        try:
            response = json.loads(line)
        except json.JSONDecodeError as error:
            raise RuntimeError("bridge stdout contained non-protocol text") from error
        if not isinstance(response, dict) or response.get("jsonrpc") != "2.0":
            raise RuntimeError("bridge response is not a JSON-RPC 2.0 object")
        if response.get("id") != request.get("id"):
            raise RuntimeError("bridge response id does not match the request")
        return response

    def close(self) -> str:
        if self.process.stdin is not None and not self.process.stdin.closed:
            self.process.stdin.close()
        try:
            self.process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            try:
                self.process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=2)
        stderr = self.process.stderr.read() if self.process.stderr is not None else ""
        self.selector.close()
        return stderr


def request(request_id: int, method: str, params: dict[str, Any]) -> dict[str, Any]:
    return {"jsonrpc": "2.0", "id": request_id, "method": method, "params": params}


def expect_result(response: dict[str, Any]) -> dict[str, Any]:
    if "error" in response:
        raise AssertionError(f"unexpected JSON-RPC error: {response['error'].get('message', 'unknown')}")
    result = response.get("result")
    if not isinstance(result, dict):
        raise AssertionError("response result must be an object")
    return result


def expect_error(response: dict[str, Any], code: str) -> None:
    error = response.get("error")
    if not isinstance(error, dict):
        raise AssertionError(f"expected error `{code}`")
    data = error.get("data")
    if not isinstance(data, dict) or data.get("code") != code:
        actual = data.get("code") if isinstance(data, dict) else None
        raise AssertionError(f"expected error `{code}`, got `{actual}`")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeout-ms", type=int, default=30_000)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("provide an executable command after --")
    if args.timeout_ms <= 0:
        parser.error("--timeout-ms must be positive")

    checks: list[Check] = []
    metadata: dict[str, Any] = {}
    client: BridgeClient | None = None
    all_capabilities = [
        "desktop.discover",
        "desktop.observe",
        "desktop.capture",
        "desktop.input",
        "desktop.pointer",
        "desktop.window",
        "desktop.app",
        "desktop.script",
        "desktop.defaults",
    ]

    def check(name: str, operation: Any) -> Any:
        started = time.monotonic()
        try:
            value = operation()
            checks.append(Check(name, True, round((time.monotonic() - started) * 1000)))
            return value
        except Exception as error:
            checks.append(
                Check(
                    name,
                    False,
                    round((time.monotonic() - started) * 1000),
                    str(error)[:500],
                )
            )
            raise

    try:
        client = BridgeClient(command, args.timeout_ms)

        initialized = check(
            "initialize",
            lambda: expect_result(
                client.call(
                    request(
                        1,
                        "initialize",
                        {
                            "client": {
                                "id": "computer-pilot.conformance",
                                "name": "Computer Pilot Conformance",
                                "version": "1.0.0",
                                "instanceId": "conformance-instance",
                            },
                            "protocol": {
                                "min": {"major": 1, "minor": 0},
                                "max": {"major": 1, "minor": 0},
                            },
                            "requestedCapabilities": [*all_capabilities, "future.capability"],
                            "launchMode": "embedded",
                        },
                    )
                )
            ),
        )
        if initialized.get("protocol") != {"major": 1, "minor": 0}:
            raise AssertionError("bridge did not negotiate protocol 1.0")
        capabilities = initialized.get("capabilities", {})
        if capabilities.get("granted") != all_capabilities:
            raise AssertionError("capability grant does not match the request")
        if capabilities.get("unsupported") != ["future.capability"]:
            raise AssertionError("unsupported capability was not reported")
        metadata["serviceVersion"] = initialized.get("serviceVersion")
        metadata["protocol"] = initialized.get("protocol")

        listed = check(
            "tools_list",
            lambda: expect_result(client.call(request(2, "tools/list", {}))),
        )
        tools = listed.get("tools")
        if not isinstance(tools, list) or len(tools) != 27:
            raise AssertionError("tools/list did not return all 27 automation tools")
        names = {tool.get("name") for tool in tools if isinstance(tool, dict)}
        if not {"computer.examples", "computer.click", "computer.tell"}.issubset(names):
            raise AssertionError("tools/list is missing required tool families")
        metadata["toolCount"] = len(tools)

        first_call = check(
            "tool_call",
            lambda: expect_result(
                client.call(
                    request(
                        3,
                        "tools/call",
                        {
                            "name": "computer.examples",
                            "arguments": {},
                            "commandId": "conformance-examples",
                            "deadlineMs": args.timeout_ms,
                        },
                    )
                )
            ),
        )
        result = first_call.get("result")
        if not isinstance(result, dict) or result.get("schema_version") != "1.0" or result.get("ok") is not True:
            raise AssertionError("tool result does not use machine schema 1.0")

        duplicate = check(
            "duplicate_command",
            lambda: expect_result(
                client.call(
                    request(
                        4,
                        "tools/call",
                        {
                            "name": "computer.examples",
                            "arguments": {},
                            "commandId": "conformance-examples",
                            "deadlineMs": args.timeout_ms,
                        },
                    )
                )
            ),
        )
        if duplicate != first_call:
            raise AssertionError("duplicate Command ID did not return the cached terminal result")

        check(
            "command_id_conflict",
            lambda: expect_error(
                client.call(
                    request(
                        5,
                        "tools/call",
                        {
                            "name": "computer.examples",
                            "arguments": {"topic": "launch-app"},
                            "commandId": "conformance-examples",
                        },
                    )
                ),
                "invalid_argument",
            ),
        )
        check(
            "unknown_argument",
            lambda: expect_error(
                client.call(
                    request(
                        6,
                        "tools/call",
                        {"name": "computer.examples", "arguments": {"shell": "ignored"}},
                    )
                ),
                "invalid_argument",
            ),
        )

        def verify_read_deadline() -> None:
            expired = expect_result(
                client.call(
                    request(
                        7,
                        "tools/call",
                        {
                            "name": "computer.wait",
                            "arguments": {
                                "app": "Finder",
                                "text": "__computer_pilot_conformance_missing__",
                                "timeout": 5,
                            },
                            "deadlineMs": 1,
                        },
                    )
                )
            )
            if expired.get("command", {}).get("status") != "command_expired":
                raise AssertionError("read-only deadline did not produce command_expired")
            if expired.get("error", {}).get("code") != "command_expired":
                raise AssertionError("read-only deadline did not return a stable error code")

        check("read_deadline", verify_read_deadline)

        check(
            "invalid_tool",
            lambda: expect_error(
                client.call(
                    request(
                        8,
                        "tools/call",
                        {"name": "computer.does_not_exist", "arguments": {}},
                    )
                ),
                "tool_not_found",
            ),
        )
        check(
            "shutdown",
            lambda: expect_result(client.call(request(9, "shutdown", {}))),
        )
        stderr = client.close()
        client = None
        if stderr.strip():
            raise AssertionError("bridge wrote diagnostics during a successful conformance run")

        def verify_launch_deny() -> None:
            nonlocal client
            client = BridgeClient(
                command,
                args.timeout_ms,
                ["--deny", "desktop.input"],
            )
            restricted = expect_result(
                client.call(
                    request(
                        1,
                        "initialize",
                        {
                            "client": {
                                "id": "computer-pilot.conformance",
                                "name": "Computer Pilot Conformance",
                                "version": "1.0.0",
                                "instanceId": "conformance-restricted",
                            },
                            "protocol": {
                                "min": {"major": 1, "minor": 0},
                                "max": {"major": 1, "minor": 0},
                            },
                            "requestedCapabilities": all_capabilities,
                            "launchMode": "embedded",
                        },
                    )
                )
            )
            if restricted.get("capabilities", {}).get("denied") != ["desktop.input"]:
                raise AssertionError("launch-time capability removal was not reported")
            restricted_tools = expect_result(client.call(request(2, "tools/list", {}))).get("tools")
            restricted_names = {
                tool.get("name") for tool in restricted_tools if isinstance(tool, dict)
            }
            if "computer.click" in restricted_names:
                raise AssertionError("denied tools remained in the runtime manifest")
            expect_error(
                client.call(
                    request(
                        3,
                        "tools/call",
                        {"name": "computer.click", "arguments": {"ref": 1}},
                    )
                ),
                "capability_denied",
            )
            stderr = client.close()
            client = None
            if stderr.strip():
                raise AssertionError("restricted bridge wrote diagnostics during EOF cleanup")

        check("launch_deny_and_eof_cleanup", verify_launch_deny)
    except Exception:
        if client is not None:
            client.close()

    passed = all(item.passed for item in checks) and len(checks) == 10
    report = {
        "schemaVersion": "computer-pilot.conformance.v1",
        "passed": passed,
        "metadata": metadata,
        "checks": [item.__dict__ for item in checks],
    }
    print(json.dumps(report, separators=(",", ":")))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
