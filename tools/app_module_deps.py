# Lists raygun-app's top-level modules: their size, which modules use them and which they use
# (crate:: paths only; app.state::<T>() lookups don't show). Run from crates/raygun-app/src;
# writes app-deps.txt there. Used for M46.56 (docs/phases/phase-6/M46.56-split-raygun-app.md).
import os,re,collections
mods={}
for root,_,files in os.walk('.'):
    for f in files:
        if not f.endswith('.rs'): continue
        p=os.path.join(root,f)[2:].replace(os.sep,'/')
        top=p.split('/')[0]
        if top.endswith('.rs'): top=top[:-3]
        mods.setdefault(top,[]).append(p)
lines={m:sum(len(open(p,encoding='utf-8').read().splitlines()) for p in ps) for m,ps in mods.items()}
deps=collections.defaultdict(set)
for m,ps in mods.items():
    for p in ps:
        s=open(p,encoding='utf-8').read()
        for x in set(re.findall(r'crate::([a-z_0-9]+)',s)):
            if x!=m and x in mods: deps[m].add(x)
rdeps=collections.defaultdict(set)
for m,d in deps.items():
    for x in d: rdeps[x].add(m)
print("most depended-on:")
for m in sorted(mods,key=lambda m:-len(rdeps[m]))[:25]:
    print(f"  {m:22} {lines[m]:6} lines, used by {len(rdeps[m]):3}, uses {len(deps[m])}")
print("big modules few depend on:")
for m in sorted(mods,key=lambda m:-lines[m]):
    if len(rdeps[m])<=3 and lines[m]>700: print(f"  {m:22} {lines[m]:6} used by {sorted(rdeps[m]-{'commands','lib','startup'})} uses {len(deps[m])}")
open('app-deps.txt','w').write("\n".join(f"{m} ({lines[m]}): {' '.join(sorted(deps[m]))}" for m in sorted(mods)))
