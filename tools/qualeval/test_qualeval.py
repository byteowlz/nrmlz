"""Metric contract tests using synthetic text only; no benchmark data or network needed."""
import unittest
import subprocess
import sys
from pathlib import Path
from qualeval import canon, distance, report, tokens


class Metrics(unittest.TestCase):
    def test_judge_requires_explicit_upload_acknowledgment(self):
        script = Path(__file__).with_name('qualeval.py')
        result = subprocess.run([sys.executable, '-I', str(script), '--judge'], capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertIn('--allow-judge-upload required', result.stderr)

    def test_edit_distance(self):
        self.assertEqual(distance(['one', 'two'], ['one', 'three']), 1)
        self.assertEqual(distance([], ['one']), 1)
        self.assertEqual(distance(['one'], []), 1)
        self.assertEqual(distance(['one', 'two'], ['one', 'two']), 0)

    def test_tokens_do_not_change_semantics(self):
        self.assertEqual(tokens('Twenty-one, euros.'), ['twenty', 'one', 'euros'])
        self.assertNotEqual(canon('one hundred'), canon('one thousand'))
        self.assertEqual(canon('drei Komma, fünf'), canon('drei Komma fünf'))

    def test_micro_wer_and_unjudged(self):
        items = [{'id': 'a', 'ref': 'one two', 'text': 'A', 'split': 'test'},
                 {'id': 'b', 'ref': 'one', 'text': 'B', 'split': 'dev'}]
        _, scores = report('en', items, {'fixture': {'a': 'one three', 'b': 'one'}},
                           lambda i, c: 'correct' if c == i['ref'] else 'unjudged', {})
        self.assertAlmostEqual(scores['fixture/all']['wer'], 1 / 3)
        self.assertEqual(scores['fixture/test']['unjudged'], 1)
        self.assertEqual(scores['fixture/all']['exact'], 1)


if __name__ == '__main__':
    unittest.main()
