#!/usr/bin/env python3
"""Observe complete Rust/JVM profiles on one recorded binary; never promote an oracle."""

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import platform
import re
import statistics
import sys
import time

import run_public_rust_evaluation as public

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / 'benchmarks/corpora/local-readiness-v1.json'
SCHEMAS = {'rust': 'codenoesis.repository-snapshot/v18',
           'kotlin': 'codenoesis.repository-snapshot/v19',
           'java': 'codenoesis.repository-snapshot/v20'}
PROFILE_FLAGS = {'--profile', '--acquisition-profile', '--workspace-profile',
                 '--repository-boundary-profile', '--manifest-profile',
                 '--rust-semantic-profile', '--rust-framework-profile',
                 '--rust-callable-profile', '--rust-expression-profile',
                 '--rust-flow-profile', '--rust-constant-profile',
                 '--output-capacity-profile', '--execution-limit-profile',
                 '--kotlin-profile', '--java-profile'}


def write_json(path, value):
    temporary = path.with_suffix(path.suffix + '.writing')
    temporary.write_text(json.dumps(value, indent=2, allow_nan=False) + '\n', encoding='utf-8')
    os.replace(temporary, path)


def validate_corpus(corpus):
    if (not isinstance(corpus, dict)
            or corpus.get('schema_version') != 'codenoesis.local-readiness-corpus/v1'
            or corpus.get('repetitions') != 3 or corpus.get('concurrency') != 1
            or type(corpus.get('concurrency')) is not int
            or corpus.get('cache_state') != 'mixed' or corpus.get('network_allowed') is not False
            or corpus.get('percentile_method') != 'nearest-rank'
            or type(corpus.get('timeout_seconds')) is not int
            or not 1 <= corpus['timeout_seconds'] <= 600
            or not isinstance(corpus.get('entries'), list) or not corpus['entries']):
        raise ValueError('invalid fixed observation protocol')
    seen = set()
    for entry in corpus['entries']:
        if not isinstance(entry, dict):
            raise ValueError('corpus entry must be an object')
        identifier = entry.get('id', '')
        arguments = entry.get('scan_arguments')
        if (not re.fullmatch(r'[a-z0-9][a-z0-9-]{0,63}', identifier) or identifier in seen
                or entry.get('language') not in SCHEMAS
                or entry.get('cohort') not in {'extraction', 'boundary'}
                or any(not re.fullmatch(r'[0-9a-f]{40}', entry.get(k, '')) for k in ('revision', 'tree'))
                or not isinstance(entry.get('repository_id'), str)
                or not entry['repository_id'].startswith('urn:codenoesis:')
                or not isinstance(arguments, list) or len(arguments) % 2
                or not arguments or len(set(arguments[::2])) != len(arguments[::2])):
            raise ValueError('invalid or duplicate corpus entry')
        if any(flag not in PROFILE_FLAGS for flag in arguments[::2]) or any(
                not isinstance(value, str) or not re.fullmatch(r'[a-z0-9-]+', value)
                for value in arguments[1::2]):
            raise ValueError('only explicit scan profile selectors are allowed')
        seen.add(identifier)


def command_for(binary, repository, store, entry):
    identity = '--repository-id' if entry['language'] == 'rust' else '--repository-identity'
    command = [str(binary), 'scan', '--repository', str(repository), identity,
            entry['repository_id'], '--revision', entry['revision'],
            *entry['scan_arguments'], '--store', str(store)]
    if entry['language'] == 'rust':
        command.extend(['--format', 'json'])
    return command


def preflight(repository, entry, home):
    checks = [(['rev-parse', '--is-shallow-repository'], 'false'),
              (['rev-parse', '--show-object-format'], 'sha1'),
              (['rev-parse', 'HEAD'], entry['revision']),
              (['rev-parse', 'HEAD^{tree}'], entry['tree']),
              (['status', '--porcelain', '--untracked-files=all'], '')]
    for command, expected in checks:
        if public.git_text(repository, command, home) != expected:
            raise ValueError('repository preflight failed: ' + entry['id'] + ' ' + ' '.join(command))


def summarize(samples, planned):
    identities = []
    for sample in samples:
        if sample['outcome'] == 'success':
            identities.append(('success', sample.get('semantic_hash'), sample.get('semantic_projection_sha256')))
        elif sample['outcome'] == 'typed_rejection':
            identities.append(('typed_rejection', sample.get('error_stderr_sha256')))
        else:
            identities.append(None)
    durations = [s['wall_time_ns'] for s in samples]
    return {'planned_samples': planned, 'attempted_samples': len(samples),
            'extraction_successes': sum(s['outcome'] == 'success' for s in samples),
            'typed_rejections': sum(s['outcome'] == 'typed_rejection' for s in samples),
            'deterministic': len(samples) == planned and None not in identities and len(set(identities)) == 1,
            'median_wall_time_ns': statistics.median(durations) if durations else None,
            'p95_wall_time_ns': public.nearest_rank(durations, 95) if durations else None}


def measure(binary, entry, repository, sample_root, timeout, home):
    sample_root.mkdir()
    stdout = sample_root / 'stdout.json'
    stderr = sample_root / 'stderr.json'
    command = command_for(binary, repository, sample_root / 'store', entry)
    sample = {'command': command, 'outcome': 'protocol_failure', 'exit_code': None}
    start = time.monotonic_ns()
    try:
        with stdout.open('wb') as out, stderr.open('wb') as err:
            code, duration = public.wait_for_process(command, out, err, timeout, home)
        sample.update(exit_code=code, wall_time_ns=duration)
        parsed = public.parse_success(stdout, stderr) if code == 0 else public.parse_rejection(code, stdout, stderr)
        if code == 0:
            if (parsed.get('snapshot_schema') != SCHEMAS[entry['language']]
                    or not public.HEX_64.fullmatch(parsed['semantic_hash'])
                    or any(parsed.get('graph_families', {}).get(k) != 'list'
                           for k in public.CANDIDATE_GRAPH_FAMILIES)):
                raise ValueError('unexpected snapshot contract')
        else:
            # Reuse the fixed error vocabulary, never infer acceptance from a message.
            # JVM rejections are failures in this extraction corpus until separately reviewed.
            if entry['language'] != 'rust':
                raise ValueError('JVM extraction failed; inspect retained typed error')
            public.validate_candidate_sample({**parsed, 'stage': 'constant', 'index': 1,
                'wall_time_ns': duration, 'stdout_bytes': stdout.stat().st_size,
                'stderr_bytes': stderr.stat().st_size}, 'constant', 1)
        sample.update(parsed)
        # Keep raw context only in retained stderr, not in the portable report.
        sample.pop('error_context', None)
    except public.EvaluationError as error:
        sample.update(outcome='timeout' if error.code == 'evaluation.timeout' else 'protocol_failure',
                      failure_code=error.code)
    except (ValueError, KeyError, TypeError, OSError) as error:
        sample.update(outcome='protocol_failure', failure_code=type(error).__name__)
    sample.setdefault('wall_time_ns', time.monotonic_ns() - start)
    for name, path in [('stdout', stdout), ('stderr', stderr)]:
        if path.exists():
            sample[name + '_bytes'] = path.stat().st_size
            sample[name + '_sha256'] = public.sha256_file(path)
    write_json(sample_root / 'sample.json', sample)
    return sample


def run(args):
    corpus = json.loads(args.corpus.read_text(encoding='utf-8'))
    validate_corpus(corpus)
    locations = json.loads(args.repositories.read_text(encoding='utf-8'))
    if (not isinstance(locations, dict) or not all(isinstance(p, str) for p in locations.values())
            or set(locations) != {e['id'] for e in corpus['entries']}):
        raise ValueError('repository map must cover the complete corpus exactly')
    build = json.loads(args.build_record.read_text(encoding='utf-8'))
    binary = args.binary.resolve(strict=True)
    if (not isinstance(build, dict) or build.get('schema_version') != 'codenoesis.benchmark-build/v1'
            or build.get('source_dirty') is not False
            or not re.fullmatch(r'[0-9a-f]{40}', build.get('product_commit', ''))
            or build.get('binary_sha256') != public.sha256_file(binary)):
        raise ValueError('binary does not match its clean-source build record')
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    home = output / 'home'
    home.mkdir()
    report = {'schema_version': 'codenoesis.local-readiness-observation/v1',
              'status': 'incomplete', 'started_at': datetime.now(timezone.utc).isoformat(),
              'build': build, 'corpus_version': corpus['id'],
              'corpus_sha256': public.sha256_file(args.corpus),
              'runner_sha256': public.sha256_file(Path(__file__)),
              'runner_dependency_sha256': public.sha256_file(Path(public.__file__)),
              'host': {'os': platform.platform(), 'architecture': platform.machine(),
                       'profile': args.host_profile, 'exclusive': False},
              'cache_state': 'mixed; fresh product stores, uncontrolled OS caches',
              'concurrency': 1, 'repetitions': 3, 'percentile_method': 'nearest-rank; n=3, p95=max',
              'enabled_extractors': ['rust-r16-source-only', 'kotlin-kmp-declarations-v1', 'java-declarations-v1'],
              'timeout_seconds': corpus['timeout_seconds'],
              'peak_rss': 'not measured', 'accuracy': 'separate source-oracle evaluation required',
              'entries': []}
    write_json(output / 'report.json', report)
    write_json(output / 'corpus.json', corpus)
    # Freeze all input admissions before starting any timed sample.
    for entry in corpus['entries']:
        preflight(Path(locations[entry['id']]).resolve(strict=True), entry, home)
    for entry in corpus['entries']:
        result = {'id': entry['id'], 'language': entry['language'], 'cohort': entry['cohort'],
                  'revision': entry['revision'], 'tree': entry['tree'], 'samples': []}
        report['entries'].append(result)
        for repeat in range(1, 4):
            if public.sha256_file(binary) != build['binary_sha256']:
                raise ValueError('binary changed during observation')
            repository = Path(locations[entry['id']]).resolve(strict=True)
            preflight(repository, entry, home)
            sample = measure(binary, entry, repository, output / f"{entry['id']}-{repeat}",
                             corpus['timeout_seconds'], home)
            sample['repeat'] = repeat
            result['samples'].append(sample)
            result['summary'] = summarize(result['samples'], 3)
            write_json(output / 'report.json', report)
            print(f"{entry['id']} {repeat}/3: {sample['outcome']} {sample['wall_time_ns']/1e9:.3f}s", flush=True)
            # A failed attempt is never retried; unattempted slots remain in the denominator.
            if sample['outcome'] not in ('success', 'typed_rejection'):
                break
        preflight(repository, entry, home)
    report['binary_unchanged'] = public.sha256_file(binary) == build['binary_sha256']
    report['finished_at'] = datetime.now(timezone.utc).isoformat()
    success = sum(e['summary']['extraction_successes'] for e in report['entries'])
    report['success_rate'] = success / (3 * len(corpus['entries']))
    report['summary'] = {'planned_samples': 3 * len(corpus['entries']),
        'attempted_samples': sum(len(e['samples']) for e in report['entries']),
        'extraction_successes': success,
        'successful_repositories': sum(e['summary']['extraction_successes'] == 3 for e in report['entries']),
        'deterministic_repositories': sum(e['summary']['deterministic'] for e in report['entries'])}
    valid = report['binary_unchanged'] and all(e['summary']['deterministic'] for e in report['entries'])
    report['status'] = 'candidate_review_required' if valid else 'observation_failed'
    write_json(output / 'report.json', report)
    return 0 if valid else 2


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--build-record', type=Path, required=True)
    parser.add_argument('--corpus', type=Path, default=CORPUS)
    parser.add_argument('--repositories', type=Path, required=True,
                        help='JSON map of corpus IDs to existing full local clones')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--host-profile', required=True)
    try:
        return run(parser.parse_args())
    except (ValueError, KeyError, TypeError, OSError, public.EvaluationError) as error:
        print(json.dumps({'error': 'benchmark.invalid_or_incomplete_run', 'detail': str(error)}), file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
