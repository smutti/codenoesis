import copy
import unittest
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compare_local_readiness import compare, identity


class LocalReadinessComparisonTests(unittest.TestCase):
    def fixture(self):
        sample = {'outcome': 'success', 'semantic_hash': 'a' * 64,
                  'semantic_projection_sha256': 'b' * 64}
        entry = {'id': 'one', 'revision': 'c' * 40, 'tree': 'd' * 40,
                 'samples': [sample, sample, sample],
                 'summary': {'deterministic': True, 'extraction_successes': 3}}
        report = {'status': 'candidate_review_required', 'binary_unchanged': True,
                  'corpus_sha256': 'e' * 64, 'entries': [entry]}
        baseline = {'schema_version': 'codenoesis.local-readiness-baseline/v1',
                    'review_status': 'candidate_pending_independent_review',
                    'corpus_sha256': 'e' * 64,
                    'entries': [{'id': 'one', 'revision': 'c' * 40, 'tree': 'd' * 40,
                                 'identity': identity(sample)}]}
        return report, baseline

    def test_matching_candidate_is_not_an_accuracy_or_release_certificate(self):
        report, baseline = self.fixture()
        result = compare(report, baseline)
        self.assertEqual(result['status'], 'candidate_identity_match')
        self.assertFalse(result['ga_accepted'])

    def test_changed_or_missing_or_duplicate_entries_and_failed_samples_fail(self):
        for mutation in ['hash', 'missing', 'duplicate', 'failure', 'corpus', 'repetition']:
            report, baseline = self.fixture()
            report = copy.deepcopy(report)
            if mutation == 'hash': report['entries'][0]['samples'][0]['semantic_hash'] = 'f' * 64
            if mutation == 'missing': report['entries'] = []
            if mutation == 'duplicate': report['entries'].append(report['entries'][0])
            if mutation == 'failure': report['status'] = 'observation_failed'
            if mutation == 'corpus': report['corpus_sha256'] = 'f' * 64
            if mutation == 'repetition': report['entries'][0]['samples'].pop()
            with self.subTest(mutation=mutation):
                self.assertEqual(compare(report, baseline)['status'], 'candidate_mismatch')


if __name__ == '__main__':
    unittest.main()
