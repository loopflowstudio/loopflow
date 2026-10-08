#!/bin/sh
set -eu
head=$(git rev-parse HEAD)
branch=$(git branch --show-current)
current_head=$head
current_branch=$branch
[ ! -f "$LF_HOME/gh-head" ] || head=$(cat "$LF_HOME/gh-head")
[ ! -f "$LF_HOME/gh-branch" ] || branch=$(cat "$LF_HOME/gh-branch")
state=OPEN
merged=false
merge=null
merged_at=null
auto=null
[ ! -f "$LF_HOME/gh-armed" ] || auto='{"enabledAt":"2026-10-08T00:00:00Z"}'
if [ -f "$LF_HOME/gh-merged" ]; then
  state=MERGED
  merged=true
  merge="\"$head\""
  merged_at='"2026-10-08T00:00:00Z"'
fi
case "$1 ${2-}" in
  '--version ') echo 'gh fixture'; exit 0 ;;
  'pr list')
    if [ ! -f "$LF_HOME/gh-created" ]; then echo '[]'; exit 0; fi
    case "$*" in
      *title,body*) jq -n --rawfile title "$LF_HOME/gh-title" --rawfile body "$LF_HOME/gh-body" --arg head "$head" '[{title:$title,body:$body,headRefOid:$head}]'; exit 0 ;;
    esac
    printf '[{"url":"https://github.com/fixture/local/pull/1","number":1,"state":"%s","isDraft":false,"headRefOid":"%s"}]\n' "$state" "$head"
    exit 0 ;;
  'pr create')
    touch "$LF_HOME/gh-created"
    echo "$current_head" > "$LF_HOME/gh-head"
    echo "$current_branch" > "$LF_HOME/gh-branch"
    while [ "$#" -gt 0 ]; do
      case "$1" in
        --title) shift; printf '%s' "$1" > "$LF_HOME/gh-title" ;;
        --body) shift; printf '%s' "$1" > "$LF_HOME/gh-body" ;;
      esac
      shift
    done
    echo 'https://github.com/fixture/local/pull/1'
    exit 0 ;;
  'pr edit')
    echo "$current_head" > "$LF_HOME/gh-head"
    while [ "$#" -gt 0 ]; do
      case "$1" in
        --title) shift; printf '%s' "$1" > "$LF_HOME/gh-title" ;;
        --body) shift; printf '%s' "$1" > "$LF_HOME/gh-body" ;;
      esac
      shift
    done
    exit 0 ;;
  'pr ready') exit 0 ;;
  'pr view')
    if [ "$*" = 'pr view --json url -q .url' ]; then
      echo 'https://github.com/fixture/local/pull/1'; exit 0
    fi ;;
  'pr merge')
    case "$*" in
      *--disable-auto*) rm -f "$LF_HOME/gh-armed" ;;
      *--auto*) touch "$LF_HOME/gh-armed" ;;
      *) echo "$*" >> "$LF_HOME/gh-unexpected"; exit 91 ;;
    esac
    exit 0 ;;
  'api graphql')
    case "$*" in
      *LoopflowPrChecks*)
        printf '{"head":"%s","commit":"%s","contexts":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[]}}\n' "$head" "$head"
        exit 0 ;;
      *'b0: pullRequests('*'r0: ref('*)
        printf '{"data":{"repository":{"b0":{"nodes":[{"headRefOid":"%s","state":"%s"}]},"r0":{"id":"ref-fixture"}}}}\n' "$head" "$state"
        exit 0 ;;
      *LoopflowPrMerge*)
        merge_commit=null
        [ "$merged" = false ] || merge_commit="{\"oid\":\"$head\"}"
        cat <<JSON
{"data":{"repository":{"pullRequest":{"id":"PR_fixture","number":1,"url":"https://github.com/fixture/local/pull/1","state":"$state","isDraft":false,"headRefName":"$branch","headRefOid":"$head","mergedAt":$merged_at,"mergeCommit":$merge_commit,"mergeStateStatus":"CLEAN","isMergeQueueEnabled":false,"autoMergeRequest":$auto,"mergeQueueEntry":null}}}}
JSON
        exit 0 ;;
    esac ;;
  'api --cache'|'api -H')
    for endpoint do :; done
    if [ "$endpoint" = repos/fixture/local/pulls/1 ]; then
      cat <<JSON
{"number":1,"html_url":"https://github.com/fixture/local/pull/1","state":"open","draft":false,"merged":$merged,"merge_commit_sha":$merge,"merged_at":$merged_at,"mergeable_state":"clean","head":{"sha":"$head"}}
JSON
      exit 0
    fi ;;
esac
echo "$*" >> "$LF_HOME/gh-unexpected"
echo "Unexpected gh command: $*" >&2
exit 91
