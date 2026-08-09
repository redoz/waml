---
type: uml.UseCase
title: Serve Locally
description: A command-line user serves a local bundle to a reader.
---

# Serve Locally

## Relationships
- associates [Command-Line User](../actors/command-line-user.md)
- associates [Reader](../actors/reader.md)

## Owning goal

- [Serve Locally](../../goals/share-and-publish/serve-locally.md)


## Scenarios

- [BROWSER-002](../../goals/share-and-publish/serve-locally.md#browser-002-—-the-printed-serve-url-starts-the-browser-artifact)
- [BROWSER-007](../../goals/share-and-publish/serve-locally.md#browser-007-—-a-same-origin-served-editor-reads-the-authenticated-model)
- [BROWSER-008](../../goals/share-and-publish/serve-locally.md#browser-008-—-a-foreign-origin-cannot-use-the-authenticated-api)
- [BROWSER-009](../../goals/share-and-publish/serve-locally.md#browser-009-—-a-browser-document-save-uses-the-baseline-guard)
- [BROWSER-010](../../goals/share-and-publish/serve-locally.md#browser-010-—-a-conflicting-browser-save-reports-the-current-revision)
- [BROWSER-017](../../goals/share-and-publish/serve-locally.md#browser-017-—-the-diagnostics-api-returns-diagnostics-and-its-revision)
- [BROWSER-018](../../goals/share-and-publish/serve-locally.md#browser-018-—-an-operations-batch-is-atomic-and-reports-changed-files)
- [BROWSER-019](../../goals/share-and-publish/serve-locally.md#browser-019-—-a-request-without-a-valid-token-is-refused-before-body-validation)
- [BROWSER-020](../../goals/share-and-publish/serve-locally.md#browser-020-—-a-mutating-request-requires-the-waml-client-header)
