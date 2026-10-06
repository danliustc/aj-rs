"""Differential test: compare aj-rs against the original Python autojump.

Usage:
    git clone https://github.com/wting/autojump /tmp/autojump
    cargo build --release
    python3 tools/difftest.py SEED /tmp/autojump/bin/autojump target/release/autojump

Builds a random directory tree + database, runs random queries through both
implementations and prints any differences. Expected (deliberate) differences:
upstream crashes (empty output) on `foo__N` with no matches, and `foo__N`
indices can differ because upstream's candidate list contains duplicates.
"""
import os, random, subprocess, sys, tempfile, shutil
random.seed(int(sys.argv[1]) if len(sys.argv) > 1 else 0)
ORIG = sys.argv[2]; RUST = sys.argv[3]
words = ["foo","bar","baz","Projects","src","docs","Documents","music","Music","rust","aj-rs","my.app","test_data","中文目录","work","tmp2","a","ab","abc","node_modules","Downloads","download"]
root = tempfile.mkdtemp()
paths = []
for _ in range(120):
    depth = random.randint(1, 4)
    p = os.path.join(root, *random.choices(words, k=depth))
    os.makedirs(p, exist_ok=True)
    paths.append(p)
paths = sorted(set(paths))
missing = [os.path.join(root, "gone", w) for w in random.sample(words, 5)]
data_home = tempfile.mkdtemp()
os.makedirs(os.path.join(data_home, "autojump"))
weights = [random.choice([10.0, 14.142135623730951, 20.0, 5.0]) if random.random() < 0.4 else round(random.uniform(1, 100), 3) for _ in paths + missing]
with open(os.path.join(data_home, "autojump", "autojump.txt"), "w") as f:
    for p, w in zip(paths + missing, weights):
        f.write("%s\t%s\n" % (w, p))
env = dict(os.environ, XDG_DATA_HOME=data_home, AUTOJUMP_SOURCED="1", HOME=root)
env.pop("AUTOJUMP_DATA_DIR", None)

def typo(w):
    if len(w) < 3: return w
    i = random.randrange(len(w)); return w[:i] + w[i+1:]
def query():
    k = random.choice([1, 1, 1, 2, 3])
    qs = []
    for _ in range(k):
        w = random.choice(words)
        r = random.random()
        if r < 0.3: w = w[:random.randint(1, len(w))]
        elif r < 0.5: w = typo(w)
        elif r < 0.6: w = w.upper()
        elif r < 0.7: w = w[random.randint(0, len(w)-1):]
        qs.append(w)
    return qs

def run(cmd, args, cwd):
    return subprocess.run(cmd + args, cwd=cwd, env=dict(env, PWD=cwd), capture_output=True, text=True).stdout

fails = 0; n = 0
for _ in range(400):
    cwd = random.choice(paths + [root])
    q = query()
    if random.random() < 0.15:
        q = [q[0] + "__" + random.choice(["", "1", "2", "3"])]
    a = run([sys.executable, ORIG], q, cwd); b = run([RUST], q, cwd); n += 1
    if a != b:
        fails += 1; print("JUMP", q, "cwd", cwd, "\n  py:", a.strip(), "\n  rs:", b.strip())
    if "__" in q[0]: continue
    a = run([sys.executable, ORIG], ["--complete"] + q[:1], cwd)
    b = run([RUST], ["--complete"] + q[:1], cwd); n += 1
    # upstream doesn't dedupe; compare unique paths in order
    def uniq(out):
        seen = []
        for line in out.splitlines():
            p = line.split("__", 2)[2]
            if p not in seen: seen.append(p)
        return seen
    ua, ub = uniq(a), uniq(b)
    if ub[:len(ua)] != ua:
        fails += 1; print("COMPLETE", q[:1], "cwd", cwd, "\n  py:", ua, "\n  rs:", ub)
print("cases:", n, "failures:", fails)
shutil.rmtree(root); shutil.rmtree(data_home)
