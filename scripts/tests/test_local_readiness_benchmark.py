import copy
import json
from pathlib import Path
import sys
import unittest
import tempfile
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import run_local_readiness_benchmark as runner


class LocalReadinessBenchmarkTests(unittest.TestCase):
    def setUp(self):
        self.corpus = json.loads((ROOT / 'benchmarks/corpora/local-readiness-v1.json').read_text())

    def test_nfr_per_001_pinned_twenty_case_protocol(self):
        runner.validate_corpus(self.corpus)
        self.assertEqual(len(self.corpus['entries']), 20)
        self.assertEqual(sum(e['cohort'] == 'boundary' for e in self.corpus['entries']), 3)

    def test_reject_duplicate_unpinned_or_mutating_arguments(self):
        for mutation in ('duplicate', 'revision', 'command', 'repetitions'):
            value = copy.deepcopy(self.corpus)
            if mutation == 'duplicate': value['entries'].append(value['entries'][0])
            if mutation == 'revision': value['entries'][0]['revision'] = 'main'
            if mutation == 'command': value['entries'][0]['scan_arguments'] += ['--store', '/tmp/reused']
            if mutation == 'repetitions': value['repetitions'] = 0
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                runner.validate_corpus(value)

    def test_complete_r16_and_jvm_commands_keep_distinct_identity_flags(self):
        for e in self.corpus['entries']:
            c = runner.command_for(Path('/binary'), Path('/repo'), Path('/fresh'), e)
            self.assertIn('--repository-id' if e['language'] == 'rust' else '--repository-identity', c)
            self.assertEqual(c[c.index('--revision') + 1], e['revision'])
            self.assertEqual(c[c.index('--store') + 1], '/fresh')
            if e['language'] == 'rust': self.assertIn('rust-safe-constant-evaluation-v1', c)
            else:
                # S8 adapters always emit JSON and intentionally reject a format selector.
                self.assertNotIn('--format', c)

    def test_typed_rejection_is_repeatable_but_not_extraction_success(self):
        samples = [dict(outcome='typed_rejection', wall_time_ns=n,
                        error_stderr_sha256='a' * 64) for n in (1, 2, 3)]
        s = runner.summarize(samples, 3)
        self.assertEqual(s['extraction_successes'], 0)
        self.assertTrue(s['deterministic'])
        self.assertEqual(s['p95_wall_time_ns'], 3)

    def test_nondeterminism_timeout_and_missing_samples_cannot_pass(self):
        good = dict(outcome='success', semantic_projection_sha256='a' * 64,
                    semantic_hash='b' * 64, wall_time_ns=10)
        for samples in ([good, good], [good, good, dict(good, semantic_hash='c' * 64)],
                        [good, good, dict(outcome='timeout', wall_time_ns=300)]):
            self.assertFalse(runner.summarize(samples, 3)['deterministic'])

    def test_sample_timeout_and_internal_failure_retain_raw_streams(self):
        for timeout in (False, True):
            with self.subTest(timeout=timeout), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                def fail(command, out, err, *args):
                    err.write(b'{"code":"internal.failure"}\n')
                    if timeout:
                        raise runner.public.EvaluationError('evaluation.timeout', 'timed out')
                    return 99, 123
                with mock.patch.object(runner.public, 'wait_for_process', side_effect=fail):
                    result = runner.measure(root/'binary', self.corpus['entries'][0], root/'repo',
                                            root/'sample', 1, root)
                self.assertEqual(result['outcome'], 'timeout' if timeout else 'protocol_failure')
                self.assertTrue((root/'sample/stderr.json').exists())
                self.assertEqual(len(result['stderr_sha256']), 64)
                self.assertTrue((root/'sample/sample.json').exists())


if __name__ == '__main__':
    unittest.main()
