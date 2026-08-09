---
type: uml.UseCase
title: Command-Line Tool
description: A caller validates, formats, queries, and changes a WAML bundle.
---

# Command-Line Tool

## Relationships
- associates [Automation Client](../actors/automation-client.md)
- associates [Command-Line User](../actors/command-line-user.md)

## Owning goal

- [Command-Line Tool](../../goals/tooling-around-the-repo/command-line-tool.md)


## Scenarios

- [CLI-001](../../goals/tooling-around-the-repo/command-line-tool.md#cli-001-—-validation-reports-positioned-errors-and-exits-non-zero)
- [CLI-002](../../goals/tooling-around-the-repo/command-line-tool.md#cli-002-—-formatting-is-canonical-and-idempotent)
- [CLI-004](../../goals/tooling-around-the-repo/command-line-tool.md#cli-004-—-add-an-attribute-with-the-direct-command)
- [CLI-005](../../goals/tooling-around-the-repo/command-line-tool.md#cli-005-—-apply-an-ndjson-batch-atomically)
- [CLI-006](../../goals/tooling-around-the-repo/command-line-tool.md#cli-006-—-show-a-resolved-classifier)
- [CLI-007](../../goals/tooling-around-the-repo/command-line-tool.md#cli-007-—-list-classifier-referrers)
- [CLI-008](../../goals/tooling-around-the-repo/command-line-tool.md#cli-008-—-list-classifiers-with-an-optional-type-filter)
- [CLI-009](../../goals/tooling-around-the-repo/command-line-tool.md#cli-009-—-bundle-a-directory-as-json-or-typescript)
- [CLI-010](../../goals/tooling-around-the-repo/command-line-tool.md#cli-010-—-format-check-rejects-noncanonical-content)
- [CLI-011](../../goals/tooling-around-the-repo/command-line-tool.md#cli-011-—-direct-commands-change-nodes,-values,-and-relationships)
