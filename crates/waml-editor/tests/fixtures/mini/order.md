---
type: uml.Class
title: Order
stereotype: [aggregateRoot]
abstract: true
---
# Order

## Attributes
- id: OrderId {1}
- total: Decimal {1}

## Relationships
- associates [Customer](./customer.md): 1 order to 1 customer
