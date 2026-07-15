---
type: "uml.Class"
stereotype: ["aggregateRoot", "entity"]
title: "Order"
description: "A customer's placed order."
---

# Order

## Attributes
- id: OrderId
- placedAt: Timestamp
- status: [OrderStatus](./order-status.md)
- shippingAddress: [Address](./address.md) {0..1}
- total: [Money](./money.md)

## Relationships
- associates [Customer](./customer.md): 1 order to 1 customer
- composes [OrderLine](./order-line.md): 1 to 1..* lines
- depends [PricingService](./pricing-service.md)

