---
type: uml.UseCase
title: Sequence Language
description: An author defines sequence behavior with the WAML sequence language.
---

# Sequence Language

## Relationships
- associates [Author](../actors/author.md)

## Owning goal

- [Sequence Language](../../goals/uml/sequence/language.md)


## Scenarios

- [SEQ-MSG-1](../../goals/uml/sequence/language.md#seq-msg-1-—-a-call-makes-a-synchronous-message)
- [SEQ-MSG-2](../../goals/uml/sequence/language.md#seq-msg-2-—-a-return-matches-an-earlier-call)
- [SEQ-MSG-3](../../goals/uml/sequence/language.md#seq-msg-3-—-a-return-without-a-call-is-an-error)
- [SEQ-MSG-4](../../goals/uml/sequence/language.md#seq-msg-4-—-async-is-valid-only-after-a-call-target)
- [SEQ-MSG-5](../../goals/uml/sequence/language.md#seq-msg-5-—-async-on-another-verb-is-an-error)
- [SEQ-MSG-6](../../goals/uml/sequence/language.md#seq-msg-6-—-an-old-verb-is-not-a-message)
- [SEQ-MSG-7](../../goals/uml/sequence/language.md#seq-msg-7-—-a-create-message-starts-the-lifeline)
- [SEQ-MSG-8](../../goals/uml/sequence/language.md#seq-msg-8-—-a-destroy-message-ends-the-lifeline)
- [SEQ-ORD-1](../../goals/uml/sequence/language.md#seq-ord-1-—-source-order-is-behavior-order)
- [SEQ-ORD-2](../../goals/uml/sequence/language.md#seq-ord-2-—-par-removes-the-order-between-branches)
- [SEQ-FRAG-1](../../goals/uml/sequence/language.md#seq-frag-1-—-alt-accepts-one-when)
- [SEQ-FRAG-2](../../goals/uml/sequence/language.md#seq-frag-2-—-alt-accepts-a-final-else)
- [SEQ-FRAG-3](../../goals/uml/sequence/language.md#seq-frag-3-—-else-must-be-last)
- [SEQ-FRAG-4](../../goals/uml/sequence/language.md#seq-frag-4-—-else-occurs-one-time-only)
- [SEQ-FRAG-5](../../goals/uml/sequence/language.md#seq-frag-5-—-opt,-loop,-and-break-accept-one-when)
- [SEQ-FRAG-6](../../goals/uml/sequence/language.md#seq-frag-6-—-a-wrong-operand-count-is-an-error)
- [SEQ-FRAG-7](../../goals/uml/sequence/language.md#seq-frag-7-—-par-needs-two-branches)
- [SEQ-FRAG-8](../../goals/uml/sequence/language.md#seq-frag-8-—-critical,-assert,-and-neg-accept-one-branch)
- [SEQ-FRAG-9](../../goals/uml/sequence/language.md#seq-frag-9-—-a-fragment-stays-in-its-parent-operand)
- [SEQ-FRAG-10](../../goals/uml/sequence/language.md#seq-frag-10-—-the-item-after-a-fragment-is-a-sibling)
- [SEQ-FRAG-11](../../goals/uml/sequence/language.md#seq-frag-11-—-a-fragment-frame-holds-its-content)
- [SEQ-BND-1](../../goals/uml/sequence/language.md#seq-bnd-1-—-outside-is-a-boundary-endpoint)
- [SEQ-BND-2](../../goals/uml/sequence/language.md#seq-bnd-2-—-outside-is-a-reserved-name)
- [SEQ-BND-3](../../goals/uml/sequence/language.md#seq-bnd-3-—-an-alias-must-not-contain-an-at-sign)
- [SEQ-BND-4](../../goals/uml/sequence/language.md#seq-bnd-4-—-a-gate-is-on-the-frame-boundary)
- [SEQ-BND-5](../../goals/uml/sequence/language.md#seq-bnd-5-—-a-reference-does-not-copy-messages)
- [SEQ-BND-6](../../goals/uml/sequence/language.md#seq-bnd-6-—-a-reference-binds-its-gates)
- [SEQ-ACT-1](../../goals/uml/sequence/language.md#seq-act-1-—-a-call-starts-an-activation)
- [SEQ-ACT-2](../../goals/uml/sequence/language.md#seq-act-2-—-correlation-uses-message-identity)
- [SEQ-EXC-1](../../goals/uml/sequence/language.md#seq-exc-1-—-an-excluded-fragment-head-is-not-accepted)
- [SEQ-REC-1](../../goals/uml/sequence/language.md#seq-rec-1-—-a-bad-item-does-not-stop-the-next-item)
- [SEQ-REC-2](../../goals/uml/sequence/language.md#seq-rec-2-—-malformed-source-stays-lossless)
