"""Build a locally test-signed Android preview; never a store/release signature."""
from pathlib import Path
import argparse, hashlib, json, os, shutil, subprocess, sys, zipfile

ROOT = Path(__file__).resolve().parents[1]
from astral_notices import notices
SDK = Path(os.environ.get('ANDROID_HOME') or os.environ.get('ANDROID_SDK_ROOT') or Path.home()/'AppData/Local/Android/Sdk')

def run(args, **kwargs):
    subprocess.run(list(map(str,args)),check=True,**kwargs)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--abi',choices=['arm64-v8a','x86_64','all'],default='all')
    parser.add_argument('--skip-rust',action='store_true')
    args=parser.parse_args()
    ndk=SDK/'ndk/28.2.13676358'
    tools=SDK/'build-tools/36.0.0'
    jar=SDK/'platforms/android-36/android.jar'
    host='windows-x86_64' if os.name=='nt' else 'darwin-x86_64' if sys.platform=='darwin' else 'linux-x86_64'
    compiler=ndk/f'toolchains/llvm/prebuilt/{host}/bin'
    work=ROOT/'build/astral-android'
    work.mkdir(parents=True,exist_ok=True)
    java_home=Path(os.environ.get('JAVA_HOME') or 'C:/Program Files/Java/jdk-26.0.1')
    suffix='.exe' if os.name=='nt' else ''
    javac=java_home/f'bin/javac{suffix}';java=java_home/f'bin/java{suffix}';keytool=java_home/f'bin/keytool{suffix}'
    for path in [compiler,tools,jar,javac,java,keytool]:
        if not path.exists():raise SystemExit(f'Missing Android build prerequisite: {path}')
    env=os.environ.copy()
    env.update(ANDROID_HOME=str(SDK),ANDROID_NDK_HOME=str(ndk))
    targets={'arm64-v8a':('aarch64-linux-android','aarch64-linux-android29'), 'x86_64':('x86_64-linux-android','x86_64-linux-android29')}
    chosen=list(targets) if args.abi=='all' else [args.abi]
    for abi in chosen:
        target,clang=targets[abi]
        env['CARGO_TARGET_'+target.replace('-','_').upper()+'_LINKER']=str(compiler/(clang+('.cmd' if os.name=='nt' else '-clang')))
        if os.name=='nt':env['CARGO_TARGET_'+target.replace('-','_').upper()+'_LINKER']=str(compiler/(clang+'-clang.cmd'))
        if not args.skip_rust:run(['cargo','build','-p','astral-mobile','--release','--lib','--target',target],cwd=ROOT,env=env)
    resource=work/'res/drawable';resource.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(ROOT/'assets/brand/astral.png',resource/'icon.png')
    compiled=work/'resources.zip'
    run([tools/f'aapt2{suffix}','compile','--dir',work/'res','-o',compiled])
    apk=work/'base.apk'
    run([tools/f'aapt2{suffix}','link','-I',jar,'--manifest',ROOT/'platforms/android/AndroidManifest.xml','-o',apk,compiled])
    classes=work/'classes';classes.mkdir(exist_ok=True)
    sources=sorted((ROOT/'platforms/android/java').rglob('*.java'))
    run([javac,'--release','8','-classpath',jar,'-d',classes,*sources])
    dex=work/'dex';dex.mkdir(exist_ok=True)
    # Invoke D8 directly rather than depending on an installed Gradle distribution.
    run([java,'-cp',tools/'lib/d8.jar','com.android.tools.r8.D8','--min-api','29','--lib',jar,'--output',dex,*sorted(classes.rglob('*.class'))])
    with zipfile.ZipFile(apk,'a',compression=zipfile.ZIP_DEFLATED) as package:
        package.write(dex/'classes.dex','classes.dex')
        for abi in chosen:
            binary=ROOT/f'target/{targets[abi][0]}/release/libastral_mobile.so'
            package.write(binary,f'lib/{abi}/libastral_mobile.so')
        package.write(ROOT/'COPYING','assets/COPYING')
        for name,data in notices(ROOT,targets[chosen[0]][0]).items():package.writestr('assets/THIRD_PARTY/'+name,data)
        package.writestr('assets/BUILD_STATUS.txt','Development preview. Not Moho feature parity. Not a final or store-signed release.\n')
    aligned=work/'aligned.apk'
    run([tools/f'zipalign{suffix}','-f','-p','4',apk,aligned])
    keystore=work/'preview-test.keystore'
    if not keystore.exists():
        run([keytool,'-genkeypair','-keystore',keystore,'-storepass','android','-keypass','android','-alias','astral-preview','-keyalg','RSA','-keysize','2048','-validity','3650','-dname','CN=Astral Local Preview, O=Astral Development, C=IN'])
    destination=ROOT/'release/preview';destination.mkdir(parents=True,exist_ok=True)
    output=destination/'Astral-0.12.0-android-preview.apk'
    run([java,'-jar',tools/'lib/apksigner.jar','sign','--ks',keystore,'--ks-pass','pass:android','--key-pass','pass:android','--out',output,aligned])
    run([java,'-jar',tools/'lib/apksigner.jar','verify','--verbose',output])
    report={'artifact':str(output.relative_to(ROOT)),'abis':chosen,'sha256':hashlib.sha256(output.read_bytes()).hexdigest(),'bytes':output.stat().st_size,'signature':'local test key; not a production identity','status':'development-preview','feature_parity':False}
    (destination/'android-preview-build.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps(report,indent=2))

if __name__=='__main__':main()
