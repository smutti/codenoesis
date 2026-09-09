#!/usr/bin/env python3
"""Score source-authored facts without converting candidate agreement into accuracy."""

import argparse
from collections import Counter, defaultdict
import hashlib
import html
import json
from pathlib import Path, PurePosixPath
import re

import run_public_rust_evaluation as public
from run_local_readiness_benchmark import write_json

ROOT = Path(__file__).resolve().parents[1]
ORACLE = ROOT / 'benchmarks/oracles/ontology-quality-v1.json'


def key(fact):
    return json.dumps(fact, sort_keys=True, ensure_ascii=False, separators=(',', ':'), allow_nan=False)


def compare_facts(expected, actual):
    """Multiset comparison: duplicates count as extra facts, empty ratios stay null."""
    wanted, got = Counter(map(key, expected)), Counter(map(key, actual))
    tp = sum((wanted & got).values())
    fp = sum((got - wanted).values())
    fn = sum((wanted - got).values())
    return {'tp': tp, 'fp': fp, 'fn': fn,
            'precision': tp / (tp + fp) if tp + fp else None,
            'recall': tp / (tp + fn) if tp + fn else None,
            'missing': [json.loads(k) for k in (wanted - got).elements()],
            'extra': [json.loads(k) for k in (got - wanted).elements()]}


def validate_oracle(oracle):
    if (not isinstance(oracle, dict)
            or oracle.get('schema_version') != 'codenoesis.ontology-quality-oracle/v1'
            or oracle.get('review_status') != 'candidate_pending_independent_review'
            or oracle.get('corpus_exposure') != 'development_exposed'
            or not isinstance(oracle.get('cases'), list) or not oracle['cases']):
        raise ValueError('v1 is a development-exposed candidate, never an independent certificate')
    ids = set()
    for case in oracle['cases']:
        if case['id'] in ids or not re.fullmatch(r'[0-9a-f]{40}', case['revision']):
            raise ValueError('duplicate or unpinned oracle case')
        ids.add(case['id'])
        paths = set()
        for file in case['files']:
            path = PurePosixPath(file['path'])
            if (path.is_absolute() or '..' in path.parts or '\\' in file['path']
                    or file['path'] in paths or not re.fullmatch(r'[0-9a-f]{64}', file['sha256'])):
                raise ValueError('invalid oracle source path or hash')
            paths.add(file['path'])
        facts = [key(item['fact']) for item in case['facts']]
        if not facts or len(set(facts)) != len(facts):
            raise ValueError('oracle facts must be nonempty and unique')
        for item in case['facts']:
            if (item['fact']['path'] not in paths or item['fact']['kind'] not in case['kinds']
                    or type(item['start_byte']) is not int or type(item['end_byte']) is not int
                    or not 0 <= item['start_byte'] < item['end_byte'] or not item['anchor']):
                raise ValueError('invalid expected fact or source anchor')
        for edge in case['relationships']:
            if (key(edge['source']) not in facts or key(edge['target']) not in facts
                    or edge['kind'] not in case['relationship_kinds']):
                raise ValueError('oracle relationship endpoint is outside its declared scope')


def score_case(case, graph, sources):
    for file in case['files']:
        if hashlib.sha256(sources[file['path']]).hexdigest() != file['sha256']:
            raise ValueError('source bytes do not match oracle: ' + file['path'])
    for item in case['facts']:
        if sources[item['fact']['path']][item['start_byte']:item['end_byte']] != item['anchor'].encode():
            raise ValueError('source anchor does not match oracle')
    evidence = {e['id']: e for e in graph['evidence']}
    entities = {e['id']: e for e in graph['entities']}
    if len(entities) != len(graph['entities']) or len(evidence) != len(graph['evidence']):
        raise ValueError('duplicate graph identifiers')
    references = defaultdict(set)
    for claim in graph['claims']:
        references[claim['subject_id']].update(claim['evidence_ids'])
    for entity in graph['entities']:
        references[entity['id']].update(entity.get('evidence_ids', []))
    normalized = {}
    for entity in graph['entities']:
        kind = entity['kind']
        if kind not in case['kinds']:
            continue
        subject = entities.get(entity.get('subject_id'), entity)
        if case['public_rust_surface'] and subject.get('visibility') != 'public':
            continue
        props = entity.get('properties', {})
        paths = {props['path']} if 'path' in props else {
            evidence[i]['path'] for i in references[entity['id']] if i in evidence}
        for path in sorted(paths & sources.keys()):
            owner = props.get('owner', subject['name'] if subject is not entity else '')
            fact = {'path': path, 'kind': kind, 'name': entity['name'], 'owner': owner}
            fact.update({k: props.get(k) for k in case['property_keys'].get(kind, [])})
            normalized[entity['id']] = fact
    expected = [item['fact'] for item in case['facts']]
    actual = list(normalized.values())
    relations = [{'kind': e['kind'], 'source': normalized[e['source']],
                  'target': normalized[e['target']], 'state': e.get('state', '')}
                 for e in graph['relationships'] if e['kind'] in case['relationship_kinds']
                 and e['source'] in normalized and e['target'] in normalized]
    rows = []
    for item in case['facts']:
        fact, start, end = item['fact'], item['start_byte'], item['end_byte']
        matches = [i for i, found in normalized.items() if key(found) == key(fact)]
        data = sources[fact['path']]
        oid = hashlib.sha1(f'blob {len(data)}\0'.encode() + data).hexdigest()
        valid = []
        for identifier in matches:
            for eid in references[identifier]:
                e = evidence.get(eid, {})
                if (e.get('path') == fact['path'] and e.get('blob_oid') == oid
                        and type(e.get('start_byte')) is int and type(e.get('end_byte')) is int
                        and 0 <= e['start_byte'] <= start < end <= e['end_byte'] <= len(data)):
                    valid.append(e)
        rows.append({'expected': fact, 'anchor': item['anchor'], 'start_byte': start, 'end_byte': end,
                     'actual': [normalized[i] for i in matches],
                     'matches': len(matches), 'evidence_covers_anchor': bool(valid),
                     'evidence_exact_anchor': any(e['start_byte'] == start and e['end_byte'] == end for e in valid),
                     'evidence_spans': sorted({(e['start_byte'], e['end_byte']) for e in valid})})
    actual_keys = set(map(key, actual))
    negatives = [{'fact': f, 'absent': key(f) not in actual_keys} for f in case['negatives']]
    return {'id': case['id'], 'language': case['language'],
            'entities': compare_facts(expected, actual),
            'entities_by_kind': {k: compare_facts([f for f in expected if f['kind'] == k],
                                                [f for f in actual if f['kind'] == k])
                                 for k in sorted(set(case['kinds']))},
            'relationships': compare_facts(case['relationships'], relations),
            'relationship_rows': {'expected': case['relationships'], 'actual': relations},
            'evidence': {'expected_facts': len(rows),
                         'covers_anchor': sum(r['evidence_covers_anchor'] for r in rows),
                         'exact_anchor': sum(r['evidence_exact_anchor'] for r in rows)},
            'rows': rows, 'hard_negatives': negatives,
            'coverage': [c for c in graph.get('coverage', []) if c.get('path', '') in ('', *sources)]}


def render_html(report):
    escape = lambda value: html.escape(str(value), quote=True)
    pieces = ['<!doctype html><html lang="en"><meta charset="utf-8">',
        '<meta name="viewport" content="width=device-width,initial-scale=1">',
        '<meta http-equiv="Content-Security-Policy" content="default-src \'none\'; style-src \'unsafe-inline\'; base-uri \'none\'">',
        '<title>CodeNoesis ontology quality — candidate review</title>',
        '<style>body{font:16px system-ui;max-width:1200px;margin:40px auto;padding:0 20px;color:#182638;background:#f6f8fc}table{border-collapse:collapse;width:100%;background:white;margin:20px 0}th,td{border:1px solid #d7deea;padding:10px;text-align:left;vertical-align:top}pre{white-space:pre-wrap;overflow-wrap:anywhere}summary{cursor:pointer}small{color:#475569}</style>',
        '<h1>Ontology quality: source → expectation → extraction</h1>',
        '<p>Candidate oracle, pending independent review. Development-exposed corpus. '
        'Agreement on selected facts; no whole-ontology accuracy or GA claim.</p>',
        '<p>Evidence must cover the source anchor in the correct Git blob. '
        'Exact-anchor equality describes the span size: a larger declaration span can also be correct.</p>',
        '<p>Product commit: <code>' + escape(report['product_commit']) + '</code></p>']
    for case in report['cases']:
        pieces.append('<h2>' + escape(case['id']) + '</h2><p>' + escape(case['entities']) + '</p>')
        pieces.append('<table><tr><th>Source / expected</th><th>Extracted</th><th>Evidence</th></tr>')
        for row in case['rows']:
            pieces.append('<tr><td><small>' + escape(row['expected']['path']) + ':' +
                escape(row['start_byte']) + '–' + escape(row['end_byte']) + '</small><pre>' +
                escape(row['anchor']) + '</pre><pre>' + escape(json.dumps(row['expected'], indent=2)) +
                '</pre></td><td><pre>' + escape(json.dumps(row['actual'], indent=2)) + '</pre></td><td>' +
                escape('covers anchor: ' + str(row['evidence_covers_anchor']) + '; exact: ' +
                       str(row['evidence_exact_anchor'])) + '<pre>' + escape(row['evidence_spans']) + '</pre></td></tr>')
        pieces.append('</table><details><summary>Relationships, extra/missing facts and explicit gaps</summary><pre>' +
                      escape(json.dumps({k: case[k] for k in ['entities', 'relationships', 'relationship_rows', 'coverage', 'hard_negatives']}, indent=2)) + '</pre></details>')
    return ''.join(pieces) + '</html>\n'


def run(args):
    oracle = json.loads(args.oracle.read_text(encoding='utf-8'))
    validate_oracle(oracle)
    observation = json.loads((args.observation / 'report.json').read_text(encoding='utf-8'))
    if observation.get('status') != 'candidate_review_required' or not observation.get('binary_unchanged'):
        raise ValueError('quality scoring requires a complete deterministic observation')
    entries = {e['id']: e for e in observation['entries']}
    locations = json.loads(args.repositories.read_text(encoding='utf-8'))
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    home = output / 'home'
    home.mkdir()
    results = []
    for case in oracle['cases']:
        entry = entries[case['id']]
        if entry['revision'] != case['revision'] or entry['tree'] != case['tree']:
            raise ValueError('oracle and observation source identities differ')
        sources = {f['path']: public.run_git(Path(locations[case['id']]),
                    ['cat-file', 'blob', case['revision'] + ':' + f['path']], home) for f in case['files']}
        sample = entry['samples'][0]
        if sample['outcome'] == 'success':
            path = args.observation / (case['id'] + '-1') / 'stdout.json'
            if public.sha256_file(path) != sample['stdout_sha256']:
                raise ValueError('snapshot bytes changed since observation')
            snapshot = public.load_json(path, maximum_bytes=public.SNAPSHOT_BYTES_MAX)
            repository = snapshot['semantic']['repository']
            if (repository['commit_oid'] != case['revision'] or repository['tree_oid'] != case['tree']
                    or repository['identity'] != case['repository_id']):
                raise ValueError('snapshot repository provenance differs from the oracle')
            graph = snapshot['semantic']['knowledge_graph']
        else:
            graph = {k: [] for k in ('entities', 'relationships', 'evidence', 'claims', 'coverage')}
        results.append(score_case(case, graph, sources))
    report = {'schema_version': 'codenoesis.ontology-quality-report/v1',
              'review_status': oracle['review_status'], 'corpus_exposure': oracle['corpus_exposure'],
              'metric_interpretation': 'candidate source-oracle agreement; accuracy not certified',
              'independent_accuracy': None, 'product_commit': observation['build']['product_commit'],
              'binary_sha256': observation['build']['binary_sha256'],
              'observation_sha256': public.sha256_file(args.observation / 'report.json'),
              'oracle_sha256': public.sha256_file(args.oracle),
              'scorer_sha256': public.sha256_file(Path(__file__)), 'cases': results}
    report['summary'] = {family: {k: sum(c[family][k] for c in results) for k in ('tp', 'fp', 'fn')}
                         for family in ('entities', 'relationships')}
    report['summary']['evidence'] = {k: sum(c['evidence'][k] for c in results)
                                    for k in ('expected_facts', 'covers_anchor', 'exact_anchor')}
    report['status'] = 'candidate_agreement' if all(
        c['entities']['fp'] == c['entities']['fn'] == c['relationships']['fp'] == c['relationships']['fn'] == 0
        and c['evidence']['covers_anchor'] == c['evidence']['expected_facts']
        and all(n['absent'] for n in c['hard_negatives']) for c in results) else 'quality_gaps_found'
    write_json(output / 'report.json', report)
    (output / 'review.html').write_text(render_html(report), encoding='utf-8')
    print(json.dumps(report['summary'], indent=2))
    return 0 if report['status'] == 'candidate_agreement' else 2


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ORACLE)
    parser.add_argument('--observation', type=Path, required=True)
    parser.add_argument('--repositories', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    try:
        return run(parser.parse_args())
    except (ValueError, KeyError, TypeError, OSError, public.EvaluationError) as error:
        print(json.dumps({'error': 'quality.invalid_evidence', 'detail': str(error)}))
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
