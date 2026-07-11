<!-- shop/order.md -->
---
type: uml.Class
stereotype: [aggregateRoot, entity]
title: Order
description: "A customer's placed order."
---
# Order

## Attributes
- id: OrderId
- status: [OrderStatus](./order-status.md)

## Relationships
- composes [OrderLine](./order-line.md): 1 to 1..* lines
- associates [Customer](./customer.md): 1 to 1

<!-- shop/order-line.md -->
---
type: uml.Class
stereotype: entity
title: OrderLine
---
# OrderLine

## Attributes
- quantity: Int
- unitPrice: [Money](./money.md)

<!-- shop/customer.md -->
---
type: uml.Class
stereotype: [aggregateRoot, entity]
title: Customer
---
# Customer

## Attributes
- id: CustomerId
- name: String

<!-- shop/order-status.md -->
---
type: uml.Enum
title: OrderStatus
---
# OrderStatus

## Values
- DRAFT
- PLACED
- SHIPPED
- CANCELLED

<!-- shop/money.md -->
---
type: uml.DataType
stereotype: valueObject
title: Money
---
# Money

## Attributes
- amount: Decimal
- currency: CurrencyCode

<!-- shop/orders-domain.md -->
---
type: Diagram
title: Orders Domain
profile: uml-domain
---
# Orders Domain

## Members
- [Order](./order.md)
- [OrderLine](./order-line.md)
- [Customer](./customer.md)
- [OrderStatus](./order-status.md)
- [Money](./money.md)
