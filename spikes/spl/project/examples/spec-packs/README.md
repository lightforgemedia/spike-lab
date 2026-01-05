# Spec Pack Examples

This directory contains example spec packs demonstrating various task types and profiles.

## Files

| Example | Profile | Description |
|---------|---------|-------------|
| `feature-add-user-auth.yaml` | standard | Full feature with behavior contracts, JWT auth |
| `hotfix-null-pointer.yaml` | hotfix | Urgent production fix with reduced gates |
| `docs-api-reference.yaml` | docs | Documentation update, read-only network |
| `refactor-extract-service.yaml` | standard | Behavior-preserving refactor |
| `integration-external-api.yaml` | standard | External API (Stripe), requires network DECISION |

## Profiles

- **standard**: Full gate requirements - pre_smoke, audit, adversarial_review, validate, post_smoke
- **hotfix**: Reduced gates for urgent fixes - skips adversarial_review
- **docs**: Minimal gates for non-code changes - skips adversarial_review and validate
- **backfill_spec**: For adding specs to existing code (anchors not required)

## Network Policies

- `deny`: No network access (default, safest)
- `allow_readonly`: HTTP GET only, for fetching references
- `allow`: Full network access (requires DECISION approval)

## Creating Your Own Spec Pack

1. Copy the closest example to your task type
2. Update `task`, `intent`, and `scope`
3. Define `use_cases` with actor/steps/postconditions
4. Add `behavior_contracts` with anchors pointing to real code
5. Set appropriate `profile` and `policy`
6. Run `spl compile <spec-pack.yaml>` to validate

## Key Rules

- `use_cases` must be example-driven (specific scenarios, not abstract)
- `behavior_contracts` must reference real code anchors (except `backfill_spec` profile)
- `scope.out` files cannot be modified by delegate
- All required gates must pass before landing
