import copy
import hashlib
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
from score_ontology_quality import compare_facts, render_html, score_case, validate_oracle


class OntologyQualityTests(unittest.TestCase):
    def test_extra_missing_duplicate_and_wrong_property_are_counted(self):
        expected = [{'kind': 'function', 'name': 'a'}, {'kind': 'function', 'name': 'b'}]
        actual = [expected[0], expected[0], {'kind': 'function', 'name': 'c'}]
        score = compare_facts(expected, actual)
        self.assertEqual((score['tp'], score['fp'], score['fn']), (1, 2, 1))
        self.assertEqual(score['precision'], 1/3)
        self.assertEqual(score['recall'], 1/2)
        self.assertIsNone(compare_facts([], [])['precision'])
        self.assertEqual(compare_facts([{'type': 'u8'}], [{'type': 'u16'}])['tp'], 0)

    def fixture(self):
        source = b'class A {}'
        fact = dict(path='src/A.java', kind='JavaClass', name='A', owner='')
        case = {'id': 'sample', 'language': 'java', 'files': [{'path': 'src/A.java',
            'sha256': hashlib.sha256(source).hexdigest()}], 'kinds': ['JavaClass'],
            'property_keys': {}, 'public_rust_surface': False,
            'facts': [{'fact': fact, 'anchor': 'class A', 'start_byte': 0, 'end_byte': 7}],
            'relationships': [], 'relationship_kinds': ['CONTAINS_DECLARATION'],
            'negatives': [dict(fact, name='Ghost')]}
        blob = hashlib.sha1(b'blob 10\0' + source).hexdigest()
        graph = {'entities': [{'id': 'a', 'kind': 'JavaClass', 'name': 'A',
                              'properties': {'path': 'src/A.java', 'owner': ''}}],
            'claims': [{'subject_id': 'a', 'evidence_ids': ['e']}],
            'evidence': [{'id': 'e', 'path': 'src/A.java', 'blob_oid': blob,
                          'start_byte': 0, 'end_byte': 10}], 'relationships': [], 'coverage': []}
        return case, graph, {'src/A.java': source}

    def test_evidence_granularity_and_provenance_are_separate_from_fact_match(self):
        case, graph, sources = self.fixture()
        score = score_case(case, graph, sources)
        self.assertEqual(score['entities']['tp'], 1)
        self.assertEqual(score['evidence']['covers_anchor'], 1)
        self.assertEqual(score['evidence']['exact_anchor'], 0)
        graph['evidence'][0]['blob_oid'] = '0' * 40
        self.assertEqual(score_case(case, graph, sources)['evidence']['covers_anchor'], 0)

    def test_extraction_failure_retains_all_expected_facts_in_denominator(self):
        case, graph, sources = self.fixture()
        graph['entities'] = []
        score = score_case(case, graph, sources)
        self.assertEqual((score['entities']['tp'], score['entities']['fn']), (0, 1))

    def test_source_tampering_cannot_be_scored(self):
        case, graph, sources = self.fixture()
        with self.assertRaises(ValueError):
            score_case(case, graph, {'src/A.java': b'class B {}'})

    def test_boolean_properties_are_not_equal_to_integer_properties(self):
        case, graph, sources = self.fixture()
        case['property_keys'] = {'JavaClass': ['flag']}
        case['facts'][0]['fact']['flag'] = True
        graph['entities'][0]['properties']['flag'] = 1
        result = score_case(case, graph, sources)
        self.assertEqual(result['entities']['tp'], 0)
        self.assertEqual(result['rows'][0]['matches'], 0)

    def test_review_page_escapes_untrusted_source_and_has_no_script(self):
        case, graph, sources = self.fixture()
        result = score_case(case, graph, sources)
        result['rows'][0]['anchor'] = '<script>alert("source")</script>'
        page = render_html({'product_commit': 'a' * 40, 'cases': [result]})
        self.assertIn('&lt;script&gt;', page)
        self.assertNotIn('<script>', page)
        self.assertIn('Content-Security-Policy', page)

    def test_wrong_relation_endpoint_and_unknown_state_do_not_match(self):
        expected = [dict(kind='EXPECT_ACTUAL_CANDIDATE', source='a', target='b', state='Unknown')]
        for actual in [dict(expected[0], target='c'), dict(expected[0], state='Observed')]:
            score = compare_facts(expected, [actual])
            self.assertEqual((score['tp'], score['fp'], score['fn']), (0, 1, 1))

    def test_agent_oracle_cannot_self_certify_accuracy_or_holdout(self):
        import json
        oracle = json.loads((ROOT / 'benchmarks/oracles/ontology-quality-v1.json').read_text())
        validate_oracle(oracle)
        for field, value in [('review_status', 'independently_approved'), ('corpus_exposure', 'held_out')]:
            modified = copy.deepcopy(oracle)
            modified[field] = value
            with self.assertRaises(ValueError): validate_oracle(modified)


if __name__ == '__main__':
    unittest.main()
