#!/bin/zsh
# LOO-867 rental hand-back walkthrough: a private Machine rents a "sandbox"
# whose provider is a stand-in `uv` bridge and whose SSH is a stand-in `ssh`
# running commands against a second private home (the rented Machine).
set -u
LF=${LF:-/tmp/lf-qa-867-bin}
R=${R:-/tmp/lf-qa-rent}
rm -rf $R; mkdir -p $R/home/.ssh $R/lfhome $R/stand $R/rhome $R/rlf $R/rbin
for v in $(env | grep -E '^(LF_|REX_)' | cut -d= -f1); do unset $v; done
ln -s $LF $R/rbin/lf
cat > $R/stand/uv <<UV
#!/bin/sh
# Stand-in provider bridge: the op is the last argument; one state file per sandbox id.
for a; do op=\$a; done
req=\$(cat)
id=\$(printf %s "\$req" | python3 -I -c 'import json,sys; print(json.load(sys.stdin).get("id") or "")')
echo "uv \$op \$id" >> $R/calls.log
case \$op in
  create) n=\$(( \$(cat $R/sandbox.count 2>/dev/null || echo 0) + 1 )); echo \$n > $R/sandbox.count; echo running > $R/sb-qa-\$n
     echo "{\"id\":\"sb-qa-\$n\",\"gone\":false,\"route\":\"qa@sandbox\$n.invalid\",\"usd_per_hour\":null}";;
  get) if [ -n "\$id" ] && [ "\$(cat $R/\$id 2>/dev/null)" = running ]; then echo "{\"id\":\"\$id\",\"gone\":false,\"route\":null,\"usd_per_hour\":null}"; else echo null; fi;;
  delete) echo gone > $R/\$id; echo null;;
esac
UV
cat > $R/stand/ssh <<SSH
#!/bin/sh
# Stand-in ssh: the last argument is the remote command; run it as the rented
# Machine (its own HOME and LF_HOME). Options and the destination are ignored.
n=1; for a; do cmd=\$a; case "\$a" in *@sandbox*) n=\$(printf %s "\$a" | sed 's/.*sandbox\([0-9]*\).*/\1/');; esac; done
mkdir -p $R/rhome\$n $R/rlf\$n
echo "ssh[\$n] \$(printf %s "\$cmd" | head -c 120 | tr '\n' ' ')" >> $R/calls.log
case "\$cmd" in
  *install.sh*) exit 0;;
  *pgrep*) printf '0\n0\n'; exit 0;;
esac
exec env -u LF_BIN HOME=$R/rhome\$n LF_HOME=$R/rlf\$n PATH=$R/rbin:/usr/bin:/bin sh -c "\$cmd"
SSH
cat > $R/stand/ssh-keygen <<'KG'
#!/bin/sh
case "$1" in -R) exit 0;; esac
exec /usr/bin/ssh-keygen "$@"
KG
chmod +x $R/stand/*
/usr/bin/ssh-keygen -q -t ed25519 -N '' -f $R/home/.ssh/id_ed25519
export HOME=$R/home LF_HOME=$R/lfhome LF_BIN=$LF PATH=$R/stand:$PATH DAYTONA_API_KEY=qa-placeholder
export CODEX_HOME=$HOME/.codex CLAUDE_CONFIG_DIR=$HOME/.claude
git init -q --bare $R/origin.git
git init -q -b main $R/repo && cd $R/repo
git config user.email qa@example.com; git config user.name 'QA Person'
mkdir -p .lf; printf 'default_agent: codex\nplanning: {provider: git, personal: true}\n' > .lf/config.yaml
git add -A; git commit -qm init; git remote add origin $R/origin.git; git push -q origin main
