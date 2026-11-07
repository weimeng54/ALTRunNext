import copy
import sys
import unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
from performance_gate import evaluate

def complete():
    return {'machine':'same-local-machine','dataset_sha256':'same-data','refresh_hz':60,'measurement_kind':'presented-pixels',
            'calibration_error_ns':1000,'duration_seconds':86400,
            'cases':{condition:{metric:[1_000_000]*1000 for metric in ('wake_presented','input_presented','selection_dispatch')} for condition in ('warm','after_idle','background_load','ime','long_run')},
            'resources':{'private_bytes_peak':16_000_000,'idle_cpu_mean':0.01,'growth_bytes':0}}

class PerformanceGateTests(unittest.TestCase):
    def test_improvement_with_complete_evidence_passes(self):
        base=complete();new=copy.deepcopy(base)
        new['cases']['warm']['wake_presented']=[900_000]*1000
        self.assertEqual(evaluate(base,new)['decision'],'pass')

    def test_p99_regression_fails_even_if_median_is_fast(self):
        base=complete();new=copy.deepcopy(base)
        new['cases']['warm']['input_presented']=[900_000]*980+[9_000_000]*20
        self.assertEqual(evaluate(base,new)['decision'],'fail')

    def test_missing_idle_ime_and_long_run_cannot_claim_pass(self):
        base=complete();new=copy.deepcopy(base)
        del new['cases']['ime'];del new['cases']['after_idle']
        new['duration_seconds']=30
        self.assertEqual(evaluate(base,new)['decision'],'pending')

    def test_message_completion_cannot_impersonate_presented_pixels(self):
        base=complete();new=copy.deepcopy(base);new['measurement_kind']='message-complete'
        self.assertEqual(evaluate(base,new)['decision'],'pending')

    def test_different_machine_or_dataset_cannot_pass(self):
        base=complete();new=copy.deepcopy(base);new['dataset_sha256']='different'
        self.assertEqual(evaluate(base,new)['decision'],'pending')

    def test_resource_regression_fails(self):
        base=complete();new=copy.deepcopy(base);new['resources']['private_bytes_peak']*=2
        self.assertEqual(evaluate(base,new)['decision'],'fail')

if __name__=='__main__':unittest.main()
