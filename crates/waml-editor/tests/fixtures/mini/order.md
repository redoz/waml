---
type: uml.Class
title: Order
stereotype: [aggregateRoot]
abstract: true
---
# Order

An order is settled once payment is confirmed through the PaymentGateway.

## Attributes
- id: OrderId {1}
- total: Decimal {1}

## Relationships
- associates [Customer](./customer.md): 1 order to 1 customer
