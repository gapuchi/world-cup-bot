alias u := update
alias r := release

update *inputs:
    nix flake update {{inputs}}

# Preview a version bump (dry run). level: patch | minor | major
release-dry level="patch":
    cargo release {{level}}

# Full release: bump, commit, tag, and push
release level="patch":
    cargo release {{level}} --execute
    git push origin main --follow-tags
