#!/usr/bin/env python3
"""Minimal Agent-neutral host for `cu bridge --stdio` (standard library only)."""

from __future__ import annotations

import argparse
import json
import os
import selectors
import subprocess
import sys
from pathlib import Path
from typing import Any


class BridgeError(RuntimeError):
    def __init__(
        self,
        code: str,
        message: str,
        *,
        retryable: bool = False,
        context: Any = None,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.retryable = retryable
        self.context = context


class ComputerPilotHost:
    """Ordered JSON-RPC client for one embedded Computer Pilot connection."""

    def __init__(
        self,
        executable: str | Path,
        *,
        denied_capabilities: list[str] | None = None,
        transport_timeout_seconds: float = 35.0,
    ) -> None:
        path = Path(executable).expanduser().resolve(strict=True)
        if not path.is_file() or not os.access(path, os.X_OK):
            raise ValueError(f"cu executable is not executable: {path}")
        if transport_timeout_seconds <= 0:
            raise ValueError("transport timeout must be positive")

        argv = [str(path), "bridge", "--stdio"]
        for capability in denied_capabilities or []:
            argv.extend(["--deny", capability])
        self._process = subprocess.Popen(
            argv,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            bufsize=1,
        )
        assert self._process.stdin is not None
        assert self._process.stdout is not None
        self._selector = selectors.DefaultSelector()
        self._selector.register(self._process.stdout, selectors.EVENT_READ)
        self._transport_timeout_seconds = transport_timeout_seconds
        self._next_id = 1
        self._initialized = False
        self._protocol_usable = True
        self._closed = False
        self._tools: dict[str, dict[str, Any]] | None = None

    def __enter__(self) -> ComputerPilotHost:
        return self

    def __exit__(self, exc_type: Any, exc: Any, traceback: Any) -> bool:
        try:
            self.close()
        except Exception:
            if exc_type is None:
                raise
        return False

    def initialize(
        self,
        *,
        requested_capabilities: list[str],
        client_id: str,
        client_name: str,
        client_version: str,
        instance_id: str,
    ) -> dict[str, Any]:
        result = self._request(
            "initialize",
            {
                "client": {
                    "id": client_id,
                    "name": client_name,
                    "version": client_version,
                    "instanceId": instance_id,
                },
                "protocol": {
                    "min": {"major": 1, "minor": 0},
                    "max": {"major": 1, "minor": 0},
                },
                "requestedCapabilities": requested_capabilities,
                "launchMode": "embedded",
            },
        )
        self._initialized = True
        return result

    def list_tools(self) -> list[dict[str, Any]]:
        self._require_initialized()
        result = self._request("tools/list", {})
        tools = result.get("tools")
        if not isinstance(tools, list) or not all(isinstance(tool, dict) for tool in tools):
            raise BridgeError("invalid_response", "tools/list returned an invalid manifest")
        named_tools = {
            tool["name"]: tool
            for tool in tools
            if isinstance(tool.get("name"), str)
        }
        if len(named_tools) != len(tools):
            raise BridgeError("invalid_response", "tool names are missing or duplicated")
        self._tools = named_tools
        return tools

    def call_tool(
        self,
        name: str,
        arguments: dict[str, Any],
        *,
        command_id: str,
        deadline_ms: int,
    ) -> dict[str, Any]:
        self._require_initialized()
        if self._tools is None:
            self.list_tools()
        assert self._tools is not None
        if name not in self._tools:
            raise BridgeError(
                "tool_not_available",
                f"tool `{name}` was not returned by tools/list",
                context={"tool": name},
            )
        return self._request(
            "tools/call",
            {
                "name": name,
                "arguments": arguments,
                "commandId": command_id,
                "deadlineMs": deadline_ms,
            },
        )

    def close(self) -> None:
        if self._closed:
            return
        shutdown_error: Exception | None = None
        if self._initialized and self._protocol_usable and self._process.poll() is None:
            try:
                self._request("shutdown", {})
            except Exception as error:
                shutdown_error = error
        if self._process.stdin is not None and not self._process.stdin.closed:
            self._process.stdin.close()
        try:
            self._process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            self._process.terminate()
            try:
                self._process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self._process.kill()
                self._process.wait(timeout=2)
        diagnostics = self._process.stderr.read() if self._process.stderr is not None else ""
        self._selector.close()
        self._closed = True
        if shutdown_error is not None:
            raise shutdown_error
        if self._process.returncode != 0:
            raise BridgeError(
                "bridge_exit",
                f"bridge exited with {self._process.returncode}: {diagnostics.strip()[:500]}",
            )

    def _require_initialized(self) -> None:
        if not self._initialized:
            raise BridgeError("not_initialized", "initialize must be called first")

    def _request(self, method: str, params: dict[str, Any]) -> dict[str, Any]:
        if self._closed:
            raise BridgeError("bridge_closed", "bridge is already closed")
        if self._process.poll() is not None:
            raise BridgeError("bridge_exit", "bridge exited before the request")
        request_id = self._next_id
        self._next_id += 1
        request = {
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params,
        }
        assert self._process.stdin is not None
        assert self._process.stdout is not None
        try:
            self._process.stdin.write(json.dumps(request, separators=(",", ":")) + "\n")
            self._process.stdin.flush()
        except (BrokenPipeError, OSError) as error:
            self._protocol_usable = False
            raise BridgeError("transport_error", f"failed to write bridge request: {error}") from error

        events = self._selector.select(self._transport_timeout_seconds)
        if not events:
            self._protocol_usable = False
            raise BridgeError(
                "transport_timeout",
                "bridge response timed out; inspect state before retrying any mutating call",
            )
        line = self._process.stdout.readline()
        if not line:
            self._protocol_usable = False
            raise BridgeError("transport_error", "bridge closed stdout before responding")
        try:
            response = json.loads(line)
        except json.JSONDecodeError as error:
            self._protocol_usable = False
            raise BridgeError("invalid_response", "bridge stdout was not JSON") from error
        if not isinstance(response, dict) or response.get("jsonrpc") != "2.0":
            self._protocol_usable = False
            raise BridgeError("invalid_response", "response is not JSON-RPC 2.0")
        if response.get("id") != request_id:
            self._protocol_usable = False
            raise BridgeError("invalid_response", "response ID does not match the request")
        rpc_error = response.get("error")
        if isinstance(rpc_error, dict):
            data = rpc_error.get("data") if isinstance(rpc_error.get("data"), dict) else {}
            raise BridgeError(
                str(data.get("code", "protocol_error")),
                str(rpc_error.get("message", "bridge request failed")),
                retryable=data.get("retryable") is True,
                context=data.get("context"),
            )
        result = response.get("result")
        if not isinstance(result, dict):
            raise BridgeError("invalid_response", "response result must be an object")
        return result


def parse_arguments(value: str) -> dict[str, Any]:
    parsed = json.loads(value)
    if not isinstance(parsed, dict):
        raise ValueError("--arguments must decode to a JSON object")
    return parsed


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cu", required=True, help="Exact cu executable to launch")
    parser.add_argument(
        "--capability",
        action="append",
        dest="capabilities",
        help="Capability to request (repeatable; default: desktop.discover)",
    )
    parser.add_argument("--deny", action="append", default=[], help="Launch-time capability removal")
    parser.add_argument("--tool", default="computer.examples")
    parser.add_argument("--arguments", default="{}", type=parse_arguments)
    parser.add_argument("--command-id", default="stdio-host-example-1")
    parser.add_argument("--deadline-ms", type=int, default=30_000)
    args = parser.parse_args()
    capabilities = args.capabilities or ["desktop.discover"]

    try:
        with ComputerPilotHost(args.cu, denied_capabilities=args.deny) as host:
            initialized = host.initialize(
                requested_capabilities=capabilities,
                client_id="computer-pilot.reference-host",
                client_name="Computer Pilot Reference Host",
                client_version="1.0.0",
                instance_id=f"reference-host:{os.getpid()}",
            )
            tools = host.list_tools()
            outcome = host.call_tool(
                args.tool,
                args.arguments,
                command_id=args.command_id,
                deadline_ms=args.deadline_ms,
            )
            print(
                json.dumps(
                    {
                        "schemaVersion": "computer-pilot.host-example.v1",
                        "ok": True,
                        "protocol": initialized.get("protocol"),
                        "grantedCapabilities": initialized.get("capabilities", {}).get("granted"),
                        "toolCount": len(tools),
                        "tool": args.tool,
                        "outcome": outcome,
                    },
                    separators=(",", ":"),
                )
            )
            return 0
    except (BridgeError, OSError, ValueError, json.JSONDecodeError) as error:
        code = error.code if isinstance(error, BridgeError) else "host_error"
        retryable = error.retryable if isinstance(error, BridgeError) else False
        print(
            json.dumps(
                {"ok": False, "code": code, "error": str(error), "retryable": retryable},
                separators=(",", ":"),
            ),
            file=sys.stderr,
        )
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
