#!/usr/bin/env python3
"""Compare current complete-profile observations with a separately reviewed candidate baseline."""
import argparse
import json
from pathlib import Path
from run_local_readiness_benchmark import write_json

FIELDS = ('outcome', 'snapshot_schema', 'semantic_hash', 'semantic_projection_sha256',
          'counts', 'error_schema', 'error_code', 'error_stage', 'error_stderr_sha256')


def identity(sample):
    return {k: sample[k] for k in FIELDS if k in sample}


def compare(report, baseline):
    differences = []
    if (baseline.get('schema_version') != 'codenoesis.local-readiness-baseline/v1'
            or report.get('status') != 'candidate_review_required' or report.get('binary_unchanged') is not True
            or baseline.get('review_status') != 'candidate_pending_independent_review'):
        differences.append('incomplete observation or unsupported baseline authority')
    if report.get('corpus_sha256') != baseline.get('corpus_sha256'):
        differences.append('corpus contract changed')
    observed = {e['id']: e for e in report['entries']}
    expected = {e['id']: e for e in baseline['entries']}
    if (not observed or len(observed) != len(report['entries'])
            or len(expected) != len(baseline['entries']) or set(observed) != set(expected)):
        differences.append('missing, extra or duplicate repository entries')
    for name in sorted(observed.keys() & expected.keys()):
        entry, reference = observed[name], expected[name]
        if (any(entry.get(k) != reference.get(k) for k in ('revision', 'tree'))
                or entry.get('summary', {}).get('deterministic') is not True
                or len(entry.get('samples', [])) != 3
                or any(identity(sample) != reference['identity'] for sample in entry['samples'])):
            differences.append(name + ': source, outcome, semantics or determinism changed')
    return {'schema_version': 'codenoesis.local-readiness-comparison/v1',
            'status': 'candidate_mismatch' if differences else 'candidate_identity_match',
            'ga_accepted': False, 'accuracy_certified': False,
            'performance_gate': 'not evaluated: non-exclusive host and uncontrolled caches',
            'differences': differences}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--baseline', type=Path, default=Path(__file__).resolve().parents[1] /
                        'benchmarks/baselines/local-readiness-v1.candidate.json')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.output.exists():
            raise ValueError('comparison output already exists')
        result = compare(json.loads(args.report.read_text()), json.loads(args.baseline.read_text()))
        write_json(args.output, result)
        print(json.dumps(result, indent=2))
        return 0 if result['status'] == 'candidate_identity_match' else 2
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(json.dumps({'error': 'comparison.invalid_input', 'detail': str(error)}))
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
