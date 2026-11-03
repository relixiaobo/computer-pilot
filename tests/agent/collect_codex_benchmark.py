#!/usr/bin/env python3
"""Collect measured GO turns from the two owned Codex task rollout files.

SESSION_DIR contains threads.json (arm -> thread ID) and LABEL-saved.json.
ROLLOUT_DIR is the local Codex session directory containing those two rollouts.
Only their tool records and usage/timing metadata are exported, never reasoning.
This is an offline benchmark reader, not an Agent integration with Codex.
"""
import argparse
import collections
import json
import re
from pathlib import Path

from benchmark_long_workflow import verify_saved


def usage_summary(records):
    """One response ID is one model round; reasoning is a subset of output."""
    tokens = dict(sum((collections.Counter(u) for u in records.values()), collections.Counter()))
    return dict(model_calls=len(records), tokens=tokens)


class UsageReader:
    """Read completed usage records incrementally, including interrupted tasks."""
    def __init__(self, path):
        self.path, self.offset, self.records = path, 0, {}
        self.completed = False

    def read(self):
        with self.path.open('rb') as stream:
            stream.seek(self.offset)
            while line := stream.readline():
                if not line.endswith(b'\n'):
                    break
                event = json.loads(line)
                if event['type'] == 'token_usage_record':
                    payload = event['payload']
                    self.records[payload['response_id']] = payload['usage']
                elif event['type'] == 'event_msg' and event['payload'].get('type') == 'task_complete':
                    self.completed = True
                self.offset = stream.tell()
        return usage_summary(self.records)


def collect_turns(path, arm, thread_id):
    turns = {}
    for line in path.read_text().splitlines():
        event = json.loads(line)
        payload = event.get('payload', {})
        turn_id = payload.get('turn_id')
        if not turn_id:
            continue
        turn = turns.setdefault(turn_id, dict(
            arm=arm, thread_id=thread_id, turn_id=turn_id,
            tools=[], usage_records={}))
        if event['type'] == 'turn_context':
            turn.update(model=payload.get('model'),
                        reasoning_effort=payload.get('effort'),
                        service_tier=payload.get('service_tier'))
        elif event['type'] == 'token_usage_record':
            turn['usage_records'][payload['response_id']] = payload['usage']
        elif event['type'] == 'event_msg':
            kind = payload.get('type')
            if kind == 'task_started':
                turn['started_at'] = event['timestamp']
            elif kind == 'task_complete':
                turn.update(completed_at=event['timestamp'],
                            elapsed_ms=payload['duration_ms'],
                            final=payload.get('last_agent_message'))
            elif kind == 'turn_aborted':
                turn['aborted_at'] = event['timestamp']
                if payload.get('duration_ms') is not None:
                    turn['interrupted_elapsed_ms'] = payload['duration_ms']
            elif kind == 'item_completed':
                item = payload['item']
                if (item.get('type') == 'FunctionCallOutput'
                        and item.get('name') == 'send_message_to_thread'):
                    match = re.search(r'GO ((?:native|cu)-\d+)\.', item.get('output', ''))
                    if match:
                        turn['label'] = match[1]
                if item.get('type') in ('McpToolCall', 'CommandExecution', 'ImageView'):
                    image_view = item['type'] == 'ImageView'
                    # ImageView completion records carry a path, not image bytes
                    # or a status. Count the operation alongside shell/MCP calls,
                    # without counting its raw function-call record a second time.
                    output = ('' if image_view else item.get('aggregated_output', '')
                              if item['type'] == 'CommandExecution' else item.get('result', {}))
                    turn['tools'].append(dict(
                        id=item['id'], kind=item['type'], server=item.get('server'),
                        tool='view_image' if image_view else item.get('tool'), command=item.get('command'),
                        arguments={'path': item.get('path')} if image_view else item.get('arguments'),
                        start_ms=payload.get('started_at_ms'),
                        end_ms=payload.get('completed_at_ms'),
                        status=item.get('status', 'completed' if image_view else None),
                        exit_code=item.get('exit_code'),
                        output=output))
    return turns.values()


def summarize(turn):
    turn.update(usage_summary(turn.pop('usage_records')))
    turn['tool_calls'] = len(turn['tools'])
    turn['tool_output_chars'] = sum(
        len(tool.get('output', '')) if isinstance(tool.get('output', ''), str)
        else sum(len(block.get('text', '')) for block in tool['output'].get('content', []))
        for tool in turn['tools'])
    turn['tool_errors'] = sum(
        tool.get('exit_code') not in (None, 0) or tool.get('status') == 'failed'
        or (isinstance(tool.get('output'), dict) and tool['output'].get('isError', False))
        for tool in turn['tools'])
    # Union overlapping intervals; residual includes model, network and orchestration.
    intervals = sorted((tool['start_ms'], tool['end_ms']) for tool in turn['tools']
                       if tool.get('start_ms') is not None and tool.get('end_ms') is not None)
    merged = []
    for start, end in intervals:
        if merged and start <= merged[-1][1]:
            merged[-1][1] = max(merged[-1][1], end)
        else:
            merged.append([start, end])
    turn['tool_interval_ms'] = sum(end - start for start, end in merged)
    elapsed = turn.get('elapsed_ms', turn.get('interrupted_elapsed_ms'))
    turn['non_tool_interval_ms'] = elapsed - turn['tool_interval_ms'] if elapsed is not None else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('session_dir', type=Path)
    parser.add_argument('rollout_dir', type=Path)
    args = parser.parse_args()
    root = args.session_dir
    threads = json.loads((root / 'threads.json').read_text())
    report = dict(
        protocol='Two isolated actual Codex tasks, shared model/settings, sequential GO turns '
                 'after untimed initialization. Repetitions retain each arm task history. '
                 'Only one arm acts at a time.',
        runs=[], setup=[])
    for arm, thread_id in threads.items():
        paths = list(args.rollout_dir.rglob(f'*{thread_id}.jsonl'))
        if len(paths) != 1:
            parser.error(f'Expected one rollout for {thread_id}; found {len(paths)}')
        for turn in collect_turns(paths[0], arm, thread_id):
            if 'elapsed_ms' not in turn:
                continue
            summarize(turn)
            if 'label' not in turn:
                report['setup'].append({k: v for k, v in turn.items() if k != 'tools'})
                continue
            turn['verification'] = verify_saved(root / (turn['label'] + '-saved.json'))
            turn['verified'] = turn['verification']['passed']
            (root / (turn['label'] + '-trace.json')).write_text(
                json.dumps(turn, ensure_ascii=False, indent=2))
            report['runs'].append({k: v for k, v in turn.items() if k != 'tools'})
    report['runs'].sort(key=lambda turn: turn['started_at'])
    (root / 'results.json').write_text(json.dumps(report, ensure_ascii=False, indent=2))
    for run in report['runs']:
        print(json.dumps({k: run[k] for k in (
            'label', 'model', 'reasoning_effort', 'elapsed_ms', 'tool_interval_ms',
            'non_tool_interval_ms', 'model_calls', 'tool_calls', 'tool_errors',
            'verified', 'tokens')}, ensure_ascii=False))


if __name__ == '__main__':
    main()
