"""Collect hash-bound software evidence; never synthesize physical/signing/stress passes."""
import argparse,hashlib,json,re,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,required=True);parser.add_argument('--rust-log',type=Path,required=True);parser.add_argument('--output',type=Path,default=ROOT/'release/Astral-0.15.0-validation.json');parser.add_argument('--evidence',type=Path,default=ROOT/'build/production-evidence.json');args=parser.parse_args()
    version=json.loads((ROOT/'platforms/release/production-gates.json').read_text(encoding='utf-8'))['version'];binary=digest(args.binary)
    if subprocess.check_output([str(args.binary.resolve()),'--version'],text=True).strip()!=f'astral {version}':raise SystemExit('Binary version does not match gate inventory')
    acceptance_path=ROOT/'artifacts/astral-production-015/report.json';acceptance=json.loads(acceptance_path.read_text(encoding='utf-8'));mcp_path=ROOT/'artifacts/astral-smoke/mcp-report.json';mcp=json.loads(mcp_path.read_text(encoding='utf-8'))
    exports_path=ROOT/'artifacts/astral-exports/report.json';exports=json.loads(exports_path.read_text(encoding='utf-8'));provenance=json.loads((exports_path.parent/'report-provenance.json').read_text(encoding='utf-8'))
    if acceptance.get('binary_sha256')!=binary or mcp.get('binary_sha256')!=binary or provenance.get('binary_sha256')!=binary:raise SystemExit('Run every acceptance suite against this exact packaged binary first')
    if not acceptance.get('passed') or not all(acceptance['checks'].values()) or not all(mcp.get(k) for k in ['stale_revision_rejected','atomic_rollback','targeted_revision_preserved_tracks','cancel_preserved_previous_output','legacy_source_preserved']):raise SystemExit('MCP acceptance is incomplete')
    if len(exports)!=16 or not all(v.get('state')=='completed' for v in exports.values()):raise SystemExit('Export regression corpus is incomplete')
    log=args.rust_log.read_text(encoding='utf-8');required=['atomic_save_reopen_recovery_and_confinement','saves_retain_five_distinct_previous_versions_and_skip_recovery_backups','panicked_background_job_finishes_with_diagnostic_instead_of_sticking']
    if 'test result: FAILED'in log or not all(re.search(r'test (?:[A-Za-z0-9_]+::)*'+re.escape(name)+r' \.\.\. ok',log)for name in required) or 'Doc-tests astral_storage'not in log:raise SystemExit('Rust integrity/worker suite is incomplete')
    tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))
    report={'version':version,'binary_sha256':binary,'production_certified':False,'rust_suite':{'tests_passed':tests,'log_sha256':digest(args.rust_log),'required_integrity_cases':required},'mcp':{'report_sha256':digest(mcp_path),'tools':mcp['tools'],'rollback':True,'cancellation':True},'new_workflows':acceptance['checks'],'exports':{'cases':len(exports),'report_sha256':digest(exports_path),'native_avi_video_audio_verified':acceptance['checks'].get('native_avi_decoded_video_audio_sync',False)},'pending':'Physical input/playback, eight-hour engine+desktop stress, exhaustive undo matrix, full localization, signing, clean VM installation and two-clean-build comparison remain unverified.'}
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8');entry={'status':'passed','binary_sha256':binary,'report':str(args.output.resolve()),'report_sha256':digest(args.output)}
    evidence={'version':version,'gates':{name:dict(entry)for name in ['data_integrity','export_regressions','mcp_integrity']}}
    args.evidence.parent.mkdir(parents=True,exist_ok=True);args.evidence.write_text(json.dumps(evidence,indent=2)+'\n',encoding='utf-8');print(json.dumps({'report':str(args.output),'software_gates':list(evidence['gates']),'production_certified':False}))
if __name__=='__main__':main()
