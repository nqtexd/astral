"""Build an unsigned iPhone/iPad simulator preview on a Mac with Xcode/XcodeGen.

This does not archive an IPA, sign a device build or submit to an app store.
The required-reason privacy API inventory remains a release gate.
"""
from pathlib import Path
import json,platform,subprocess

ROOT=Path(__file__).resolve().parents[1]
def run(*args,**kwargs):subprocess.run(list(map(str,args)),check=True,**kwargs)
def main():
    if platform.system()!='Darwin':raise SystemExit('Apple builds require a Mac/Xcode runner. No Apple artifact was built on this host.')
    run('xcodebuild','-version')
    run('xcodegen','--version')
    simulator='aarch64-apple-ios-sim' if platform.machine()=='arm64' else 'x86_64-apple-ios'
    targets=['aarch64-apple-ios',simulator]
    for target in targets:
        run('rustup','target','add',target)
        run('cargo','build','-p','astral-mobile','--release','--lib','--target',target,'--locked',cwd=ROOT)
    out=ROOT/'build/apple';out.mkdir(parents=True,exist_ok=True)
    framework=out/'AstralEngine.xcframework'
    # xcodebuild refuses to overwrite existing XCFrameworks. Keep previous builds
    # under unique names instead of deleting a computed directory recursively.
    if framework.exists():
        import time
        framework.rename(out/f'AstralEngine-previous-{time.time_ns()}.xcframework')
    command=['xcodebuild','-create-xcframework']
    for target in targets:command+=['-library',str(ROOT/f'target/{target}/release/libastral_mobile.a'),'-headers',str(ROOT/'platforms/apple/include')]
    run(*command,'-output',framework)
    run('xcodegen','generate','--spec','project.yml',cwd=ROOT/'platforms/apple')
    run('xcodebuild','-project',ROOT/'platforms/apple/AstralPreview.xcodeproj','-scheme','AstralPreview','-sdk','iphonesimulator','-configuration','Debug','-destination','generic/platform=iOS Simulator','-derivedDataPath',out/'DerivedData','CODE_SIGNING_ALLOWED=NO','build')
    report={'status':'unsigned-simulator-build','physical_device_tested':False,'store_signed':False,'feature_parity':False,'simulator_target':simulator,'app':'build/apple/DerivedData/Build/Products/Debug-iphonesimulator/AstralPreview.app'}
    (out/'build-report.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps(report,indent=2))
if __name__=='__main__':main()
