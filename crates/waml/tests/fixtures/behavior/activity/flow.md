---
type: uml.Activity
title: Order Flow
---
# Order Flow

## Nodes
### initial Start
- transitions to Validate
### Validate
- partition: Intake
- transitions to Check
### decision Check
- partition: Intake
- when `low` transitions to Ship
- when `high` transitions to Review
- else transitions to Retry
### Review
- partition: Intake
- transitions to Retry
### Retry
- partition: Intake
- transitions to Check
### Ship
- partition: Fulfillment
- transitions to Split
### fork Split
- partition: Fulfillment
- transitions to Pack
- transitions to Notify
### Pack
- partition: Fulfillment
- transitions to Merge
### Notify
- partition: Fulfillment
- transitions to Merge
### join Merge
- partition: Fulfillment
- transitions to Archive
### object [Order](./order.md)
- transitions to End
### Archive
- transitions to Order
### final End
