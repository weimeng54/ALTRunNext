"""Conservative evidence gate; no default absolute millisecond or RAM budget.

This compares observed distributions, not a statistical claim of equivalence.
A pass still requires review of the raw, interleaved A/B traces and calibration.
"""
import argparse
import json
import math
from pathlib import Path

CONDITIONS=('warm','after_idle','background_load','ime','long_run')
METRICS=('wake_presented','input_presented','selection_dispatch')

def finite_number(value):
    return type(value) in (int,float) and math.isfinite(value)

def reject_json_constant(value):
    raise ValueError('Nonstandard numeric JSON constant: '+value)

def percentile(values, p):
    ordered=sorted(values)
    return ordered[max(0,math.ceil(len(ordered)*p)-1)]

def evaluate(baseline, candidate):
    missing=[];regressions=[];comparisons=[]
    for field in ('machine','dataset_sha256','refresh_hz'):
        if not baseline.get(field) or baseline.get(field)!=candidate.get(field):
            missing.append('mismatched '+field)
    for data in (baseline,candidate):
        if data.get('measurement_kind')!='presented-pixels':
            missing.append('no presented-pixel measurement')
        if not finite_number(data.get('duration_seconds')) or data['duration_seconds']<86400:
            missing.append('24-hour observation missing')
        if not finite_number(data.get('calibration_error_ns')) or data['calibration_error_ns']<0:
            missing.append('calibration missing')
    if missing:return {'decision':'pending','reasons':sorted(set(missing)),'comparisons':[]}
    error=max(baseline['calibration_error_ns'],candidate['calibration_error_ns'])
    for condition in CONDITIONS:
        for metric in METRICS:
            a=baseline.get('cases',{}).get(condition,{}).get(metric,[])
            b=candidate.get('cases',{}).get(condition,{}).get(metric,[])
            if min(len(a),len(b))<1000:
                missing.append(condition+'/'+metric+': need 1000 samples each');continue
            if not all(finite_number(x) and x>=0 for x in [*a,*b]):
                missing.append(condition+'/'+metric+': invalid duration');continue
            row={'condition':condition,'metric':metric,'baseline':{},'candidate':{}}
            for label,p in [('p50',.5),('p95',.95),('p99',.99),('max',1)]:
                av=percentile(a,p);bv=percentile(b,p)
                row['baseline'][label]=av;row['candidate'][label]=bv
                if bv>av+error:regressions.append(condition+'/'+metric+'/'+label)
            comparisons.append(row)
    for metric in ('private_bytes_peak','working_set_bytes_peak','idle_cpu_mean','growth_bytes','handle_growth','thread_growth','gdi_growth','user_growth'):
        a=baseline.get('resources',{}).get(metric);b=candidate.get('resources',{}).get(metric)
        if not all(finite_number(x) and x>=0 for x in (a,b)):
            missing.append('missing resource '+metric)
        elif b>a:
            regressions.append('resources/'+metric)
    decision='fail' if regressions else 'pending' if missing else 'pass'
    return {'decision':decision,'reasons':regressions+missing,'comparisons':comparisons,
            'limitation':'Observed distributions require raw-trace and calibration review; not proof of statistical equivalence.'}

def main():
    p=argparse.ArgumentParser();p.add_argument('baseline',type=Path);p.add_argument('candidate',type=Path);p.add_argument('--output',required=True,type=Path)
    args=p.parse_args()
    result=evaluate(json.loads(args.baseline.read_text(encoding='utf-8'),parse_constant=reject_json_constant),json.loads(args.candidate.read_text(encoding='utf-8'),parse_constant=reject_json_constant))
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(result,indent=2),encoding='utf-8')
    print(json.dumps({'decision':result['decision'],'reasons':result['reasons']}))
    raise SystemExit(0 if result['decision']=='pass' else 2 if result['decision']=='pending' else 1)

if __name__=='__main__':main()
