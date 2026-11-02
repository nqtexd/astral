import hashlib,importlib.util,json,tempfile,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('astral_readiness',ROOT/'scripts/check-astral-readiness.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
class ReleaseGateTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup);self.report=Path(self.temp.name)/'report.json';self.report.write_text('{"passed":true}',encoding='utf-8');self.gates={'version':'0.15.0','gates':[{'id':'data_integrity','requirement':'Save/reopen'},{'id':'memory_8h','requirement':'Eight hours'}]}
    def evidence(self):
        entry={'status':'passed','binary_sha256':'exact-binary','report':str(self.report),'report_sha256':hashlib.sha256(self.report.read_bytes()).hexdigest()};return {'version':'0.15.0','gates':{'data_integrity':dict(entry),'memory_8h':{**entry,'engine_seconds':28800,'desktop_seconds':28800}}}
    def test_no_evidence_never_certifies(self):self.assertFalse(module.evaluate(self.gates,{},'exact-binary')['production_ready'])
    def test_evidence_requires_exact_binary_and_report(self):
        evidence=self.evidence();self.assertTrue(module.evaluate(self.gates,evidence,'exact-binary')['production_ready']);self.assertFalse(module.evaluate(self.gates,evidence,'another-binary')['production_ready']);self.report.write_text('changed',encoding='utf-8');self.assertFalse(module.evaluate(self.gates,evidence,'exact-binary')['production_ready'])
    def test_short_engine_or_missing_desktop_stress_is_not_eight_hours(self):
        evidence=self.evidence();evidence['gates']['memory_8h']['engine_seconds']=180;self.assertFalse(module.evaluate(self.gates,evidence,'exact-binary')['production_ready']);evidence['gates']['memory_8h']['engine_seconds']=28800;evidence['gates']['memory_8h']['desktop_seconds']=0;self.assertFalse(module.evaluate(self.gates,evidence,'exact-binary')['production_ready'])
    def test_old_version_or_missing_report_does_not_pass(self):
        evidence=self.evidence();evidence['version']='0.14.0';self.assertFalse(module.evaluate(self.gates,evidence,'exact-binary')['production_ready']);evidence['version']='0.15.0';self.report.unlink();self.assertFalse(module.evaluate(self.gates,evidence,'exact-binary')['production_ready'])
if __name__=='__main__':unittest.main()
