"""These are tests of the planning tools, NOT of opencode-rk."""
import copy
import importlib.util
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
def load(name):
    spec=importlib.util.spec_from_file_location(name,ROOT/'tools'/f'{name}.py')
    module=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module
validator=load('validate_plan')
renderer=load('render_lane')

class PlanToolTests(unittest.TestCase):
    def setUp(self):
        self.board=json.loads((ROOT/'phase1-tasks.json').read_text())
    def test_draft_valid_without_product_claim(self):
        report=validator.validate(ROOT,self.board)
        self.assertTrue(report['plan_valid'],report['errors'])
        self.assertFalse(report['product_verified'])
        self.assertEqual(report['application_tests_executed_by_this_check'],0)
    def test_duplicate_id_rejected(self):
        self.board['lanes'].append(copy.deepcopy(self.board['lanes'][0]))
        report=validator.validate(ROOT,self.board)
        self.assertFalse(report['plan_valid'])
        self.assertTrue(any('duplicate' in e for e in report['errors']))
    def test_unknown_dependency_rejected(self):
        self.board['lanes'][0]['depends_on'].append('DOES-NOT-EXIST')
        self.assertFalse(validator.validate(ROOT,self.board)['plan_valid'])
    def test_dependency_cycle_rejected(self):
        self.board['lanes'][0]['depends_on'].append(self.board['lanes'][0]['id'])
        report=validator.validate(ROOT,self.board)
        self.assertTrue(any('cycle' in e for e in report['errors']))
    def test_conflicting_one_file_owners_rejected(self):
        first=next(l for l in self.board['lanes'] if l['id']=='P1-W1-02-T')
        second=next(l for l in self.board['lanes'] if l['id']=='P1-W1-03-T')
        second['owned_file']=first['owned_file']
        report=validator.validate(ROOT,self.board)
        self.assertTrue(any('ownership conflict' in e for e in report['errors']))
    def test_native_lane_packet_has_bounded_contract(self):
        packet=renderer.render(ROOT,'P1-W1-04-L01')
        self.assertIn('Owned file:',packet)
        self.assertIn('DRAFT PACKET',packet)
        self.assertIn('Acceptance',packet)
        self.assertLess(len(packet),18000)
    def test_unknown_lane_does_not_dispatch(self):
        with self.assertRaises(ValueError):renderer.render(ROOT,'NO-SUCH-LANE')

if __name__=='__main__':unittest.main()
