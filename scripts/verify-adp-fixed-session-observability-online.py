#!/usr/bin/env python3
import argparse
import asyncio
import json
import sys

try:
    import websockets
except ImportError as exc:
    raise SystemExit(
        "missing Python package `websockets`; install it or use the repo test host that already provides it"
    ) from exc


def adp_command(request_id, command):
    return {"kind": "command", "request_id": request_id, "command": command}


def adp_query(request_id, query):
    return {"kind": "query", "request_id": request_id, "query": query}


async def recv_until(ws, request_id, timeout_seconds):
    deadline = asyncio.get_event_loop().time() + timeout_seconds
    while True:
        remaining = deadline - asyncio.get_event_loop().time()
        if remaining <= 0:
            return {"kind": "timeout", "request_id": request_id}
        msg = json.loads(await asyncio.wait_for(ws.recv(), timeout=remaining))
        if msg.get("request_id") == request_id:
            return msg


async def query_snapshot(url, session_id, label, timeout_seconds):
    async with websockets.connect(url) as ws:
        requests = [
            adp_query(f"{label}-turns", {"QuerySessionTurns": {"session_id": session_id}}),
            adp_query(f"{label}-agents", {"QueryAgentBoard": {}}),
            adp_query(
                f"{label}-tasks",
                {"QueryTaskBoard": {"include_terminal": False}},
            ),
        ]
        for request in requests:
            await ws.send(json.dumps(request))
        responses = {}
        for request in requests:
            responses[request["request_id"]] = await recv_until(
                ws, request["request_id"], timeout_seconds
            )
        return summarize_snapshot(session_id, responses)


def summarize_snapshot(session_id, responses):
    turns = []
    turn_response = responses.get(next(k for k in responses if k.endswith("-turns")), {})
    if turn_response.get("kind") == "query_result":
        turns = (
            turn_response.get("result", {})
            .get("SessionTurns", {})
            .get("turns", [])
        )
    agents = []
    agent_response = responses.get(next(k for k in responses if k.endswith("-agents")), {})
    if agent_response.get("kind") == "query_result":
        agents = (
            agent_response.get("result", {})
            .get("AgentBoard", {})
            .get("agents", [])
        )
    tasks = []
    task_response = responses.get(next(k for k in responses if k.endswith("-tasks")), {})
    if task_response.get("kind") == "query_result":
        tasks = (
            task_response.get("result", {})
            .get("TaskBoard", {})
            .get("tasks", [])
        )
    worker = next((agent for agent in agents if agent.get("agent_id") == "worker"), None)
    blocked_tasks = [task for task in tasks if task.get("status") == "blocked"][:5]
    return {
        "session_id": session_id,
        "turn_count": len(turns),
        "turns": [
            {
                "turn_id": turn.get("turn_id"),
                "terminal_status": turn.get("terminal_status"),
                "user_text": turn.get("user_text"),
                "terminal_text": turn.get("terminal_text"),
            }
            for turn in turns
        ],
        "worker": None
        if worker is None
        else {
            "state": worker.get("state"),
            "current_task_id": worker.get("current_task_id"),
            "current_execution_id": worker.get("current_execution_id"),
            "current_activity": worker.get("current_activity"),
        },
        "blocked_tasks": [
            {
                "task_id": task.get("task_id"),
                "status": task.get("status"),
                "active_execution_id": task.get("active_execution_id"),
                "last_event_seq": task.get("last_event_seq"),
            }
            for task in blocked_tasks
        ],
    }


async def run(args):
    submit_ws = await websockets.connect(args.url)
    try:
        await submit_ws.send(
            json.dumps(
                adp_command(
                    "fixed-session-submit",
                    {
                        "SubmitUserInput": {
                            "text": args.prompt,
                            "session_id": args.session,
                        }
                    },
                )
            )
        )
        await asyncio.sleep(args.pending_delay)
        pending = await query_snapshot(args.url, args.session, "pending", args.timeout)
        receipt = await recv_until(submit_ws, "fixed-session-submit", args.receipt_timeout)
        final = await query_snapshot(args.url, args.session, "final", args.timeout)
        ok = (
            pending["turn_count"] > 0
            and pending["turns"][0].get("user_text") == args.prompt
            and final["turn_count"] > 0
            and final["turns"][0].get("user_text") == args.prompt
        )
        result = {
            "ok": ok,
            "url": args.url,
            "session_id": args.session,
            "pending": pending,
            "receipt": receipt,
            "final": final,
        }
        print(json.dumps(result, ensure_ascii=False, sort_keys=True))
        return 0 if ok else 1
    finally:
        await submit_ws.close()


def main():
    parser = argparse.ArgumentParser(
        description="Verify fixed-session ADP submit remains observable through session/task/agent truth."
    )
    parser.add_argument("--url", default="ws://127.0.0.1:4042/adp")
    parser.add_argument("--session", default="online-fixed-observability-standard")
    parser.add_argument(
        "--prompt",
        default="Fixed observability proof: keep this prompt visible while provider runs or fails.",
    )
    parser.add_argument("--pending-delay", type=float, default=2.0)
    parser.add_argument("--timeout", type=float, default=10.0)
    parser.add_argument("--receipt-timeout", type=float, default=120.0)
    args = parser.parse_args()
    raise SystemExit(asyncio.run(run(args)))


if __name__ == "__main__":
    main()
