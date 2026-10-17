#!/usr/bin/env python3
"""Supervise paired P3b release probes; sanitize inherited diagnostic knobs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import statistics
import subprocess
import time
ROOT=Path(__file__).resolve().parents[4]
OUT=ROOT/'target/validation/performance-20261016/p3b'
RECIPES=['even-random-circles','clustered-dispersion-random-links','source-weighted-dispersion-voronoi']


def run(side,mode,source,recipe,scale,repeats=1,suffix='',env=None,allocation=False,timeout=120):
    """Run one fresh bounded process; every actual environment override and binary hash is recorded."""
    name=f'{side}-{mode}-{source}-{recipe.replace(":","-")}-f{scale}{suffix}'
    directory=OUT/name;directory.mkdir(exist_ok=False)
    binary=OUT/'binaries'/('baseline-allocation-p3b-harness' if allocation and side=='baseline' else 'candidate-allocation' if allocation else 'baseline-p3b-harness' if side=='baseline' else 'candidate')
    overrides=env or {}
    environment={k:v for k,v in os.environ.items() if not k.startswith('P3_')}
    environment.update(overrides)
    argv=['/usr/bin/time','-v','-o',str(directory/'process.time'),str(binary),mode,source,recipe,scale,str(directory),str(repeats)]
    started=time.monotonic()
    with (directory/'records.jsonl').open('w')as stdout,(directory/'stderr.log').open('w')as stderr:
        process=subprocess.Popen(argv,cwd=ROOT,env=environment,stdout=stdout,stderr=stderr,start_new_session=True)
        try:status=process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid,signal.SIGTERM)
            try:process.wait(timeout=5)
            except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait()
            status='timeout'
    invocation={'case':name,'status':status,'wall_s':time.monotonic()-started,'timeout_s':timeout,'argv':argv,'environment_overrides':overrides,'inherited_P3_variables_removed':True,'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest()}
    records=[json.loads(line) for line in (directory/'records.jsonl').read_text().splitlines()]
    families=[r['us'] for r in records if r['kind']=='family']
    if families:invocation['family_median_us']=statistics.median(families)
    cold=[r['api_us'] for r in records if r['kind']=='engine' and r['label']=='cold']
    if cold:invocation['cold_api_us']=cold[0]
    (directory/'invocation.json').write_text(json.dumps(invocation,indent=2));print(json.dumps(invocation),flush=True)
    if status!=0:raise RuntimeError(f'failed case {name}: {status}')


def main():
    """Select one serialized bounded partition; compilation and review remain outside measurement."""
    parser=argparse.ArgumentParser();parser.add_argument('batch',choices=['family','engine','allocation','cancel','edits','project']);args=parser.parse_args()
    group=0
    if args.batch=='family':
        for recipe in RECIPES:
            for source in ['png','svg']:
                for scale in ['1','025']:
                    for side in (['baseline','candidate'] if group%2==0 else ['candidate','baseline']):run(side,'family',source,recipe,scale,3)
                    group+=1
        for control in ['raw','stratified','raw-exclusion','clustered-exclusion','raw-lloyd','raw-lloyd-weighted']:
            for source in (['png','svg'] if 'weighted'in control else ['png']):
                for side in ['baseline','candidate']:run(side,'family',source,'control:'+control,'1',3)
        for seed in ['0','123456789']:
            for recipe in RECIPES:
                for side in ['baseline','candidate']:run(side,'family','png',recipe,'1',2,'-seed'+seed,{'P3_SEED':seed})
    elif args.batch=='engine':
        for recipe in RECIPES:
            for source in ['png','svg']:
                for scale in ['1','025']:
                    for trial in range(3 if recipe==RECIPES[0] and scale=='025' else 1):
                        for side in (['baseline','candidate'] if trial%2==0 else ['candidate','baseline']):
                            run(side,'engine',source,recipe,scale,1,f'-trial{trial}',{'P3_SKIP_SVG':'1'} if recipe==RECIPES[1] or trial else {})
    elif args.batch=='allocation':
        for recipe in RECIPES:
            for source in ['png','svg']:
                for side in ['baseline','candidate']:run(side,'family',source,recipe,'025',1,'-allocation',allocation=True)
    elif args.batch=='cancel':
        for recipe in RECIPES:
            for side in ['baseline','candidate']:run(side,'family','png',recipe,'025',1,'-limit',{'P3_LIMIT':'100'})
            for delay in ['0','10','50']:
                for side in ['baseline','candidate']:run(side,'family','png',recipe,'025',1,'-deadline'+delay,{'P3_CANCEL_MS':delay})
        for source in ['png','svg']:
            for delay in ['0','50','250']:
                for side in ['baseline','candidate']:run(side,'engine-cancel',source,RECIPES[0],'025',1,'-signal'+delay,{'P3_CANCEL_MS':delay})
    elif args.batch=='edits':
        for recipe in [RECIPES[0],RECIPES[2]]:
            for side in ['baseline','candidate']:run(side,'engine','png',recipe,'1',1,'-edits',{'P3_SKIP_SVG':'1','P3_EDITS':'1'})
    elif args.batch=='project':
        for source in ['png','svg']:run('candidate','project',source,RECIPES[0],'025')

if __name__=='__main__':main()
