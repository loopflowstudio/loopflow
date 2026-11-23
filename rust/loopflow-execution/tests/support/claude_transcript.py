def keep_claude_turn(request, answer, structured=None):
    """Append the turn lf sent as `request` and its answer to the transcript
    Claude keeps for the conversation, where lf reads final answers and
    usage. Claude dates each row and keeps a `--json-schema` answer as its
    `StructuredOutput` tool call."""
    import json, os, re, time

    home = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
    project = os.path.join(home, "projects", re.sub("[^A-Za-z0-9]", "-", os.getcwd()))
    os.makedirs(project, exist_ok=True)
    content = [{"type": "text", "text": answer}]
    if structured is not None:
        content.append(
            {"type": "tool_use", "id": "structured", "name": "StructuredOutput", "input": structured}
        )
    stamp = time.strftime("%Y-%m-%dT%H:%M:%S.000Z", time.gmtime())
    rows = [
        {"type": "user", "userType": "external", "uuid": request["uuid"], "timestamp": stamp,
         "message": {"role": "user", "content": request["message"]["content"]}},
        {"type": "assistant", "uuid": request["uuid"] + "-answer", "timestamp": stamp,
         "message": {"id": request["uuid"] + "-answer", "role": "assistant",
                     "stop_reason": "end_turn", "content": content,
                     "usage": {"input_tokens": 20, "output_tokens": 5}}},
    ]
    with open(os.path.join(project, request["session_id"] + ".jsonl"), "a") as transcript:
        for row in rows:
            transcript.write(json.dumps(row) + "\n")
