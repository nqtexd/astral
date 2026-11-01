from pathlib import Path
import copy,hashlib,importlib.util,json,tempfile,unittest
source=Path(__file__).with_name('check-astral-release.py')
spec=importlib.util.spec_from_file_location('gate',source);gate=importlib.util.module_from_spec(spec);spec.loader.exec_module(gate)

class ReleaseGateTest(unittest.TestCase):
    def test_actual_manifest_never_calls_this_a_final_release(self):
        manifest=json.loads((gate.ROOT/'platforms/release/readiness.json').read_text(encoding='utf-8'))
        self.assertFalse(gate.inspect(manifest)['final_release_ready'])

    def test_claimed_platform_without_matching_physical_and_signed_evidence_fails(self):
        with tempfile.TemporaryDirectory()as directory:
            root=Path(directory);(root/'feature.rs').write_text('proof')
            manifest={'product':'Test','version':'1.0.0','inventory_complete':True,'features':[{'id':'draw','status':'implemented','evidence':'feature.rs'}],
                      'platforms':[{'id':'iphone','status':'verified','evidence':'report.json'}],'publication':{'rights':True}}
            (root/'app.ipa').write_bytes(b'test')
            report={'version':'1.0.0','platform':'iphone','passed':True,'physical_device_tested':False,'production_signed':True,'artifact':'app.ipa','sha256':hashlib.sha256(b'test').hexdigest()}
            (root/'report.json').write_text(json.dumps(report));self.assertFalse(gate.inspect(manifest,root)['final_release_ready'])
            report['physical_device_tested']=True;(root/'report.json').write_text(json.dumps(report));self.assertTrue(gate.inspect(manifest,root)['final_release_ready'])
            (root/'app.ipa').write_bytes(b'changed');self.assertFalse(gate.inspect(manifest,root)['final_release_ready'])

if __name__=='__main__':unittest.main()
