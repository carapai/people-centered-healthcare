#!/usr/bin/env bash
# Applies the repository's merge and security settings (deployment.md § Git and merge rules;
# ADR 0045). Idempotent: run it again after changing a required check.
#
#   tools/github/configure-repository.sh [owner/repo]   # default: the current gh repository
#
# Needs an admin token (gh auth) and a public repository, or a plan with rulesets on private
# repositories.
#
# - Merging: squash only, with the PR title as the commit subject; head branches are deleted.
# - Security: the dependency graph (for dependency review), and secret scanning with push
#   protection.
# - The `main` ruleset: no deletion or force push, linear history, a PR with one approval and
#   code-owner review, resolved conversations, and every required check passing on an
#   up-to-date branch. ADR changes need a second approval (adr-approvals.yml).
# - Bypass: repository admins, through a PR only. While the maintainer is the only reviewer,
#   GitHub does not let them approve their own PR, so merging uses this bypass, which GitHub
#   records. Remove it once a second reviewer exists.
#
# The merge queue needs an organization-owned repository; the workflows already listen for
# `merge_group`, so it can be enabled here then.
set -euo pipefail

REPO="${1:-$(gh repo view --json nameWithOwner --jq .nameWithOwner)}"
ACTIONS_APP_ID=15368 # GitHub Actions: required checks must come from it

# Check names, as the workflow jobs name them.
REQUIRED_CHECKS=(
  # rust-ci.yml
  "fmt, clippy, doc, architecture, deny"
  "build, nextest, doc tests"
  "FHIR corpus, element table, validation budget"
  "coverage ≥ 80% lines (domain and application crates)"
  "HL7 Java validator differential"
  # frontend-ci.yml
  "install, codegen, typecheck, lint, build, bundle scan"
  # security.yml
  "cargo-deny advisories"
  "osv-scanner (bun.lock)"
  "gitleaks"
  "zizmor"
  "CodeQL (JavaScript and TypeScript)"
  # dependency-review.yml
  "licences and advisories of changed dependencies"
  # documentation.yml
  "markdownlint, ADR index, Mermaid"
  "lychee"
  "surreal validate, surql-fmt"
  # pr-title.yml, adr-approvals.yml
  "Conventional Commit title"
  "two approvals on ADR changes"
)

echo "merge settings for $REPO"
gh api --method PATCH "repos/$REPO" --silent \
  -F allow_squash_merge=true \
  -F allow_merge_commit=false \
  -F allow_rebase_merge=false \
  -F allow_auto_merge=true \
  -F delete_branch_on_merge=true \
  -f squash_merge_commit_title=PR_TITLE \
  -f squash_merge_commit_message=PR_BODY

# Dependency review reads the dependency graph, which Dependabot alerts turn on. Renovate,
# not Dependabot, raises the update PRs.
echo "dependency graph (Dependabot alerts)"
gh api --method PUT "repos/$REPO/vulnerability-alerts" --silent

echo "secret scanning with push protection"
gh api --method PATCH "repos/$REPO" --silent --input - <<'JSON'
{
  "security_and_analysis": {
    "secret_scanning": { "status": "enabled" },
    "secret_scanning_push_protection": { "status": "enabled" }
  }
}
JSON

checks=$(printf '%s\n' "${REQUIRED_CHECKS[@]}" \
  | jq -R --argjson app "$ACTIONS_APP_ID" '{context: ., integration_id: $app}' | jq -s .)
ruleset=$(jq -n --argjson checks "$checks" '{
  name: "main",
  target: "branch",
  enforcement: "active",
  conditions: { ref_name: { include: ["~DEFAULT_BRANCH"], exclude: [] } },
  bypass_actors: [ { actor_type: "RepositoryRole", actor_id: 5, bypass_mode: "pull_request" } ],
  rules: [
    { type: "deletion" },
    { type: "non_fast_forward" },
    { type: "required_linear_history" },
    { type: "pull_request", parameters: {
        required_approving_review_count: 1,
        require_code_owner_review: true,
        dismiss_stale_reviews_on_push: true,
        require_last_push_approval: false,
        required_review_thread_resolution: true,
        allowed_merge_methods: ["squash"]
    } },
    { type: "required_status_checks", parameters: {
        strict_required_status_checks_policy: true,
        do_not_enforce_on_create: false,
        required_status_checks: $checks
    } }
  ]
}')

existing=$(gh api "repos/$REPO/rulesets" --jq '.[] | select(.name == "main") | .id')
if [[ -n "$existing" ]]; then
  echo "updating ruleset main ($existing)"
  gh api --method PUT "repos/$REPO/rulesets/$existing" --silent --input - <<<"$ruleset"
else
  echo "creating ruleset main"
  gh api --method POST "repos/$REPO/rulesets" --silent --input - <<<"$ruleset"
fi
echo "done: ${#REQUIRED_CHECKS[@]} required checks"
