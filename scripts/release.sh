#!/usr/bin/env bash
set -euo pipefail

# Release flow for the release branch (dev by default):
#   1. pick the version bump, 2. build notes from every commit since the last
#   tag (grouped by conventional-commit type, PR list, collapsible full commit
#   list), 3. bump Cargo.toml, commit, tag, push, 4. create the GitHub release,
#   5. open or refresh the release PR into main with the same notes.
#
#   scripts/release.sh          interactive release
#   scripts/release.sh --notes  only print the notes for what is unreleased

cd "$(git rev-parse --show-toplevel)"

for tool in gum gh jq cargo; do
  if ! command -v "$tool" &>/dev/null; then
    echo "Error: $tool is not installed." >&2
    exit 1
  fi
done

RELEASE_BRANCH=${RELEASE_BRANCH:-dev}
TARGET_BRANCH=${TARGET_BRANCH:-main}
MANIFEST=crates/app/Cargo.toml
PACKAGE=gpuishell

NOTES_ONLY=0
[[ "${1:-}" == "--notes" ]] && NOTES_ONLY=1

REPO_SLUG=$(gh repo view --json nameWithOwner -q .nameWithOwner)
REPO_URL="https://github.com/$REPO_SLUG"
REPO_OWNER=${REPO_SLUG%%/*}
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
HEAD_SHA=$(git rev-parse HEAD)

CURRENT_VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$MANIFEST" | head -1)
if [[ -z "$CURRENT_VERSION" ]]; then
  echo "Error: could not read version from $MANIFEST" >&2
  exit 1
fi
PREV_TAG="v$CURRENT_VERSION"

if git rev-parse -q --verify "$PREV_TAG^{commit}" >/dev/null; then
  BASE_REF=$PREV_TAG
else
  BASE_REF=$(git rev-list --max-parents=0 HEAD | tail -1)
fi

# ---------------------------------------------------------------------------
# Release notes
# ---------------------------------------------------------------------------

# Commits since the base as JSON: sha, subject, author name, email.
collect_commits() {
  git log --no-merges --format='%H%x1f%s%x1f%an%x1f%ae' "$BASE_REF..HEAD" \
    | jq -Rn '[inputs | split("\u001f") | {sha: .[0], subject: .[1], name: .[2], email: .[3]}]'
}

# sha -> GitHub login, via the compare API. Needs HEAD on the remote; returns {}
# otherwise so the notes still build, just without @mentions.
collect_logins() {
  gh api "repos/$REPO_SLUG/compare/$BASE_REF...$HEAD_SHA" --paginate \
    -q '.commits[] | select(.author.login != null) | {key: .sha, value: .author.login}' 2>/dev/null \
    | jq -s 'from_entries' || echo '{}'
}

# PRs merged into the release branch whose merge commit is in the range.
collect_prs() {
  local shas
  shas=$(git log --format=%H "$BASE_REF..HEAD")
  gh pr list --base "$CURRENT_BRANCH" --state merged --limit 200 \
    --json number,title,author,mergedAt,url,mergeCommit \
    | jq --arg shas "$shas" '
        ($shas | split("\n") | map(select(length > 0))) as $set
        | [.[] | select(.mergeCommit != null and (.mergeCommit.oid as $oid | $set | index($oid)))]
        | sort_by(.mergedAt)'
}

# build_notes <compare-url> > notes.md
build_notes() {
  local compare_url=$1
  local commits logins prs
  commits=$(collect_commits)
  logins=$(collect_logins)
  prs=$(collect_prs)

  jq -rn \
    --argjson commits "$commits" \
    --argjson logins "$logins" \
    --argjson prs "$prs" \
    --arg repo_url "$REPO_URL" \
    --arg owner "$REPO_OWNER" \
    --arg prev "$PREV_TAG" \
    --arg compare "$compare_url" '
    def group_of($t):
      if ($t | startswith("feat")) then "Features"
      elif ($t | startswith("fi")) then "Fixes"
      elif $t == "perf" then "Performance"
      elif ($t | startswith("refact")) then "Refactoring"
      elif ($t | startswith("doc")) then "Documentation"
      elif ($t | IN("chore", "ci", "build", "test", "style")) then "Maintenance"
      else "Other" end;

    def classify:
      ([.subject | capture("^(?<t>[A-Za-z]+)(\\((?<s>[^)]*)\\))?!?: *(?<d>.*)$")] | .[0]) as $m
      | if $m == null then . + {group: "Other", text: .subject}
        else . + {group: group_of($m.t | ascii_downcase),
                  text: (if $m.s then "**\($m.s)**: \($m.d)" else $m.d end)}
        end;

    def login: $logins[.sha] // null;
    def short: .sha[0:7];
    def link: "[`\(short)`](\($repo_url)/commit/\(.sha))";
    def credit: (login as $l
      | if $l == null then (if .name == $owner then "" else " by \(.name)" end)
        elif $l == $owner then "" else " by @\($l)" end);

    ($commits | map(classify)) as $c
    | (["Features", "Fixes", "Performance", "Refactoring", "Documentation", "Maintenance", "Other"]) as $order
    | ($c | map(login) | map(select(. != null and . != $owner)) | unique) as $contributors

    | "## What'"'"'s Changed", "",
      (if ($c | length) == 0 then "_No commits since \($prev)._", "" else
        ($order[] as $g
          | ($c | map(select(.group == $g))) as $items
          | select(($items | length) > 0)
          | "### \($g)", ($items[] | "- \(.text) (\(link))\(credit)"), "")
      end),

      (if ($prs | length) > 0 then
        "## Pull Requests", "",
        ($prs[] | "- \(.title) by @\(.author.login) in [#\(.number)](\(.url))"), ""
      else empty end),

      (if ($contributors | length) > 0 then
        "## Contributors", "",
        "Thanks to \($contributors | map("@" + .) | join(", "))!", ""
      else empty end),

      "<details>",
      "<summary>All commits (\($c | length))</summary>", "",
      ($c[] | "- \(link) \(.subject)\(credit)"), "",
      "</details>", "",
      "**Full Changelog**: \($compare)"
  '
}

if [[ $NOTES_ONLY -eq 1 ]]; then
  build_notes "$REPO_URL/compare/$BASE_REF...$CURRENT_BRANCH"
  exit 0
fi

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------

if [[ "$CURRENT_BRANCH" != "$RELEASE_BRANCH" ]]; then
  gum confirm "You are on '$CURRENT_BRANCH', releases usually go from '$RELEASE_BRANCH'. Continue?" || exit 0
fi

if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
  echo "Error: working tree has uncommitted changes." >&2
  exit 1
fi

gum spin --spinner dot --title "Fetching origin..." -- git fetch origin --tags --quiet

if [[ "$(git rev-parse "origin/$CURRENT_BRANCH" 2>/dev/null)" != "$HEAD_SHA" ]]; then
  echo "Error: '$CURRENT_BRANCH' is not in sync with origin. Pull or push first." >&2
  exit 1
fi

echo "Current version: $PREV_TAG  ($(git rev-list --count "$BASE_REF..HEAD") commits since)"
echo

# ---------------------------------------------------------------------------
# Version
# ---------------------------------------------------------------------------

IFS='.' read -r MAJOR MINOR PATCH <<<"$CURRENT_VERSION"
NEXT_MAJOR="$((MAJOR + 1)).0.0"
NEXT_MINOR="$MAJOR.$((MINOR + 1)).0"
NEXT_PATCH="$MAJOR.$MINOR.$((PATCH + 1))"

BUMP_TYPE=$(gum choose --header "Select version bump:" \
  "minor (v$NEXT_MINOR)" "patch (v$NEXT_PATCH)" "major (v$NEXT_MAJOR)" "custom")

case "$BUMP_TYPE" in
  major*) NEW_VERSION=$NEXT_MAJOR ;;
  minor*) NEW_VERSION=$NEXT_MINOR ;;
  patch*) NEW_VERSION=$NEXT_PATCH ;;
  custom)
    NEW_VERSION=$(gum input --placeholder "Enter version (e.g. 1.2.3)")
    NEW_VERSION=${NEW_VERSION#v}
    ;;
  *) exit 0 ;;
esac

if [[ ! "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Error: invalid version '$NEW_VERSION', expected major.minor.patch" >&2
  exit 1
fi

VERSION_TAG="v$NEW_VERSION"

if git rev-parse -q --verify "$VERSION_TAG" >/dev/null \
  || [[ -n "$(git ls-remote --tags origin "refs/tags/$VERSION_TAG")" ]]; then
  echo "Error: tag $VERSION_TAG already exists." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Notes
# ---------------------------------------------------------------------------

NOTES_FILE=$(mktemp -t release-notes.XXXXXX.md)
trap 'rm -f "$NOTES_FILE"' EXIT

COMPARE_URL="$REPO_URL/compare/$PREV_TAG...$VERSION_TAG"
echo "Building release notes since $PREV_TAG..."
build_notes "$COMPARE_URL" > "$NOTES_FILE"

show_notes() {
  gum style --border rounded --padding "1 2" --border-foreground 240 "$(cat "$NOTES_FILE")"
  echo
}

echo
gum style --border normal --padding "1 2" --border-foreground 212 \
  "Version bump: $PREV_TAG → $VERSION_TAG" \
  "Branch: $CURRENT_BRANCH → release PR into $TARGET_BRANCH"
echo
gum style --foreground 212 --bold "Release notes preview:"
show_notes

if gum confirm "Edit release notes before publishing?" --default=false; then
  gum write --value "$(cat "$NOTES_FILE")" --width 100 --height 25 \
    --header "Edit release notes (Ctrl+D to save)" > "$NOTES_FILE.new"
  mv "$NOTES_FILE.new" "$NOTES_FILE"
  echo
  gum style --foreground 212 --bold "Updated notes:"
  show_notes
fi

gum confirm "Bump to $VERSION_TAG, commit and tag?" || { echo "Release cancelled."; exit 0; }

# ---------------------------------------------------------------------------
# Bump, commit, tag
# ---------------------------------------------------------------------------

sed -i "0,/^version = \".*\"/s//version = \"$NEW_VERSION\"/" "$MANIFEST"
gum spin --spinner dot --title "Updating Cargo.lock..." -- cargo update -p "$PACKAGE" --offline
git add "$MANIFEST" Cargo.lock

COMMIT_MSG="chore: bump version to $VERSION_TAG"
git commit -q -m "$COMMIT_MSG"
git tag -a "$VERSION_TAG" -m "$VERSION_TAG"

echo
gum style --foreground 212 "✓ $COMMIT_MSG"
gum style --foreground 212 "✓ Tag created: $VERSION_TAG"
echo

# ---------------------------------------------------------------------------
# Push, release, PR
# ---------------------------------------------------------------------------

keep_notes() {
  local kept="${TMPDIR:-/tmp}/release-notes-$VERSION_TAG.md"
  cp "$NOTES_FILE" "$kept"
  echo "  (notes saved to $kept)"
}

if ! gum confirm "Push commit + tag and create the GitHub release?"; then
  echo
  gum style --foreground 220 "⚠ Committed and tagged locally, nothing pushed. To finish manually:"
  echo "  git push && git push origin $VERSION_TAG"
  echo "  gh release create $VERSION_TAG --title $VERSION_TAG --notes-file <notes> --verify-tag"
  keep_notes
  exit 0
fi

gum spin --spinner dot --title "Pushing $CURRENT_BRANCH and $VERSION_TAG..." -- \
  git push --atomic origin "$CURRENT_BRANCH" "$VERSION_TAG"
gum spin --spinner dot --title "Creating GitHub release..." -- \
  gh release create "$VERSION_TAG" --title "$VERSION_TAG" --notes-file "$NOTES_FILE" --verify-tag

echo
gum style --foreground 212 --bold "🚀 Release $VERSION_TAG published!"
gum style --foreground 212 "   $REPO_URL/releases/tag/$VERSION_TAG"
gum style --foreground 240 "   Build artifacts are attached by the Release workflow once it finishes."
echo

if [[ "$CURRENT_BRANCH" == "$TARGET_BRANCH" ]]; then
  exit 0
fi

if ! gum confirm "Open the release PR $CURRENT_BRANCH → $TARGET_BRANCH?"; then
  keep_notes
  exit 0
fi

PR_TITLE=$(gum input --value "release: $VERSION_TAG" --width 100 --header "PR title")
EXISTING_PR=$(gh pr list --head "$CURRENT_BRANCH" --base "$TARGET_BRANCH" --state open --json number -q '.[0].number')

if [[ -n "$EXISTING_PR" ]]; then
  gum spin --spinner dot --title "Updating PR #$EXISTING_PR..." -- \
    gh pr edit "$EXISTING_PR" --title "$PR_TITLE" --body-file "$NOTES_FILE"
  PR_URL=$(gh pr view "$EXISTING_PR" --json url -q .url)
else
  PR_URL=$(gh pr create --base "$TARGET_BRANCH" --head "$CURRENT_BRANCH" \
    --title "$PR_TITLE" --body-file "$NOTES_FILE")
fi

gum style --foreground 212 "✓ Release PR: $PR_URL"
