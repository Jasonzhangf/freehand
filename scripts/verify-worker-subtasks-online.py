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


async def query_adp(url, request_id, query, timeout_seconds):
    async with websockets.connect(url) as ws:
        await ws.send(json.dumps(adp_query(request_id, query), ensure_ascii=False))
        return await recv_until(ws, request_id, timeout_seconds)


def unwrap_query_result(response, key):
    if response.get("kind") != "query_result":
        raise RuntimeError(f"{key} query failed: {json.dumps(response, ensure_ascii=False)}")
    result = response.get("result", {})
    if key not in result:
        raise RuntimeError(f"{key} query returned unexpected result: {json.dumps(response, ensure_ascii=False)}")
    return result[key]


def task_visible_in_parent(task, parent_session_id):
    if task.get("parent_session_id") == parent_session_id:
        return True
    attached = task.get("attached_session_ids") or []
    return parent_session_id in attached


def forbidden_worker_user_text(turn):
    user_text = turn.get("user_text")
    if not user_text:
        return False
    return (
        "Execute the assigned Task Center task" in user_text
        or "The tool result has been returned" in user_text
        or "Task ID:" in user_text
    )


async def run(args):
    board_response = await query_adp(
        args.url,
        "worker-subtasks-taskboard",
        {"QueryTaskBoard": {"include_terminal": args.include_terminal}},
        args.timeout,
    )
    board = unwrap_query_result(board_response, "TaskBoard")
    tasks = [
        task
        for task in board.get("tasks", [])
        if task_visible_in_parent(task, args.parent_session)
    ]
    tasks.sort(key=lambda task: str(task.get("task_id") or ""))
    if args.require_count is not None and len(tasks) != args.require_count:
        result = {
            "ok": False,
            "error": f"expected {args.require_count} child task(s), found {len(tasks)}",
            "parent_session_id": args.parent_session,
            "task_count": len(tasks),
            "tasks": tasks,
        }
        print(json.dumps(result, ensure_ascii=False, sort_keys=True))
        return 1

    inspections = []
    ok = True
    for index, task in enumerate(tasks, start=1):
        worker_session_id = task.get("worker_session_id")
        inspection = {
            "index": index,
            "task_id": task.get("task_id"),
            "title": task.get("title"),
            "status": task.get("status"),
            "assignee_agent_id": task.get("assignee_agent_id"),
            "active_execution_id": task.get("active_execution_id"),
            "worker_session_id": worker_session_id,
            "turn_count": 0,
            "user_text_leak_count": 0,
            "terminal_statuses": [],
            "error": None,
        }
        if not worker_session_id:
            inspection["error"] = "missing worker_session_id in TaskBoard projection"
            inspections.append(inspection)
            ok = False
            continue
        try:
            transcript_response = await query_adp(
                args.url,
                f"worker-subtasks-turns-{index}",
                {"QuerySessionTurns": {"session_id": worker_session_id}},
                args.timeout,
            )
            transcript = unwrap_query_result(transcript_response, "SessionTurns")
            turns = transcript.get("turns", [])
            inspection["turn_count"] = len(turns)
            inspection["user_text_leak_count"] = sum(
                1 for turn in turns if forbidden_worker_user_text(turn)
            )
            inspection["terminal_statuses"] = [
                turn.get("terminal_status") for turn in turns if turn.get("terminal_status")
            ]
            if inspection["user_text_leak_count"]:
                inspection["error"] = "worker internal prompt projected as user_text"
                ok = False
            if args.require_transcript and not turns:
                inspection["error"] = inspection["error"] or "worker transcript is empty"
                ok = False
        except Exception as exc:
            inspection["error"] = str(exc)
            ok = False
        inspections.append(inspection)

    result = {
        "ok": ok,
        "url": args.url,
        "parent_session_id": args.parent_session,
        "task_count": len(tasks),
        "inspections": inspections,
    }
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0 if ok else 1


def main():
    parser = argparse.ArgumentParser(
        description="Read-only online verifier for inspecting every Worker child task of one parent session."
    )
    parser.add_argument("--url", default="ws://127.0.0.1:4042/adp")
    parser.add_argument("--parent-session", required=True)
    parser.add_argument("--timeout", type=float, default=20.0)
    parser.add_argument("--require-count", type=int)
    parser.add_argument("--include-terminal", action="store_true")
    parser.add_argument("--require-transcript", action="store_true")
    args = parser.parse_args()
    raise SystemExit(asyncio.run(run(args)))


if __name__ == "__main__":
    main()
